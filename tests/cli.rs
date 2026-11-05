//! Binary-level tests launch only owned fixture daemons with fully replaced environments.
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use serde_json::Value;
use session_manager::{interface::Command as Rpc, transport};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Fixture {
    root: PathBuf,
    runtime: PathBuf,
    config: PathBuf,
    state: PathBuf,
    projects: PathBuf,
    data: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("sm-cli-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join(".owned"), "session-manager-cli-fixture").unwrap();
        for name in [
            "home",
            "config/session-manager",
            "state",
            "runtime",
            "projects",
            "data/applications",
            "bin",
        ] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        let runtime = root.join("runtime");
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let config = root.join("config/session-manager");
        let state = root.join("state");
        let projects = root.join("projects");
        let data = root.join("data");
        fs::write(
            config.join("config-v2.toml"),
            format!(
                "schema_version=2\nproject_source={:?}",
                projects.to_str().unwrap()
            ),
        )
        .unwrap();
        fs::write(
            data.join("applications/app.desktop"),
            "[Desktop Entry]\nType=Application\nName=Fixture App\nExec=never-executed\n",
        )
        .unwrap();
        Self {
            root,
            runtime,
            config,
            state,
            projects,
            data,
        }
    }
    fn command(&self) -> Command {
        // The sandbox hides the original build path; its harness maps this checkout's
        // executable explicitly. Never discover or invoke an installed daemon binary.
        let binary = std::env::var_os("SESSION_MANAGER_TEST_BINARY").map_or_else(
            || PathBuf::from(env!("CARGO_BIN_EXE_session-manager")),
            PathBuf::from,
        );
        assert_eq!(
            binary.file_name(),
            Some(std::ffi::OsStr::new("session-manager"))
        );
        assert!(binary.is_file(), "fixture executable unavailable");
        let mut command = Command::new(binary);
        command
            .env_clear()
            .env("HOME", self.root.join("home"))
            .env("XDG_CONFIG_HOME", self.config.parent().unwrap())
            .env("XDG_STATE_HOME", &self.state)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("XDG_DATA_HOME", &self.data)
            .env("XDG_DATA_DIRS", &self.data)
            .env("PATH", self.root.join("bin"));
        command
    }
    fn output(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }
    fn json(&self, args: &[&str]) -> Value {
        let output = self.command().arg("--json").args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn daemon(&self) -> Daemon {
        let child = self
            .command()
            .arg("daemon")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut daemon = Daemon { child: Some(child) };
        let start = Instant::now();
        loop {
            assert!(
                daemon.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "fixture daemon exited during startup"
            );
            if self.runtime.join("session-manager/ipc-v2.sock").exists()
                && transport::call(&self.runtime.join("session-manager"), &Rpc::Sessions).is_ok()
            {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "fixture startup timed out"
            );
            thread::sleep(Duration::from_millis(10));
        }
        daemon
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.starts_with(std::env::temp_dir())
            && fs::read(self.root.join(".owned")).is_ok_and(|b| b == b"session-manager-cli-fixture")
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
struct Daemon {
    child: Option<Child>,
}
impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if child.try_wait().unwrap().is_none() {
                // This PID belongs to the child handle above, never a discovered live daemon.
                let _ = kill(
                    Pid::from_raw(i32::try_from(child.id()).unwrap()),
                    Signal::SIGTERM,
                );
                let start = Instant::now();
                while child.try_wait().unwrap().is_none()
                    && start.elapsed() < Duration::from_secs(10)
                {
                    thread::sleep(Duration::from_millis(10));
                }
                if child.try_wait().unwrap().is_none() {
                    let _ = child.kill();
                }
            }
            let _ = child.wait();
        }
    }
}

#[test]
fn binary_full_flow_default_custom_template_json_errors_and_restart() {
    let f = Fixture::new();
    let custom = f.config.join("profile-template/Custom");
    fs::create_dir_all(&custom).unwrap();
    fs::write(custom.join("README.md"), "fixture template").unwrap();
    let daemon = f.daemon();
    assert_eq!(
        f.json(&["template", "list"])["templates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        f.json(&["application", "list"])["applications"][0]["name"],
        "Fixture App"
    );
    let empty = f.json(&[
        "project",
        "create",
        "--base-name",
        "Empty",
        "--template",
        "Default Profile",
    ]);
    assert_eq!(empty["project"]["name"], "Empty-1");
    assert_eq!(fs::read_dir(f.projects.join("Empty-1")).unwrap().count(), 0);
    let p = f.json(&[
        "project",
        "create",
        "--template",
        "Custom",
        "--base-name",
        "With Spaces",
    ]);
    let name = p["project"]["name"].as_str().unwrap();
    assert_eq!(
        fs::read_to_string(f.projects.join(name).join("README.md")).unwrap(),
        "fixture template"
    );
    let started = f.json(&["session", "start", name]);
    let id = started["session"]["id"].as_str().unwrap();
    let listed = f.output(&["session", "list", "--json"]);
    assert!(listed.status.success());
    let list: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(list["active_session_id"], id);
    let before = fs::read(f.state.join("session-manager/runtime-v2.json")).unwrap();
    let bad = f.output(&["--json", "session", "start", "../escape"]);
    assert!(!bad.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&bad.stdout).unwrap()["code"],
        "validation.invalid_request"
    );
    assert_eq!(
        fs::read(f.state.join("session-manager/runtime-v2.json")).unwrap(),
        before
    );
    drop(daemon);
    assert!(!f.runtime.join("session-manager/ipc-v2.sock").exists());
    let _daemon = f.daemon();
    assert_eq!(f.json(&["session", "list"])["active_session_id"], id);
    fs::remove_dir_all(f.projects.join(name)).unwrap();
    assert_eq!(
        f.json(&["application", "list"])["applications"][0]["name"],
        "Fixture App"
    );
    f.json(&["session", "switch", id]);
    f.json(&["session", "stop", id]);
    assert!(f.json(&["session", "list"])["active_session_id"].is_null());
}

#[test]
fn clients_do_not_read_config_state_or_application_environment_and_help_is_offline() {
    let f = Fixture::new();
    let _daemon = f.daemon();
    let output = f
        .command()
        .env("XDG_CONFIG_HOME", "relative-invalid")
        .env("XDG_STATE_HOME", "relative-invalid")
        .env("XDG_DATA_DIRS", "relative-invalid")
        .env_remove("HOME")
        .args(["--json", "application", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["applications"][0]["name"],
        "Fixture App"
    );
    let output = f
        .command()
        .env_remove("XDG_RUNTIME_DIR")
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("session start")
    );
}

#[test]
fn invalid_arguments_and_startup_failure_never_create_a_project_or_discard_state() {
    let f = Fixture::new();
    let output = f.output(&["--json", "profile", "delete", "anything"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["code"],
        "cli.invalid_arguments"
    );
    assert_eq!(fs::read_dir(&f.projects).unwrap().count(), 0);
    fs::write(
        f.config.join("config-v2.toml"),
        "schema_version=1\nprofiles=[]\n",
    )
    .unwrap();
    let output = f.output(&["daemon"]);
    assert!(!output.status.success());
    assert!(!f.runtime.join("session-manager/ipc-v2.sock").exists());
    assert!(!f.state.join("session-manager/runtime-v2.json").exists());
}

#[test]
fn old_artifacts_are_ignored_and_a_second_daemon_cannot_take_over() {
    let f = Fixture::new();
    fs::write(f.config.join("config.toml"), "corrupt old config").unwrap();
    let legacy = f.state.join("session-manager/transactions");
    fs::create_dir_all(&legacy).unwrap();
    fs::set_permissions(legacy.parent().unwrap(), fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(legacy.join("pending.json"), "corrupt old intent").unwrap();
    let old_state = legacy.parent().unwrap();
    let checkpoints = old_state.join("checkpoints");
    fs::create_dir(&checkpoints).unwrap();
    let old_files = [
        old_state.join("runtime.json"),
        old_state.join("runtime.previous.json"),
        checkpoints.join("old-envelope.json"),
        checkpoints.join("old-payload.bin"),
    ];
    let bytes = [0xff, 0, 17, 42];
    for file in &old_files {
        fs::write(file, bytes).unwrap();
    }
    let _daemon = f.daemon();
    let output = f.output(&["daemon"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("conflict.state_locked")
    );
    assert_eq!(
        fs::read_to_string(legacy.join("pending.json")).unwrap(),
        "corrupt old intent"
    );
    assert_eq!(f.json(&["session", "list"])["revision"], 0);
    fs::create_dir(f.projects.join("New")).unwrap();
    f.json(&["session", "start", "New"]);
    for file in &old_files {
        assert_eq!(fs::read(file).unwrap(), bytes);
    }
}

#[test]
fn missing_state_parents_are_created_but_existing_socket_files_are_never_overwritten() {
    let f = Fixture::new();
    let nested = f.root.join("new/deep/state");
    let child = f
        .command()
        .env("XDG_STATE_HOME", &nested)
        .arg("daemon")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut daemon = Daemon { child: Some(child) };
    let start = Instant::now();
    while !nested.join("session-manager").exists() {
        assert!(daemon.child.as_mut().unwrap().try_wait().unwrap().is_none());
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(10));
    }
    drop(daemon);
    let runtime = f.runtime.join("session-manager");
    fs::create_dir_all(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let socket = runtime.join("ipc-v2.sock");
    fs::write(&socket, "not a socket: keep").unwrap();
    let output = f.output(&["daemon"]);
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(socket).unwrap(), "not a socket: keep");
}

#[test]
fn generic_launcher_example_only_passes_data_to_an_explicit_fixture_adapter() {
    let f = Fixture::new();
    let _daemon = f.daemon();
    let binary = f.command().get_program().to_owned();
    let adapter = "import json,sys; apps=[json.loads(line) for line in sys.stdin]; print(json.dumps({'id':apps[0]['id']}))";
    let output = f.command();
    // Use the fixture's replaced environment, but run only Python and this embedded example.
    let mut python = Command::new("/usr/bin/python3");
    python
        .env_clear()
        .envs(output.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))));
    let output = python
        .arg("-c")
        .arg(include_str!("../examples/launcher-client.py"))
        .arg("--session-manager")
        .arg(binary)
        .args(["--", "/usr/bin/python3", "-c", adapter])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let selected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(selected["id"], "app.desktop");
    assert_eq!(selected["exec"], "never-executed");
}

#[test]
fn unsafe_runtime_permissions_and_symlink_state_parents_fail_without_writes() {
    let f = Fixture::new();
    fs::set_permissions(&f.runtime, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!f.output(&["daemon"]).status.success());
    assert!(!f.state.join("session-manager").exists());
    fs::set_permissions(&f.runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::remove_dir(&f.state).unwrap();
    symlink(&outside, &f.state).unwrap();
    assert!(!f.output(&["daemon"]).status.success());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    fs::remove_file(&f.state).unwrap();
}

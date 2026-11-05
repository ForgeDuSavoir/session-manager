//! Protocol/transport acceptance with exclusively owned temporary state and sockets.
#![allow(clippy::many_single_char_names)]
use crate::{
    application::Core,
    applications::{ApplicationEnvironment, ApplicationResolver},
    canonical_json,
    cli::{self, Action},
    configuration::Locations,
    interface::{Command, MAX_LINE_BYTES, Response, Service},
    test_support::TemporaryXdgRoot,
    transport::{self, FrameError, IoLimits, Server},
};
use std::{
    fs,
    io::{Read, Write},
    net::Shutdown,
    os::unix::{fs::symlink, net::UnixStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use uuid::Uuid;
struct Fixture {
    root: Option<TemporaryXdgRoot>,
    locations: Locations,
    source: PathBuf,
    data: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = TemporaryXdgRoot::new().unwrap();
        let locations = Locations {
            config: root.root().join("config/session-manager"),
            state: root.root().join("state/session-manager"),
            runtime: root.root().join("runtime/session-manager"),
        };
        fs::create_dir(&locations.config).unwrap();
        let source = root.root().join("projects");
        fs::create_dir(&source).unwrap();
        fs::write(
            locations.config.join("config-v2.toml"),
            format!(
                "schema_version=2\nproject_source={:?}",
                source.to_str().unwrap()
            ),
        )
        .unwrap();
        let data = root.root().join("data");
        fs::create_dir_all(data.join("applications")).unwrap();
        fs::write(
            data.join("applications/a.desktop"),
            "[Desktop Entry]\nType=Application\nName=Installed\nExec=never-executed\n",
        )
        .unwrap();
        Self {
            root: Some(root),
            locations,
            source,
            data,
        }
    }
    fn service(&self) -> Service {
        let env = ApplicationEnvironment::from_values(
            &self.root.as_ref().unwrap().home(),
            Some(self.data.to_str().unwrap()),
            Some(self.data.to_str().unwrap()),
            Some(""),
            Some(""),
        )
        .unwrap();
        Service::new(
            Core::open(&self.locations).unwrap(),
            ApplicationResolver::new(env),
        )
    }
    fn running(&self) -> Running {
        let server = Server::bind(&self.locations.runtime, self.service()).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let worker = thread::spawn(move || server.serve_until(&flag).unwrap());
        Running {
            stop,
            worker: Some(worker),
        }
    }
    fn call(&self, cmd: &Command) -> serde_json::Value {
        transport::call(&self.locations.runtime, cmd).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            let _ = root.cleanup();
        }
    }
}
struct Running {
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(w) = self.worker.take() {
            w.join().unwrap();
        }
    }
}
fn start(name: &str) -> Command {
    Command::Start {
        project_name: name.into(),
    }
}
fn request(service: &Service, cmd: &Command) -> Response {
    service.handle(&canonical_json::encode_line(&cmd.request(Uuid::nil())).unwrap())
}
fn short_limits() -> IoLimits {
    IoLimits {
        stall: Duration::from_millis(60),
        total: Duration::from_millis(150),
        max: 1024,
    }
}

#[test]
fn api_full_flow_restart_fallback_and_cli_compatible_results() {
    let f = Fixture::new();
    let running = f.running();
    assert_eq!(
        f.call(&Command::Templates)["templates"][0]["name"],
        "Default Profile"
    );
    assert_eq!(
        f.call(&Command::Applications)["applications"][0]["name"],
        "Installed"
    );
    let project = f.call(&Command::Create {
        base_name: "Work".into(),
        template_name: "Default Profile".into(),
    });
    assert_eq!(project["project"]["name"], "Work-1");
    let a = f.call(&start("Work-1"));
    let a_id = Uuid::parse_str(a["session"]["id"].as_str().unwrap()).unwrap();
    let b = f.call(&start("Work-1"));
    let b_id = Uuid::parse_str(b["session"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(a["revision"], 1);
    assert_eq!(b["revision"], 2);
    assert_eq!(b["session"]["name"], "Work-1-2");
    let apps = f.source.join("Work-1/.session-manager/apps");
    fs::create_dir_all(&apps).unwrap();
    fs::write(
        apps.join("a.desktop"),
        "[Desktop Entry]\nType=Application\nName=Project\nExec=never-executed\n",
    )
    .unwrap();
    assert_eq!(
        f.call(&Command::Applications)["applications"][0]["name"],
        "Project"
    );
    let switched = f.call(&Command::Switch { session_id: a_id });
    assert_eq!(switched["revision"], 3);
    assert_eq!(switched["session"]["active"], true);
    drop(running);
    assert!(!f.locations.runtime.join("ipc-v2.sock").exists());
    let _running = f.running();
    assert_eq!(
        f.call(&Command::Sessions)["active_session_id"],
        a_id.to_string()
    );
    fs::remove_dir_all(f.source.join("Work-1")).unwrap();
    assert_eq!(
        f.call(&Command::Applications)["applications"][0]["name"],
        "Installed"
    );
    let stopped = f.call(&Command::Stop { session_id: a_id });
    assert_eq!(stopped["revision"], 4);
    assert_eq!(stopped["stopped_session_id"], a_id.to_string());
    assert!(f.call(&Command::Sessions)["active_session_id"].is_null());
    f.call(&Command::Switch { session_id: b_id });
    f.call(&Command::Stop { session_id: b_id });
    assert_eq!(
        f.call(&Command::Sessions)["sessions"],
        serde_json::json!([])
    );
}

#[test]
fn deactivate_preserves_sessions_and_installed_only_apps_across_restart() {
    let f = Fixture::new();
    fs::create_dir(f.source.join("P")).unwrap();
    let running = f.running();
    let idle = f.call(&Command::Deactivate);
    assert_eq!(
        idle,
        serde_json::json!({"revision":0,"active_session_id":null})
    );
    let a = f.call(&start("P"));
    let a_id = Uuid::parse_str(a["session"]["id"].as_str().unwrap()).unwrap();
    let b = f.call(&start("P"));
    let b_id = Uuid::parse_str(b["session"]["id"].as_str().unwrap()).unwrap();
    fs::create_dir_all(f.source.join("P/.session-manager/apps")).unwrap();
    fs::write(
        f.source.join("P/.session-manager/apps/a.desktop"),
        "[Desktop Entry]\nType=Application\nName=Project\nExec=never-executed\n",
    )
    .unwrap();
    assert_eq!(
        f.call(&Command::Applications)["applications"][0]["name"],
        "Project"
    );
    let neutral = f.call(&Command::Deactivate);
    assert_eq!(
        neutral,
        serde_json::json!({"revision":3,"active_session_id":null})
    );
    assert_eq!(f.call(&Command::Deactivate), neutral);
    assert_eq!(
        f.call(&Command::Applications)["applications"][0]["name"],
        "Installed"
    );
    let sessions = f.call(&Command::Sessions);
    assert_eq!(sessions["sessions"].as_array().unwrap().len(), 2);
    assert!(
        sessions["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["active"] == false)
    );
    drop(running);
    let _running = f.running();
    assert_eq!(
        f.call(&Command::Sessions)["active_session_id"],
        serde_json::Value::Null
    );
    assert_eq!(
        f.call(&Command::Switch { session_id: a_id })["session"]["active"],
        true
    );
    assert_eq!(
        f.call(&Command::Applications)["applications"][0]["name"],
        "Project"
    );
    f.call(&Command::Stop { session_id: b_id });
    assert_eq!(
        f.call(&Command::Sessions)["active_session_id"],
        a_id.to_string()
    );
    let bad = serde_json::json!({"protocol_version":2,"request_id":Uuid::nil(),
        "method":"session.deactivate","params":{"session_id":a_id}});
    let mut conn = UnixStream::connect(f.locations.runtime.join("ipc-v2.sock")).unwrap();
    conn.write_all(&canonical_json::encode_line(&bad).unwrap())
        .unwrap();
    let mut reply = String::new();
    conn.read_to_string(&mut reply).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&reply).unwrap()["error"]["code"],
        "validation.invalid_request"
    );
    assert_eq!(f.call(&Command::Sessions)["revision"], 5);
}

#[test]
fn strict_requests_reject_unknown_fields_types_versions_and_duplicate_keys_without_mutation() {
    let f = Fixture::new();
    fs::create_dir(f.source.join("P")).unwrap();
    let service = f.service();
    let original = Command::Start {
        project_name: "P".into(),
    }
    .request(Uuid::nil());
    let mut bad = Vec::new();
    for (key, value) in [
        ("protocol_version", serde_json::json!(1)),
        ("params", serde_json::json!([])),
        (
            "params",
            serde_json::json!({"project_name":"P","task_name":"old"}),
        ),
        ("request_id", serde_json::json!("not-a-uuid")),
        ("method", serde_json::json!("profile.create")),
    ] {
        let mut v = original.clone();
        v[key] = value;
        bad.push(v);
    }
    let mut extra = original.clone();
    extra["configuration_etag"] = serde_json::json!("old");
    bad.push(extra);
    for value in bad {
        let r = service.handle(&canonical_json::encode_line(&value).unwrap());
        assert!(r.result.is_none() && r.error.is_some());
    }
    for raw in [b"{\"request_id\":\"00000000-0000-0000-0000-000000000000\",\"params\":{},\"params\":{}}".as_slice(),b"{\"protocol_version\":2,\"request_id\":\"00000000-0000-0000-0000-000000000000\",\"method\":\"session.start\",\"params\":{\"project_name\":\"P\",\"project_name\":\"P\"}}",b"\xff\n",b"{} {}\n"] {assert_eq!(service.handle(raw).error.unwrap().code,"protocol.invalid_request");}
    assert_eq!(service.core.sessions().unwrap().revision, 0);
    assert!(!f.locations.state.join("runtime-v2.json").exists());
    let mut old = original;
    old["protocol_version"] = serde_json::json!(1);
    assert_eq!(
        service
            .handle(&canonical_json::encode_line(&old).unwrap())
            .error
            .unwrap()
            .code,
        "protocol.unsupported_version"
    );
}

#[test]
fn protocol_inventory_is_closed_and_errors_are_canonical_correlated_and_private() {
    let f = Fixture::new();
    let service = f.service();
    for method in [
        "system.hello",
        "profile.list",
        "task.list",
        "adapter.list",
        "session.pause",
        "session.resume",
        "event.subscribe",
        "config.reload",
    ] {
        let v = serde_json::json!({"protocol_version":2,"request_id":Uuid::nil(),"method":method,"params":{}});
        let r = service.handle(&canonical_json::encode_line(&v).unwrap());
        assert_eq!(r.error.as_ref().unwrap().code, "protocol.unknown_method");
        assert_eq!(r.request_id, Some(Uuid::nil()));
        let bytes = r.encode().unwrap();
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert!(
            !String::from_utf8(bytes)
                .unwrap()
                .contains(f.source.to_str().unwrap())
        );
    }
    let r = request(&service, &start("missing"));
    assert_eq!(r.error.unwrap().code, "not_found.project");
    let null = service.handle(b"invalid\n");
    assert!(null.request_id.is_none());
}

#[test]
fn concurrent_mutation_results_contain_their_own_committed_revisions() {
    let f = Fixture::new();
    fs::create_dir(f.source.join("P")).unwrap();
    let _running = f.running();
    let workers: Vec<_> = (0..12)
        .map(|_| {
            let runtime = f.locations.runtime.clone();
            thread::spawn(move || transport::call(&runtime, &start("P")).unwrap())
        })
        .collect();
    let mut results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    results.sort_by_key(|r| r["revision"].as_u64().unwrap());
    for (i, r) in results.iter().enumerate() {
        assert_eq!(r["revision"].as_u64().unwrap(), i as u64 + 1);
        assert_eq!(r["session"]["number"].as_u64().unwrap(), i as u64 + 1);
        assert_eq!(r["session"]["active"], true);
    }
    assert_eq!(f.call(&Command::Sessions)["revision"], 12);
}

#[test]
fn socket_permissions_stale_entries_and_replacement_cleanup_preserve_user_files() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new();
    let service = f.service();
    let path = f.locations.runtime.join("ipc-v2.sock");
    fs::write(&path, "keep").unwrap();
    assert!(Server::bind(&f.locations.runtime, service).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "keep");
    fs::remove_file(&path).unwrap();
    let server = Server::bind(&f.locations.runtime, f.service()).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
    fs::remove_file(&path).unwrap();
    fs::write(&path, "replacement").unwrap();
    drop(server);
    assert_eq!(fs::read_to_string(&path).unwrap(), "replacement");
    fs::remove_file(&path).unwrap();
    symlink("/nonexistent", &path).unwrap();
    assert!(Server::bind(&f.locations.runtime, f.service()).is_err());
    assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
    fs::remove_file(path).unwrap();
}

#[test]
fn foreign_peer_uid_is_rejected_before_request_decode_or_mutation() {
    let f = Fixture::new();
    fs::create_dir(f.source.join("P")).unwrap();
    let service = f.service();
    let (a, mut b) = UnixStream::pair().unwrap();
    let bytes = canonical_json::encode_line(&start("P").request(Uuid::nil())).unwrap();
    b.write_all(&bytes).unwrap();
    transport::serve_connection(
        a,
        &service,
        nix::unistd::getuid().as_raw().wrapping_add(1),
        short_limits(),
    );
    let mut buf = [0; 32];
    assert!(!matches!(b.read(&mut buf),Ok(n) if n>0));
    assert_eq!(service.core.sessions().unwrap().revision, 0);
}

#[test]
fn framing_enforces_lf_exact_size_partial_eof_and_read_stalls() {
    let limits = IoLimits {
        max: 8,
        ..short_limits()
    };
    let (mut a, mut b) = UnixStream::pair().unwrap();
    b.write_all(b"1234567\nignored\n").unwrap();
    assert_eq!(
        transport::read_frame(&mut a, limits).unwrap().unwrap(),
        b"1234567\n"
    );
    let (mut a, mut b) = UnixStream::pair().unwrap();
    b.write_all(b"12345678").unwrap();
    assert_eq!(
        transport::read_frame(&mut a, limits).unwrap_err(),
        FrameError::TooLarge
    );
    let (mut a, mut b) = UnixStream::pair().unwrap();
    b.write_all(b"partial").unwrap();
    b.shutdown(Shutdown::Write).unwrap();
    assert!(transport::read_frame(&mut a, limits).unwrap().is_none());
    let (mut a, _b) = UnixStream::pair().unwrap();
    assert_eq!(
        transport::read_frame(&mut a, limits).unwrap_err(),
        FrameError::Timeout
    );
}

#[test]
fn trickle_progress_does_not_reset_total_deadline_and_writes_are_bounded() {
    let (mut a, mut b) = UnixStream::pair().unwrap();
    let writer = thread::spawn(move || {
        for _ in 0..20 {
            if b.write_all(b"x").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(15));
        }
    });
    let limits = IoLimits {
        stall: Duration::from_millis(100),
        total: Duration::from_millis(70),
        max: 1024,
    };
    assert_eq!(
        transport::read_frame(&mut a, limits).unwrap_err(),
        FrameError::Timeout
    );
    drop(a);
    writer.join().unwrap();
    let (mut a, _b) = UnixStream::pair().unwrap();
    let bytes = vec![b'x'; MAX_LINE_BYTES];
    assert_eq!(
        transport::write_frame(
            &mut a,
            &bytes,
            IoLimits {
                max: MAX_LINE_BYTES,
                ..short_limits()
            }
        )
        .unwrap_err(),
        FrameError::Timeout
    );
}

#[test]
fn connection_admission_closes_the_33rd_without_reading_or_starting_a_session() {
    let f = Fixture::new();
    fs::create_dir(f.source.join("P")).unwrap();
    let _running = f.running();
    let streams: Vec<_> = (0..32)
        .map(|_| UnixStream::connect(f.locations.runtime.join("ipc-v2.sock")).unwrap())
        .collect();
    assert!(transport::call(&f.locations.runtime, &start("P")).is_err());
    for stream in streams {
        let _ = stream.shutdown(Shutdown::Both);
    }
    let start = std::time::Instant::now();
    loop {
        if let Ok(result) = transport::call(&f.locations.runtime, &Command::Sessions) {
            assert_eq!(result["revision"], 0);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn one_connection_never_admits_a_second_request_and_partial_mutations_are_not_admitted() {
    let f = Fixture::new();
    fs::create_dir(f.source.join("P")).unwrap();
    let _running = f.running();
    let bytes = canonical_json::encode_line(&start("P").request(Uuid::nil())).unwrap();
    let mut stream = UnixStream::connect(f.locations.runtime.join("ipc-v2.sock")).unwrap();
    stream
        .write_all(&[bytes.as_slice(), bytes.as_slice()].concat())
        .unwrap();
    let response = transport::read_frame(&mut stream, IoLimits::default())
        .unwrap()
        .unwrap();
    assert!(transport::decode_response(&response, Uuid::nil()).is_ok());
    let mut stream = UnixStream::connect(f.locations.runtime.join("ipc-v2.sock")).unwrap();
    stream.write_all(&bytes[..bytes.len() - 1]).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let mut closed = Vec::new();
    let _ = stream.read_to_end(&mut closed);
    assert_eq!(closed.len(), 0);
    assert_eq!(
        f.call(&Command::Sessions)["sessions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn client_rejects_mismatched_or_malformed_replies() {
    for response in [
        serde_json::json!({"protocol_version":2,"request_id":Uuid::new_v4(),"type":"response","result":{}}),
        serde_json::json!({"protocol_version":2,"request_id":Uuid::nil(),"type":"response","result":{},"error":{"code":"x"}}),
        serde_json::json!({"protocol_version":2,"request_id":Uuid::nil(),"type":"response","error":{"code":"bad\nsecret"}}),
        serde_json::json!({"protocol_version":1,"request_id":Uuid::nil(),"type":"response","result":{}}),
    ] {
        assert_eq!(
            transport::decode_response(
                &canonical_json::encode_line(&response).unwrap(),
                Uuid::nil()
            )
            .unwrap_err()
            .code,
            "transport.invalid_response"
        );
    }
}

#[test]
fn cli_grammar_matches_the_closed_inventory_and_escapes_names_as_data() {
    let args = [
        "project",
        "create",
        "--base-name",
        "--json",
        "--template",
        "--json",
    ]
    .map(str::to_owned);
    let invocation = cli::parse(&args).unwrap();
    assert!(!invocation.json);
    assert_eq!(
        invocation.action,
        Action::Request(Command::Create {
            base_name: "--json".into(),
            template_name: "--json".into()
        })
    );
    for args in [
        vec!["project", "list", "--json"],
        vec!["--json", "session", "start", "Name With Spaces"],
        vec!["session", "deactivate", "--json"],
        vec![
            "project",
            "create",
            "--template",
            "Default Profile",
            "--base-name",
            "Name",
        ],
    ] {
        let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
        assert!(matches!(
            cli::parse(&args).unwrap().action,
            Action::Request(_)
        ));
    }
    for args in [
        vec!["daemon", "--json"],
        vec!["session", "pause", "x"],
        vec!["session", "deactivate", "unexpected"],
        vec!["project", "create", "--base-name", "A"],
        vec!["project", "list", "extra"],
        vec!["session", "stop", "not-a-uuid"],
        vec!["--json", "--json", "session", "list"],
    ] {
        let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
        assert!(cli::parse(&args).is_err());
    }
}

#[test]
fn reply_encoder_never_emits_an_over_bound_response() {
    let response = Response {
        protocol_version: 2,
        request_id: Some(Uuid::nil()),
        kind: "response",
        result: Some(serde_json::json!({"too_large":"x".repeat(MAX_LINE_BYTES)})),
        error: None,
    };
    let bytes = response.encode().unwrap();
    assert!(bytes.len() < 1024);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["error"]["code"],
        "limit.exceeded"
    );
}

#[test]
fn lost_mutation_reply_is_uncertain_without_retry_or_rollback() {
    let f = Fixture::new();
    fs::create_dir(f.source.join("P")).unwrap();
    let service = f.service();
    let path = f.locations.runtime.join("ipc-v2.sock");
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let frame = transport::read_frame(&mut stream, IoLimits::default())
            .unwrap()
            .unwrap();
        assert!(service.handle(&frame).result.is_some());
        drop(stream); // The mutation committed, but its response is deliberately lost.
        listener.set_nonblocking(true).unwrap();
        thread::sleep(Duration::from_millis(40));
        assert!(matches!(listener.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));
        service.core.sessions().unwrap()
    });
    assert_eq!(
        transport::call(&f.locations.runtime, &start("P"))
            .unwrap_err()
            .code,
        "transport.uncertain"
    );
    let state = worker.join().unwrap();
    assert_eq!(state.revision, 1);
    assert_eq!(state.sessions.len(), 1);
    fs::remove_file(path).unwrap();
}

#[test]
fn client_refuses_nonprivate_sockets_and_oversized_input_is_rejected_before_decode() {
    let f = Fixture::new();
    let service = f.service();
    let response = service.handle(&vec![b'x'; MAX_LINE_BYTES + 1]);
    assert_eq!(response.error.unwrap().code, "protocol.message_too_large");
    assert_eq!(service.core.sessions().unwrap().revision, 0);
    let server = Server::bind(&f.locations.runtime, service).unwrap();
    let path = f.locations.runtime.join("ipc-v2.sock");
    fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o666)).unwrap();
    assert_eq!(
        transport::call(&f.locations.runtime, &Command::Sessions)
            .unwrap_err()
            .code,
        "transport.unavailable"
    );
    drop(server);
}

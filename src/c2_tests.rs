//! Isolated C2 acceptance: no ambient environment, installed applications, or execution.
#![allow(clippy::many_single_char_names)]

use crate::{
    application::Core,
    applications::{
        Application, ApplicationEnvironment, ApplicationList, ApplicationResolver,
        ExecutableLookup, Origin, parse_exclusions,
    },
    canonical_json,
    configuration::Locations,
    desktop_entry::DesktopEntry,
    error::{Boundary, Faults, Result},
    test_support::TemporaryXdgRoot,
};
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{PermissionsExt, symlink},
    },
    path::PathBuf,
    sync::{Arc, Mutex},
};

struct Fixture {
    root: Option<TemporaryXdgRoot>,
    locations: Locations,
    source: PathBuf,
    data: [PathBuf; 3],
    bin: PathBuf,
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
        let data = [
            root.root().join("user-data"),
            root.root().join("system-first"),
            root.root().join("system-second"),
        ];
        for dir in &data {
            fs::create_dir_all(dir.join("applications")).unwrap();
        }
        let bin = root.root().join("bin");
        fs::create_dir(&bin).unwrap();
        Self {
            root: Some(root),
            locations,
            source,
            data,
            bin,
        }
    }
    fn core(&self) -> Core {
        Core::open(&self.locations).unwrap()
    }
    fn environment(&self, desktop: &str) -> ApplicationEnvironment {
        ApplicationEnvironment::from_values(
            &self.root.as_ref().unwrap().home(),
            Some(self.data[0].to_str().unwrap()),
            Some(&format!(
                "{}:{}",
                self.data[1].display(),
                self.data[2].display()
            )),
            Some(desktop),
            Some(self.bin.to_str().unwrap()),
        )
        .unwrap()
    }
    fn resolver(&self) -> ApplicationResolver {
        ApplicationResolver::new(self.environment("KDE"))
    }
    fn installed(&self, root: usize, relative: &str, name: &str, extra: &str) -> PathBuf {
        let path = self.data[root].join("applications").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, entry(name, extra)).unwrap();
        path
    }
    fn project(&self, name: &str) -> PathBuf {
        let path = self.source.join(name);
        fs::create_dir(&path).unwrap();
        path
    }
    fn local(&self, project: &str, id: &str, name: &str, extra: &str) -> PathBuf {
        let path = self
            .source
            .join(project)
            .join(".session-manager/apps")
            .join(id);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, entry(name, extra)).unwrap();
        path
    }
    fn exclude(&self, project: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self
            .source
            .join(project)
            .join(".session-manager/apps/exclude.txt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, contents).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(root) = self.root.take() {
            let _ = root.cleanup();
        }
    }
}
fn entry(name: &str, extra: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec=never-run %U\n{extra}")
}
fn code<T>(result: Result<T>, expected: &str) {
    match result {
        Err(e) => assert_eq!(e.0, expected),
        Ok(_) => panic!("expected {expected}"),
    }
}
fn ids(list: &ApplicationList) -> Vec<&str> {
    list.applications.iter().map(|a| a.id.as_str()).collect()
}
struct Hook<F>(F);
impl<F: FnMut(Boundary) -> Result<()>> Faults for Hook<F> {
    fn check(&mut self, b: Boundary) -> Result<()> {
        (self.0)(b)
    }
}
struct Probe(Arc<Mutex<Vec<String>>>);
impl ExecutableLookup for Probe {
    fn is_executable(&self, program: &str) -> bool {
        self.0.lock().unwrap().push(program.into());
        program == "available"
    }
}
struct NeverProbe;
impl ExecutableLookup for NeverProbe {
    fn is_executable(&self, program: &str) -> bool {
        panic!("unexpected executable lookup: {program}")
    }
}

#[test]
fn injected_xdg_defaults_priority_and_validation_do_not_touch_the_host() {
    let f = Fixture::new();
    let home = f.root.as_ref().unwrap().home();
    code(
        ApplicationEnvironment::from_values(
            &PathBuf::from("/tmp/bad\nhome"),
            None,
            None,
            None,
            None,
        ),
        "validation.invalid_request",
    );
    let defaults = ApplicationEnvironment::from_values(&home, None, None, None, None).unwrap();
    assert_eq!(
        defaults.application_roots(),
        [
            home.join(".local/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            PathBuf::from("/usr/share/applications")
        ]
    );
    // Inspect defaults only; never enumerate their real system roots.
    let duplicate = ApplicationEnvironment::from_values(
        &home,
        Some(f.data[0].to_str().unwrap()),
        Some(&format!(
            "{}:{}:{}",
            f.data[1].display(),
            f.data[0].display(),
            f.data[1].display()
        )),
        None,
        Some(""),
    )
    .unwrap();
    assert_eq!(duplicate.application_roots().len(), 2);
    for (data_home, data_dirs, path) in [
        (Some("relative"), None, None),
        (None, Some("/ok:relative"), None),
        (None, None, Some(":/bin")),
        (None, None, Some("relative")),
    ] {
        code(
            ApplicationEnvironment::from_values(&home, data_home, data_dirs, None, path),
            "validation.invalid_request",
        );
    }
    code(
        ApplicationEnvironment::from_values(&home, None, None, Some(&"x".repeat(65537)), None),
        "limit.exceeded",
    );
    code(
        ApplicationEnvironment::from_values(&home, None, None, Some("KDE\nGNOME"), None),
        "validation.invalid_request",
    );
}

#[test]
fn installed_priority_recursive_ids_ties_and_output_order_are_deterministic() {
    let f = Fixture::new();
    f.installed(2, "z.desktop", "Last", "");
    f.installed(2, "shared.desktop", "Last Shared", "");
    f.installed(1, "shared.desktop", "First Shared", "");
    f.installed(0, "shared.desktop", "User Shared", "");
    f.installed(0, "a/b.desktop", "Nested First", "");
    f.installed(0, "a-b.desktop", "Flat Later", "");
    f.installed(1, "only-system.desktop", "System", "");
    let resolver = f.resolver();
    let result = resolver.installed().unwrap();
    assert_eq!(
        ids(&result),
        [
            "a-b.desktop",
            "only-system.desktop",
            "shared.desktop",
            "z.desktop"
        ]
    );
    assert_eq!(result.applications[0].name, "Nested First");
    assert_eq!(result.applications[2].name, "User Shared");
    assert!(
        result
            .applications
            .iter()
            .all(|app| app.origin == Origin::Installed)
    );
    assert_eq!(
        canonical_json::encode_line(&result).unwrap(),
        canonical_json::encode_line(&resolver.installed().unwrap()).unwrap()
    );
}

#[test]
fn hidden_and_ineligible_high_priority_entries_mask_lower_files() {
    let f = Fixture::new();
    for (id, extra) in [
        ("hidden.desktop", "Hidden=true\n"),
        ("menu.desktop", "NoDisplay=true\n"),
        ("other.desktop", "OnlyShowIn=GNOME;\n"),
        ("missing.desktop", "TryExec=missing\n"),
    ] {
        f.installed(1, id, "Lower Visible", "");
        f.installed(0, id, "Higher Hidden", extra);
    }
    // The masked lower version is not parsed, so it cannot replace the winner or break the query.
    let masked = f.installed(1, "safe.desktop", "Masked", "");
    fs::write(masked, "invalid desktop text").unwrap();
    f.installed(0, "safe.desktop", "Winner", "");
    let result = f.resolver().installed().unwrap();
    assert_eq!(ids(&result), ["safe.desktop"]);
}

#[test]
fn desktop_visibility_uses_exact_current_desktop_tokens() {
    let f = Fixture::new();
    f.installed(0, "only.desktop", "Only", "OnlyShowIn=KDE;GNOME;\n");
    f.installed(0, "not.desktop", "Not", "NotShowIn=KDE;\n");
    f.installed(0, "case.desktop", "Case", "OnlyShowIn=kde;\n");
    f.installed(0, "all.desktop", "All", "");
    let query = |desktop| {
        ApplicationResolver::new(f.environment(desktop))
            .installed()
            .unwrap()
    };
    assert_eq!(ids(&query("KDE:Other")), ["all.desktop", "only.desktop"]);
    assert_eq!(ids(&query("")), ["all.desktop", "not.desktop"]);
    assert_eq!(
        ids(&query("kde")),
        ["all.desktop", "case.desktop", "not.desktop"]
    );
}

#[test]
fn desktop_text_is_decoded_without_translating_or_expanding_exec() {
    let f = Fixture::new();
    let p=f.installed(0,"native.desktop","Base\\sName","Name[fr]=Other\nIcon=icon-name\nTerminal=true\nCategories=Utility;TextEditor;\nActions=Extra;\nX-Test=opaque\n[Desktop Action Extra]\nName=Extra\nExec=ignored\n");
    let text = fs::read_to_string(&p).unwrap().replace(
        "Exec=never-run %U",
        "Exec=program \"quoted argument\" %U --literal=$VALUE",
    );
    fs::write(p, text.replace('\n', "\r\n")).unwrap();
    let app = f.resolver().installed().unwrap().applications.remove(0);
    assert_eq!(app.name, "Base Name");
    assert_eq!(app.exec, "program \"quoted argument\" %U --literal=$VALUE");
    assert_eq!(app.icon.as_deref(), Some("icon-name"));
    assert!(app.terminal);
}

#[test]
fn malformed_installed_files_fail_with_stable_private_errors() {
    let f = Fixture::new();
    let p = f.installed(0, "bad.desktop", "Bad", "");
    let malformed = [
        "",
        "[Desktop Entry]\nType=Application\nName=NoExec\n",
        "[Desktop Entry]\nType=Link\nName=Wrong\nExec=x\n",
        "[Desktop Entry]\nType=Application\nName=A\nName=B\nExec=x\n",
        "[Desktop Entry]\nType=Application\nName=A\nExec=x\n[Desktop Entry]\n",
        "[Desktop Entry]\nType=Application\nName=A\nExec=bad\\q\n",
        "[Desktop Entry]\nType=Application\nName=A\nExec=x\nTerminal=1\n",
        "[Desktop Entry]\nType=Application\nName=A\nExec=x\nEncoding=legacy\n",
        "[Desktop Entry]\nType=Application\nName=A\nExec=x\nOnlyShowIn=KDE;\nNotShowIn=GNOME;\n",
        "[Desktop Entry]\nType=Application\nName=A\rB\nExec=x\n",
        "[Desktop Entry]\nType=Application\nName=A\nExec=x\r",
    ];
    for contents in malformed {
        fs::write(&p, contents).unwrap();
        code(
            f.resolver().installed(),
            "application.invalid_installed_file",
        );
    }
    fs::write(&p, [0xff]).unwrap();
    code(
        f.resolver().installed(),
        "application.invalid_installed_file",
    );
    fs::write(&p, format!("\u{feff}{}", entry("BOM", ""))).unwrap();
    code(
        f.resolver().installed(),
        "application.invalid_installed_file",
    );
    let err = f.resolver().installed().unwrap_err();
    assert_eq!(err.to_string(), "application.invalid_installed_file");
    assert!(
        !err.to_string()
            .contains(f.root.as_ref().unwrap().root().to_str().unwrap())
    );
}

#[test]
fn exclusions_then_local_overrides_can_reintroduce_and_remove_ids() {
    let f = Fixture::new();
    for id in [
        "excluded.desktop",
        "overridden.desktop",
        "untouched.desktop",
        "local-hidden.desktop",
    ] {
        f.installed(0, id, "Installed", "");
    }
    f.project("P");
    f.exclude("P", "# comment\r\nexcluded.desktop\r\n\r\n");
    f.local("P", "excluded.desktop", "Reintroduced", "");
    f.local("P", "overridden.desktop", "Project", "");
    f.local("P", "new.desktop", "New", "");
    f.local("P", "local-hidden.desktop", "Removed", "Hidden=true\n");
    let c = f.core();
    c.start("P").unwrap();
    let result = c.applications(&f.resolver()).unwrap();
    assert_eq!(
        ids(&result),
        [
            "excluded.desktop",
            "new.desktop",
            "overridden.desktop",
            "untouched.desktop"
        ]
    );
    assert_eq!(result.applications[0].name, "Reintroduced");
    assert_eq!(result.applications[0].origin, Origin::Project);
    assert_eq!(result.applications[3].origin, Origin::Installed);
}

#[test]
fn ineligible_local_entries_mask_installed_even_after_exclusion() {
    let f = Fixture::new();
    f.project("P");
    for (id, extra) in [
        ("a.desktop", "NoDisplay=true\n"),
        ("b.desktop", "OnlyShowIn=Other;\n"),
        ("c.desktop", "TryExec=absent\n"),
    ] {
        f.installed(0, id, "Installed", "");
        f.local("P", id, "Local", extra);
    }
    f.exclude("P", "a.desktop\nb.desktop\n");
    let c = f.core();
    c.start("P").unwrap();
    assert_eq!(
        c.applications(&f.resolver()).unwrap().applications,
        Vec::new()
    );
}

#[test]
fn no_active_session_does_not_read_any_project_settings() {
    let f = Fixture::new();
    f.installed(0, "a.desktop", "Installed", "");
    f.project("P");
    f.exclude("P", [0xff]);
    let c = f.core();
    assert_eq!(ids(&c.applications(&f.resolver()).unwrap()), ["a.desktop"]);
    c.start("P").unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
    let id = c.sessions().unwrap().active_session_id.unwrap();
    c.stop(id).unwrap();
    assert_eq!(ids(&c.applications(&f.resolver()).unwrap()), ["a.desktop"]);
}

#[test]
fn queries_reflect_installed_and_project_edits_without_state_writes_or_restart() {
    let f = Fixture::new();
    let installed = f.installed(0, "a.desktop", "First", "");
    let p = f.project("P");
    let c = f.core();
    c.start("P").unwrap();
    let resolver = f.resolver();
    let current = f.locations.state.join("runtime-v2.json");
    let state = fs::read(&current).unwrap();
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "First"
    );
    fs::write(installed, entry("Second", "")).unwrap();
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Second"
    );
    let exclusions = f.exclude("P", "a.desktop\n");
    assert_eq!(c.applications(&resolver).unwrap().applications.len(), 0);
    let overlay = f.local("P", "a.desktop", "Local", "");
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Local"
    );
    fs::remove_file(overlay).unwrap();
    fs::remove_file(exclusions).unwrap();
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Second"
    );
    assert_eq!(fs::read(current).unwrap(), state);
    assert!(p.is_dir());
}

#[test]
fn rename_removal_and_same_name_replacement_use_the_current_directory() {
    let f = Fixture::new();
    f.installed(0, "a.desktop", "Installed", "");
    let p = f.project("P");
    f.local("P", "a.desktop", "Old Project", "");
    let c = f.core();
    let s = c.start("P").unwrap().session;
    let resolver = f.resolver();
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Old Project"
    );
    fs::rename(&p, f.source.join("Renamed")).unwrap();
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Installed"
    );
    f.project("P");
    f.local("P", "a.desktop", "New Project", "");
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "New Project"
    );
    fs::remove_dir_all(&p).unwrap();
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Installed"
    );
    assert_eq!(c.sessions().unwrap().sessions[0].session, s);
}

#[test]
fn missing_project_during_read_falls_back_even_with_detached_invalid_files() {
    let f = Fixture::new();
    f.installed(0, "a.desktop", "Installed", "");
    let p = f.project("P");
    f.exclude("P", [0xff]);
    let c = f.core();
    c.start("P").unwrap();
    let mut hook = Hook(|b| {
        if b == Boundary::ApplicationProjectOpened {
            fs::rename(&p, f.source.join("Detached")).unwrap();
        }
        Ok(())
    });
    let result = c
        .applications_with_faults(&f.resolver(), &mut hook)
        .unwrap();
    assert_eq!(result.applications[0].name, "Installed");
}

#[test]
fn missing_source_and_symlink_project_fall_back_without_following_links() {
    let f = Fixture::new();
    f.installed(0, "a.desktop", "Installed", "");
    let p = f.project("P");
    let c = f.core();
    c.start("P").unwrap();
    fs::remove_dir(&p).unwrap();
    symlink(&f.data[0], &p).unwrap();
    assert_eq!(
        c.applications(&f.resolver()).unwrap().applications[0].name,
        "Installed"
    );
    fs::remove_file(p).unwrap();
    fs::remove_dir(&f.source).unwrap();
    assert_eq!(
        c.applications(&f.resolver()).unwrap().applications[0].name,
        "Installed"
    );
}

#[test]
fn exclusion_format_is_exact_strict_and_bounded() {
    for invalid in [
        b"\xef\xbb\xbfa.desktop".as_slice(),
        b"a.desktop\na.desktop\n",
        b"/a.desktop\n",
        b"a\\b.desktop",
        b"a.desktop # inline",
        b"not-an-id",
        b"a\0.desktop",
        b"# control\t",
        b"a\rb.desktop",
        &[0xff],
    ] {
        code(
            parse_exclusions(invalid),
            "application.invalid_project_file",
        );
    }
    code(
        parse_exclusions("a\u{0085}.desktop".as_bytes()),
        "application.invalid_project_file",
    );
    code(
        parse_exclusions(b"a.desktop\r"),
        "application.invalid_project_file",
    );
    let ids = parse_exclusions(b"# comment\r\n\r\n a.desktop\nA.desktop\na.desktop\n").unwrap();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(" a.desktop"));
    let many = (0..1025)
        .map(|i| format!("{i}.desktop\n"))
        .collect::<String>();
    code(parse_exclusions(many.as_bytes()), "limit.exceeded");
    code(parse_exclusions(&vec![b'#'; 65537]), "limit.exceeded");
}

#[test]
fn malformed_project_files_fail_without_silently_using_installed_entries() {
    let f = Fixture::new();
    f.installed(0, "a.desktop", "Installed", "");
    f.project("P");
    let local = f.local("P", "a.desktop", "Local", "");
    let c = f.core();
    c.start("P").unwrap();
    fs::write(&local, "bad desktop content").unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
    fs::remove_file(local).unwrap();
    f.exclude("P", "a.desktop\na.desktop");
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
}

#[test]
fn local_symlinks_directories_fifos_and_symlink_ancestors_are_invalid() {
    let f = Fixture::new();
    f.project("P");
    let local = f.local("P", "a.desktop", "Local", "");
    let apps = local.parent().unwrap().to_owned();
    let c = f.core();
    c.start("P").unwrap();
    fs::remove_file(&local).unwrap();
    symlink("/nonexistent/foreign", &local).unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
    fs::remove_file(&local).unwrap();
    fs::create_dir(apps.join("nested")).unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
    fs::remove_dir(apps.join("nested")).unwrap();
    nix::unistd::mkfifo(&apps.join("fifo"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
    fs::remove_file(apps.join("fifo")).unwrap();
    fs::remove_dir(&apps).unwrap();
    symlink(&f.data[0], &apps).unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
    fs::remove_file(&apps).unwrap();
    let settings = apps.parent().unwrap();
    fs::remove_dir(settings).unwrap();
    symlink(&f.data[0], settings).unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
    fs::remove_file(settings).unwrap();
}

#[test]
fn installed_symlinks_and_nonregular_entries_are_skipped_without_opening() {
    let f = Fixture::new();
    f.installed(1, "a.desktop", "System", "");
    let high = f.data[0].join("applications");
    symlink("/nonexistent/foreign", high.join("a.desktop")).unwrap();
    symlink(&f.data[1], high.join("linked-directory")).unwrap();
    nix::unistd::mkfifo(&high.join("fifo.desktop"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    assert_eq!(ids(&f.resolver().installed().unwrap()), ["a.desktop"]);
    fs::remove_file(high.join("a.desktop")).unwrap();
    fs::remove_file(high.join("linked-directory")).unwrap();
    fs::remove_file(high.join("fifo.desktop")).unwrap();
}

#[test]
fn try_exec_checks_only_injected_path_or_absolute_executables_and_never_runs_them() {
    let f = Fixture::new();
    let program = f.bin.join("probe");
    let marker = f.bin.join("executed");
    fs::write(&program, format!("#!/bin/sh\ntouch {marker:?}\n")).unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
    f.installed(0, "bare.desktop", "Bare", "TryExec=probe\n");
    f.installed(
        0,
        "absolute.desktop",
        "Absolute",
        &format!("TryExec={}\n", program.display()),
    );
    f.installed(0, "relative.desktop", "Relative", "TryExec=./probe\n");
    f.installed(0, "missing.desktop", "Missing", "TryExec=missing\n");
    assert_eq!(
        ids(&f.resolver().installed().unwrap()),
        ["absolute.desktop", "bare.desktop"]
    );
    assert!(!marker.exists());
    fs::set_permissions(&program, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(f.resolver().installed().unwrap().applications.len(), 0);
}

#[test]
fn executable_lookup_can_be_trapped_and_is_not_used_for_exec_or_hidden_entries() {
    let f = Fixture::new();
    f.installed(0, "normal.desktop", "Normal", "");
    f.installed(
        0,
        "hidden.desktop",
        "Hidden",
        "Hidden=true\nTryExec=not-looked-up\n",
    );
    let result = ApplicationResolver::with_executables(f.environment(""), NeverProbe)
        .installed()
        .unwrap();
    assert_eq!(ids(&result), ["normal.desktop"]);
    let calls = Arc::new(Mutex::new(Vec::new()));
    f.installed(0, "yes.desktop", "Yes", "TryExec=available\n");
    f.installed(0, "no.desktop", "No", "TryExec=unavailable\n");
    let resolver =
        ApplicationResolver::with_executables(f.environment(""), Probe(Arc::clone(&calls)));
    let result = resolver.installed().unwrap();
    assert_eq!(ids(&result), ["normal.desktop", "yes.desktop"]);
    assert_eq!(*calls.lock().unwrap(), ["unavailable", "available"]);
}

#[test]
fn desktop_file_and_field_byte_limits_are_enforced() {
    let f = Fixture::new();
    let p = f.installed(0, "large.desktop", &"n".repeat(4096), "");
    assert_eq!(
        f.resolver().installed().unwrap().applications[0].name.len(),
        4096
    );
    fs::write(&p, entry(&"n".repeat(4097), "")).unwrap();
    code(f.resolver().installed(), "limit.exceeded");
    fs::File::create(&p)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    code(f.resolver().installed(), "limit.exceeded");
    f.project("P");
    let local = f.exclude("P", "");
    fs::File::create(local).unwrap().set_len(65537).unwrap();
    fs::remove_file(p).unwrap();
    let c = f.core();
    c.start("P").unwrap();
    code(c.applications(&f.resolver()), "limit.exceeded");
}

#[test]
fn installed_depth_and_total_traversal_bounds_include_ignored_entries() {
    let f = Fixture::new();
    let mut deep = f.data[0].join("applications");
    for _ in 0..16 {
        deep = deep.join("d");
        fs::create_dir(&deep).unwrap();
    }
    fs::write(deep.join("a.desktop"), entry("Deep", "")).unwrap();
    assert_eq!(f.resolver().installed().unwrap().applications.len(), 1);
    fs::create_dir(deep.join("overflow")).unwrap();
    code(f.resolver().installed(), "limit.exceeded");
    fs::remove_dir_all(f.data[0].join("applications/d")).unwrap();
    for i in 0..16384 {
        fs::write(f.data[0].join(format!("applications/ignored-{i}")), []).unwrap();
    }
    assert_eq!(f.resolver().installed().unwrap().applications.len(), 0);
    fs::write(f.data[1].join("applications/overflow"), []).unwrap();
    code(f.resolver().installed(), "limit.exceeded");
}

#[test]
fn unique_installed_id_local_count_and_final_result_bounds_fail_without_truncation() {
    let f = Fixture::new();
    for i in 0..4096 {
        f.installed(0, &format!("a{i:04}.desktop"), "App", "");
    }
    assert_eq!(f.resolver().installed().unwrap().applications.len(), 4096);
    let overflow = f.installed(0, "overflow.desktop", "Overflow", "");
    code(f.resolver().installed(), "limit.exceeded");
    fs::remove_file(overflow).unwrap();
    f.project("P");
    f.local("P", "new.desktop", "New", "");
    let c = f.core();
    c.start("P").unwrap();
    code(c.applications(&f.resolver()), "limit.exceeded");
    f.exclude("P", "a0000.desktop");
    assert_eq!(
        c.applications(&f.resolver()).unwrap().applications.len(),
        4096
    );
    for i in 0..1024 {
        f.local("P", &format!("local-{i}.desktop"), "Local", "");
    }
    code(c.applications(&f.resolver()), "limit.exceeded");
}

#[test]
fn complete_canonical_reply_limit_includes_protocol_framing() {
    let app = Application {
        id: "x.desktop".into(),
        name: "Quoted \" name\nUnicode é".into(),
        exec: "opaque %F".into(),
        icon: None,
        terminal: false,
        origin: Origin::Installed,
    };
    let list = ApplicationList {
        applications: vec![app],
    };
    let response = serde_json::json!({"protocol_version":2,"request_id":uuid::Uuid::nil(),"type":"response","result":&list});
    assert_eq!(
        list.response_size().unwrap(),
        canonical_json::line_size(&response).unwrap()
    );
    let f = Fixture::new();
    for i in 0..4096 {
        f.installed(0, &format!("a{i:04}.desktop"), &"n".repeat(4096), "");
    }
    code(f.resolver().installed(), "limit.exceeded");
}

#[test]
fn malformed_desktop_ids_fail_and_non_utf8_data_is_never_lossily_decoded() {
    let f = Fixture::new();
    let path = f.data[0]
        .join("applications")
        .join(std::ffi::OsString::from_vec(b"bad\xff.desktop".to_vec()));
    fs::write(&path, entry("Bad", "")).unwrap();
    code(
        f.resolver().installed(),
        "application.invalid_installed_file",
    );
    fs::remove_file(path).unwrap();
    let mut deep = f.data[0].join("applications");
    for _ in 0..2 {
        deep = deep.join("x".repeat(125));
        fs::create_dir(&deep).unwrap();
    }
    fs::write(deep.join("long.desktop"), entry("Long ID", "")).unwrap();
    code(
        f.resolver().installed(),
        "application.invalid_installed_file",
    );
}

#[test]
fn absent_xdg_roots_are_empty_but_unreadable_present_files_fail() {
    let f = Fixture::new();
    for dir in &f.data {
        fs::remove_dir_all(dir).unwrap();
    }
    assert_eq!(f.resolver().installed().unwrap().applications.len(), 0);
    if nix::unistd::getuid().is_root() {
        return;
    }
    let p = f.installed(0, "unreadable.desktop", "No", "");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o0)).unwrap();
    code(f.resolver().installed(), "filesystem.unavailable");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
    f.project("P");
    let p = f.local("P", "unreadable.desktop", "No", "");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o0)).unwrap();
    let c = f.core();
    c.start("P").unwrap();
    code(c.applications(&f.resolver()), "filesystem.unavailable");
    fs::set_permissions(p, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn parser_validates_ignored_localized_keys_groups_and_escapes() {
    for extra in [
        "Name[fr]=bad\\q\n",
        "X-Vendor=bad\\q\n",
        "[Other]\nKey=1\nKey=2\n",
        "[Other]\nKey=x\n[Other]\nKey=y\n",
        "StartupNotify=yes\n",
    ] {
        code(
            DesktopEntry::parse(entry("Valid", extra).as_bytes()),
            "application.invalid_desktop_entry",
        );
    }
    let parsed =
        DesktopEntry::parse(entry("A\\sB", "OnlyShowIn=KDE;Other\\;Name;\n").as_bytes()).unwrap();
    assert_eq!(parsed.only_show_in.unwrap(), ["KDE", "Other;Name"]);
}

#[test]
fn c2_production_sources_never_read_ambient_environment_or_execute_processes() {
    let source = [
        include_str!("applications.rs"),
        include_str!("desktop_entry.rs"),
        include_str!("application.rs"),
    ]
    .join("\n");
    for forbidden in [
        "std::env",
        "std::process",
        "Command::",
        "UnixStream",
        "crate::adapter_contract",
        "crate::checkpoint",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden C2 dependency: {forbidden}"
        );
    }
    let f = Fixture::new();
    f.project("P");
    f.installed(0, "a.desktop", "Fixture Only", "");
    let c = f.core();
    c.start("P").unwrap();
    let result = c
        .applications(&ApplicationResolver::with_executables(
            f.environment(""),
            NeverProbe,
        ))
        .unwrap();
    assert_eq!(ids(&result), ["a.desktop"]);
}

#[test]
fn switching_and_restart_select_only_the_active_recorded_project() {
    let f = Fixture::new();
    f.installed(0, "a.desktop", "Installed", "");
    f.project("A");
    f.project("B");
    f.local("A", "a.desktop", "Project A", "");
    f.local("B", "a.desktop", "Project B", "");
    let resolver = f.resolver();
    let c = f.core();
    let a = c.start("A").unwrap().session;
    let b = c.start("B").unwrap().session;
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Project B"
    );
    c.switch(a.id).unwrap();
    // A concurrent switch must not deadlock, or splice B's files into A's request.
    let mut hook = Hook(|point| {
        if point == Boundary::ApplicationProjectOpened {
            c.switch(b.id).unwrap();
        }
        Ok(())
    });
    assert_eq!(
        c.applications_with_faults(&resolver, &mut hook)
            .unwrap()
            .applications[0]
            .name,
        "Project A"
    );
    drop(c);
    let c = f.core();
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Project B"
    );
}

#[test]
fn installed_roots_appearing_disappearing_or_becoming_links_are_resolved_again() {
    let f = Fixture::new();
    let resolver = f.resolver();
    assert_eq!(resolver.installed().unwrap().applications.len(), 0);
    f.installed(0, "a.desktop", "New", "");
    assert_eq!(ids(&resolver.installed().unwrap()), ["a.desktop"]);
    fs::remove_dir_all(f.data[0].join("applications")).unwrap();
    assert_eq!(resolver.installed().unwrap().applications.len(), 0);
    let poison = f.root.as_ref().unwrap().root().join("not-an-xdg-root");
    fs::create_dir(&poison).unwrap();
    fs::write(poison.join("bad.desktop"), "malformed").unwrap();
    symlink(&poison, f.data[0].join("applications")).unwrap();
    f.installed(1, "b.desktop", "System", "");
    assert_eq!(ids(&resolver.installed().unwrap()), ["b.desktop"]);
    fs::remove_file(f.data[0].join("applications")).unwrap();
}

#[test]
fn executable_symlinks_presence_and_permissions_are_rechecked_without_execution() {
    let f = Fixture::new();
    let resolver = f.resolver();
    f.installed(0, "a.desktop", "Probe", "TryExec=alias\n");
    assert_eq!(resolver.installed().unwrap().applications.len(), 0);
    let target = f.bin.join("target");
    fs::write(&target, "never executed").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&target, f.bin.join("alias")).unwrap();
    assert_eq!(ids(&resolver.installed().unwrap()), ["a.desktop"]);
    fs::remove_file(&target).unwrap();
    assert_eq!(resolver.installed().unwrap().applications.len(), 0);
    fs::create_dir(&target).unwrap();
    assert_eq!(resolver.installed().unwrap().applications.len(), 0);
    fs::remove_file(f.bin.join("alias")).unwrap();
}

#[test]
fn local_non_utf8_desktop_ids_are_invalid_but_other_regular_files_are_not_config() {
    let f = Fixture::new();
    f.project("P");
    let local = f.local("P", "ok.desktop", "Local", "");
    let apps = local.parent().unwrap();
    fs::write(apps.join("README.md"), [0xff]).unwrap();
    let c = f.core();
    c.start("P").unwrap();
    assert_eq!(ids(&c.applications(&f.resolver()).unwrap()), ["ok.desktop"]);
    let bad = apps.join(std::ffi::OsString::from_vec(b"bad\xff.desktop".to_vec()));
    fs::write(&bad, entry("Bad", "")).unwrap();
    code(
        c.applications(&f.resolver()),
        "application.invalid_project_file",
    );
}

#[test]
fn recovery_gate_never_pretends_there_is_no_active_context() {
    let f = Fixture::new();
    let c = f.core();
    drop(c);
    fs::write(f.locations.state.join("runtime-v2.json"), "invalid").unwrap();
    f.installed(
        0,
        "a.desktop",
        "No probe allowed",
        "TryExec=must-not-be-looked-up\n",
    );
    let c = f.core();
    let resolver = ApplicationResolver::with_executables(f.environment(""), NeverProbe);
    code(c.applications(&resolver), "recovery.invalid_runtime");
}

#[test]
fn exact_reply_capacity_is_accepted_and_one_more_byte_is_rejected() {
    let limit = 16 * 1024 * 1024;
    let big = Application {
        id: "a0000.desktop".into(),
        name: "n".repeat(4096),
        exec: "x".into(),
        icon: None,
        terminal: false,
        origin: Origin::Installed,
    };
    let mut tail = Application {
        id: "tail.desktop".into(),
        name: "n".into(),
        ..big.clone()
    };
    let empty = ApplicationList {
        applications: Vec::new(),
    }
    .response_size()
    .unwrap();
    let unit = canonical_json::line_size(&big).unwrap();
    let tail_cost = canonical_json::line_size(&tail).unwrap();
    let count = (limit - empty - tail_cost) / unit;
    let mut list = ApplicationList {
        applications: (0..count)
            .map(|i| Application {
                id: format!("a{i:04}.desktop"),
                ..big.clone()
            })
            .collect(),
    };
    let padding = limit - list.response_size().unwrap() - tail_cost;
    let name_padding = padding.min(4095);
    tail.name.push_str(&"n".repeat(name_padding));
    tail.exec.push_str(&"x".repeat(padding - name_padding));
    assert!(tail.exec.len() <= 4096 && list.applications.len() < 4096);
    list.applications.push(tail);
    assert_eq!(list.response_size().unwrap(), limit);
    let response = serde_json::json!({"protocol_version": 2, "request_id": uuid::Uuid::nil(), "type": "response", "result": &list});
    assert_eq!(canonical_json::line_size(&response).unwrap(), limit);
    list.applications.last_mut().unwrap().exec.push('x');
    code(list.response_size(), "limit.exceeded");
}

#[test]
fn project_replacement_after_read_discards_old_overrides_and_next_query_uses_new_files() {
    let f = Fixture::new();
    f.installed(0, "a.desktop", "Installed", "");
    let project = f.project("P");
    f.local("P", "a.desktop", "Old", "");
    let c = f.core();
    c.start("P").unwrap();
    let resolver = f.resolver();
    let mut hook = Hook(|point| {
        if point == Boundary::ApplicationOverridesRead {
            fs::rename(&project, f.source.join("Detached")).unwrap();
            f.project("P");
            f.local("P", "a.desktop", "Replacement", "");
        }
        Ok(())
    });
    assert_eq!(
        c.applications_with_faults(&resolver, &mut hook)
            .unwrap()
            .applications[0]
            .name,
        "Installed"
    );
    assert_eq!(
        c.applications(&resolver).unwrap().applications[0].name,
        "Replacement"
    );
}

#[test]
fn nondirectory_and_unreadable_installed_roots_fail_instead_of_becoming_empty() {
    let f = Fixture::new();
    let apps = f.data[0].join("applications");
    fs::remove_dir(&apps).unwrap();
    fs::write(&apps, "not a directory").unwrap();
    code(f.resolver().installed(), "filesystem.unavailable");
    fs::remove_file(&apps).unwrap();
    fs::create_dir(&apps).unwrap();
    if !nix::unistd::getuid().is_root() {
        fs::set_permissions(&apps, fs::Permissions::from_mode(0o0)).unwrap();
        code(f.resolver().installed(), "filesystem.unavailable");
        fs::set_permissions(&apps, fs::Permissions::from_mode(0o700)).unwrap();
    }
}

#[test]
fn traversal_budget_reserves_parent_entries_before_recursing() {
    let f = Fixture::new();
    let apps = f.data[0].join("applications");
    let nested = apps.join("A");
    fs::create_dir(&nested).unwrap();
    fs::write(apps.join("ignored"), []).unwrap();
    for i in 0..16382 {
        fs::write(nested.join(format!("ignored-{i}")), []).unwrap();
    }
    fs::write(nested.join("a.desktop"), "malformed").unwrap();
    // The directory scan itself is over budget, before parsing any selected file.
    code(f.resolver().installed(), "limit.exceeded");
}

//! C1 acceptance uses only explicit, sentinel-owned temporary roots.
#![allow(clippy::many_single_char_names)]

use crate::{
    application::Core,
    configuration::{Locations, Settings},
    domain::{self, Lifecycle, RuntimeState, Session},
    error::{Boundary, Error, Faults, Result},
    projects::DEFAULT_TEMPLATE,
    runtime,
    test_support::{FakeUuidGenerator, TemporaryXdgRoot},
};
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, PermissionsExt, symlink},
    },
    path::PathBuf,
    sync::Arc,
};
use uuid::Uuid;

struct Fixture {
    root: Option<TemporaryXdgRoot>,
    locations: Locations,
    source: PathBuf,
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
                "schema_version = 2\nproject_source = {:?}\n",
                source.to_str().unwrap()
            ),
        )
        .unwrap();
        Self {
            root: Some(root),
            locations,
            source,
        }
    }
    fn core(&self) -> Core<FakeUuidGenerator> {
        Core::with_uuids(&self.locations, FakeUuidGenerator::new(1)).unwrap()
    }
    fn project(&self, name: &str) -> PathBuf {
        let p = self.source.join(name);
        fs::create_dir(&p).unwrap();
        p
    }
    fn template(&self, name: &str) -> PathBuf {
        let p = self.locations.config.join("profile-template").join(name);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn current(&self) -> PathBuf {
        self.locations.state.join("runtime-v2.json")
    }
    fn persist(&self, state: &RuntimeState) {
        fs::create_dir_all(&self.locations.state).unwrap();
        fs::set_permissions(&self.locations.state, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(self.current(), runtime::encode(state).unwrap()).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // A failed test with an unexpected link/socket leaves evidence, never unsafe cleanup.
        if let Some(root) = self.root.take() {
            let _ = root.cleanup();
        }
    }
}
fn code<T>(result: Result<T>, expected: &str) {
    match result {
        Err(e) => assert_eq!(e.0, expected),
        Ok(_) => panic!("expected {expected}"),
    }
}
struct Hook<F>(F);
impl<F: FnMut(Boundary) -> Result<()>> Faults for Hook<F> {
    fn check(&mut self, b: Boundary) -> Result<()> {
        (self.0)(b)
    }
}
fn fail_at(point: Boundary) -> impl Faults {
    Hook(move |b| {
        if b == point {
            Err(Error("filesystem.unavailable"))
        } else {
            Ok(())
        }
    })
}
fn sample(project: &str, number: u32, id: u128) -> Session {
    Session {
        id: Uuid::from_u128(id),
        project_name: project.into(),
        number,
        name: domain::generated_name(project, number),
        lifecycle: Lifecycle::Running,
    }
}

#[test]
fn discovery_is_immediate_current_and_metadata_free() {
    let f = Fixture::new();
    f.project("Zulu");
    let p = f.project("Alpha");
    fs::create_dir(p.join("nested")).unwrap();
    fs::write(f.source.join("file"), "not a project").unwrap();
    f.project("bad.");
    f.project(".session-manager-create-orphan");
    symlink(&p, f.source.join("alias")).unwrap();
    let c = f.core();
    assert_eq!(
        c.projects()
            .unwrap()
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        ["Alpha", "Zulu"]
    );
    assert_eq!(c.incomplete_creations().len(), 1);
    assert_eq!(c.lookup_project("Alpha").unwrap().path, p);
    code(c.start("alias"), "not_found.project");
    fs::rename(&p, f.source.join("Renamed")).unwrap();
    assert_eq!(c.projects().unwrap()[0].name, "Renamed");
    fs::remove_file(f.source.join("alias")).unwrap();
}

#[test]
fn missing_or_replaced_source_is_not_an_empty_list() {
    let f = Fixture::new();
    let c = f.core();
    fs::remove_dir(&f.source).unwrap();
    code(c.projects(), "filesystem.unavailable");
    code(c.start("missing"), "filesystem.unavailable");
    fs::write(&f.source, "file").unwrap();
    code(c.projects(), "filesystem.unavailable");
}

#[test]
fn names_settings_and_explicit_locations_are_strict() {
    for name in [
        "", ".", "..", "a/b", "a\\b", "a\0b", "a\nb", " a", "a ", "a.",
    ] {
        assert!(!domain::valid_name(name), "{name:?}");
    }
    assert!(domain::valid_name("MontageVidéo"));
    assert!(!domain::valid_name(&"é".repeat(65)));
    assert!(domain::valid_name(&"a".repeat(128)));
    for text in [
        "schema_version=1\nproject_source='/work'",
        "schema_version=2\nproject_source='relative'",
        "schema_version=2\nproject_source='/a/../b'",
        "schema_version=2\nproject_source='/'",
        "schema_version=2\nproject_source='/work'\nprofiles=[]",
        "schema_version=2\nproject_source='/work'\nschema_version=2",
    ] {
        code(
            Settings::decode(text.as_bytes()),
            "validation.invalid_request",
        );
    }
    code(Settings::decode(&[0xff]), "validation.invalid_request");
    code(Settings::decode(&vec![b' '; 65537]), "limit.exceeded");
    let f = Fixture::new();
    fs::write(
        f.locations.config.join("config-v2.toml"),
        format!(
            "schema_version=2\nproject_source={:?}",
            f.locations.state.to_str().unwrap()
        ),
    )
    .unwrap();
    code(Core::open(&f.locations), "validation.invalid_request");
}

#[test]
fn symlink_source_ancestors_and_config_are_rejected() {
    let f = Fixture::new();
    let alias = f.source.with_file_name("alias");
    symlink(&f.source, &alias).unwrap();
    fs::write(
        f.locations.config.join("config-v2.toml"),
        format!(
            "schema_version=2\nproject_source={:?}",
            alias.to_str().unwrap()
        ),
    )
    .unwrap();
    code(Core::open(&f.locations), "filesystem.unavailable");
    fs::remove_file(alias).unwrap();
    let config = f.locations.config.join("config-v2.toml");
    let real = f.locations.config.join("real.toml");
    fs::rename(&config, &real).unwrap();
    symlink(&real, &config).unwrap();
    code(Core::open(&f.locations), "filesystem.unavailable");
    fs::remove_file(config).unwrap();
}

#[test]
fn built_in_and_custom_templates_copy_only_contents_with_private_modes() {
    let f = Fixture::new();
    let template = f.template("Custom");
    fs::create_dir(template.join(".session-manager")).unwrap();
    fs::write(
        template.join(".session-manager/tasks"),
        "ignored definition",
    )
    .unwrap();
    fs::write(template.join("README.md"), "hello").unwrap();
    fs::write(template.join("data.bin"), [0xff, 0, 42]).unwrap();
    fs::set_permissions(
        template.join("README.md"),
        fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    let c = f.core();
    assert_eq!(c.templates().unwrap(), ["Custom", DEFAULT_TEMPLATE]);
    let empty = c.create_project("Empty", DEFAULT_TEMPLATE).unwrap();
    assert_eq!(fs::read_dir(&empty.path).unwrap().count(), 0);
    let p = c.create_project("Work", "Custom").unwrap();
    assert_eq!(p.name, "Work-1");
    assert_eq!(fs::read(p.path.join("data.bin")).unwrap(), [0xff, 0, 42]);
    assert_eq!(
        fs::read_to_string(p.path.join("README.md")).unwrap(),
        "hello"
    );
    assert_eq!(fs::metadata(&p.path).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(p.path.join("README.md")).unwrap().mode() & 0o777,
        0o600
    );
    c.start(&p.name).unwrap(); // A task-named file is irrelevant to C1.
}

#[test]
fn template_selection_reserved_names_and_first_free_all_entry_types() {
    let f = Fixture::new();
    f.project("Name-1");
    fs::write(f.source.join("Name-2"), "keep").unwrap();
    symlink("absent", f.source.join("Name-3")).unwrap();
    f.project("Name-5");
    let c = f.core();
    let p = c.create_project("Name", DEFAULT_TEMPLATE).unwrap();
    assert_eq!(p.name, "Name-4");
    assert_eq!(fs::read_to_string(f.source.join("Name-2")).unwrap(), "keep");
    fs::remove_dir(f.source.join("Name-1")).unwrap();
    assert_eq!(
        c.create_project("Name", DEFAULT_TEMPLATE).unwrap().name,
        "Name-1"
    );
    code(c.create_project("Name", "absent"), "not_found.template");
    code(
        c.create_project("../escape", DEFAULT_TEMPLATE),
        "validation.invalid_request",
    );
    code(
        c.create_project(&"x".repeat(118), DEFAULT_TEMPLATE),
        "validation.invalid_request",
    );
    code(
        c.create_project(".session-manager-create-x", DEFAULT_TEMPLATE),
        "validation.invalid_request",
    );
    f.template(DEFAULT_TEMPLATE);
    code(c.templates(), "template.invalid");
    fs::remove_file(f.source.join("Name-3")).unwrap();
}

#[test]
fn unsafe_templates_never_publish() {
    let f = Fixture::new();
    let t = f.template("Unsafe");
    let c = f.core();
    symlink("/", t.join("link")).unwrap();
    code(c.create_project("No", "Unsafe"), "template.invalid");
    fs::remove_file(t.join("link")).unwrap();
    fs::write(t.join("a"), "bytes").unwrap();
    fs::hard_link(t.join("a"), t.join("b")).unwrap();
    code(c.create_project("No", "Unsafe"), "template.invalid");
    fs::remove_file(t.join("b")).unwrap();
    nix::unistd::mkfifo(&t.join("fifo"), nix::sys::stat::Mode::S_IRUSR).unwrap();
    code(c.create_project("No", "Unsafe"), "template.invalid");
    fs::remove_file(t.join("fifo")).unwrap();
    let invalid = std::ffi::OsString::from_vec(vec![0xff]);
    fs::write(t.join(&invalid), "bytes").unwrap();
    code(c.create_project("No", "Unsafe"), "template.invalid");
    fs::remove_file(t.join(invalid)).unwrap();
    assert_eq!(c.projects().unwrap(), Vec::new());
}

#[test]
fn template_copy_bounds_cover_file_depth_entries_and_aggregate_bytes() {
    let f = Fixture::new();
    let t = f.template("Bounds");
    let c = f.core();
    fs::File::create(t.join("large"))
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    code(c.create_project("No", "Bounds"), "limit.exceeded");
    fs::remove_file(t.join("large")).unwrap();
    let mut deep = t.clone();
    for _ in 0..17 {
        deep = deep.join("d");
        fs::create_dir(&deep).unwrap();
    }
    code(c.create_project("No", "Bounds"), "limit.exceeded");
    fs::remove_dir_all(t.join("d")).unwrap();
    for n in 0..2049 {
        fs::write(t.join(n.to_string()), []).unwrap();
    }
    code(c.create_project("No", "Bounds"), "limit.exceeded");
    for entry in fs::read_dir(&t).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    for n in 0..17 {
        fs::write(t.join(n.to_string()), vec![0; 1024 * 1024]).unwrap();
    }
    code(c.create_project("No", "Bounds"), "limit.exceeded");
    assert_eq!(c.projects().unwrap(), Vec::new());
}

#[test]
fn template_file_substitution_during_read_is_detected() {
    let f = Fixture::new();
    let t = f.template("Race");
    fs::write(t.join("a"), "original").unwrap();
    let c = f.core();
    let mut hook = Hook(|b| {
        if b == Boundary::TemplateFileRead {
            fs::rename(t.join("a"), t.join("old")).unwrap();
            symlink("/etc/passwd", t.join("a")).unwrap();
        }
        Ok(())
    });
    code(
        c.create_project_with_faults("No", "Race", &mut hook),
        "template.invalid",
    );
    assert_eq!(c.projects().unwrap(), Vec::new());
    fs::remove_file(t.join("a")).unwrap();
}

#[test]
fn interrupted_copy_cleans_only_its_staging_and_never_exposes_half_project() {
    let f = Fixture::new();
    let t = f.template("Two");
    fs::write(t.join("a"), "a").unwrap();
    fs::write(t.join("b"), "b").unwrap();
    let c = f.core();
    for point in [Boundary::CopyFile, Boundary::BeforePublish] {
        code(
            c.create_project_with_faults("No", "Two", &mut fail_at(point)),
            "filesystem.unavailable",
        );
        assert_eq!(c.projects().unwrap(), Vec::new());
        assert_eq!(fs::read_dir(&f.source).unwrap().count(), 0);
    }
    code(
        c.create_project_with_faults("Yes", "Two", &mut fail_at(Boundary::ProjectPublished)),
        "persistence.uncertain",
    );
    assert_eq!(fs::read_to_string(f.source.join("Yes-1/b")).unwrap(), "b");
}

#[test]
fn external_creation_race_never_overwrites_the_winner() {
    let f = Fixture::new();
    let c = f.core();
    let mut raced = false;
    let mut hook = Hook(|b| {
        if b == Boundary::ProjectTargetChecked && !raced {
            raced = true;
            let p = f.project("Race-1");
            fs::write(p.join("keep"), "winner").unwrap();
        }
        Ok(())
    });
    assert_eq!(
        c.create_project_with_faults("Race", DEFAULT_TEMPLATE, &mut hook)
            .unwrap()
            .name,
        "Race-2"
    );
    assert_eq!(
        fs::read_to_string(f.source.join("Race-1/keep")).unwrap(),
        "winner"
    );
}

#[test]
fn concurrent_project_creates_publish_distinct_complete_directories() {
    let f = Fixture::new();
    let c = Arc::new(f.core());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let c = Arc::clone(&c);
            std::thread::spawn(move || c.create_project("Race", DEFAULT_TEMPLATE).unwrap().name)
        })
        .collect();
    let mut names: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 8);
    assert_eq!(c.projects().unwrap().len(), 8);
}

#[test]
fn replaced_staging_and_source_roots_are_not_published_or_recursively_deleted() {
    let f = Fixture::new();
    let outside = f.root.as_ref().unwrap().root().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), "safe").unwrap();
    let c = f.core();
    let mut hook = Hook(|b| {
        if b == Boundary::BeforePublish {
            let stage = fs::read_dir(&f.source)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            fs::rename(&stage, f.source.join(".session-manager-create-moved")).unwrap();
            symlink(&outside, &stage).unwrap();
        }
        Ok(())
    });
    code(
        c.create_project_with_faults("No", DEFAULT_TEMPLATE, &mut hook),
        "filesystem.unavailable",
    );
    assert_eq!(fs::read_to_string(outside.join("keep")).unwrap(), "safe");
    for e in fs::read_dir(&f.source).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_symlink() {
            fs::remove_file(e.path()).unwrap();
        }
    }
    assert_eq!(c.projects().unwrap(), Vec::new());
    drop(c);
    let c = f.core();
    assert_eq!(c.incomplete_creations().len(), 1);
}

#[test]
fn sessions_survive_rename_removal_reuse_and_restart_without_snapshots() {
    let f = Fixture::new();
    let p = f.project("Work");
    let c = f.core();
    let a = c.start("Work").unwrap().session;
    let b = c.start("Work").unwrap().session;
    assert_eq!(
        (&a.name, &b.name),
        (&"Work-1".to_owned(), &"Work-2".to_owned())
    );
    fs::rename(&p, f.source.join("Renamed")).unwrap();
    code(c.start("Work"), "not_found.project");
    c.switch(a.id).unwrap();
    assert_eq!(
        c.sessions()
            .unwrap()
            .sessions
            .iter()
            .filter(|s| s.active)
            .count(),
        1
    );
    let renamed = c.start("Renamed").unwrap().session;
    assert_eq!(renamed.number, 1);
    f.project("Work");
    assert_eq!(c.start("Work").unwrap().session.number, 3);
    fs::remove_dir(f.source.join("Work")).unwrap();
    c.stop(b.id).unwrap();
    c.switch(a.id).unwrap();
    let bytes = fs::read(f.current()).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut fields: Vec<_> = json["sessions"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        ["id", "lifecycle", "name", "number", "project_name"]
    );
    drop(c);
    let c = f.core();
    assert_eq!(c.sessions().unwrap().active_session_id, Some(a.id));
    c.stop(a.id).unwrap();
    assert_eq!(c.sessions().unwrap().active_session_id, None);
    assert!(f.source.join("Renamed").is_dir());
    code(c.stop(a.id), "not_found.session");
}

#[test]
fn numbers_use_highest_live_without_gaps_and_reset_after_last_stop() {
    let f = Fixture::new();
    f.project("QMK");
    let c = f.core();
    let a = c.start("QMK").unwrap().session;
    let b = c.start("QMK").unwrap().session;
    c.stop(a.id).unwrap();
    assert_eq!(c.start("QMK").unwrap().session.number, 3);
    for s in c.sessions().unwrap().sessions {
        c.stop(s.session.id).unwrap();
    }
    assert_eq!(c.start("QMK").unwrap().session.number, 1);
    code(c.switch(b.id), "not_found.session");
}

#[test]
fn exhaustion_collision_and_uuid_collision_do_not_mutate_state() {
    let f = Fixture::new();
    f.project("Full");
    let state = RuntimeState {
        sessions: vec![sample("Full", u32::MAX, 42)],
        ..RuntimeState::default()
    };
    f.persist(&state);
    let c = f.core();
    let before = fs::read(f.current()).unwrap();
    code(c.start("Full"), "conflict.session_number_exhausted");
    assert_eq!(fs::read(f.current()).unwrap(), before);
    drop(c);
    // A corrupt caller-supplied session collection still cannot bypass global-name checks.
    let mut held = sample("Other", 1, 43);
    held.name = "Full-1".into();
    code(
        domain::allocate("Full", &[held]),
        "conflict.session_name_collision",
    );
    let state = RuntimeState {
        sessions: vec![Session {
            id: Uuid::parse_str("00000000-0000-7000-8000-000000000001").unwrap(),
            ..sample("Full", 1, 1)
        }],
        ..RuntimeState::default()
    };
    f.persist(&state);
    let c = f.core();
    code(c.start("Full"), "internal.error");
}

#[test]
fn switching_active_is_a_noop_and_revision_exhaustion_is_atomic() {
    let f = Fixture::new();
    f.project("P");
    let c = f.core();
    let s = c.start("P").unwrap().session;
    let before = fs::read(f.current()).unwrap();
    c.switch(s.id).unwrap();
    assert_eq!(fs::read(f.current()).unwrap(), before);
    drop(c);
    let state = RuntimeState {
        revision: u64::MAX,
        sessions: vec![s.clone()],
        active_session_id: Some(s.id),
        ..RuntimeState::default()
    };
    f.persist(&state);
    let c = f.core();
    c.switch(s.id).unwrap();
    code(c.stop(s.id), "limit.exceeded");
    code(c.start("P"), "limit.exceeded");
    assert_eq!(
        runtime::decode(&fs::read(f.current()).unwrap()).unwrap(),
        state
    );
}

#[test]
fn concurrent_starts_are_serialized_and_second_owner_is_rejected() {
    let f = Fixture::new();
    f.project("P");
    let c = Arc::new(f.core());
    code(Core::open(&f.locations), "conflict.state_locked");
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let c = Arc::clone(&c);
            std::thread::spawn(move || c.start("P").unwrap().session.number)
        })
        .collect();
    let mut numbers: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, (1..=8).collect::<Vec<_>>());
    assert_eq!(c.sessions().unwrap().revision, 8);
}

#[test]
fn interrupted_runtime_commits_never_expose_partial_sessions() {
    for point in [
        Boundary::RuntimeCandidateSynced,
        Boundary::RuntimeBackupSynced,
        Boundary::RuntimeReplaced,
    ] {
        let f = Fixture::new();
        f.project("P");
        let c = f.core();
        let first = c.start("P").unwrap().session;
        let result = c.start_with_faults("P", &mut fail_at(point));
        if point == Boundary::RuntimeReplaced {
            code(result, "persistence.uncertain");
            assert!(c.recovery_required().unwrap());
        } else {
            code(result, "filesystem.unavailable");
            assert_eq!(c.sessions().unwrap().sessions.len(), 1);
        }
        drop(c);
        let c = f.core();
        let list = c.sessions().unwrap();
        assert_eq!(
            list.sessions.len(),
            if point == Boundary::RuntimeReplaced {
                2
            } else {
                1
            }
        );
        assert!(list.sessions.iter().any(|s| s.session.id == first.id));
    }
}

#[test]
fn corrupt_current_never_silently_resurrects_a_stopped_session() {
    let f = Fixture::new();
    f.project("P");
    let c = f.core();
    let s = c.start("P").unwrap().session;
    c.stop(s.id).unwrap();
    drop(c);
    assert_eq!(
        runtime::decode(&fs::read(f.locations.state.join("runtime-v2.previous.json")).unwrap())
            .unwrap()
            .sessions
            .len(),
        1
    );
    fs::write(f.current(), "corrupt").unwrap();
    let c = f.core();
    assert!(c.recovery_required().unwrap());
    code(c.start("P"), "recovery.invalid_runtime");
    code(
        c.create_project("No", DEFAULT_TEMPLATE),
        "recovery.invalid_runtime",
    );
    assert_eq!(fs::read_to_string(f.current()).unwrap(), "corrupt");
}

#[test]
fn strict_runtime_rejects_unknown_fields_duplicates_versions_and_oversized_sparse_files() {
    let state = RuntimeState::default();
    let mut json = serde_json::to_value(&state).unwrap();
    json["snapshot"] = serde_json::json!({});
    code(
        runtime::decode(&serde_json::to_vec(&json).unwrap()),
        "recovery.invalid_runtime",
    );
    code(runtime::decode(b"{\"runtime_version\":2,\"runtime_version\":2,\"revision\":0,\"active_session_id\":null,\"sessions\":[]}"),"recovery.invalid_runtime");
    code(runtime::decode(&[0xff]), "recovery.invalid_runtime");
    let with_session = RuntimeState {
        sessions: vec![sample("P", 1, 1)],
        ..state.clone()
    };
    for (key, value) in [
        ("source_root", serde_json::json!("/old/root")),
        ("lifecycle", serde_json::json!("paused")),
    ] {
        let mut json = serde_json::to_value(&with_session).unwrap();
        json["sessions"][0][key] = value;
        code(
            runtime::decode(&serde_json::to_vec(&json).unwrap()),
            "recovery.invalid_runtime",
        );
    }
    for mut bad in [
        RuntimeState {
            sessions: vec![Session {
                name: "wrong-generated-name".into(),
                ..sample("P", 1, 1)
            }],
            ..state.clone()
        },
        RuntimeState {
            sessions: vec![sample("P", 1, 1), sample("P", 2, 1)],
            ..state.clone()
        },
        RuntimeState {
            runtime_version: 1,
            ..state.clone()
        },
        RuntimeState {
            active_session_id: Some(Uuid::from_u128(1)),
            ..state.clone()
        },
        RuntimeState {
            sessions: vec![sample("P", 1, 1), sample("P", 1, 2)],
            ..state.clone()
        },
    ] {
        code(bad.validate(), "recovery.invalid_runtime");
        bad.sessions.clear();
    }
    let f = Fixture::new();
    let c = f.core();
    drop(c);
    fs::File::create(f.current())
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    let c = f.core();
    assert!(c.recovery_required().unwrap());
}

#[test]
fn legacy_artifacts_are_ignored_unchanged_even_when_corrupt_or_pending() {
    let f = Fixture::new();
    fs::write(
        f.locations.config.join("config.toml"),
        "invalid legacy configuration",
    )
    .unwrap();
    fs::create_dir_all(f.locations.state.join("transactions")).unwrap();
    fs::set_permissions(&f.locations.state, fs::Permissions::from_mode(0o700)).unwrap();
    let legacy = f.locations.state.join("transactions/pending.json");
    fs::write(&legacy, "corrupt pending transaction").unwrap();
    let c = f.core();
    assert!(!c.recovery_required().unwrap());
    f.project("P");
    c.start("P").unwrap();
    assert_eq!(
        fs::read_to_string(legacy).unwrap(),
        "corrupt pending transaction"
    );
}

#[test]
fn c1_has_no_legacy_adapter_or_host_discovery_paths() {
    let source = [
        include_str!("application.rs"),
        include_str!("configuration.rs"),
        include_str!("projects.rs"),
        include_str!("runtime.rs"),
        include_str!("safe_fs.rs"),
    ]
    .join("\n");
    for forbidden in [
        "std::env",
        "std::process",
        "Command::",
        "UnixStream",
        "crate::adapter_contract",
        "crate::checkpoint",
        "crate::transaction",
        "XDG_DATA_HOME",
        "/usr/share/applications",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden C1 dependency: {forbidden}"
        );
    }
    let f = Fixture::new();
    let c = f.core();
    let p = c.create_project("P", DEFAULT_TEMPLATE).unwrap();
    let s = c.start(&p.name).unwrap();
    c.switch(s.session.id).unwrap();
    c.stop(s.session.id).unwrap();
}

#[test]
fn discovery_and_template_counts_fail_instead_of_truncating() {
    let f = Fixture::new();
    let c = f.core();
    for n in 0..1024 {
        f.project(&format!("P{n}"));
    }
    assert_eq!(c.projects().unwrap().len(), 1024);
    f.project("overflow");
    code(c.projects(), "limit.exceeded");
    for n in 0..255 {
        f.template(&format!("T{n}"));
    }
    assert_eq!(c.templates().unwrap().len(), 256);
    f.template("overflow");
    code(c.templates(), "limit.exceeded");
}

#[test]
fn ignored_root_entries_still_count_toward_scan_bound() {
    let f = Fixture::new();
    let c = f.core();
    for n in 0..16385 {
        fs::write(f.source.join(format!("ignored-{n}")), []).unwrap();
    }
    code(c.projects(), "limit.exceeded");
    code(c.create_project("No", DEFAULT_TEMPLATE), "limit.exceeded");
}

#[test]
fn exact_template_byte_and_depth_limits_are_accepted() {
    let f = Fixture::new();
    let t = f.template("Maximum");
    for n in 0..16 {
        fs::write(t.join(n.to_string()), vec![42; 1024 * 1024]).unwrap();
    }
    let mut deep = t.clone();
    for _ in 0..16 {
        deep = deep.join("d");
        fs::create_dir(&deep).unwrap();
    }
    let c = f.core();
    let p = c.create_project("Max", "Maximum").unwrap();
    assert_eq!(fs::read(p.path.join("0")).unwrap().len(), 1024 * 1024);
    assert!(p.path.join("d/".repeat(16)).is_dir());
}

#[test]
fn source_replacement_during_copy_cannot_redirect_publication() {
    let f = Fixture::new();
    let c = f.core();
    let moved = f.source.with_file_name("moved-source");
    let mut hook = Hook(|b| {
        if b == Boundary::BeforePublish {
            fs::rename(&f.source, &moved).unwrap();
            fs::create_dir(&f.source).unwrap();
            fs::write(f.source.join("keep"), "replacement").unwrap();
        }
        Ok(())
    });
    code(
        c.create_project_with_faults("No", DEFAULT_TEMPLATE, &mut hook),
        "filesystem.unavailable",
    );
    assert_eq!(
        fs::read_to_string(f.source.join("keep")).unwrap(),
        "replacement"
    );
    assert_eq!(fs::read_dir(moved).unwrap().count(), 0);
}

#[test]
fn external_runtime_change_or_state_root_substitution_never_gets_overwritten() {
    let f = Fixture::new();
    f.project("P");
    let c = f.core();
    c.start("P").unwrap();
    fs::write(f.current(), "external edit").unwrap();
    code(c.start("P"), "recovery.invalid_runtime");
    assert_eq!(fs::read_to_string(f.current()).unwrap(), "external edit");
    drop(c);
    f.persist(&RuntimeState::default());
    let c = f.core();
    fs::rename(
        &f.locations.state,
        f.locations.state.with_file_name("moved-state"),
    )
    .unwrap();
    fs::create_dir(&f.locations.state).unwrap();
    code(c.start("P"), "recovery.invalid_runtime");
    assert!(!f.current().exists());
}

#[test]
fn uncertain_stop_is_not_replayed_or_resurrected_on_restart() {
    let f = Fixture::new();
    f.project("P");
    let c = f.core();
    let s = c.start("P").unwrap().session;
    code(
        c.stop_with_faults(s.id, &mut fail_at(Boundary::RuntimeReplaced)),
        "persistence.uncertain",
    );
    assert!(c.recovery_required().unwrap());
    drop(c);
    let c = f.core();
    assert_eq!(c.sessions().unwrap().sessions.len(), 0);
    code(c.stop(s.id), "not_found.session");
}

#[test]
fn missing_current_with_existing_backup_is_not_a_fresh_install() {
    let f = Fixture::new();
    f.project("P");
    let c = f.core();
    c.start("P").unwrap();
    drop(c);
    fs::remove_file(f.current()).unwrap();
    let c = f.core();
    assert!(c.recovery_required().unwrap());
    assert!(!f.current().exists());
}

#[test]
fn runtime_session_count_bound_and_private_files_are_enforced() {
    let f = Fixture::new();
    f.project("P");
    let state = RuntimeState {
        sessions: (1..=1024).map(|n| sample("P", n, u128::from(n))).collect(),
        ..RuntimeState::default()
    };
    f.persist(&state);
    let c = f.core();
    code(c.start("P"), "limit.exceeded");
    drop(c);
    f.persist(&RuntimeState::default());
    let c = f.core();
    c.start("P").unwrap();
    assert_eq!(fs::metadata(f.current()).unwrap().mode() & 0o777, 0o600);
    assert_eq!(
        fs::metadata(f.locations.state.join("runtime-v2.previous.json"))
            .unwrap()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(f.locations.runtime.join("state.lock"))
            .unwrap()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn template_root_links_and_project_replacement_links_are_never_followed() {
    let f = Fixture::new();
    let target = f.project("Target");
    symlink(&target, f.locations.config.join("profile-template")).unwrap();
    let c = f.core();
    code(c.templates(), "filesystem.unavailable");
    fs::remove_file(f.locations.config.join("profile-template")).unwrap();
    let p = f.project("P");
    let s = c.start("P").unwrap().session;
    fs::remove_dir(&p).unwrap();
    symlink(&target, &p).unwrap();
    code(c.start("P"), "not_found.project");
    c.switch(s.id).unwrap();
    c.stop(s.id).unwrap();
    assert!(target.is_dir());
    fs::remove_file(p).unwrap();
}

#[test]
fn partial_copy_is_invisible_to_concurrent_discovery() {
    let f = Fixture::new();
    let t = f.template("Two");
    fs::write(t.join("a"), "one").unwrap();
    fs::write(t.join("b"), "two").unwrap();
    let c = f.core();
    let mut observed = 0;
    let mut hook = Hook(|b| {
        if b == Boundary::CopyFile {
            observed += 1;
            assert_eq!(c.projects().unwrap(), Vec::new());
        }
        Ok(())
    });
    c.create_project_with_faults("Full", "Two", &mut hook)
        .unwrap();
    assert_eq!(observed, 2);
    assert_eq!(c.projects().unwrap().len(), 1);
}

#[test]
fn mid_commit_external_edit_is_preserved_and_gates_mutations() {
    let f = Fixture::new();
    f.project("P");
    let c = f.core();
    c.start("P").unwrap();
    let mut hook = Hook(|b| {
        if b == Boundary::RuntimeBackupSynced {
            fs::write(f.current(), "external edit").unwrap();
        }
        Ok(())
    });
    code(
        c.start_with_faults("P", &mut hook),
        "recovery.invalid_runtime",
    );
    assert_eq!(fs::read_to_string(f.current()).unwrap(), "external edit");
    assert!(c.recovery_required().unwrap());
}

#[test]
fn first_commit_interruption_never_invents_a_session_on_restart() {
    for point in [
        Boundary::RuntimeCandidateSynced,
        Boundary::RuntimeBackupSynced,
    ] {
        let f = Fixture::new();
        f.project("P");
        let c = f.core();
        code(
            c.start_with_faults("P", &mut fail_at(point)),
            "filesystem.unavailable",
        );
        drop(c);
        let c = f.core();
        if point == Boundary::RuntimeCandidateSynced {
            assert_eq!(c.sessions().unwrap().sessions.len(), 0);
        } else {
            assert!(c.recovery_required().unwrap());
        }
    }
}

#[test]
fn unreadable_template_files_fail_without_publishing() {
    // The superuser bypasses Unix read permission bits; other fixtures still test no-follow IO.
    if nix::unistd::getuid().is_root() {
        return;
    }
    let f = Fixture::new();
    let t = f.template("Unreadable");
    fs::write(t.join("a"), "private").unwrap();
    fs::set_permissions(t.join("a"), fs::Permissions::from_mode(0o0)).unwrap();
    let c = f.core();
    code(
        c.create_project("No", "Unreadable"),
        "filesystem.unavailable",
    );
    assert_eq!(c.projects().unwrap(), Vec::new());
    fs::set_permissions(t.join("a"), fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn abandoned_runtime_candidates_never_reserve_numbers_or_override_current() {
    let f = Fixture::new();
    f.project("P");
    let c = f.core();
    let first = c.start("P").unwrap().session;
    drop(c);
    let candidate = RuntimeState {
        revision: 99,
        sessions: vec![sample("P", 90, 90)],
        ..RuntimeState::default()
    };
    fs::write(
        f.locations.state.join(".runtime-v2-abandoned"),
        runtime::encode(&candidate).unwrap(),
    )
    .unwrap();
    fs::write(
        f.locations.state.join("runtime-v2.previous.json"),
        runtime::encode(&candidate).unwrap(),
    )
    .unwrap();
    let c = Core::open(&f.locations).unwrap();
    assert_eq!(c.sessions().unwrap().sessions[0].session, first);
    assert_eq!(c.start("P").unwrap().session.number, 2);
}

#[test]
fn inherited_special_directory_permissions_are_not_copied() {
    let f = Fixture::new();
    fs::set_permissions(&f.source, fs::Permissions::from_mode(0o2700)).unwrap();
    let c = f.core();
    let p = c.create_project("Private", DEFAULT_TEMPLATE).unwrap();
    assert_eq!(fs::metadata(p.path).unwrap().mode() & 0o7777, 0o700);
}

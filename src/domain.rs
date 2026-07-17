//! Pure profile, task, and logical-session rules.

use std::collections::BTreeSet;
use uuid::Uuid;

pub const MAX_PROFILES: usize = 256;
pub const MAX_TASKS_PER_PROFILE: usize = 256;
pub const MAX_DESKTOP_ENTRIES: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub name: String,
    pub desktop_entries: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub root: String,
    pub desktop_entries: Vec<String>,
    pub tasks: Vec<Task>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LaunchSource {
    Profile(String),
    Task { profile: String, task: String },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    Running,
    Paused,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: Uuid,
    pub checkpoint_id: Option<Uuid>,
    pub source: LaunchSource,
    /// Immutable root copied from the launch source at start time.
    pub source_root: String,
    /// Immutable desktop-entry list copied from the launch source at start time.
    pub source_desktop_entries: Vec<String>,
    pub number: u32,
    pub name: String,
    pub lifecycle: Lifecycle,
    pub active: bool,
}

/// Immutable runtime information used by the pure configuration reload policy.
///
/// M4 compares `runtime_revision` under its joint commit lock. M1 only decides
/// whether the supplied live-reference snapshot permits adopting a candidate.
#[derive(Debug, Clone, Copy)]
pub struct LiveReferenceSnapshot<'a> {
    pub runtime_revision: u64,
    pub sessions: &'a [Session],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Validation { path: String, message: &'static str },
    SessionNumberExhausted,
    SessionNameCollision,
    ReferencedSource,
}
impl Error {
    fn validation(path: impl Into<String>, message: &'static str) -> Self {
        Self::Validation {
            path: path.into(),
            message,
        }
    }
}

impl Configuration {
    pub fn validate(&self) -> Result<(), Error> {
        if self.profiles.len() > MAX_PROFILES {
            return Err(Error::validation("profiles", "too many profiles"));
        }
        let mut profiles = BTreeSet::new();
        for (i, profile) in self.profiles.iter().enumerate() {
            validate_name(&profile.name, &format!("profiles[{i}].name"))?;
            if !profiles.insert(&profile.name) {
                return Err(Error::validation(
                    format!("profiles[{i}].name"),
                    "duplicate profile name",
                ));
            }
            validate_root(&profile.root, &format!("profiles[{i}].root"))?;
            validate_entries(
                &profile.desktop_entries,
                &format!("profiles[{i}].desktop_entries"),
                MAX_DESKTOP_ENTRIES,
            )?;
            if profile.tasks.len() > MAX_TASKS_PER_PROFILE {
                return Err(Error::validation(
                    format!("profiles[{i}].tasks"),
                    "too many tasks",
                ));
            }
            let mut tasks = BTreeSet::new();
            for (j, task) in profile.tasks.iter().enumerate() {
                let prefix = format!("profiles[{i}].tasks[{j}]");
                validate_name(&task.name, &format!("{prefix}.name"))?;
                if !tasks.insert(&task.name) {
                    return Err(Error::validation(
                        format!("{prefix}.name"),
                        "duplicate task name",
                    ));
                }
                validate_entries(
                    &task.desktop_entries,
                    &format!("{prefix}.desktop_entries"),
                    MAX_DESKTOP_ENTRIES,
                )?;
                if task
                    .desktop_entries
                    .iter()
                    .any(|entry| !profile.desktop_entries.contains(entry))
                {
                    return Err(Error::validation(
                        format!("{prefix}.desktop_entries"),
                        "entry is not in parent profile",
                    ));
                }
            }
        }
        Ok(())
    }
    pub fn source(&self, source: &LaunchSource) -> Option<&[String]> {
        match source {
            LaunchSource::Profile(name) => self
                .profiles
                .iter()
                .find(|p| &p.name == name)
                .map(|p| p.desktop_entries.as_slice()),
            LaunchSource::Task { profile, task } => self
                .profiles
                .iter()
                .find(|p| &p.name == profile)?
                .tasks
                .iter()
                .find(|t| &t.name == task)
                .map(|t| t.desktop_entries.as_slice()),
        }
    }
}

pub fn allocate(source: &LaunchSource, sessions: &[Session]) -> Result<(u32, String), Error> {
    let highest = sessions
        .iter()
        .filter(|s| &s.source == source)
        .map(|s| s.number)
        .max();
    let number = match highest {
        None => 1,
        Some(u32::MAX) => return Err(Error::SessionNumberExhausted),
        Some(n) => n + 1,
    };
    let name = generated_name(source, number);
    if sessions.iter().any(|s| s.name == name) {
        return Err(Error::SessionNameCollision);
    }
    Ok((number, name))
}
pub fn generated_name(source: &LaunchSource, number: u32) -> String {
    match source {
        LaunchSource::Profile(p) => format!("{p}-{number}"),
        LaunchSource::Task { profile, task } => format!("{profile}-{task}-{number}"),
    }
}
pub fn rename_profile(
    config: &mut Configuration,
    sessions: &mut [Session],
    old: &str,
    new: String,
) -> Result<(), Error> {
    let mut candidate_config = config.clone();
    let mut candidate_sessions = sessions.to_vec();
    let candidate = config
        .profiles
        .iter()
        .position(|p| p.name == old)
        .ok_or_else(|| Error::validation("profile", "not found"))?;
    candidate_config.profiles[candidate].name = new.clone();
    candidate_config.validate()?;
    for session in candidate_sessions.iter_mut().filter(|s| {
        matches!(&s.source, LaunchSource::Profile(p) if p == old)
            || matches!(&s.source, LaunchSource::Task { profile, .. } if profile == old)
    }) {
        match &mut session.source {
            LaunchSource::Profile(p) => *p = new.clone(),
            LaunchSource::Task { profile, .. } => *profile = new.clone(),
        }
        session.name = generated_name(&session.source, session.number);
    }
    validate_sessions(&candidate_sessions)?;
    *config = candidate_config;
    sessions.clone_from_slice(&candidate_sessions);
    Ok(())
}
pub fn rename_task(
    config: &mut Configuration,
    sessions: &mut [Session],
    profile: &str,
    old: &str,
    new: String,
) -> Result<(), Error> {
    let mut next_config = config.clone();
    let mut next_sessions = sessions.to_vec();
    let p = next_config
        .profiles
        .iter_mut()
        .find(|p| p.name == profile)
        .ok_or_else(|| Error::validation("profile", "not found"))?;
    let t = p
        .tasks
        .iter_mut()
        .find(|t| t.name == old)
        .ok_or_else(|| Error::validation("task", "not found"))?;
    t.name = new.clone();
    next_config.validate()?;
    for s in next_sessions
        .iter_mut()
        .filter(|s| matches!(&s.source,LaunchSource::Task{profile:p,task:t}if p==profile&&t==old))
    {
        if let LaunchSource::Task { task, .. } = &mut s.source {
            *task = new.clone();
        }
        s.name = generated_name(&s.source, s.number);
    }
    validate_sessions(&next_sessions)?;
    *config = next_config;
    sessions.clone_from_slice(&next_sessions);
    Ok(())
}
pub fn reload_allowed(
    current: &Configuration,
    candidate: &Configuration,
    snapshot: LiveReferenceSnapshot<'_>,
) -> Result<(), Error> {
    candidate.validate()?;
    for session in snapshot.sessions {
        if current.source(&session.source).is_some() && candidate.source(&session.source).is_none()
        {
            return Err(Error::ReferencedSource);
        }
    }
    Ok(())
}
pub fn validate_live_names(sessions: &[Session]) -> Result<(), Error> {
    let mut names = BTreeSet::new();
    if sessions.iter().any(|s| !names.insert(&s.name)) {
        Err(Error::SessionNameCollision)
    } else {
        Ok(())
    }
}
/// Validates the platform-independent invariants of the accepted live-session set.
pub fn validate_sessions(sessions: &[Session]) -> Result<(), Error> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut active = 0_usize;
    for (index, session) in sessions.iter().enumerate() {
        let prefix = format!("sessions[{index}]");
        if !ids.insert(session.id) {
            return Err(Error::validation(
                format!("{prefix}.id"),
                "duplicate session UUID",
            ));
        }
        if session.number == 0 {
            return Err(Error::validation(
                format!("{prefix}.number"),
                "session number must be positive",
            ));
        }
        if session.name != generated_name(&session.source, session.number) {
            return Err(Error::validation(
                format!("{prefix}.name"),
                "name does not match source and number",
            ));
        }
        if !names.insert(&session.name) {
            return Err(Error::SessionNameCollision);
        }
        validate_name(
            &source_profile(&session.source),
            &format!("{prefix}.source.profile"),
        )?;
        if let LaunchSource::Task { task, .. } = &session.source {
            validate_name(task, &format!("{prefix}.source.task"))?;
        }
        validate_root(&session.source_root, &format!("{prefix}.source_root"))?;
        validate_entries(
            &session.source_desktop_entries,
            &format!("{prefix}.source_desktop_entries"),
            MAX_DESKTOP_ENTRIES,
        )?;
        match (session.lifecycle, session.checkpoint_id) {
            (Lifecycle::Running, None) | (Lifecycle::Paused, Some(_)) => {}
            (Lifecycle::Running, Some(_)) => {
                return Err(Error::validation(
                    format!("{prefix}.checkpoint_id"),
                    "running session has a paused checkpoint",
                ));
            }
            (Lifecycle::Paused, None) => {
                return Err(Error::validation(
                    format!("{prefix}.checkpoint_id"),
                    "paused session requires a checkpoint",
                ));
            }
        }
        if session.active {
            active += 1;
            if session.lifecycle != Lifecycle::Running {
                return Err(Error::validation(
                    format!("{prefix}.active"),
                    "only a running session may be active",
                ));
            }
        }
    }
    if active > 1 {
        return Err(Error::validation(
            "sessions",
            "more than one active session",
        ));
    }
    Ok(())
}
/// Validates an explicit delete request.
///
/// Reaching this function is the caller's confirmation; the core deliberately
/// exposes no force or cascade mode.
pub fn can_delete(source: &LaunchSource, sessions: &[Session]) -> Result<(), Error> {
    if sessions.iter().any(|s| match (source, &s.source) {
        (LaunchSource::Profile(p), LaunchSource::Profile(q)) => p == q,
        (LaunchSource::Profile(p), LaunchSource::Task { profile, .. }) => p == profile,
        (
            LaunchSource::Task { profile, task },
            LaunchSource::Task {
                profile: p,
                task: t,
            },
        ) => profile == p && task == t,
        _ => false,
    }) {
        Err(Error::ReferencedSource)
    } else {
        Ok(())
    }
}

fn validate_name(value: &str, path: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 128
        || value.bytes().any(|b| b == 0 || b.is_ascii_control())
    {
        Err(Error::validation(path, "invalid name"))
    } else {
        Ok(())
    }
}
fn validate_root(value: &str, path: &str) -> Result<(), Error> {
    if value.len() > 4096
        || !value.starts_with('/')
        || value.contains('\0')
        || (value != "/"
            && value
                .split('/')
                .skip(1)
                .any(|p| p.is_empty() || p == "." || p == ".."))
    {
        Err(Error::validation(path, "root is not lexically normalized"))
    } else {
        Ok(())
    }
}
fn validate_entries(values: &[String], path: &str, limit: usize) -> Result<(), Error> {
    if values.len() > limit {
        return Err(Error::validation(path, "too many desktop entries"));
    }
    let mut set = BTreeSet::new();
    for value in values {
        if value.is_empty()
            || value.len() > 255
            || !value.ends_with(".desktop")
            || value.contains('/')
            || value.contains('\0')
            || !set.insert(value)
        {
            return Err(Error::validation(
                path,
                "invalid or duplicate desktop entry",
            ));
        }
    }
    Ok(())
}

fn source_profile(source: &LaunchSource) -> String {
    match source {
        LaunchSource::Profile(profile) | LaunchSource::Task { profile, .. } => profile.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session(source: LaunchSource, number: u32) -> Session {
        Session {
            id: Uuid::new_v4(),
            checkpoint_id: None,
            source: source.clone(),
            source_root: "/work".into(),
            source_desktop_entries: vec![],
            number,
            name: generated_name(&source, number),
            lifecycle: Lifecycle::Running,
            active: false,
        }
    }
    #[test]
    fn allocation_obeys_highest_live_and_global_names() {
        let p = LaunchSource::Profile("QMK".into());
        assert_eq!(allocate(&p, &[session(p.clone(), 2)]).unwrap().0, 3);
        assert_eq!(allocate(&p, &[]).unwrap().0, 1);
        let a = LaunchSource::Profile("A-B".into());
        let b = LaunchSource::Task {
            profile: "A".into(),
            task: "B".into(),
        };
        assert_eq!(
            allocate(&b, &[session(a, 1)]),
            Err(Error::SessionNameCollision)
        );
    }
    #[test]
    fn configuration_bounds_and_subset_are_checked() {
        let config = Configuration {
            profiles: vec![Profile {
                name: "A".into(),
                root: "/work".into(),
                desktop_entries: vec!["a.desktop".into()],
                tasks: vec![Task {
                    name: "T".into(),
                    desktop_entries: vec!["b.desktop".into()],
                }],
            }],
        };
        assert!(
            matches!(config.validate(), Err(Error::Validation { path, .. }) if path == "profiles[0].tasks[0].desktop_entries")
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn configuration_boundaries_report_stable_paths() {
        let profile = Profile {
            name: "P".into(),
            root: "/work".into(),
            desktop_entries: vec![],
            tasks: vec![],
        };
        let too_many_profiles = Configuration {
            profiles: (0..=MAX_PROFILES)
                .map(|index| Profile {
                    name: format!("P{index}"),
                    ..profile.clone()
                })
                .collect(),
        };
        assert!(matches!(
            too_many_profiles.validate(),
            Err(Error::Validation { path, .. }) if path == "profiles"
        ));
        let too_many_tasks = Configuration {
            profiles: vec![Profile {
                tasks: (0..=MAX_TASKS_PER_PROFILE)
                    .map(|index| Task {
                        name: format!("T{index}"),
                        desktop_entries: vec![],
                    })
                    .collect(),
                ..profile.clone()
            }],
        };
        assert!(matches!(
            too_many_tasks.validate(),
            Err(Error::Validation { path, .. }) if path == "profiles[0].tasks"
        ));
        let too_many_entries = Configuration {
            profiles: vec![Profile {
                desktop_entries: (0..=MAX_DESKTOP_ENTRIES)
                    .map(|index| format!("app{index}.desktop"))
                    .collect(),
                ..profile.clone()
            }],
        };
        assert!(matches!(
            too_many_entries.validate(),
            Err(Error::Validation { path, .. }) if path == "profiles[0].desktop_entries"
        ));
        for (invalid, expected_path) in [
            (
                Configuration {
                    profiles: vec![Profile {
                        name: "\u{0000}".into(),
                        ..profile.clone()
                    }],
                },
                "profiles[0].name",
            ),
            (
                Configuration {
                    profiles: vec![Profile {
                        name: "x".repeat(129),
                        ..profile.clone()
                    }],
                },
                "profiles[0].name",
            ),
            (
                Configuration {
                    profiles: vec![Profile {
                        root: "/work/../escape".into(),
                        ..profile.clone()
                    }],
                },
                "profiles[0].root",
            ),
            (
                Configuration {
                    profiles: vec![Profile {
                        root: format!("/{}", "x".repeat(4096)),
                        ..profile.clone()
                    }],
                },
                "profiles[0].root",
            ),
            (
                Configuration {
                    profiles: vec![Profile {
                        desktop_entries: vec!["bad-entry".into()],
                        ..profile.clone()
                    }],
                },
                "profiles[0].desktop_entries",
            ),
            (
                Configuration {
                    profiles: vec![Profile {
                        desktop_entries: vec![format!("{}.desktop", "x".repeat(248))],
                        ..profile.clone()
                    }],
                },
                "profiles[0].desktop_entries",
            ),
            (
                Configuration {
                    profiles: vec![Profile {
                        tasks: vec![Task {
                            name: "x".repeat(129),
                            desktop_entries: vec![],
                        }],
                        ..profile.clone()
                    }],
                },
                "profiles[0].tasks[0].name",
            ),
            (
                Configuration {
                    profiles: vec![Profile {
                        tasks: vec![Task {
                            name: "T".into(),
                            desktop_entries: vec!["bad-entry".into()],
                        }],
                        ..profile.clone()
                    }],
                },
                "profiles[0].tasks[0].desktop_entries",
            ),
        ] {
            assert!(matches!(
                invalid.validate(),
                Err(Error::Validation { path, .. }) if path == expected_path
            ));
        }
    }

    #[test]
    fn session_invariants_distinguish_lifecycle_and_active_role() {
        let source = LaunchSource::Profile("FDS".into());
        let mut paused = session(source.clone(), 1);
        paused.lifecycle = Lifecycle::Paused;
        paused.checkpoint_id = Some(Uuid::new_v4());
        assert!(validate_sessions(&[paused.clone()]).is_ok());

        paused.active = true;
        assert!(matches!(
            validate_sessions(&[paused]),
            Err(Error::Validation { path, .. }) if path == "sessions[0].active"
        ));

        let mut running = session(source, 2);
        running.checkpoint_id = Some(Uuid::new_v4());
        assert!(matches!(
            validate_sessions(&[running]),
            Err(Error::Validation { path, .. }) if path == "sessions[0].checkpoint_id"
        ));

        let mut first = session(LaunchSource::Profile("A".into()), 1);
        let mut second = session(LaunchSource::Profile("B".into()), 1);
        first.active = true;
        second.active = true;
        assert!(matches!(
            validate_sessions(&[first, second]),
            Err(Error::Validation { path, .. }) if path == "sessions"
        ));
    }

    #[test]
    fn lifecycle_state_matrix_accepts_only_valid_checkpoint_and_active_combinations() {
        let source = LaunchSource::Profile("FDS".into());
        for (lifecycle, checkpoint, active, expected) in [
            (Lifecycle::Running, None, false, true),
            (Lifecycle::Running, None, true, true),
            (Lifecycle::Paused, Some(Uuid::new_v4()), false, true),
            (Lifecycle::Running, Some(Uuid::new_v4()), false, false),
            (Lifecycle::Paused, None, false, false),
            (Lifecycle::Paused, Some(Uuid::new_v4()), true, false),
        ] {
            let mut candidate = session(source.clone(), 1);
            candidate.lifecycle = lifecycle;
            candidate.checkpoint_id = checkpoint;
            candidate.active = active;
            assert_eq!(validate_sessions(&[candidate]).is_ok(), expected);
        }
    }

    #[test]
    fn rename_preserves_immutable_snapshots_and_session_identity() {
        let mut config = Configuration {
            profiles: vec![Profile {
                name: "FDS".into(),
                root: "/work/fds".into(),
                desktop_entries: vec!["editor.desktop".into()],
                tasks: vec![Task {
                    name: "Edit".into(),
                    desktop_entries: vec!["editor.desktop".into()],
                }],
            }],
        };
        let source = LaunchSource::Task {
            profile: "FDS".into(),
            task: "Edit".into(),
        };
        let mut sessions = vec![session(source, 1)];
        sessions[0].id = Uuid::new_v4();
        sessions[0].checkpoint_id = Some(Uuid::new_v4());
        sessions[0].lifecycle = Lifecycle::Paused;
        sessions[0].source_root = "/work/fds".into();
        sessions[0].source_desktop_entries = vec!["editor.desktop".into()];
        let mut active = session(LaunchSource::Profile("FDS".into()), 2);
        active.active = true;
        active.source_root = "/work/fds".into();
        active.source_desktop_entries = vec!["editor.desktop".into()];
        sessions.push(active);
        let original = sessions.clone();

        rename_profile(&mut config, &mut sessions, "FDS", "FDS2".into()).unwrap();
        rename_task(&mut config, &mut sessions, "FDS2", "Edit", "Write".into()).unwrap();

        for (renamed, original) in sessions.iter().zip(&original) {
            assert_eq!(renamed.id, original.id);
            assert_eq!(renamed.checkpoint_id, original.checkpoint_id);
            assert_eq!(renamed.number, original.number);
            assert_eq!(renamed.lifecycle, original.lifecycle);
            assert_eq!(renamed.active, original.active);
            assert_eq!(renamed.source_root, original.source_root);
            assert_eq!(
                renamed.source_desktop_entries,
                original.source_desktop_entries
            );
        }
        assert_eq!(sessions[0].name, "FDS2-Write-1");
        assert_eq!(sessions[1].name, "FDS2-2");
    }

    #[test]
    fn allocation_reuses_stopped_numbers_but_not_lower_live_gaps_or_maximum() {
        let source = LaunchSource::Profile("QMK".into());
        let uncommitted_candidate = allocate(&source, &[]).unwrap();
        assert_eq!(uncommitted_candidate.0, 1);
        assert_eq!(allocate(&source, &[]).unwrap(), uncommitted_candidate);
        assert_eq!(
            allocate(
                &source,
                &[session(source.clone(), 1), session(source.clone(), 3)]
            )
            .unwrap()
            .0,
            4
        );
        assert_eq!(
            allocate(&source, &[session(source.clone(), u32::MAX)]),
            Err(Error::SessionNumberExhausted)
        );
    }

    #[test]
    fn fds_and_qmk_sequences_are_per_source_and_stopped_numbers_are_reusable() {
        let fds = LaunchSource::Profile("FDS".into());
        let edit = LaunchSource::Task {
            profile: "FDS".into(),
            task: "MontageVidéo".into(),
        };
        let qmk = LaunchSource::Profile("QMK".into());
        let live = [
            session(fds.clone(), 1),
            session(edit.clone(), 1),
            session(qmk.clone(), 1),
        ];
        assert_eq!(allocate(&fds, &live).unwrap(), (2, "FDS-2".into()));
        assert_eq!(
            allocate(&edit, &live).unwrap(),
            (2, "FDS-MontageVidéo-2".into())
        );
        assert_eq!(allocate(&qmk, &live).unwrap(), (2, "QMK-2".into()));

        let after_fds_stop = [session(edit, 1), session(qmk, 1)];
        assert_eq!(
            allocate(&fds, &after_fds_stop).unwrap(),
            (1, "FDS-1".into())
        );
    }

    #[test]
    fn rename_collision_is_all_or_nothing() {
        let mut config = Configuration {
            profiles: vec![
                Profile {
                    name: "C".into(),
                    root: "/c".into(),
                    desktop_entries: vec![],
                    tasks: vec![],
                },
                Profile {
                    name: "A".into(),
                    root: "/a".into(),
                    desktop_entries: vec![],
                    tasks: vec![Task {
                        name: "B".into(),
                        desktop_entries: vec![],
                    }],
                },
            ],
        };
        let mut sessions = vec![
            session(LaunchSource::Profile("C".into()), 1),
            session(
                LaunchSource::Task {
                    profile: "A".into(),
                    task: "B".into(),
                },
                1,
            ),
        ];
        let original_config = config.clone();
        let original_sessions = sessions.clone();

        assert_eq!(
            rename_profile(&mut config, &mut sessions, "C", "A-B".into()),
            Err(Error::SessionNameCollision)
        );
        assert_eq!(config, original_config);
        assert_eq!(sessions, original_sessions);
        assert_eq!(
            allocate(&LaunchSource::Profile("C".into()), &sessions)
                .unwrap()
                .0,
            2
        );

        let mut task_config = Configuration {
            profiles: vec![
                Profile {
                    name: "A-B".into(),
                    root: "/a-b".into(),
                    desktop_entries: vec![],
                    tasks: vec![],
                },
                Profile {
                    name: "A".into(),
                    root: "/a".into(),
                    desktop_entries: vec![],
                    tasks: vec![Task {
                        name: "C".into(),
                        desktop_entries: vec![],
                    }],
                },
            ],
        };
        let mut task_sessions = vec![
            session(LaunchSource::Profile("A-B".into()), 1),
            session(
                LaunchSource::Task {
                    profile: "A".into(),
                    task: "C".into(),
                },
                1,
            ),
        ];
        let original_task_config = task_config.clone();
        let original_task_sessions = task_sessions.clone();
        assert_eq!(
            rename_task(&mut task_config, &mut task_sessions, "A", "C", "B".into()),
            Err(Error::SessionNameCollision)
        );
        assert_eq!(task_config, original_task_config);
        assert_eq!(task_sessions, original_task_sessions);
    }

    #[test]
    fn deletion_guards_cover_profile_and_task_live_references() {
        let profile = LaunchSource::Profile("FDS".into());
        let task = LaunchSource::Task {
            profile: "FDS".into(),
            task: "Edit".into(),
        };
        let mut paused = session(task.clone(), 1);
        paused.lifecycle = Lifecycle::Paused;
        paused.checkpoint_id = Some(Uuid::new_v4());
        assert_eq!(
            can_delete(&profile, &[paused.clone()]),
            Err(Error::ReferencedSource)
        );
        assert_eq!(can_delete(&task, &[paused]), Err(Error::ReferencedSource));
        assert!(can_delete(&task, &[session(profile, 1)]).is_ok());

        let profile = LaunchSource::Profile("FDS".into());
        assert_eq!(
            can_delete(&profile, &[session(profile.clone(), 1)]),
            Err(Error::ReferencedSource)
        );
    }

    #[test]
    fn reload_policy_guards_live_source_identity_but_allows_safe_changes() {
        let current = Configuration {
            profiles: vec![
                Profile {
                    name: "FDS".into(),
                    root: "/work/fds".into(),
                    desktop_entries: vec!["editor.desktop".into()],
                    tasks: vec![Task {
                        name: "Edit".into(),
                        desktop_entries: vec!["editor.desktop".into()],
                    }],
                },
                Profile {
                    name: "Unused".into(),
                    root: "/work/unused".into(),
                    desktop_entries: vec![],
                    tasks: vec![],
                },
            ],
        };
        let profile_live = [session(LaunchSource::Profile("FDS".into()), 1)];
        let snapshot = LiveReferenceSnapshot {
            runtime_revision: 7,
            sessions: &profile_live,
        };

        let mut non_identity_edit = current.clone();
        non_identity_edit.profiles[0].root = "/work/renamed-on-disk".into();
        non_identity_edit.profiles.pop();
        assert!(reload_allowed(&current, &non_identity_edit, snapshot).is_ok());

        let mut addition = current.clone();
        addition.profiles.push(Profile {
            name: "Added".into(),
            root: "/work/added".into(),
            desktop_entries: vec![],
            tasks: vec![],
        });
        assert!(reload_allowed(&current, &addition, snapshot).is_ok());

        let removal = Configuration {
            profiles: vec![current.profiles[1].clone()],
        };
        assert_eq!(
            reload_allowed(&current, &removal, snapshot),
            Err(Error::ReferencedSource)
        );

        let apparent_rename = Configuration {
            profiles: vec![Profile {
                name: "FDS2".into(),
                ..current.profiles[0].clone()
            }],
        };
        assert_eq!(
            reload_allowed(&current, &apparent_rename, snapshot),
            Err(Error::ReferencedSource)
        );

        let mut collision = current.clone();
        collision.profiles.push(current.profiles[0].clone());
        assert!(matches!(
            reload_allowed(&current, &collision, snapshot),
            Err(Error::Validation { path, .. }) if path == "profiles[2].name"
        ));

        let task_live = [session(
            LaunchSource::Task {
                profile: "FDS".into(),
                task: "Edit".into(),
            },
            1,
        )];
        let without_task = Configuration {
            profiles: vec![Profile {
                tasks: vec![],
                ..current.profiles[0].clone()
            }],
        };
        assert_eq!(
            reload_allowed(
                &current,
                &without_task,
                LiveReferenceSnapshot {
                    runtime_revision: 7,
                    sessions: &task_live,
                },
            ),
            Err(Error::ReferencedSource)
        );
    }
}

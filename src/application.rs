//! Serialized project/session use cases and read-only current application queries.

use crate::{
    applications::{ApplicationList, ApplicationResolver, ExecutableLookup},
    configuration::{Locations, Settings},
    domain::{Lifecycle, MAX_SESSIONS, RuntimeState, Session, allocate},
    error::{Error, Faults, NoFault, Result},
    projects::{Project, Projects},
    runtime::RuntimeStore,
    safe_fs::Directory,
    storage_port::UuidGenerator,
};
use nix::fcntl::{Flock, FlockArg, OFlag, openat};
use nix::sys::stat::Mode;
use serde::Serialize;
use std::{
    fs::File,
    os::unix::fs::MetadataExt,
    sync::{Mutex, MutexGuard},
};
use uuid::Uuid;

pub struct RandomUuids;
impl UuidGenerator for RandomUuids {
    fn next_uuid(&mut self) -> String {
        Uuid::new_v4().to_string()
    }
}

/// Owns the process lock for its entire lifetime, including read-only recovery.
pub struct Core<U: UuidGenerator = RandomUuids> {
    projects: Projects,
    inner: Mutex<Inner<U>>,
    _lock: Flock<File>,
    orphans: Vec<std::ffi::OsString>,
}
struct Inner<U> {
    store: RuntimeStore,
    state: Option<RuntimeState>,
    uuids: U,
}
#[derive(Debug, Clone, Serialize)]
pub struct SessionView {
    #[serde(flatten)]
    pub session: Session,
    pub active: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct SessionMutation {
    pub revision: u64,
    pub session: SessionView,
}
#[derive(Debug, Clone, Serialize)]
pub struct StopMutation {
    pub revision: u64,
    pub stopped_session_id: Uuid,
}
#[derive(Debug, Clone, Serialize)]
pub struct DeactivateMutation {
    pub revision: u64,
    pub active_session_id: Option<Uuid>,
}
#[derive(Debug, Clone, Serialize)]
pub struct SessionList {
    pub revision: u64,
    pub active_session_id: Option<Uuid>,
    pub sessions: Vec<SessionView>,
}
impl Core<RandomUuids> {
    pub fn open(locations: &Locations) -> Result<Self> {
        Self::with_uuids(locations, RandomUuids)
    }
}
impl<U: UuidGenerator> Core<U> {
    pub fn with_uuids(locations: &Locations, uuids: U) -> Result<Self> {
        let settings = Settings::load(locations)?;
        let runtime_dir = Directory::private(&locations.runtime)?;
        let file = File::from(openat(
            &runtime_dir.0,
            "state.lock",
            OFlag::O_RDWR
                | OFlag::O_CREAT
                | OFlag::O_NOFOLLOW
                | OFlag::O_NONBLOCK
                | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )?);
        let meta = file.metadata()?;
        if !meta.is_file()
            || meta.nlink() != 1
            || meta.uid() != nix::unistd::getuid().as_raw()
            || meta.mode() & 0o777 != 0o600
        {
            return Err(Error("filesystem.unavailable"));
        }
        let lock = Flock::lock(file, FlockArg::LockExclusiveNonblock)
            .map_err(|_| Error("conflict.state_locked"))?;
        let store = RuntimeStore::open(&locations.state)?;
        let state = store.load().ok();
        let projects = Projects::new(
            settings.project_source,
            locations.config.join("profile-template"),
        );
        let orphans = projects.orphans()?;
        Ok(Self {
            projects,
            inner: Mutex::new(Inner {
                store,
                state,
                uuids,
            }),
            _lock: lock,
            orphans,
        })
    }
    fn lock(&self) -> Result<MutexGuard<'_, Inner<U>>> {
        self.inner.lock().map_err(|_| Error("internal.error"))
    }
    pub fn recovery_required(&self) -> Result<bool> {
        Ok(self.lock()?.state.is_none())
    }
    /// Undiscoverable creation remnants; never automatically deleted on startup.
    pub fn incomplete_creations(&self) -> &[std::ffi::OsString] {
        &self.orphans
    }
    pub fn projects(&self) -> Result<Vec<Project>> {
        self.projects.list()
    }
    pub fn templates(&self) -> Result<Vec<String>> {
        self.projects.templates()
    }
    pub fn lookup_project(&self, name: &str) -> Result<Project> {
        self.projects.lookup(name)
    }
    pub fn create_project(&self, base: &str, template: &str) -> Result<Project> {
        self.create_project_with_faults(base, template, &mut NoFault)
    }
    pub fn create_project_with_faults(
        &self,
        base: &str,
        template: &str,
        faults: &mut impl Faults,
    ) -> Result<Project> {
        let inner = self.lock()?;
        if inner.state.is_none() {
            return Err(Error("recovery.invalid_runtime"));
        }
        self.projects.create(base, template, faults)
    }
    pub fn sessions(&self) -> Result<SessionList> {
        let inner = self.lock()?;
        let state = inner
            .state
            .as_ref()
            .ok_or(Error("recovery.invalid_runtime"))?;
        let mut sessions: Vec<_> = state.sessions.iter().map(|s| view(state, s)).collect();
        sessions.sort_by_key(|s| s.session.id);
        Ok(SessionList {
            revision: state.revision,
            active_session_id: state.active_session_id,
            sessions,
        })
    }
    /// Snapshot the active role under the lock, then read its current context without
    /// holding the mutation lock across untrusted filesystem reads.
    pub fn applications<P: ExecutableLookup>(
        &self,
        resolver: &ApplicationResolver<P>,
    ) -> Result<ApplicationList> {
        self.applications_with_faults(resolver, &mut NoFault)
    }
    pub fn applications_with_faults<P: ExecutableLookup>(
        &self,
        resolver: &ApplicationResolver<P>,
        faults: &mut impl Faults,
    ) -> Result<ApplicationList> {
        let project_name = {
            let inner = self.lock()?;
            let state = inner
                .state
                .as_ref()
                .ok_or(Error("recovery.invalid_runtime"))?;
            state
                .active_session_id
                .and_then(|id| state.sessions.iter().find(|s| s.id == id))
                .map(|s| s.project_name.clone())
        };
        let context = project_name
            .as_deref()
            .map(|name| self.projects.current_context(name))
            .transpose()?
            .flatten();
        resolver.current(context.as_ref(), faults)
    }
    pub fn start(&self, project: &str) -> Result<SessionView> {
        self.start_with_faults(project, &mut NoFault)
    }
    pub fn start_with_faults(
        &self,
        project: &str,
        faults: &mut impl Faults,
    ) -> Result<SessionView> {
        Ok(self.start_commit(project, faults)?.session)
    }
    pub fn start_result(&self, project: &str) -> Result<SessionMutation> {
        self.start_commit(project, &mut NoFault)
    }
    fn start_commit(&self, project: &str, faults: &mut impl Faults) -> Result<SessionMutation> {
        let mut inner = self.lock()?;
        let mut next = inner
            .state
            .clone()
            .ok_or(Error("recovery.invalid_runtime"))?;
        self.projects.lookup(project)?;
        if next.sessions.len() == MAX_SESSIONS {
            return Err(Error("limit.exceeded"));
        }
        next.next_revision()?;
        let (number, name) = allocate(project, &next.sessions)?;
        let id = Uuid::parse_str(&inner.uuids.next_uuid()).map_err(|_| Error("internal.error"))?;
        if next.sessions.iter().any(|s| s.id == id) {
            return Err(Error("internal.error"));
        }
        let session = Session {
            id,
            project_name: project.into(),
            number,
            name,
            lifecycle: Lifecycle::Running,
        };
        next.sessions.push(session.clone());
        next.active_session_id = Some(id);
        self.projects.lookup(project)?;
        commit(&mut inner, next, faults)?;
        let state = inner
            .state
            .as_ref()
            .ok_or(Error("recovery.invalid_runtime"))?;
        Ok(SessionMutation {
            revision: state.revision,
            session: view(state, &session),
        })
    }
    pub fn switch(&self, id: Uuid) -> Result<SessionView> {
        Ok(self.switch_result(id)?.session)
    }
    pub fn switch_result(&self, id: Uuid) -> Result<SessionMutation> {
        let mut inner = self.lock()?;
        let mut next = inner
            .state
            .clone()
            .ok_or(Error("recovery.invalid_runtime"))?;
        let session = next
            .sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .ok_or(Error("not_found.session"))?;
        if next.active_session_id != Some(id) {
            next.active_session_id = Some(id);
            next.next_revision()?;
            commit(&mut inner, next, &mut NoFault)?;
        }
        let state = inner
            .state
            .as_ref()
            .ok_or(Error("recovery.invalid_runtime"))?;
        Ok(SessionMutation {
            revision: state.revision,
            session: view(state, &session),
        })
    }
    /// Clear the active role without stopping any running session.
    /// Repeating this operation while already neutral does not commit a revision.
    pub fn deactivate(&self) -> Result<DeactivateMutation> {
        let mut inner = self.lock()?;
        let mut next = inner
            .state
            .clone()
            .ok_or(Error("recovery.invalid_runtime"))?;
        if next.active_session_id.is_some() {
            next.active_session_id = None;
            next.next_revision()?;
            commit(&mut inner, next, &mut NoFault)?;
        }
        let state = inner
            .state
            .as_ref()
            .ok_or(Error("recovery.invalid_runtime"))?;
        Ok(DeactivateMutation {
            revision: state.revision,
            active_session_id: state.active_session_id,
        })
    }
    pub fn stop(&self, id: Uuid) -> Result<()> {
        self.stop_with_faults(id, &mut NoFault)
    }
    pub fn stop_with_faults(&self, id: Uuid, faults: &mut impl Faults) -> Result<()> {
        self.stop_commit(id, faults).map(|_| ())
    }
    pub fn stop_result(&self, id: Uuid) -> Result<StopMutation> {
        self.stop_commit(id, &mut NoFault)
    }
    fn stop_commit(&self, id: Uuid, faults: &mut impl Faults) -> Result<StopMutation> {
        let mut inner = self.lock()?;
        let mut next = inner
            .state
            .clone()
            .ok_or(Error("recovery.invalid_runtime"))?;
        if !next.sessions.iter().any(|s| s.id == id) {
            return Err(Error("not_found.session"));
        }
        next.sessions.retain(|s| s.id != id);
        if next.active_session_id == Some(id) {
            next.active_session_id = None;
        }
        next.next_revision()?;
        let revision = next.revision;
        commit(&mut inner, next, faults)?;
        Ok(StopMutation {
            revision,
            stopped_session_id: id,
        })
    }
}
fn view(state: &RuntimeState, session: &Session) -> SessionView {
    SessionView {
        session: session.clone(),
        active: state.active_session_id == Some(session.id),
    }
}
fn commit<U>(inner: &mut Inner<U>, next: RuntimeState, faults: &mut impl Faults) -> Result<()> {
    let old = inner
        .state
        .as_ref()
        .ok_or(Error("recovery.invalid_runtime"))?;
    match inner.store.commit(old, &next, faults) {
        Ok(()) => {
            inner.state = Some(next);
            Ok(())
        }
        Err(e) => {
            if e.0 == "persistence.uncertain"
                || e.0 == "recovery.invalid_runtime"
                || inner.store.load().is_err()
            {
                inner.state = None;
            }
            Err(e)
        }
    }
}

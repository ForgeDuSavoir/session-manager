//! Minimal version 2 runtime persistence; no legacy state is read.

use crate::{
    canonical_json,
    domain::RuntimeState,
    error::{Boundary, Error, Faults, Result},
    safe_fs::Directory,
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const CURRENT: &str = "runtime-v2.json";
const PREVIOUS: &str = "runtime-v2.previous.json";
pub const MAX_RUNTIME_BYTES: usize = 1024 * 1024;
pub(crate) struct RuntimeStore {
    dir: Directory,
    path: PathBuf,
}
impl RuntimeStore {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            dir: Directory::private(path)?,
            path: path.to_owned(),
        })
    }
    pub fn load(&self) -> Result<RuntimeState> {
        if !crate::safe_fs::same_file(
            &self.dir.0.metadata()?,
            &Directory::open(&self.path)?.0.metadata()?,
        ) {
            return Err(Error("recovery.invalid_runtime"));
        }
        if !self.dir.exists(Path::new(CURRENT))? {
            if self.dir.exists(Path::new(PREVIOUS))? {
                return Err(Error("recovery.invalid_runtime"));
            }
            return Ok(RuntimeState::default());
        }
        let bytes = self
            .dir
            .read_file(Path::new(CURRENT), MAX_RUNTIME_BYTES)
            .map_err(|_| Error("recovery.invalid_runtime"))?;
        decode(&bytes)
    }
    pub fn commit(
        &self,
        old: &RuntimeState,
        next: &RuntimeState,
        faults: &mut impl Faults,
    ) -> Result<()> {
        let bytes = encode(next)?;
        if next.revision != old.revision.checked_add(1).ok_or(Error("limit.exceeded"))? {
            return Err(Error("recovery.invalid_runtime"));
        }
        // Do not overwrite externally damaged/changed current state or back it up as valid.
        if &self.load()? != old {
            return Err(Error("recovery.invalid_runtime"));
        }
        let had_current = self.dir.exists(Path::new(CURRENT))?;
        let candidate = format!(".runtime-v2-{}", Uuid::new_v4());
        let backup = format!(".runtime-v2-backup-{}", Uuid::new_v4());
        let result = (|| {
            self.dir.new_file(Path::new(&candidate), &bytes)?;
            faults.check(Boundary::RuntimeCandidateSynced)?;
            self.dir.new_file(Path::new(&backup), &encode(old)?)?;
            self.dir.rename(Path::new(&backup), Path::new(PREVIOUS))?;
            self.dir.sync()?;
            faults.check(Boundary::RuntimeBackupSynced)?;
            // A first commit has just created a previous revision but not current yet.
            // Check that specific pre-state instead of interpreting it as startup recovery.
            if !crate::safe_fs::same_file(
                &self.dir.0.metadata()?,
                &Directory::open(&self.path)?.0.metadata()?,
            ) || self.dir.exists(Path::new(CURRENT))? != had_current
                || had_current
                    && decode(&self.dir.read_file(Path::new(CURRENT), MAX_RUNTIME_BYTES)?)? != *old
            {
                return Err(Error("recovery.invalid_runtime"));
            }
            self.dir.rename(Path::new(&candidate), Path::new(CURRENT))?;
            // From this point a failure must never be advertised as a known rollback.
            faults
                .check(Boundary::RuntimeReplaced)
                .and_then(|()| self.dir.sync())
                .map_err(|_| Error("persistence.uncertain"))
        })();
        let _ = self.dir.unlink(Path::new(&candidate));
        let _ = self.dir.unlink(Path::new(&backup));
        result
    }
}
pub fn encode(state: &RuntimeState) -> Result<Vec<u8>> {
    state.validate()?;
    let bytes = canonical_json::encode_line(state).map_err(|_| Error("internal.error"))?;
    if bytes.len() > MAX_RUNTIME_BYTES {
        return Err(Error("limit.exceeded"));
    }
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<RuntimeState> {
    if bytes.len() > MAX_RUNTIME_BYTES {
        return Err(Error("recovery.invalid_runtime"));
    }
    let state: RuntimeState =
        serde_json::from_slice(bytes).map_err(|_| Error("recovery.invalid_runtime"))?;
    state.validate()?;
    Ok(state)
}

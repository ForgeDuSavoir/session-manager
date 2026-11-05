//! Process-environment boundary. Clients resolve only the runtime socket location.

use crate::{
    applications::ApplicationEnvironment,
    configuration::Locations,
    error::{Error, Result},
    safe_fs::{Directory, validate_path},
};
use std::{
    env,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

fn variable(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) if value.len() <= 65536 && !value.chars().any(|c| c.is_ascii_control()) => {
            Ok(Some(value))
        }
        Ok(_) | Err(env::VarError::NotUnicode(_)) => Err(Error("validation.invalid_request")),
        Err(env::VarError::NotPresent) => Ok(None),
    }
}
fn home() -> Result<PathBuf> {
    let path = PathBuf::from(
        variable("HOME")?
            .filter(|s| !s.is_empty())
            .ok_or(Error("validation.invalid_request"))?,
    );
    validate_path(&path)?;
    Ok(path)
}
pub fn client_runtime() -> Result<PathBuf> {
    let path = PathBuf::from(
        variable("XDG_RUNTIME_DIR")?
            .filter(|s| !s.is_empty())
            .ok_or(Error("validation.invalid_request"))?,
    );
    validate_runtime_home(&path)?;
    Ok(path.join("session-manager"))
}
pub fn validate_runtime_home(path: &Path) -> Result<()> {
    let dir = Directory::open(path)?;
    let m = dir.0.metadata()?;
    if m.uid() != nix::unistd::getuid().as_raw() || m.mode() & 0o7777 != 0o700 {
        return Err(Error("filesystem.unavailable"));
    }
    Ok(())
}
pub struct Startup {
    pub locations: Locations,
    pub applications: ApplicationEnvironment,
}
impl Startup {
    pub fn capture() -> Result<Self> {
        let home = home()?;
        let config = variable("XDG_CONFIG_HOME")?
            .filter(|s| !s.is_empty())
            .map_or_else(|| home.join(".config"), PathBuf::from);
        let state = variable("XDG_STATE_HOME")?
            .filter(|s| !s.is_empty())
            .map_or_else(|| home.join(".local/state"), PathBuf::from);
        let runtime = client_runtime()?;
        validate_path(&config)?;
        validate_path(&state)?;
        let applications = ApplicationEnvironment::from_values(
            &home,
            variable("XDG_DATA_HOME")?.as_deref(),
            variable("XDG_DATA_DIRS")?.as_deref(),
            variable("XDG_CURRENT_DESKTOP")?.as_deref(),
            variable("PATH")?.as_deref(),
        )?;
        Ok(Self {
            locations: Locations {
                config: config.join("session-manager"),
                state: state.join("session-manager"),
                runtime,
            },
            applications,
        })
    }
    /// Create missing state parents only. Never create config, project or runtime roots.
    pub fn prepare_state_parent(&self) -> Result<()> {
        let path = self
            .locations
            .state
            .parent()
            .ok_or(Error("validation.invalid_request"))?;
        validate_path(path)?;
        // Validate settings before any bootstrap writes, including source/root disjointness.
        crate::configuration::Settings::load(&self.locations)?;
        let mut dir = Directory::open(Path::new("/"))?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                let p = Path::new(name);
                dir = if let Some(child) = dir
                    .optional_child(p)
                    .map_err(|_| Error("filesystem.unavailable"))?
                {
                    child
                } else {
                    let child = dir.mkdir(p)?;
                    child.sync()?;
                    dir.sync()?;
                    child
                };
            }
        }
        Ok(())
    }
}

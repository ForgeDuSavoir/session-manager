//! Strict version 2 settings with explicitly injected filesystem locations.

use crate::{
    error::{Error, Result},
    safe_fs::{Directory, validate_path},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub project_source: PathBuf,
}
#[derive(Debug, Clone)]
pub struct Locations {
    /// Session Manager's directory beneath `XDG_CONFIG_HOME`.
    pub config: PathBuf,
    /// Session Manager's directory beneath `XDG_STATE_HOME`.
    pub state: PathBuf,
    /// Session Manager's directory beneath `XDG_RUNTIME_DIR`.
    pub runtime: PathBuf,
}
impl Settings {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 65536 {
            return Err(Error("limit.exceeded"));
        }
        let text = std::str::from_utf8(bytes).map_err(|_| Error("validation.invalid_request"))?;
        let settings: Self =
            toml::from_str(text).map_err(|_| Error("validation.invalid_request"))?;
        if settings.schema_version != 2 {
            return Err(Error("validation.invalid_request"));
        }
        validate_path(&settings.project_source)?;
        if settings.project_source == Path::new("/") {
            return Err(Error("validation.invalid_request"));
        }
        Ok(settings)
    }
    pub fn load(locations: &Locations) -> Result<Self> {
        let config = Directory::open(&locations.config)?;
        let bytes = config.read_file(Path::new("config-v2.toml"), 65536)?;
        let settings = Self::decode(&bytes)?;
        for root in [&locations.config, &locations.state, &locations.runtime] {
            validate_path(root)?;
            // Reject overlap in either direction; nested state must not be discovered as a project.
            if settings.project_source.starts_with(root)
                || root.starts_with(&settings.project_source)
            {
                return Err(Error("validation.invalid_request"));
            }
        }
        Directory::open(&settings.project_source)?;
        Ok(settings)
    }
}

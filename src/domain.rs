//! Pure running-session identity and allocation rules.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const STAGING_PREFIX: &str = ".session-manager-create-";
pub const MAX_SESSIONS: usize = 1024;

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "."
        && name != ".."
        && name.trim() == name
        && !name.ends_with('.')
        && !name
            .chars()
            .any(|c| c.is_ascii_control() || c == '/' || c == '\\')
}
pub fn valid_project_name(name: &str) -> bool {
    valid_name(name) && !name.starts_with(STAGING_PREFIX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Running,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: Uuid,
    pub project_name: String,
    pub number: u32,
    pub name: String,
    pub lifecycle: Lifecycle,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeState {
    pub runtime_version: u32,
    pub revision: u64,
    pub active_session_id: Option<Uuid>,
    pub sessions: Vec<Session>,
}
impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            runtime_version: 2,
            revision: 0,
            active_session_id: None,
            sessions: Vec::new(),
        }
    }
}
impl RuntimeState {
    pub fn validate(&self) -> Result<()> {
        let invalid = Error("recovery.invalid_runtime");
        if self.runtime_version != 2 || self.sessions.len() > MAX_SESSIONS {
            return Err(invalid);
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut numbers = BTreeSet::new();
        for s in &self.sessions {
            if !valid_project_name(&s.project_name)
                || s.number == 0
                || s.name != generated_name(&s.project_name, s.number)
                || !ids.insert(s.id)
                || !names.insert(&s.name)
                || !numbers.insert((&s.project_name, s.number))
            {
                return Err(invalid);
            }
        }
        if self.active_session_id.is_some_and(|id| !ids.contains(&id)) {
            return Err(invalid);
        }
        Ok(())
    }
    pub fn next_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(Error("limit.exceeded"))?;
        Ok(())
    }
}
pub fn generated_name(project: &str, number: u32) -> String {
    format!("{project}-{number}")
}
pub fn allocate(project: &str, sessions: &[Session]) -> Result<(u32, String)> {
    let highest = sessions
        .iter()
        .filter(|s| s.project_name == project)
        .map(|s| s.number)
        .max()
        .unwrap_or(0);
    let number = highest
        .checked_add(1)
        .ok_or(Error("conflict.session_number_exhausted"))?;
    let name = generated_name(project, number);
    if sessions.iter().any(|s| s.name == name) {
        return Err(Error("conflict.session_name_collision"));
    }
    Ok((number, name))
}

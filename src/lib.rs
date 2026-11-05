//! Version 2 core: directory-backed projects and independent running sessions.
//!
//! All filesystem locations are explicit. No desktop effects, legacy state,
//! or adapter startup are part of this core. Application discovery uses explicit roots.

pub mod application;
pub mod applications;
pub mod bootstrap;
pub mod canonical_json;
pub mod cli;
pub mod configuration;
mod desktop_entry;
pub mod domain;
pub mod error;
pub mod interface;
pub mod projects;
pub mod runtime;
mod safe_fs;
pub mod storage_port;
pub mod transport;

#[cfg(test)]
mod c1_tests;
#[cfg(test)]
mod c2_tests;
#[cfg(test)]
mod c3_tests;
#[cfg(test)]
pub(crate) mod test_support;

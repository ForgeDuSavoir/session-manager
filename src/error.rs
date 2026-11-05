//! Stable, privacy-safe core errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub &'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self("filesystem.unavailable")
    }
}
impl From<nix::errno::Errno> for Error {
    fn from(_: nix::errno::Errno) -> Self {
        Self("filesystem.unavailable")
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// Deterministic interruption points; production callers use `NoFault`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    ApplicationProjectOpened,
    ApplicationOverridesRead,
    TemplateFileRead,
    CopyFile,
    BeforePublish,
    ProjectTargetChecked,
    ProjectPublished,
    RuntimeCandidateSynced,
    RuntimeBackupSynced,
    RuntimeReplaced,
}
pub trait Faults {
    fn check(&mut self, boundary: Boundary) -> Result<()>;
}
pub struct NoFault;
impl Faults for NoFault {
    fn check(&mut self, _: Boundary) -> Result<()> {
        Ok(())
    }
}

//! Test-only isolation tools. They are not compiled into production artifacts.

use std::{
    cell::RefCell,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;

use crate::storage_port::{Clock, UuidGenerator};

const SENTINEL: &str = ".session-manager-test-root";
static ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Deterministic clock for tests.
#[derive(Debug, Clone, Copy)]
pub struct FakeClock {
    now_unix_ms: u64,
}

impl FakeClock {
    pub const fn new(now_unix_ms: u64) -> Self {
        Self { now_unix_ms }
    }

    pub fn set(&mut self, now_unix_ms: u64) {
        self.now_unix_ms = now_unix_ms;
    }
}

impl Clock for FakeClock {
    fn now_unix_ms(&self) -> u64 {
        self.now_unix_ms
    }
}

/// Deterministic UUID sequence for tests.
#[derive(Debug, Clone)]
pub struct FakeUuidGenerator {
    next: u128,
}

impl FakeUuidGenerator {
    pub const fn new(first: u128) -> Self {
        Self { next: first }
    }
}

impl UuidGenerator for FakeUuidGenerator {
    fn next_uuid(&mut self) -> String {
        let value = self.next;
        self.next = self
            .next
            .checked_add(1)
            .expect("test UUID sequence exhausted");
        format!(
            "{high:08x}-{middle:04x}-7000-8000-{low:012x}",
            high = value >> 96,
            middle = (value >> 80) & 0xffff,
            low = value & 0xffff_ffff_ffff
        )
    }
}

/// A canonical, sentinel-owned XDG test root.
#[derive(Debug)]
pub struct TemporaryXdgRoot {
    root: PathBuf,
}

impl TemporaryXdgRoot {
    pub fn new() -> io::Result<Self> {
        let sequence = ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = std::env::temp_dir().join(format!(
            "session-manager-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&candidate)?;
        let root = fs::canonicalize(candidate)?;
        fs::write(root.join(SENTINEL), b"session-manager temporary XDG root\n")?;
        for name in ["home", "config", "state", "runtime"] {
            fs::create_dir(root.join(name))?;
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// Returns only a path contained in this root; absolute and escaping inputs fail.
    pub fn contained(&self, relative: &Path) -> io::Result<PathBuf> {
        if relative.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute test path",
            ));
        }
        let candidate = self.root.join(relative);
        self.assert_contained(&candidate)?;
        Ok(candidate)
    }

    /// Verifies canonical containment and rejects a path whose existing ancestors escape.
    pub fn assert_contained(&self, path: &Path) -> io::Result<()> {
        if !path.is_absolute() || !path.starts_with(&self.root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path outside test root",
            ));
        }
        let existing = path
            .ancestors()
            .find(|ancestor| ancestor.exists())
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no existing path ancestor"))?;
        if !fs::canonicalize(existing)?.starts_with(&self.root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "symlink escape",
            ));
        }
        Ok(())
    }

    /// Removes this root only after sentinel, containment, and file-type validation.
    pub fn cleanup(self) -> io::Result<()> {
        self.assert_cleanup_safe()?;
        fs::remove_dir_all(&self.root)
    }

    fn assert_cleanup_safe(&self) -> io::Result<()> {
        if self.root == Path::new("/") || self.root == user_home()? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe cleanup root",
            ));
        }
        if !self.root.join(SENTINEL).is_file() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "missing ownership sentinel",
            ));
        }
        self.assert_contained(&self.root)?;
        inspect_tree(&self.root, &self.root)
    }
}

fn user_home() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|path| fs::canonicalize(path).ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home is unavailable"))
}

fn inspect_tree(root: &Path, current: &Path) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_symlink() || file_type.is_socket() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe test-root entry",
            ));
        }
        if file_type.is_dir() {
            let canonical = fs::canonicalize(&path)?;
            if !canonical.starts_with(root) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "directory escape",
                ));
            }
            inspect_tree(root, &path)?;
        }
    }
    Ok(())
}

/// Records forbidden interactions attempted by a test.
#[derive(Debug, Default)]
pub struct TrapEndpoints {
    attempts: RefCell<Vec<&'static str>>,
}

impl TrapEndpoints {
    #[allow(dead_code)]
    pub fn installed_adapter_discovery(&self) -> io::Result<()> {
        self.reject("installed-adapter discovery")
    }
    #[allow(dead_code)]
    pub fn display(&self) -> io::Result<()> {
        self.reject("display")
    }
    #[allow(dead_code)]
    pub fn compositor(&self) -> io::Result<()> {
        self.reject("compositor")
    }
    #[allow(dead_code)]
    pub fn user_bus(&self) -> io::Result<()> {
        self.reject("user bus")
    }
    #[allow(dead_code)]
    pub fn process_manager(&self) -> io::Result<()> {
        self.reject("process manager")
    }
    #[allow(dead_code)]
    pub fn host_xdg(&self) -> io::Result<()> {
        self.reject("host XDG")
    }

    pub fn assert_unreached(&self) {
        assert!(
            self.attempts.borrow().is_empty(),
            "forbidden attempts: {:?}",
            self.attempts.borrow()
        );
    }

    fn reject(&self, endpoint: &'static str) -> io::Result<()> {
        self.attempts.borrow_mut().push(endpoint);
        Err(io::Error::new(io::ErrorKind::PermissionDenied, endpoint))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn fakes_are_deterministic() {
        let mut clock = FakeClock::new(42);
        assert_eq!(clock.now_unix_ms(), 42);
        clock.set(99);
        assert_eq!(clock.now_unix_ms(), 99);
        let mut ids = FakeUuidGenerator::new(1);
        assert_eq!(ids.next_uuid(), "00000000-0000-7000-8000-000000000001");
        assert_eq!(ids.next_uuid(), "00000000-0000-7000-8000-000000000002");
    }

    #[test]
    fn temporary_root_rejects_unsafe_paths_and_cleans_owned_root() {
        let root = TemporaryXdgRoot::new().unwrap();
        assert!(root.home().starts_with(root.root()));
        assert!(root.contained(Path::new("/tmp")).is_err());
        assert!(root.assert_contained(Path::new("relative")).is_err());
        assert!(root.assert_contained(Path::new("/")).is_err());
        assert!(root.assert_contained(&user_home().unwrap()).is_err());
        root.cleanup().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn temporary_root_rejects_symlink_escapes_foreign_sockets_and_missing_sentinels() {
        let root = TemporaryXdgRoot::new().unwrap();
        std::os::unix::fs::symlink("/tmp", root.root().join("escape")).unwrap();
        assert!(
            root.assert_contained(&root.root().join("escape/file"))
                .is_err()
        );
        assert!(root.assert_cleanup_safe().is_err());
        fs::remove_file(root.root().join("escape")).unwrap();
        let socket = match UnixListener::bind(root.root().join("foreign.sock")) {
            Ok(socket) => socket,
            // Some restricted test sandboxes prohibit Unix-socket creation. The
            // production guard is still covered where the fixture is permitted.
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                fs::remove_file(root.root().join(SENTINEL)).unwrap();
                assert!(root.assert_cleanup_safe().is_err());
                fs::write(root.root().join(SENTINEL), b"restored\n").unwrap();
                root.cleanup().unwrap();
                return;
            }
            Err(error) => panic!("could not create foreign socket fixture: {error}"),
        };
        assert!(root.assert_cleanup_safe().is_err());
        drop(socket);
        fs::remove_file(root.root().join("foreign.sock")).unwrap();
        fs::remove_file(root.root().join(SENTINEL)).unwrap();
        assert!(root.assert_cleanup_safe().is_err());
        fs::write(root.root().join(SENTINEL), b"restored\n").unwrap();
        root.cleanup().unwrap();
    }

    #[test]
    fn trap_endpoints_are_unreached_by_default() {
        let traps = TrapEndpoints::default();
        traps.assert_unreached();
    }
}

//! Linux directory-relative, no-follow filesystem operations.

use crate::error::{Error, Result};
use nix::sys::stat::fchmod;
use nix::{
    dir::Dir,
    errno::Errno,
    fcntl::{AtFlags, OFlag, RenameFlags, open, openat, renameat, renameat2},
    sys::stat::{Mode, fstatat, mkdirat},
    unistd::{UnlinkatFlags, unlinkat},
};
use std::{
    ffi::OsString,
    fs::{File, Metadata},
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Component, Path},
};

pub(crate) struct Directory(pub File);

pub fn validate_path(path: &Path) -> Result<()> {
    let s = path.to_str().ok_or(Error("validation.invalid_request"))?;
    if !s.starts_with('/')
        || s.len() > 4096
        || s.contains('\0')
        || s != "/"
            && s[1..]
                .split('/')
                .any(|c| c.is_empty() || c == "." || c == "..")
    {
        return Err(Error("validation.invalid_request"));
    }
    Ok(())
}
impl Directory {
    /// Optional discovery traversal preserves absence separately from unsafe paths and IO.
    pub fn open_optional(path: &Path) -> Result<Option<Self>> {
        validate_path(path)?;
        let mut current = Self::open(Path::new("/"))?;
        for part in path.components() {
            if let Component::Normal(name) = part {
                let Some(next) = current.optional_child(Path::new(name))? else {
                    return Ok(None);
                };
                current = next;
            }
        }
        Ok(Some(current))
    }
    pub fn optional_child(&self, name: &Path) -> Result<Option<Self>> {
        match self.stat(name) {
            Err(Errno::ENOENT) => return Ok(None),
            Err(e) => return Err(e.into()),
            Ok(s) if s.st_mode & nix::libc::S_IFMT == nix::libc::S_IFLNK => {
                return Err(Error("filesystem.symlink"));
            }
            Ok(s) if s.st_mode & nix::libc::S_IFMT != nix::libc::S_IFDIR => {
                return Err(Error("filesystem.unsafe_path"));
            }
            Ok(_) => (),
        }
        match self.child(name) {
            Ok(dir) => Ok(Some(dir)),
            Err(e) => match self.stat(name) {
                Err(Errno::ENOENT) => Ok(None),
                Ok(s) if s.st_mode & nix::libc::S_IFMT == nix::libc::S_IFLNK => {
                    Err(Error("filesystem.symlink"))
                }
                Ok(s) if s.st_mode & nix::libc::S_IFMT != nix::libc::S_IFDIR => {
                    Err(Error("filesystem.unsafe_path"))
                }
                _ => Err(e),
            },
        }
    }
    pub fn open(path: &Path) -> Result<Self> {
        validate_path(path)?;
        let mut current = Self(File::from(open(
            "/",
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
            Mode::empty(),
        )?));
        for part in path.components() {
            if let Component::Normal(name) = part {
                current = current.child(Path::new(name))?;
            }
        }
        Ok(current)
    }
    pub fn child(&self, name: &Path) -> Result<Self> {
        Ok(Self(File::from(openat(
            &self.0,
            name,
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::empty(),
        )?)))
    }
    pub fn entries(&self, limit: usize) -> Result<Vec<OsString>> {
        // A new open file description prevents enumeration from sharing an offset.
        let fd = openat(
            &self.0,
            ".",
            OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
            Mode::empty(),
        )?;
        let mut dir = Dir::from_fd(fd)?;
        let mut names = Vec::new();
        for item in dir.iter() {
            let item = item?;
            let name = item.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            if names.len() == limit {
                return Err(Error("limit.exceeded"));
            }
            names.push(std::ffi::OsStr::from_bytes(name).to_owned());
        }
        names.sort();
        Ok(names)
    }
    pub fn stat(&self, name: &Path) -> std::result::Result<nix::sys::stat::FileStat, Errno> {
        fstatat(&self.0, name, AtFlags::AT_SYMLINK_NOFOLLOW)
    }
    pub fn exists(&self, name: &Path) -> Result<bool> {
        match self.stat(name) {
            Ok(_) => Ok(true),
            Err(Errno::ENOENT) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    pub fn mkdir(&self, name: &Path) -> Result<Self> {
        mkdirat(&self.0, name, Mode::S_IRWXU)?;
        let dir = self.child(name)?;
        fchmod(&dir.0, Mode::S_IRWXU)?;
        Ok(dir)
    }
    pub fn private(path: &Path) -> Result<Self> {
        validate_path(path)?;
        let parent = Self::open(path.parent().ok_or(Error("validation.invalid_request"))?)?;
        let name = Path::new(
            path.file_name()
                .ok_or(Error("validation.invalid_request"))?,
        );
        let created = match mkdirat(&parent.0, name, Mode::S_IRWXU) {
            Ok(()) => true,
            Err(Errno::EEXIST) => false,
            Err(e) => return Err(e.into()),
        };
        let dir = parent.child(name)?;
        if created {
            fchmod(&dir.0, Mode::S_IRWXU)?;
            dir.sync()?;
            parent.sync()?;
        }
        let m = dir.0.metadata()?;
        if m.uid() != nix::unistd::getuid().as_raw() || m.mode() & 0o7777 != 0o700 {
            return Err(Error("filesystem.unavailable"));
        }
        Ok(dir)
    }
    pub fn read_file(&self, name: &Path, limit: usize) -> Result<Vec<u8>> {
        let mut file = self.regular(name)?;
        read_bounded(&mut file, limit)
    }
    pub fn regular(&self, name: &Path) -> Result<File> {
        let file = File::from(openat(
            &self.0,
            name,
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )?);
        if !file.metadata()?.is_file() {
            return Err(Error("filesystem.unavailable"));
        }
        Ok(file)
    }
    pub fn new_file(&self, name: &Path, bytes: &[u8]) -> Result<()> {
        let mut f = File::from(openat(
            &self.0,
            name,
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_EXCL | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )?);
        fchmod(&f, Mode::S_IRUSR | Mode::S_IWUSR)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        Ok(())
    }
    pub fn sync(&self) -> Result<()> {
        self.0.sync_all()?;
        Ok(())
    }
    pub fn rename(&self, old: &Path, new: &Path) -> Result<()> {
        renameat(&self.0, old, &self.0, new)?;
        Ok(())
    }
    pub fn publish(&self, old: &Path, new: &Path) -> std::result::Result<(), Errno> {
        renameat2(&self.0, old, &self.0, new, RenameFlags::RENAME_NOREPLACE)
    }
    pub fn unlink(&self, name: &Path) -> Result<()> {
        unlinkat(&self.0, name, UnlinkatFlags::NoRemoveDir)?;
        Ok(())
    }
    /// Never recursively remove an entry not still identified as the owned tree.
    pub fn remove_owned(&self, name: &Path, owned: &Self) -> Result<()> {
        self.remove_owned_bounded(name, owned, 0, &mut 0)
    }
    fn remove_owned_bounded(
        &self,
        name: &Path,
        owned: &Self,
        depth: usize,
        entries: &mut usize,
    ) -> Result<()> {
        if depth > 16 {
            return Err(Error("limit.exceeded"));
        }
        let stat = self.stat(name)?;
        let meta = owned.0.metadata()?;
        if stat.st_dev != meta.dev() || stat.st_ino != meta.ino() {
            return Err(Error("filesystem.unavailable"));
        }
        for entry in owned.entries(2048)? {
            *entries += 1;
            if *entries > 2048 {
                return Err(Error("limit.exceeded"));
            }
            let path = Path::new(&entry);
            let stat = owned.stat(path)?;
            if stat.st_mode & nix::libc::S_IFMT == nix::libc::S_IFDIR {
                let child = owned.child(path)?;
                owned.remove_owned_bounded(path, &child, depth + 1, entries)?;
            } else {
                owned.unlink(path)?;
            }
        }
        // Verify identity again before unlinking the staging name.
        let stat = self.stat(name)?;
        if stat.st_dev != meta.dev() || stat.st_ino != meta.ino() {
            return Err(Error("filesystem.unavailable"));
        }
        unlinkat(&self.0, name, UnlinkatFlags::RemoveDir)?;
        Ok(())
    }
}
pub(crate) fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
pub(crate) fn unchanged(a: &Metadata, b: &Metadata) -> bool {
    same_file(a, b)
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
        && a.nlink() == b.nlink()
}
pub(crate) fn read_bounded(file: &mut File, limit: usize) -> Result<Vec<u8>> {
    if file.metadata()?.len() > limit as u64 {
        return Err(Error("limit.exceeded"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(Error("limit.exceeded"));
    }
    Ok(bytes)
}

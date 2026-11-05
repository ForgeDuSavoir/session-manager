//! On-demand project discovery and bounded, atomic template publication.

use crate::{
    domain::{STAGING_PREFIX, valid_name, valid_project_name},
    error::{Boundary, Error, Faults, Result},
    safe_fs::{Directory, read_bounded, unchanged},
};
use nix::{
    errno::Errno,
    libc::{S_IFDIR, S_IFMT, S_IFREG},
};
use serde::Serialize;
use std::{
    ffi::OsString,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const DEFAULT_TEMPLATE: &str = "Default Profile";
const SCAN_LIMIT: usize = 16384;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
}

pub struct Projects {
    source: PathBuf,
    templates: PathBuf,
}

/// Request-scoped context only; never retained in runtime state.
pub(crate) struct ProjectContext<'a> {
    source: &'a Path,
    name: String,
    parent: Directory,
    pub dir: Directory,
}
impl ProjectContext<'_> {
    pub fn is_current(&self) -> Result<bool> {
        let Some(parent) = Directory::open_optional(self.source).map_err(public_io)? else {
            return Ok(false);
        };
        if !crate::safe_fs::same_file(&parent.0.metadata()?, &self.parent.0.metadata()?) {
            return Ok(false);
        }
        match parent.stat(Path::new(&self.name)) {
            Err(Errno::ENOENT) => Ok(false),
            Err(e) => Err(e.into()),
            Ok(s) => {
                let m = self.dir.0.metadata()?;
                Ok(s.st_mode & S_IFMT == S_IFDIR && s.st_dev == m.dev() && s.st_ino == m.ino())
            }
        }
    }
}
fn public_io(error: Error) -> Error {
    if matches!(error.0, "filesystem.unsafe_path" | "filesystem.symlink") {
        Error("filesystem.unavailable")
    } else {
        error
    }
}
impl Projects {
    pub fn new(source: PathBuf, templates: PathBuf) -> Self {
        Self { source, templates }
    }
    fn source(&self) -> Result<Directory> {
        Directory::open(&self.source)
    }
    pub(crate) fn current_context(&self, name: &str) -> Result<Option<ProjectContext<'_>>> {
        if !valid_project_name(name) {
            return Err(Error("validation.invalid_request"));
        }
        let Some(parent) = Directory::open_optional(&self.source).map_err(public_io)? else {
            return Ok(None);
        };
        let dir = match parent.optional_child(Path::new(name)) {
            Ok(Some(dir)) => dir,
            Ok(None) | Err(Error("filesystem.unsafe_path" | "filesystem.symlink")) => {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
        Ok(Some(ProjectContext {
            source: &self.source,
            name: name.into(),
            parent,
            dir,
        }))
    }
    pub fn list(&self) -> Result<Vec<Project>> {
        let dir = self.source()?;
        let mut result = Vec::new();
        for name in dir.entries(SCAN_LIMIT)? {
            let Some(text) = name.to_str().filter(|s| valid_project_name(s)) else {
                continue;
            };
            match dir.stat(Path::new(&name)) {
                Ok(s) if s.st_mode & S_IFMT == S_IFDIR => result.push(Project {
                    name: text.into(),
                    path: self.source.join(&name),
                }),
                Ok(_) | Err(Errno::ENOENT) => (),
                Err(e) => return Err(e.into()),
            }
            if result.len() > 1024 {
                return Err(Error("limit.exceeded"));
            }
        }
        Ok(result)
    }
    /// Verifies existence now; no handle or path is retained by a session.
    pub fn lookup(&self, name: &str) -> Result<Project> {
        if !valid_project_name(name) {
            return Err(Error("validation.invalid_request"));
        }
        let dir = self.source()?;
        match dir.stat(Path::new(name)) {
            Ok(s) if s.st_mode & S_IFMT == S_IFDIR => {
                if let Err(error) = dir.child(Path::new(name)) {
                    match dir.stat(Path::new(name)) {
                        Err(Errno::ENOENT) => return Err(Error("not_found.project")),
                        Ok(s) if s.st_mode & S_IFMT != S_IFDIR => {
                            return Err(Error("not_found.project"));
                        }
                        _ => return Err(error),
                    }
                }
            }
            Ok(_) | Err(Errno::ENOENT) => return Err(Error("not_found.project")),
            Err(e) => return Err(e.into()),
        }
        Ok(Project {
            name: name.into(),
            path: self.source.join(name),
        })
    }
    fn template_root(&self) -> Result<Option<Directory>> {
        // The config parent must exist; only the final template directory may be absent.
        let parent = Directory::open(
            self.templates
                .parent()
                .ok_or(Error("validation.invalid_request"))?,
        )?;
        let name = Path::new(
            self.templates
                .file_name()
                .ok_or(Error("validation.invalid_request"))?,
        );
        if !parent.exists(name)? {
            return Ok(None);
        }
        Ok(Some(parent.child(name)?))
    }
    pub fn templates(&self) -> Result<Vec<String>> {
        let mut result = vec![DEFAULT_TEMPLATE.to_owned()];
        if let Some(dir) = self.template_root()? {
            for name in dir.entries(SCAN_LIMIT)? {
                if name == DEFAULT_TEMPLATE {
                    return Err(Error("template.invalid"));
                }
                let Some(text) = name.to_str().filter(|s| valid_name(s)) else {
                    continue;
                };
                match dir.stat(Path::new(&name)) {
                    Ok(s) if s.st_mode & S_IFMT == S_IFDIR => result.push(text.into()),
                    Ok(_) | Err(Errno::ENOENT) => (),
                    Err(e) => return Err(e.into()),
                }
                if result.len() > 256 {
                    return Err(Error("limit.exceeded"));
                }
            }
        }
        result.sort();
        Ok(result)
    }
    pub fn orphans(&self) -> Result<Vec<OsString>> {
        Ok(self
            .source()?
            .entries(SCAN_LIMIT)?
            .into_iter()
            .filter(|n| n.as_bytes().starts_with(STAGING_PREFIX.as_bytes()))
            .collect())
    }
    pub fn create(&self, base: &str, template: &str, faults: &mut impl Faults) -> Result<Project> {
        if !valid_project_name(base) || base.len() + 11 > 128 || !valid_name(template) {
            return Err(Error("validation.invalid_request"));
        }
        if !self.templates()?.iter().any(|t| t == template) {
            return Err(Error("not_found.template"));
        }
        let tree = if template == DEFAULT_TEMPLATE {
            Tree::default()
        } else {
            let root = self.template_root()?.ok_or(Error("not_found.template"))?;
            let dir = root.child(Path::new(template))?;
            let stat = root.stat(Path::new(template))?;
            let meta = dir.0.metadata()?;
            if stat.st_dev != meta.dev() || stat.st_ino != meta.ino() {
                return Err(Error("template.invalid"));
            }
            let mut budget = Budget::default();
            let tree = read_tree(&dir, 0, &mut budget, faults)?;
            let after = root.stat(Path::new(template))?;
            if after.st_dev != meta.dev() || after.st_ino != meta.ino() {
                return Err(Error("template.invalid"));
            }
            tree
        };
        let source = self.source()?;
        source.entries(SCAN_LIMIT)?;
        let stage_name = format!("{STAGING_PREFIX}{}", Uuid::new_v4());
        let stage_path = Path::new(&stage_name);
        let stage = source.mkdir(stage_path)?;
        let prepublish = (|| {
            write_tree(&stage, &tree, faults)?;
            faults.check(Boundary::BeforePublish)?;
            let mut number = 1u32;
            loop {
                // Lowest currently free suffix, counting every occupied entry.
                let name = format!("{base}-{number}");
                if !source.exists(Path::new(&name))? {
                    faults.check(Boundary::ProjectTargetChecked)?;
                    // Detect source-root and staging-name substitution before publication.
                    let current_source = self.source()?;
                    if !crate::safe_fs::same_file(
                        &source.0.metadata()?,
                        &current_source.0.metadata()?,
                    ) {
                        return Err(Error("filesystem.unavailable"));
                    }
                    let stat = source.stat(stage_path)?;
                    let meta = stage.0.metadata()?;
                    if stat.st_dev != meta.dev() || stat.st_ino != meta.ino() {
                        return Err(Error("filesystem.unavailable"));
                    }
                    match source.publish(stage_path, Path::new(&name)) {
                        Ok(()) => {
                            return Ok(Project {
                                name,
                                path: self.source.join(format!("{base}-{number}")),
                            });
                        }
                        Err(Errno::EEXIST | Errno::ENOTEMPTY) => {
                            number = 1;
                            continue;
                        }
                        Err(e) => return Err(e.into()),
                    }
                }
                number = number
                    .checked_add(1)
                    .ok_or(Error("conflict.project_name_exhausted"))?;
            }
        })();
        match prepublish {
            Ok(project) => {
                faults
                    .check(Boundary::ProjectPublished)
                    .and_then(|()| source.sync())
                    .map_err(|_| Error("persistence.uncertain"))?;
                Ok(project)
            }
            Err(error) => {
                // Failed cleanup leaves an undiscoverable orphan for explicit inspection.
                let _ = source.remove_owned(stage_path, &stage);
                let _ = source.sync();
                Err(error)
            }
        }
    }
}
use std::os::unix::ffi::OsStrExt;
#[derive(Default)]
struct Budget {
    entries: usize,
    bytes: usize,
}
#[derive(Default)]
struct Tree {
    entries: Vec<(OsString, Node)>,
}
enum Node {
    Directory(Tree),
    File(Vec<u8>),
}
fn read_tree(
    dir: &Directory,
    depth: usize,
    budget: &mut Budget,
    faults: &mut impl Faults,
) -> Result<Tree> {
    if depth > 16 {
        return Err(Error("limit.exceeded"));
    }
    let before = dir.0.metadata()?;
    let mut tree = Tree::default();
    for name in dir.entries(2048)? {
        budget.entries += 1;
        if budget.entries > 2048 {
            return Err(Error("limit.exceeded"));
        }
        let text = name.to_str().ok_or(Error("template.invalid"))?;
        if text.contains(['/', '\0']) || text == "." || text == ".." {
            return Err(Error("template.invalid"));
        }
        let path = Path::new(&name);
        let stat = dir.stat(path)?;
        let node = match stat.st_mode & S_IFMT {
            S_IFDIR => {
                let child = dir.child(path)?;
                let m = child.0.metadata()?;
                if stat.st_dev != m.dev() || stat.st_ino != m.ino() {
                    return Err(Error("template.invalid"));
                }
                Node::Directory(read_tree(&child, depth + 1, budget, faults)?)
            }
            S_IFREG => {
                let mut file = dir.regular(path)?;
                let m = file.metadata()?;
                if m.nlink() != 1 || stat.st_dev != m.dev() || stat.st_ino != m.ino() {
                    return Err(Error("template.invalid"));
                }
                let bytes = read_bounded(&mut file, 1024 * 1024)?;
                faults.check(Boundary::TemplateFileRead)?;
                if !unchanged(&m, &file.metadata()?) {
                    return Err(Error("template.invalid"));
                }
                let final_stat = dir.stat(path)?;
                if final_stat.st_dev != m.dev() || final_stat.st_ino != m.ino() {
                    return Err(Error("template.invalid"));
                }
                budget.bytes += bytes.len();
                if budget.bytes > 16 * 1024 * 1024 {
                    return Err(Error("limit.exceeded"));
                }
                Node::File(bytes)
            }
            _ => return Err(Error("template.invalid")),
        };
        tree.entries.push((name, node));
    }
    if !unchanged(&before, &dir.0.metadata()?) {
        return Err(Error("template.invalid"));
    }
    Ok(tree)
}
fn write_tree(dir: &Directory, tree: &Tree, faults: &mut impl Faults) -> Result<()> {
    for (name, node) in &tree.entries {
        match node {
            Node::Directory(tree) => write_tree(&dir.mkdir(Path::new(name))?, tree, faults)?,
            Node::File(bytes) => {
                dir.new_file(Path::new(name), bytes)?;
                faults.check(Boundary::CopyFile)?;
            }
        }
    }
    dir.sync()
}

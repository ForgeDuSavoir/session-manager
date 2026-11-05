//! Bounded current XDG application lists. All environment values are injected.

use crate::{
    canonical_json,
    desktop_entry::DesktopEntry,
    error::{Boundary, Error, Faults, Result},
    projects::ProjectContext,
    safe_fs::{Directory, read_bounded, unchanged, validate_path},
};
use nix::{
    libc::{S_IFDIR, S_IFMT, S_IFREG},
    unistd::{AccessFlags, access},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

const MAX_ENTRIES: usize = 16384;
const MAX_IDS: usize = 4096;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const INVALID_LOCAL: Error = Error("application.invalid_project_file");

/// A validated startup snapshot of environment *locations*, never application data.
/// Callers supply values explicitly; this type does not read the process environment.
#[derive(Debug, Clone)]
pub struct ApplicationEnvironment {
    roots: Vec<PathBuf>,
    desktops: BTreeSet<String>,
    path: Vec<PathBuf>,
}
impl ApplicationEnvironment {
    pub fn from_values(
        home: &Path,
        data_home: Option<&str>,
        data_dirs: Option<&str>,
        current_desktop: Option<&str>,
        path: Option<&str>,
    ) -> Result<Self> {
        validate_path(home)?;
        if home.as_os_str().as_bytes().iter().any(u8::is_ascii_control) {
            return Err(Error("validation.invalid_request"));
        }
        for value in [data_home, data_dirs, current_desktop, path]
            .into_iter()
            .flatten()
        {
            if value.len() > 65536 {
                return Err(Error("limit.exceeded"));
            }
            if value.chars().any(|c| c.is_ascii_control()) {
                return Err(Error("validation.invalid_request"));
            }
        }
        let data_home = data_home
            .filter(|v| !v.is_empty())
            .map_or_else(|| home.join(".local/share"), PathBuf::from);
        let data_dirs = path_list(
            data_dirs
                .filter(|v| !v.is_empty())
                .unwrap_or("/usr/local/share:/usr/share"),
        )?;
        validate_path(&data_home)?;
        let mut roots = vec![data_home.join("applications")];
        roots.extend(data_dirs.into_iter().map(|p| p.join("applications")));
        let mut seen = BTreeSet::new();
        for root in &roots {
            validate_path(root)?;
        }
        roots.retain(|root| seen.insert(root.clone()));
        let desktops = current_desktop
            .unwrap_or("")
            .split(':')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        Ok(Self {
            roots,
            desktops,
            path: path_list(path.unwrap_or(""))?,
        })
    }
    pub fn application_roots(&self) -> &[PathBuf] {
        &self.roots
    }
}
fn path_list(value: &str) -> Result<Vec<PathBuf>> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(':')
        .map(|part| {
            let path = PathBuf::from(part);
            validate_path(&path)?;
            Ok(path)
        })
        .collect()
}

/// A metadata-only executable-presence check, replaceable by a trap in tests.
pub trait ExecutableLookup {
    fn is_executable(&self, program: &str) -> bool;
}
pub struct PathExecutables {
    path: Vec<PathBuf>,
}
impl ExecutableLookup for PathExecutables {
    fn is_executable(&self, program: &str) -> bool {
        let executable = |path: &Path| {
            fs::metadata(path).is_ok_and(|m| m.is_file()) && access(path, AccessFlags::X_OK).is_ok()
        };
        if program.is_empty() {
            return false;
        }
        let program = Path::new(program);
        if program.is_absolute() {
            return executable(program);
        }
        // A relative pathname is not searched against the daemon's working directory.
        if program.components().count() != 1 || program.as_os_str().as_bytes().contains(&b'/') {
            return false;
        }
        self.path.iter().any(|dir| executable(&dir.join(program)))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Installed,
    Project,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub terminal: bool,
    pub origin: Origin,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplicationList {
    pub applications: Vec<Application>,
}
impl ApplicationList {
    fn bounded(applications: BTreeMap<String, Application>) -> Result<Self> {
        if applications.len() > MAX_IDS {
            return Err(Error("limit.exceeded"));
        }
        let list = Self {
            applications: applications.into_values().collect(),
        };
        list.response_size()?;
        Ok(list)
    }
    // Count complete framing, not just the result; a UUID has fixed encoded width.
    pub(crate) fn response_size(&self) -> Result<usize> {
        let empty = serde_json::json!({"protocol_version":2,"request_id":uuid::Uuid::nil(),"type":"response","result":{"applications":[]}});
        let mut size = canonical_json::line_size(&empty).map_err(|_| Error("internal.error"))?;
        for (i, app) in self.applications.iter().enumerate() {
            size += canonical_json::line_size(app).map_err(|_| Error("internal.error"))? - 1
                + usize::from(i != 0);
            if size > MAX_RESPONSE_BYTES {
                return Err(Error("limit.exceeded"));
            }
        }
        Ok(size)
    }
}
pub struct ApplicationResolver<P = PathExecutables> {
    roots: Vec<PathBuf>,
    desktops: BTreeSet<String>,
    executables: P,
}
impl ApplicationResolver<PathExecutables> {
    pub fn new(environment: ApplicationEnvironment) -> Self {
        let executables = PathExecutables {
            path: environment.path.clone(),
        };
        Self::with_executables(environment, executables)
    }
}
impl<P: ExecutableLookup> ApplicationResolver<P> {
    pub fn with_executables(environment: ApplicationEnvironment, executables: P) -> Self {
        Self {
            roots: environment.roots,
            desktops: environment.desktops,
            executables,
        }
    }
    pub fn installed(&self) -> Result<ApplicationList> {
        ApplicationList::bounded(self.installed_map()?)
    }
    pub(crate) fn current(
        &self,
        project: Option<&ProjectContext<'_>>,
        faults: &mut impl Faults,
    ) -> Result<ApplicationList> {
        let mut apps = self.installed_map()?;
        if let Some(project) = project {
            faults.check(Boundary::ApplicationProjectOpened)?;
            // An open directory FD may outlive its name. Do not serve detached context.
            if !project.is_current()? {
                return ApplicationList::bounded(apps);
            }
            let overlay = self.local(&project.dir);
            faults.check(Boundary::ApplicationOverridesRead)?;
            if project.is_current()? {
                let (exclusions, local) = overlay?;
                for id in exclusions {
                    apps.remove(&id);
                }
                for (id, entry) in local {
                    apps.remove(&id);
                    if let Some(app) = entry {
                        apps.insert(id, app);
                    }
                }
            }
        }
        ApplicationList::bounded(apps)
    }
    fn visible(&self, id: String, entry: DesktopEntry, origin: Origin) -> Option<Application> {
        if entry.hidden
            || entry.no_display
            || entry
                .only_show_in
                .as_ref()
                .is_some_and(|names| !names.iter().any(|name| self.desktops.contains(name)))
            || entry
                .not_show_in
                .iter()
                .any(|name| self.desktops.contains(name))
            || entry
                .try_exec
                .as_ref()
                .is_some_and(|program| !self.executables.is_executable(program))
        {
            return None;
        }
        Some(Application {
            id,
            name: entry.name,
            exec: entry.exec,
            icon: entry.icon,
            terminal: entry.terminal,
            origin,
        })
    }
    fn installed_map(&self) -> Result<BTreeMap<String, Application>> {
        let mut scan = Scan::default();
        for path in &self.roots {
            let dir = match Directory::open_optional(path) {
                Ok(Some(dir)) => dir,
                Ok(None) | Err(Error("filesystem.symlink")) => continue,
                Err(Error("filesystem.unsafe_path")) => {
                    return Err(Error("filesystem.unavailable"));
                }
                Err(e) => return Err(e),
            };
            self.scan(&dir, "", 0, &mut scan)?;
        }
        Ok(scan.apps)
    }
    fn scan(&self, dir: &Directory, prefix: &str, depth: usize, scan: &mut Scan) -> Result<()> {
        if depth > 16 {
            return Err(Error("limit.exceeded"));
        }
        // Reserve every enumerated name before descending into any child.
        let names = dir.entries(MAX_ENTRIES - scan.entries)?;
        scan.entries += names.len();
        for name in names {
            let path = Path::new(&name);
            let stat = dir.stat(path)?;
            match stat.st_mode & S_IFMT {
                S_IFDIR => {
                    let text = name
                        .to_str()
                        .ok_or(Error("application.invalid_installed_file"))?;
                    self.scan(
                        &dir.child(path)?,
                        &format!("{prefix}{text}-"),
                        depth + 1,
                        scan,
                    )?;
                }
                S_IFREG if name.as_bytes().ends_with(b".desktop") => {
                    let id = format!(
                        "{prefix}{}",
                        name.to_str()
                            .ok_or(Error("application.invalid_installed_file"))?
                    );
                    if !valid_id(&id) {
                        return Err(Error("application.invalid_installed_file"));
                    }
                    if !scan.ids.insert(id.clone()) {
                        continue;
                    }
                    if scan.ids.len() > MAX_IDS {
                        return Err(Error("limit.exceeded"));
                    }
                    let entry = parse_file(dir, path, Origin::Installed)?;
                    if let Some(app) = self.visible(id.clone(), entry, Origin::Installed) {
                        scan.apps.insert(id, app);
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }
    fn local(&self, project: &Directory) -> Result<LocalEntries> {
        let Some(settings) = project
            .optional_child(Path::new(".session-manager"))
            .map_err(local_path_error)?
        else {
            return Ok((BTreeSet::new(), BTreeMap::new()));
        };
        let Some(dir) = settings
            .optional_child(Path::new("apps"))
            .map_err(local_path_error)?
        else {
            return Ok((BTreeSet::new(), BTreeMap::new()));
        };
        let mut exclusions = BTreeSet::new();
        let mut apps = BTreeMap::new();
        for name in dir.entries(MAX_ENTRIES)? {
            let path = Path::new(&name);
            if dir.stat(path)?.st_mode & S_IFMT != S_IFREG {
                return Err(INVALID_LOCAL);
            }
            if name == "exclude.txt" {
                exclusions = parse_exclusions(&read_file(&dir, path, 65536)?)?;
            } else if name.as_bytes().ends_with(b".desktop") {
                let id = name
                    .to_str()
                    .filter(|s| valid_id(s))
                    .ok_or(INVALID_LOCAL)?
                    .to_owned();
                if apps.len() == 1024 {
                    return Err(Error("limit.exceeded"));
                }
                let entry = parse_file(&dir, path, Origin::Project)?;
                apps.insert(id.clone(), self.visible(id, entry, Origin::Project));
            }
            // Other regular files (e.g. an editor backup) are not configuration.
        }
        Ok((exclusions, apps))
    }
}
#[derive(Default)]
struct Scan {
    entries: usize,
    ids: BTreeSet<String>,
    apps: BTreeMap<String, Application>,
}
type LocalEntries = (BTreeSet<String>, BTreeMap<String, Option<Application>>);
fn local_path_error(error: Error) -> Error {
    if matches!(error.0, "filesystem.unsafe_path" | "filesystem.symlink") {
        INVALID_LOCAL
    } else {
        error
    }
}
fn read_file(dir: &Directory, path: &Path, limit: usize) -> Result<Vec<u8>> {
    let stat = dir.stat(path)?;
    let mut file = dir.regular(path)?;
    let before = file.metadata()?;
    if stat.st_dev != before.dev() || stat.st_ino != before.ino() {
        return Err(Error("filesystem.unavailable"));
    }
    let bytes = read_bounded(&mut file, limit)?;
    let after = dir.stat(path)?;
    if !unchanged(&before, &file.metadata()?)
        || after.st_dev != before.dev()
        || after.st_ino != before.ino()
    {
        return Err(Error("filesystem.unavailable"));
    }
    Ok(bytes)
}
fn parse_file(dir: &Directory, path: &Path, origin: Origin) -> Result<DesktopEntry> {
    DesktopEntry::parse(&read_file(dir, path, MAX_FILE_BYTES)?).map_err(|error| {
        if error.0 == "application.invalid_desktop_entry" {
            match origin {
                Origin::Installed => Error("application.invalid_installed_file"),
                Origin::Project => INVALID_LOCAL,
            }
        } else {
            error
        }
    })
}
pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id.ends_with(".desktop")
        && !id.chars().any(|c| c.is_control() || c == '/' || c == '\\')
}
pub(crate) fn parse_exclusions(bytes: &[u8]) -> Result<BTreeSet<String>> {
    if bytes.len() > 65536 {
        return Err(Error("limit.exceeded"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| INVALID_LOCAL)?;
    if text.starts_with('\u{feff}') {
        return Err(INVALID_LOCAL);
    }
    let mut result = BTreeSet::new();
    for line in text.split_inclusive('\n') {
        let line = line
            .strip_suffix('\n')
            .map_or(line, |line| line.strip_suffix('\r').unwrap_or(line));
        if line.chars().any(char::is_control) {
            return Err(INVALID_LOCAL);
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !valid_id(line) || !result.insert(line.to_owned()) {
            return Err(INVALID_LOCAL);
        }
        if result.len() > 1024 {
            return Err(Error("limit.exceeded"));
        }
    }
    Ok(result)
}

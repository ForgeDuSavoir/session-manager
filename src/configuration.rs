//! Strict, offline configuration decoding and accepted-document identity.

use crate::domain::{Configuration, Error, Profile, Task};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use toml::Value;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, value};

#[cfg(unix)]
use nix::{
    fcntl::{Flock, FlockArg, OFlag, open},
    sys::stat::Mode,
};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

pub const MAX_CONFIGURATION_BYTES: usize = 1_048_576;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedConfiguration {
    pub bytes: Vec<u8>,
    pub etag: String,
    pub model: Configuration,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub device: u64,
    pub inode: u64,
    pub length: u64,
}
#[derive(Debug, Clone)]
pub struct StoredConfiguration {
    pub accepted: AcceptedConfiguration,
    pub identity: FileIdentity,
    pub path: PathBuf,
}

pub fn validate_bytes(bytes: &[u8]) -> Result<AcceptedConfiguration, Error> {
    if bytes.len() > MAX_CONFIGURATION_BYTES {
        return Err(Error::Validation {
            path: "document".into(),
            message: "configuration exceeds byte limit",
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Validation {
        path: "document".into(),
        message: "configuration is not UTF-8",
    })?;
    let root: Value = text.parse().map_err(|_| Error::Validation {
        path: "document".into(),
        message: "invalid TOML",
    })?;
    let table = root
        .as_table()
        .ok_or_else(|| invalid("document", "top level must be a table"))?;
    let syntax: DocumentMut = text
        .parse()
        .map_err(|_| invalid("document", "invalid TOML"))?;
    reject_unknown(table, &["schema_version", "profiles"], "document")?;
    if table.get("schema_version").and_then(Value::as_integer) != Some(1) {
        return Err(invalid("schema_version", "must equal 1"));
    }
    validate_collection_syntax(&syntax)?;
    let profiles = match table.get("profiles") {
        None => vec![],
        Some(Value::Array(values)) if values.is_empty() => {
            return Err(invalid(
                "profiles",
                "explicit empty profiles array is forbidden",
            ));
        }
        Some(Value::Array(values)) => values
            .iter()
            .enumerate()
            .map(parse_profile)
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(invalid("profiles", "must be an array of tables")),
    };
    let model = Configuration { profiles };
    model.validate()?;
    Ok(AcceptedConfiguration {
        bytes: bytes.to_vec(),
        etag: hex(&Sha256::digest(bytes)),
        model,
    })
}

fn validate_collection_syntax(document: &DocumentMut) -> Result<(), Error> {
    let Some(profiles) = document.get("profiles") else {
        return Ok(());
    };
    let profiles = profiles
        .as_array_of_tables()
        .ok_or_else(|| invalid("profiles", "must use array-of-tables syntax"))?;
    for (profile_index, profile) in profiles.iter().enumerate() {
        if let Some(tasks) = profile.get("tasks") {
            tasks.as_array_of_tables().ok_or_else(|| {
                invalid(
                    format!("profiles[{profile_index}].tasks"),
                    "must use array-of-tables syntax",
                )
            })?;
        }
    }
    Ok(())
}
pub fn validate_file(path: &Path) -> Result<AcceptedConfiguration, io::Error> {
    let (bytes, _) = read_regular(path)?;
    validate_bytes(&bytes).map_err(invalid_data)
}
pub fn load(path: &Path) -> Result<StoredConfiguration, io::Error> {
    let (bytes, metadata) = read_regular(path)?;
    let accepted = validate_bytes(&bytes).map_err(invalid_data)?;
    Ok(StoredConfiguration {
        accepted,
        identity: identity(&metadata),
        path: path.to_owned(),
    })
}
pub fn ensure_unchanged(stored: &StoredConfiguration) -> Result<(), io::Error> {
    let (bytes, metadata) = read_regular(&stored.path).map_err(|_| configuration_changed())?;
    if bytes != stored.accepted.bytes || identity(&metadata) != stored.identity {
        Err(configuration_changed())
    } else {
        Ok(())
    }
}
/// Renames one profile by changing only its `name` value in the lossless document.
pub fn rename_profile_bytes(bytes: &[u8], old: &str, new: &str) -> Result<Vec<u8>, Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("document", "configuration is not UTF-8"))?;
    let mut document: DocumentMut = text
        .parse()
        .map_err(|_| invalid("document", "invalid TOML"))?;
    let profiles = document["profiles"]
        .as_array_of_tables_mut()
        .ok_or_else(|| invalid("profiles", "must be array of tables"))?;
    let profile = profiles
        .iter_mut()
        .find(|table| table["name"].as_str() == Some(old))
        .ok_or_else(|| invalid("profile", "not found"))?;
    profile["name"] = value(new);
    let candidate = document.to_string().into_bytes();
    validate_bytes(&candidate)?;
    Ok(candidate)
}
/// Renames one task by changing only its `name` value in the lossless document.
pub fn rename_task_bytes(
    bytes: &[u8],
    profile_name: &str,
    old: &str,
    new: &str,
) -> Result<Vec<u8>, Error> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("document", "configuration is not UTF-8"))?;
    let mut document: DocumentMut = text
        .parse()
        .map_err(|_| invalid("document", "invalid TOML"))?;
    let profiles = document["profiles"]
        .as_array_of_tables_mut()
        .ok_or_else(|| invalid("profiles", "must be array of tables"))?;
    let profile = profiles
        .iter_mut()
        .find(|table| table["name"].as_str() == Some(profile_name))
        .ok_or_else(|| invalid("profile", "not found"))?;
    let tasks = profile["tasks"]
        .as_array_of_tables_mut()
        .ok_or_else(|| invalid("task", "not found"))?;
    let task = tasks
        .iter_mut()
        .find(|table| table["name"].as_str() == Some(old))
        .ok_or_else(|| invalid("task", "not found"))?;
    task["name"] = value(new);
    let candidate = document.to_string().into_bytes();
    validate_bytes(&candidate)?;
    Ok(candidate)
}
/// Appends a new profile table without reserializing existing profile tables.
pub fn create_profile_bytes(bytes: &[u8], profile: &Profile) -> Result<Vec<u8>, Error> {
    let mut document = editable_document(bytes)?;
    let profiles = profiles_mut_or_create(&mut document);
    profiles.push(profile_table(profile));
    let last = profiles
        .len()
        .checked_sub(1)
        .ok_or_else(|| invalid("profiles", "failed to append profile"))?;
    let created = profiles
        .get_mut(last)
        .ok_or_else(|| invalid("profiles", "failed to append profile"))?;
    if !profile.tasks.is_empty() {
        let tasks = tasks_mut_or_create(created);
        for task in &profile.tasks {
            tasks.push(task_table(task));
        }
    }
    finish_edit(document)
}
/// Updates one profile's root and desktop-entry boundaries, retaining its tasks.
pub fn update_profile_bytes(
    bytes: &[u8],
    name: &str,
    root: &str,
    desktop_entries: &[String],
) -> Result<Vec<u8>, Error> {
    let mut document = editable_document(bytes)?;
    let profile = find_profile_mut(&mut document, name)?;
    profile["root"] = value(root);
    profile["desktop_entries"] = value(entries_array(desktop_entries));
    finish_edit(document)
}
/// Removes one complete profile table, including its nested task tables.
pub fn delete_profile_bytes(bytes: &[u8], name: &str) -> Result<Vec<u8>, Error> {
    let mut document = editable_document(bytes)?;
    let profiles = profiles_mut(&mut document)?;
    let index = profiles
        .iter()
        .position(|table| table["name"].as_str() == Some(name))
        .ok_or_else(|| invalid("profile", "not found"))?;
    profiles.remove(index);
    if profiles.is_empty() {
        document.remove("profiles");
    }
    finish_edit(document)
}
/// Appends a new task table to one profile without reserializing other tasks.
pub fn create_task_bytes(bytes: &[u8], profile_name: &str, task: &Task) -> Result<Vec<u8>, Error> {
    let mut document = editable_document(bytes)?;
    let profile = find_profile_mut(&mut document, profile_name)?;
    let tasks = tasks_mut_or_create(profile);
    tasks.push(task_table(task));
    finish_edit(document)
}
/// Updates one task's desktop-entry boundary while retaining its name.
pub fn update_task_bytes(
    bytes: &[u8],
    profile_name: &str,
    name: &str,
    desktop_entries: &[String],
) -> Result<Vec<u8>, Error> {
    let mut document = editable_document(bytes)?;
    let profile = find_profile_mut(&mut document, profile_name)?;
    let tasks = tasks_mut(profile)?;
    let task = tasks
        .iter_mut()
        .find(|table| table["name"].as_str() == Some(name))
        .ok_or_else(|| invalid("task", "not found"))?;
    task["desktop_entries"] = value(entries_array(desktop_entries));
    finish_edit(document)
}
/// Removes one task table and omits the task collection when it becomes empty.
pub fn delete_task_bytes(bytes: &[u8], profile_name: &str, name: &str) -> Result<Vec<u8>, Error> {
    let mut document = editable_document(bytes)?;
    let profile = find_profile_mut(&mut document, profile_name)?;
    let tasks = tasks_mut(profile)?;
    let index = tasks
        .iter()
        .position(|table| table["name"].as_str() == Some(name))
        .ok_or_else(|| invalid("task", "not found"))?;
    tasks.remove(index);
    if tasks.is_empty() {
        profile.remove("tasks");
    }
    finish_edit(document)
}

fn editable_document(bytes: &[u8]) -> Result<DocumentMut, Error> {
    validate_bytes(bytes)?;
    let text = std::str::from_utf8(bytes)
        .map_err(|_| invalid("document", "configuration is not UTF-8"))?;
    text.parse()
        .map_err(|_| invalid("document", "invalid TOML"))
}
fn finish_edit(document: DocumentMut) -> Result<Vec<u8>, Error> {
    let candidate = document.to_string().into_bytes();
    validate_bytes(&candidate)?;
    Ok(candidate)
}
fn profiles_mut(document: &mut DocumentMut) -> Result<&mut ArrayOfTables, Error> {
    document["profiles"]
        .as_array_of_tables_mut()
        .ok_or_else(|| invalid("profiles", "must be array of tables"))
}
fn profiles_mut_or_create(document: &mut DocumentMut) -> &mut ArrayOfTables {
    if document.get("profiles").is_none() {
        document["profiles"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    document["profiles"]
        .as_array_of_tables_mut()
        .expect("validated configuration has profile array-of-tables")
}
fn find_profile_mut<'a>(document: &'a mut DocumentMut, name: &str) -> Result<&'a mut Table, Error> {
    profiles_mut(document)?
        .iter_mut()
        .find(|table| table["name"].as_str() == Some(name))
        .ok_or_else(|| invalid("profile", "not found"))
}
fn tasks_mut(profile: &mut Table) -> Result<&mut ArrayOfTables, Error> {
    profile["tasks"]
        .as_array_of_tables_mut()
        .ok_or_else(|| invalid("task", "not found"))
}
fn tasks_mut_or_create(profile: &mut Table) -> &mut ArrayOfTables {
    if profile.get("tasks").is_none() {
        profile["tasks"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    profile["tasks"]
        .as_array_of_tables_mut()
        .expect("validated profile task collection uses array-of-tables")
}
fn profile_table(profile: &Profile) -> Table {
    let mut table = Table::new();
    table["name"] = value(&profile.name);
    table["root"] = value(&profile.root);
    table["desktop_entries"] = value(entries_array(&profile.desktop_entries));
    table
}
fn task_table(task: &Task) -> Table {
    let mut table = Table::new();
    table["name"] = value(&task.name);
    table["desktop_entries"] = value(entries_array(&task.desktop_entries));
    table
}
fn entries_array(entries: &[String]) -> Array {
    let mut array = Array::new();
    for entry in entries {
        array.push(entry.as_str());
    }
    array
}
/// Atomically replaces the accepted file after two on-disk divergence checks.
pub fn commit_replace(
    stored: &StoredConfiguration,
    candidate: &[u8],
) -> Result<StoredConfiguration, io::Error> {
    let _lock = commit_lock(&stored.path)?;
    ensure_unchanged(stored)?;
    validate_bytes(candidate).map_err(invalid_data)?;
    let parent = stored.path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "configuration has no parent")
    })?;
    let temporary = parent.join(format!(".config.toml.{}.tmp", uuid::Uuid::new_v4()));
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        file.write_all(candidate)?;
        file.sync_all()?;
    }
    if let Err(error) = ensure_unchanged(stored) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, &stored.path)?;
    #[cfg(unix)]
    {
        fs::File::open(parent)?.sync_all()?;
    }
    load(&stored.path)
}
#[cfg(unix)]
fn read_regular(path: &Path) -> Result<(Vec<u8>, fs::Metadata), io::Error> {
    let descriptor = open(
        path,
        OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW,
        Mode::empty(),
    )
    .map_err(nix_error)?;
    let mut file = fs::File::from(descriptor);
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "configuration file must be a regular non-symlink file",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len().try_into().unwrap_or(0));
    file.read_to_end(&mut bytes)?;
    Ok((bytes, metadata))
}
#[cfg(not(unix))]
fn read_regular(path: &Path) -> Result<(Vec<u8>, fs::Metadata), io::Error> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "configuration file must be a regular file",
        ));
    }
    Ok((fs::read(path)?, metadata))
}
#[cfg(unix)]
fn commit_lock(path: &Path) -> Result<Flock<fs::File>, io::Error> {
    let mut lock_path = path.as_os_str().to_os_string();
    lock_path.push(".lock");
    let descriptor = open(
        Path::new(&lock_path),
        OFlag::O_RDWR | OFlag::O_CREAT | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW,
        Mode::from_bits_truncate(0o600),
    )
    .map_err(nix_error)?;
    Flock::lock(fs::File::from(descriptor), FlockArg::LockExclusive)
        .map_err(|(_, error)| nix_error(error))
}
#[cfg(not(unix))]
fn commit_lock(_path: &Path) -> Result<(), io::Error> {
    Ok(())
}
#[cfg(unix)]
fn nix_error(error: nix::errno::Errno) -> io::Error {
    io::Error::from_raw_os_error(error as i32)
}
fn configuration_changed() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "conflict.configuration_changed_on_disk",
    )
}
#[cfg(unix)]
fn identity(metadata: &fs::Metadata) -> FileIdentity {
    use std::os::unix::fs::MetadataExt;
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
    }
}
#[cfg(not(unix))]
fn identity(metadata: &fs::Metadata) -> FileIdentity {
    FileIdentity {
        device: 0,
        inode: 0,
        length: metadata.len(),
    }
}
fn invalid_data(error: Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}"))
}

fn parse_profile((index, value): (usize, &Value)) -> Result<Profile, Error> {
    let path = format!("profiles[{index}]");
    let t = value
        .as_table()
        .ok_or_else(|| invalid(&path, "must be a table"))?;
    reject_unknown(t, &["name", "root", "desktop_entries", "tasks"], &path)?;
    let tasks = match t.get("tasks") {
        None => vec![],
        Some(Value::Array(v)) => v
            .iter()
            .enumerate()
            .map(|(i, v)| parse_task(index, i, v))
            .collect::<Result<_, _>>()?,
        _ => return Err(invalid(format!("{path}.tasks"), "must be an array")),
    };
    Ok(Profile {
        name: string(t, "name", &path)?,
        root: string(t, "root", &path)?,
        desktop_entries: entries(t, "desktop_entries", &path)?,
        tasks,
    })
}
fn parse_task(profile: usize, index: usize, value: &Value) -> Result<Task, Error> {
    let path = format!("profiles[{profile}].tasks[{index}]");
    let t = value
        .as_table()
        .ok_or_else(|| invalid(&path, "must be a table"))?;
    reject_unknown(t, &["name", "desktop_entries"], &path)?;
    Ok(Task {
        name: string(t, "name", &path)?,
        desktop_entries: entries(t, "desktop_entries", &path)?,
    })
}
fn reject_unknown(
    t: &toml::map::Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), Error> {
    if let Some(key) = t.keys().find(|k| !allowed.contains(&k.as_str())) {
        Err(invalid(format!("{path}.{key}"), "unknown field"))
    } else {
        Ok(())
    }
}
fn string(t: &toml::map::Map<String, Value>, key: &str, path: &str) -> Result<String, Error> {
    t.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{path}.{key}"), "required string"))
}
fn entries(t: &toml::map::Map<String, Value>, key: &str, path: &str) -> Result<Vec<String>, Error> {
    t.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{path}.{key}"), "required array"))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid(format!("{path}.{key}"), "array item must be string"))
        })
        .collect()
}
fn invalid(path: impl Into<String>, message: &'static str) -> Error {
    Error::Validation {
        path: path.into(),
        message,
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TemporaryXdgRoot;
    use std::fs;

    fn valid_bytes() -> &'static [u8] {
        b"schema_version = 1\n[[profiles]]\nname = 'FDS'\nroot = '/work'\ndesktop_entries = ['editor.desktop']\n"
    }

    #[test]
    fn strict_toml_preserves_exact_etag_and_rejects_unknowns() {
        let bytes=b"schema_version = 1\n# preserved\n[[profiles]]\nname='FDS'\nroot='/work'\ndesktop_entries=['a.desktop']\n";
        let c = validate_bytes(bytes).unwrap();
        assert_eq!(c.bytes, bytes);
        assert_eq!(c.model.profiles[0].name, "FDS");
        assert!(
            matches!(validate_bytes(b"schema_version=1\nextra=1\n"),Err(Error::Validation{path,..})if path=="document.extra")
        );
        assert!(matches!(
            validate_bytes(b"schema_version=1\nprofiles=[{name='FDS',root='/work',desktop_entries=[]}]\n"),
            Err(Error::Validation { path, .. }) if path == "profiles"
        ));
        assert!(matches!(
            validate_bytes(b"schema_version=1\n[[profiles]]\nname='FDS'\nroot='/work'\ndesktop_entries=[]\ntasks=[{name='Edit',desktop_entries=[]}]\n"),
            Err(Error::Validation { path, .. }) if path == "profiles[0].tasks"
        ));
    }

    #[test]
    fn zero_profiles_omits_the_placeholder_array() {
        let empty = validate_bytes(b"schema_version = 1\n").unwrap();
        assert!(empty.model.profiles.is_empty());
        assert!(matches!(
            validate_bytes(b"schema_version = 1\nprofiles = []\n"),
            Err(Error::Validation { path, .. }) if path == "profiles"
        ));
    }

    #[test]
    fn configuration_byte_limit_has_a_stable_document_path() {
        let bytes = vec![b' '; MAX_CONFIGURATION_BYTES + 1];
        assert!(matches!(
            validate_bytes(&bytes),
            Err(Error::Validation { path, .. }) if path == "document"
        ));
    }

    #[test]
    fn offline_validation_reads_only_the_explicit_test_file() {
        let root = TemporaryXdgRoot::new().unwrap();
        let path = root.contained(Path::new("config/config.toml")).unwrap();
        fs::write(&path, valid_bytes()).unwrap();
        let trap = crate::test_support::TrapEndpoints::default();
        assert_eq!(validate_file(&path).unwrap().model.profiles[0].name, "FDS");
        trap.assert_unreached();
        root.cleanup().unwrap();
    }

    #[test]
    fn lossless_rename_changes_only_the_target_name_boundary() {
        let bytes = b"schema_version = 1\n# keep this exact comment\n[[profiles]]\nname = 'FDS'\nroot = '/work'\ndesktop_entries = ['editor.desktop']\n\n[[profiles.tasks]]\nname = 'Edit'\ndesktop_entries = ['editor.desktop']\n";
        let profile = rename_profile_bytes(bytes, "FDS", "FDS2").unwrap();
        let task = rename_task_bytes(bytes, "FDS", "Edit", "Write").unwrap();
        assert_eq!(
            String::from_utf8(profile)
                .unwrap()
                .replace("name = \"FDS2\"", "name = 'FDS'"),
            String::from_utf8(bytes.to_vec()).unwrap()
        );
        assert_eq!(
            String::from_utf8(task)
                .unwrap()
                .replace("name = \"Write\"", "name = 'Edit'"),
            String::from_utf8(bytes.to_vec()).unwrap()
        );
    }

    #[test]
    fn lossless_profile_and_task_crud_preserves_unrelated_noncanonical_ranges() {
        let bytes = b"schema_version = 1\n# keep this top-level comment\n\n[[profiles]]\n# profile comment\nname = 'FDS'\nroot = '/work'\ndesktop_entries = [ 'editor.desktop' ]\n\n[[profiles.tasks]]\n# task comment\nname = 'Edit'\ndesktop_entries = [ 'editor.desktop' ]\n\n# second profile is unrelated\n[[profiles]]\nname = 'Other'\nroot = '/other'\ndesktop_entries = []\n";
        let unrelated_profile = "# second profile is unrelated\n[[profiles]]\nname = 'Other'\nroot = '/other'\ndesktop_entries = []\n";

        let created_profile = create_profile_bytes(
            bytes,
            &Profile {
                name: "Added".into(),
                root: "/added".into(),
                desktop_entries: vec!["editor.desktop".into()],
                tasks: vec![Task {
                    name: "Open".into(),
                    desktop_entries: vec!["editor.desktop".into()],
                }],
            },
        )
        .unwrap();
        assert!(created_profile.starts_with(bytes));
        let created_model = validate_bytes(&created_profile).unwrap().model;
        assert_eq!(created_model.profiles.len(), 3);
        assert_eq!(created_model.profiles[2].tasks[0].name, "Open");

        let updated_profile =
            update_profile_bytes(bytes, "FDS", "/new-work", &["editor.desktop".into()]).unwrap();
        let updated_profile = String::from_utf8(updated_profile).unwrap();
        assert!(updated_profile.contains(unrelated_profile));
        assert!(updated_profile.contains("# profile comment\nname = 'FDS'\nroot = \"/new-work\""));

        let deleted_profile =
            String::from_utf8(delete_profile_bytes(bytes, "FDS").unwrap()).unwrap();
        assert!(deleted_profile.contains(unrelated_profile));
        assert_eq!(
            validate_bytes(deleted_profile.as_bytes())
                .unwrap()
                .model
                .profiles
                .len(),
            1
        );

        let created_task = create_task_bytes(
            bytes,
            "FDS",
            &Task {
                name: "Review".into(),
                desktop_entries: vec!["editor.desktop".into()],
            },
        )
        .unwrap();
        let created_task = String::from_utf8(created_task).unwrap();
        assert!(created_task.contains(unrelated_profile));
        assert_eq!(
            validate_bytes(created_task.as_bytes())
                .unwrap()
                .model
                .profiles[0]
                .tasks
                .len(),
            2
        );

        let updated_task = update_task_bytes(bytes, "FDS", "Edit", &[]).unwrap();
        let updated_task = String::from_utf8(updated_task).unwrap();
        assert!(updated_task.contains(unrelated_profile));
        assert!(updated_task.contains("# task comment\nname = 'Edit'\ndesktop_entries = []"));

        let deleted_task = delete_task_bytes(bytes, "FDS", "Edit").unwrap();
        let deleted_task = String::from_utf8(deleted_task).unwrap();
        assert!(deleted_task.contains(unrelated_profile));
        let model = validate_bytes(deleted_task.as_bytes()).unwrap().model;
        assert!(model.profiles[0].tasks.is_empty());
    }

    #[test]
    fn accepted_file_identity_and_bytes_detect_external_replacement() {
        let root = TemporaryXdgRoot::new().unwrap();
        let path = root.contained(Path::new("config/config.toml")).unwrap();
        fs::write(&path, valid_bytes()).unwrap();
        let stored = load(&path).unwrap();
        ensure_unchanged(&stored).unwrap();

        fs::write(&path, b"schema_version = 1\n").unwrap();
        assert_eq!(
            ensure_unchanged(&stored).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );

        fs::write(&path, valid_bytes()).unwrap();
        let replacement = root
            .contained(Path::new("config/replacement.toml"))
            .unwrap();
        fs::write(&replacement, valid_bytes()).unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert_eq!(
            ensure_unchanged(&stored).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        root.cleanup().unwrap();
    }

    #[test]
    fn missing_and_unreadable_accepted_files_are_conflicts() {
        let root = TemporaryXdgRoot::new().unwrap();
        let path = root.contained(Path::new("config/config.toml")).unwrap();
        fs::write(&path, valid_bytes()).unwrap();
        let stored = load(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(
            ensure_unchanged(&stored).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );

        fs::write(&path, valid_bytes()).unwrap();
        let unreadable = load(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
            assert_eq!(
                ensure_unchanged(&unreadable).unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        root.cleanup().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn accepted_file_rejects_a_later_symlink_replacement() {
        use std::os::unix::fs::symlink;

        let root = TemporaryXdgRoot::new().unwrap();
        let path = root.contained(Path::new("config/config.toml")).unwrap();
        let target = root.contained(Path::new("config/target.toml")).unwrap();
        fs::write(&path, valid_bytes()).unwrap();
        fs::write(&target, valid_bytes()).unwrap();
        let stored = load(&path).unwrap();
        fs::remove_file(&path).unwrap();
        symlink(&target, &path).unwrap();
        assert_eq!(
            ensure_unchanged(&stored).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        fs::remove_file(&path).unwrap();
        root.cleanup().unwrap();
    }

    #[test]
    fn commit_replace_adopts_a_complete_valid_document_or_keeps_the_previous_one() {
        let root = TemporaryXdgRoot::new().unwrap();
        let path = root.contained(Path::new("config/config.toml")).unwrap();
        fs::write(&path, valid_bytes()).unwrap();
        let stored = load(&path).unwrap();
        let candidate = b"schema_version = 1\n";
        let committed = commit_replace(&stored, candidate).unwrap();
        assert_eq!(committed.accepted.bytes, candidate);
        assert_eq!(fs::read(&path).unwrap(), candidate);

        fs::write(&path, valid_bytes()).unwrap();
        let error = commit_replace(&committed, candidate).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(fs::read(&path).unwrap(), valid_bytes());
        root.cleanup().unwrap();
    }
}

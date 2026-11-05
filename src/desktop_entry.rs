//! Desktop-entry text parsing only. Exec text and field codes are never interpreted.

use crate::error::{Error, Result};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub terminal: bool,
    pub hidden: bool,
    pub no_display: bool,
    pub only_show_in: Option<Vec<String>>,
    pub not_show_in: Vec<String>,
    pub try_exec: Option<String>,
}
const INVALID: Error = Error("application.invalid_desktop_entry");

impl DesktopEntry {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let main = parse_groups(bytes)?;
        let string = |key| {
            main.get(key)
                .map(|v| decode(v, false).map(|mut parts| parts.remove(0)))
                .transpose()
        };
        if string("Type")?.as_deref() != Some("Application") {
            return Err(INVALID);
        }
        if string("Encoding")?.is_some_and(|s| s != "UTF-8") {
            return Err(INVALID);
        }
        let name = string("Name")?.filter(|s| !s.is_empty()).ok_or(INVALID)?;
        let exec = string("Exec")?.filter(|s| !s.is_empty()).ok_or(INVALID)?;
        let icon = string("Icon")?;
        let try_exec = string("TryExec")?;
        if [&name, &exec]
            .into_iter()
            .chain(icon.as_ref())
            .chain(try_exec.as_ref())
            .any(|s| s.len() > 4096)
        {
            return Err(Error("limit.exceeded"));
        }
        let boolean = |key| -> Result<bool> {
            match main.get(key).copied() {
                None | Some("false") => Ok(false),
                Some("true") => Ok(true),
                _ => Err(INVALID),
            }
        };
        // These standardized booleans are validated even when not exposed in v2.
        for key in [
            "Terminal",
            "Hidden",
            "NoDisplay",
            "DBusActivatable",
            "StartupNotify",
            "PrefersNonDefaultGPU",
            "SingleMainWindow",
        ] {
            boolean(key)?;
        }
        if main.contains_key("OnlyShowIn") && main.contains_key("NotShowIn") {
            return Err(INVALID);
        }
        let list = |key| main.get(key).map(|v| decode(v, true)).transpose();
        Ok(Self {
            name,
            exec,
            icon,
            terminal: boolean("Terminal")?,
            hidden: boolean("Hidden")?,
            no_display: boolean("NoDisplay")?,
            only_show_in: list("OnlyShowIn")?,
            not_show_in: list("NotShowIn")?.unwrap_or_default(),
            try_exec,
        })
    }
}
fn parse_groups(bytes: &[u8]) -> Result<BTreeMap<&str, &str>> {
    if bytes.len() > 1024 * 1024 {
        return Err(Error("limit.exceeded"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| INVALID)?;
    if text.starts_with('\u{feff}')
        || text
            .chars()
            .any(|c| c.is_ascii_control() && !['\n', '\r', '\t'].contains(&c))
    {
        return Err(INVALID);
    }
    let mut groups = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut main = BTreeMap::new();
    let mut group = "";
    for line in text.split_inclusive('\n') {
        let line = line
            .strip_suffix('\n')
            .map_or(line, |line| line.strip_suffix('\r').unwrap_or(line));
        if line.contains('\r') {
            return Err(INVALID);
        }
        let line = line.trim_matches([' ', '\t']);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            let name = line
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .ok_or(INVALID)?;
            if name.is_empty()
                || name.contains(['[', ']'])
                || name.chars().any(char::is_control)
                || !groups.insert(name)
            {
                return Err(INVALID);
            }
            group = name;
            keys.clear();
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(INVALID)?;
        let key = key.trim_matches([' ', '\t']);
        let value = value.trim_matches([' ', '\t']);
        if group.is_empty() || !valid_key(key) || !keys.insert(key) {
            return Err(INVALID);
        }
        // Validate escapes even for ignored/localized/vendor keys and action groups.
        let list = is_list_key(key);
        decode(value, list)?;
        if group == "Desktop Entry" {
            main.insert(key, value);
        }
    }
    if !groups.contains("Desktop Entry") {
        return Err(INVALID);
    }
    Ok(main)
}
fn valid_key(key: &str) -> bool {
    let (base, locale) = key
        .split_once('[')
        .map_or((key, None), |(b, l)| (b, Some(l)));
    !base.is_empty()
        && base.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && locale.is_none_or(|l| {
            l.ends_with(']')
                && l.len() > 1
                && l[..l.len() - 1]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_@.-".contains(&b))
        })
}
fn is_list_key(key: &str) -> bool {
    [
        "OnlyShowIn",
        "NotShowIn",
        "Categories",
        "MimeType",
        "Actions",
        "Keywords",
        "Implements",
    ]
    .contains(&key.split('[').next().unwrap_or(key))
}
fn decode(value: &str, list: bool) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut part = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => part.push(match chars.next().ok_or(INVALID)? {
                's' => ' ',
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '\\' => '\\',
                ';' if list => ';',
                _ => return Err(INVALID),
            }),
            ';' if list => {
                if !part.is_empty() {
                    parts.push(std::mem::take(&mut part));
                }
            }
            c => part.push(c),
        }
    }
    if !list || !part.is_empty() {
        parts.push(part);
    }
    Ok(parts)
}

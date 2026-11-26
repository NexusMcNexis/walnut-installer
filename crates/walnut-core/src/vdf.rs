//! Steam's binary `shortcuts.vdf` (the non-Steam games in a user's library).
//!
//! The file is a map named `shortcuts` whose entries are keyed `"0"`, `"1"`,
//! ... Each value starts with a type byte: `0x00` a nested map, `0x01` a
//! string, `0x02` a little-endian 32-bit int; `0x08` ends a map. Strings are
//! NUL-terminated UTF-8.
//!
//! Every entry keeps its own raw bytes, so entries that are not ours are
//! written back exactly as they were read. Ours is found by its executable
//! path, refreshed in place (never duplicated) and removed alone.

use std::collections::BTreeMap;

const MAP: u8 = 0x00;
const TEXT: u8 = 0x01;
const INT: u8 = 0x02;
const END: u8 = 0x08;

/// A value inside an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Text(String),
    Int(i32),
    Map(BTreeMap<String, Value>),
}

/// One shortcut: its key in the file, its raw bytes and its parsed fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub raw: Vec<u8>,
    pub fields: BTreeMap<String, Value>,
}

impl Entry {
    pub fn text(&self, name: &str) -> Option<&str> {
        // Steam has written both "AppName" and "appname" over the years.
        let found = self
            .fields
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value);
        match found {
            Some(Value::Text(text)) => Some(text.as_str()),
            _ => None,
        }
    }

    pub fn int(&self, name: &str) -> Option<i32> {
        match self.fields.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)) {
            Some((_, Value::Int(number))) => Some(*number),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct VdfError(pub String);

impl std::fmt::Display for VdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shortcuts.vdf: {}", self.0)
    }
}

impl std::error::Error for VdfError {}

/// What a shortcut of ours holds.
#[derive(Debug, Clone)]
pub struct Shortcut {
    /// The name shown in the library (never with a version).
    pub name: String,
    /// The executable, quoted as Steam keeps it: `"\"/path/to/game\""`.
    pub exe: String,
    /// The folder it starts in, quoted the same way.
    pub start_dir: String,
    pub icon: String,
    pub launch_options: String,
    pub tags: Vec<String>,
}

/// Steam's id for a non-Steam shortcut, which also names its grid artwork:
/// CRC-32 of the quoted executable followed by the name, top bit set. It is
/// written into the entry's `appid` field (as a signed 32-bit int), and the
/// grid files use the same number, so the art always matches the entry.
pub fn app_id(exe: &str, name: &str) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(exe.as_bytes());
    hasher.update(name.as_bytes());
    hasher.finalize() | 0x8000_0000
}

/// Parses a shortcuts.vdf. Empty input is a file not made yet: no entries.
pub fn parse(data: &[u8]) -> Result<Vec<Entry>, VdfError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    let mut at = 0usize;
    if byte(data, &mut at)? != MAP || !text(data, &mut at)?.eq_ignore_ascii_case("shortcuts") {
        return Err(VdfError("not a shortcuts file".into()));
    }
    let mut entries = Vec::new();
    loop {
        let start = at;
        let kind = byte(data, &mut at)?;
        if kind == END {
            return Ok(entries);
        }
        if kind != MAP {
            return Err(VdfError(format!("unexpected type {kind:#04x} at {start}")));
        }
        let key = text(data, &mut at)?;
        let fields = map(data, &mut at)?;
        entries.push(Entry { key, raw: data[start..at].to_vec(), fields });
    }
}

/// The file for `entries`, each written from its own bytes.
pub fn build(entries: &[Entry]) -> Vec<u8> {
    let mut out = vec![MAP];
    out.extend_from_slice(b"shortcuts\0");
    for entry in entries {
        out.extend_from_slice(&entry.raw);
    }
    out.push(END);
    out.push(END);
    out
}

/// `data` with our shortcut added, or refreshed in place when an entry with
/// the same executable (or the same name) is there already. Every other
/// entry is unchanged, byte for byte.
pub fn with_ours(data: &[u8], ours: &Shortcut) -> Result<Vec<u8>, VdfError> {
    let mut entries = parse(data)?;
    let mine = entries
        .iter()
        .position(|entry| entry.text("Exe") == Some(ours.exe.as_str()) || entry.text("AppName") == Some(ours.name.as_str()));
    let key = match mine {
        Some(index) => entries[index].key.clone(),
        None => (entries.iter().filter_map(|entry| entry.key.parse::<i64>().ok()).max().unwrap_or(-1) + 1).to_string(),
    };
    let raw = entry_bytes(&key, ours);
    let mut at = 0usize;
    byte(&raw, &mut at)?;
    text(&raw, &mut at)?;
    let fields = map(&raw, &mut at)?;
    let entry = Entry { key, raw, fields };
    match mine {
        Some(index) => entries[index] = entry,
        None => entries.push(entry),
    }
    Ok(build(&entries))
}

/// `data` without the entry whose executable is `exe`.
pub fn without_ours(data: &[u8], exe: &str) -> Result<Vec<u8>, VdfError> {
    let entries: Vec<Entry> = parse(data)?.into_iter().filter(|entry| entry.text("Exe") != Some(exe)).collect();
    Ok(build(&entries))
}

/// One entry's bytes, with the fields Steam writes for a shortcut.
pub fn entry_bytes(key: &str, ours: &Shortcut) -> Vec<u8> {
    let mut body = Vec::new();
    put_int(&mut body, "appid", app_id(&ours.exe, &ours.name) as i32);
    put_text(&mut body, "AppName", &ours.name);
    put_text(&mut body, "Exe", &ours.exe);
    put_text(&mut body, "StartDir", &ours.start_dir);
    put_text(&mut body, "icon", &ours.icon);
    put_text(&mut body, "ShortcutPath", "");
    put_text(&mut body, "LaunchOptions", &ours.launch_options);
    put_int(&mut body, "IsHidden", 0);
    put_int(&mut body, "AllowDesktopConfig", 1);
    put_int(&mut body, "AllowOverlay", 1);
    put_int(&mut body, "OpenVR", 0);
    put_int(&mut body, "Devkit", 0);
    put_text(&mut body, "DevkitGameID", "");
    put_int(&mut body, "DevkitOverrideAppID", 0);
    put_int(&mut body, "LastPlayTime", 0);
    put_text(&mut body, "FlatpakAppID", "");
    body.push(MAP);
    body.extend_from_slice(b"tags\0");
    for (index, tag) in ours.tags.iter().enumerate() {
        put_text(&mut body, &index.to_string(), tag);
    }
    body.push(END);
    let mut out = vec![MAP];
    out.extend_from_slice(key.as_bytes());
    out.push(0);
    out.extend_from_slice(&body);
    out.push(END);
    out
}

fn put_text(out: &mut Vec<u8>, key: &str, value: &str) {
    out.push(TEXT);
    out.extend_from_slice(key.as_bytes());
    out.push(0);
    out.extend_from_slice(value.as_bytes());
    out.push(0);
}

fn put_int(out: &mut Vec<u8>, key: &str, value: i32) {
    out.push(INT);
    out.extend_from_slice(key.as_bytes());
    out.push(0);
    out.extend_from_slice(&value.to_le_bytes());
}

fn map(data: &[u8], at: &mut usize) -> Result<BTreeMap<String, Value>, VdfError> {
    let mut fields = BTreeMap::new();
    loop {
        let kind = byte(data, at)?;
        if kind == END {
            return Ok(fields);
        }
        let key = text(data, at)?;
        let value = match kind {
            MAP => Value::Map(map(data, at)?),
            TEXT => Value::Text(text(data, at)?),
            INT => {
                let bytes = data.get(*at..*at + 4).ok_or_else(|| VdfError("an int runs off the end".into()))?;
                *at += 4;
                Value::Int(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            }
            other => return Err(VdfError(format!("unknown type {other:#04x}"))),
        };
        fields.insert(key, value);
    }
}

fn byte(data: &[u8], at: &mut usize) -> Result<u8, VdfError> {
    let value = *data.get(*at).ok_or_else(|| VdfError("ends too soon".into()))?;
    *at += 1;
    Ok(value)
}

fn text(data: &[u8], at: &mut usize) -> Result<String, VdfError> {
    let end = data[*at..].iter().position(|b| *b == 0).ok_or_else(|| VdfError("a string runs off the end".into()))? + *at;
    let value = String::from_utf8(data[*at..end].to_vec()).map_err(|_| VdfError("a string is not UTF-8".into()))?;
    *at = end + 1;
    Ok(value)
}

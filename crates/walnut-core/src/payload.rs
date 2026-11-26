//! The installer file: the installer program, then the payload (a
//! zstd-compressed tar of the game under `game/` and Walnut's own files
//! under `.walnut/`), then the manifest (the project as TOML), then a
//! 64-byte footer:
//!
//! ```text
//! magic "WALNUT01" | payload length u64 LE | manifest length u64 LE |
//! SHA-256 of the payload and the manifest (32 bytes) | reserved (8 zero bytes)
//! ```
//!
//! The SHA-256 covers the payload and the manifest together, and is checked
//! before either is used; a file that doesn't match is refused.

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

pub const MAGIC: &[u8; 8] = b"WALNUT01";
pub const FOOTER_LEN: u64 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footer {
    pub payload_len: u64,
    pub manifest_len: u64,
    pub sha256: [u8; 32],
}

impl Footer {
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut out = [0u8; 64];
        out[0..8].copy_from_slice(MAGIC);
        out[8..16].copy_from_slice(&self.payload_len.to_le_bytes());
        out[16..24].copy_from_slice(&self.manifest_len.to_le_bytes());
        out[24..56].copy_from_slice(&self.sha256);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Footer> {
        if bytes.len() != 64 || &bytes[0..8] != MAGIC {
            return None;
        }
        let mut sha256 = [0u8; 32];
        sha256.copy_from_slice(&bytes[24..56]);
        Some(Footer {
            payload_len: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
            manifest_len: u64::from_le_bytes(bytes[16..24].try_into().ok()?),
            sha256,
        })
    }
}

/// Where the parts of an installer file are.
#[derive(Debug, Clone)]
pub struct Layout {
    pub program_len: u64,
    pub payload_at: u64,
    pub manifest_at: u64,
    pub footer: Footer,
}

/// A tar of `files` (archive path -> file on disk), zstd-compressed.
pub fn pack(files: &[(String, PathBuf)]) -> io::Result<Vec<u8>> {
    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        builder.mode(tar::HeaderMode::Deterministic);
        for (name, path) in files {
            let mut file = File::open(path)?;
            builder.append_file(name, &mut file)?;
        }
        builder.finish()?;
    }
    Ok(ruzstd::encoding::compress_to_vec(&tar_bytes[..], ruzstd::encoding::CompressionLevel::Fastest))
}

/// Writes an installer: `program`, then `payload`, the manifest and the footer.
pub fn write_installer(out: &Path, program: &[u8], payload: &[u8], manifest: &str) -> io::Result<()> {
    let footer = Footer {
        payload_len: payload.len() as u64,
        manifest_len: manifest.len() as u64,
        sha256: {
            let mut hasher = Sha256::new();
            hasher.update(payload);
            hasher.update(manifest.as_bytes());
            hasher.finalize().into()
        },
    };
    let mut file = File::create(out)?;
    file.write_all(program)?;
    file.write_all(payload)?;
    file.write_all(manifest.as_bytes())?;
    file.write_all(&footer.to_bytes())?;
    file.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(out, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Finds the parts of the installer file at `path`.
pub fn layout(path: &Path) -> io::Result<Layout> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if size < FOOTER_LEN {
        return Err(invalid("this file carries no game"));
    }
    file.seek(SeekFrom::Start(size - FOOTER_LEN))?;
    let mut bytes = [0u8; 64];
    file.read_exact(&mut bytes)?;
    let footer = Footer::from_bytes(&bytes).ok_or_else(|| invalid("this file carries no game"))?;
    let tail = footer.payload_len.checked_add(footer.manifest_len).and_then(|t| t.checked_add(FOOTER_LEN)).ok_or_else(|| invalid("damaged footer"))?;
    if tail > size {
        return Err(invalid("damaged footer"));
    }
    let program_len = size - tail;
    Ok(Layout { program_len, payload_at: program_len, manifest_at: program_len + footer.payload_len, footer })
}

pub fn read_manifest(path: &Path, layout: &Layout) -> io::Result<String> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(layout.manifest_at))?;
    let mut text = String::new();
    file.take(layout.footer.manifest_len).read_to_string(&mut text)?;
    Ok(text)
}

/// Checks the SHA-256 of the payload and the manifest against the footer.
/// Nothing is read from, or unpacked out of, a file that fails this.
pub fn verify(path: &Path, layout: &Layout) -> io::Result<()> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(layout.payload_at))?;
    let mut hasher = Sha256::new();
    io::copy(&mut file.take(layout.footer.payload_len + layout.footer.manifest_len), &mut hasher)?;
    let digest: [u8; 32] = hasher.finalize().into();
    if digest != layout.footer.sha256 {
        return Err(invalid("the game inside this installer is damaged (checksum mismatch); download it again"));
    }
    Ok(())
}

/// The installer program alone (for the uninstaller).
pub fn program_bytes(path: &Path, layout: &Layout) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut out = Vec::with_capacity(layout.program_len as usize);
    file.take(layout.program_len).read_to_end(&mut out)?;
    Ok(out)
}

/// Unpacks the (already verified) payload into `dest`, saying progress as
/// (bytes done, bytes total) and stopping when `keep_going` says no. Every
/// file and folder it made is returned; on any failure or stop they are all
/// removed again before the error is returned.
pub fn unpack(
    path: &Path,
    layout: &Layout,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
    keep_going: &dyn Fn() -> bool,
) -> io::Result<Vec<PathBuf>> {
    let mut made: Vec<PathBuf> = Vec::new();
    let result = unpack_into(path, layout, dest, progress, keep_going, &mut made);
    if let Err(error) = result {
        rollback(&made);
        return Err(error);
    }
    Ok(made)
}

fn unpack_into(
    path: &Path,
    layout: &Layout,
    dest: &Path,
    progress: &mut dyn FnMut(u64, u64),
    keep_going: &dyn Fn() -> bool,
    made: &mut Vec<PathBuf>,
) -> io::Result<()> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(layout.payload_at))?;
    // Progress is the share of the compressed payload read so far.
    let read = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let compressed = Counting { inner: file.take(layout.footer.payload_len), read: read.clone() };
    let decoder = ruzstd::decoding::StreamingDecoder::new(compressed).map_err(|e| invalid(&format!("payload: {e}")))?;
    let mut archive = tar::Archive::new(decoder);
    let total = layout.footer.payload_len.max(1);
    if !dest.exists() {
        create_dirs(dest, made)?;
    }
    for entry in archive.entries()? {
        if !keep_going() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
        }
        let mut entry = entry?;
        let relative = safe_relative(&entry.path()?)?;
        let target = dest.join(&relative);
        if let Some(parent) = target.parent() {
            create_dirs(parent, made)?;
        }
        let mode = entry.header().mode().unwrap_or(0o644);
        let mut out = File::create(&target)?;
        made.push(target.clone());
        io::copy(&mut entry, &mut out)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))?;
        }
        progress(read.get().min(total), total);
    }
    progress(total, total);
    Ok(())
}

/// A reader that counts the bytes read through it.
struct Counting<R> {
    inner: R,
    read: std::rc::Rc<std::cell::Cell<u64>>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.set(self.read.get() + n as u64);
        Ok(n)
    }
}

/// Removes what an unpack made, newest first (files, then their folders).
pub fn rollback(made: &[PathBuf]) {
    for path in made.iter().rev() {
        if path.is_dir() {
            let _ = fs::remove_dir(path);
        } else {
            let _ = fs::remove_file(path);
        }
    }
}

fn create_dirs(dir: &Path, made: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut missing = Vec::new();
    let mut at = Some(dir);
    while let Some(path) = at {
        if path.exists() {
            break;
        }
        missing.push(path.to_path_buf());
        at = path.parent();
    }
    for path in missing.into_iter().rev() {
        fs::create_dir(&path)?;
        made.push(path);
    }
    Ok(())
}

/// A tar path that stays inside the install folder (no `..`, no root).
fn safe_relative(path: &Path) -> io::Result<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(name) => out.push(name),
            Component::CurDir => {}
            _ => return Err(invalid("the payload holds a path outside its folder")),
        }
    }
    if out.as_os_str().is_empty() {
        return Err(invalid("the payload holds an empty path"));
    }
    Ok(out)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

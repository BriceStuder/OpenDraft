//! The files of a library folder (specification 10.3) and how they are
//! written: to a temporary file in the same folder, then renamed over the old
//! one, so a crash leaves the old file or the new one and never half of one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::error::LibraryError;
use super::state::State;
use crate::blocks_payload::{BlocksPackage, KIND, KIND_VERSION};
use crate::envelope::header::RawHeader;
use crate::envelope::{Envelope, Header, Licence};

pub const PERSONAL: &str = "personal.odpack";
pub const BACKUP: &str = "personal.bak";
pub const DAMAGED: &str = "personal.damaged";
pub const STATE: &str = "library.json";
pub const STATE_BAD: &str = "library.json.bad";
pub const TRASH: &str = "trash";
pub const PERSONAL_NAME: &str = "local/personal";

pub fn personal(root: &Path) -> PathBuf {
    root.join(PERSONAL)
}

pub fn trash_file(root: &Path, id: u64) -> PathBuf {
    root.join(TRASH).join(format!("{id}.odpack"))
}

/// Writes `bytes` to `path` atomically.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let temp = path.with_file_name(format!("{}.tmp", name.unwrap_or_default()));
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// The header of the personal package (and of the small packages the Trash
/// keeps). `revision` is the patch number: it rises with every write.
pub fn personal_header(revision: u32) -> Result<Header, LibraryError> {
    let raw = RawHeader {
        name: PERSONAL_NAME.into(),
        version: format!("0.0.{revision}"),
        title: "My blocks".into(),
        summary: "Blocks kept in your own library".into(),
        description: String::new(),
        kind: KIND.into(),
        kind_version: KIND_VERSION,
        license: Licence::Personal.spdx().into(),
        authors: Vec::new(),
        keywords: Vec::new(),
        repository: None,
        copyright: None,
        thumbnail: None,
    };
    Header::check(&raw).map_err(|e| LibraryError::Package(e.to_string()))
}

pub fn encode(package: &BlocksPackage, revision: u32) -> Result<Vec<u8>, LibraryError> {
    let envelope = package.to_envelope(personal_header(revision)?)?;
    Ok(envelope.to_bytes()?)
}

/// A personal package read from `path`: `None` when there is no such file,
/// else the package and its revision.
pub fn read_package(path: &Path) -> Result<Option<(BlocksPackage, u32)>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let envelope = Envelope::read(&bytes).map_err(|e| e.to_string())?;
    if !envelope.header.name.is_personal() {
        return Err("is not the personal package".into());
    }
    let package = BlocksPackage::from_envelope(&envelope).map_err(|e| e.to_string())?;
    Ok(Some((package, envelope.header.version.patch)))
}

/// Replaces the personal package, keeping the one before it as the backup.
pub fn save_package(
    root: &Path,
    package: &BlocksPackage,
    revision: u32,
) -> Result<(), LibraryError> {
    let bytes = encode(package, revision)?;
    let path = personal(root);
    if path.exists() {
        fs::copy(&path, root.join(BACKUP))?;
    }
    write_atomic(&path, &bytes)?;
    Ok(())
}

pub fn save_state(root: &Path, state: &State) -> Result<(), LibraryError> {
    let mut bytes =
        serde_json::to_vec_pretty(state).map_err(|e| LibraryError::Io(e.to_string()))?;
    bytes.push(b'\n');
    write_atomic(&root.join(STATE), &bytes)?;
    Ok(())
}

/// Writes a small package for the Trash.
pub fn save_trash_file(
    root: &Path,
    id: u64,
    package: &BlocksPackage,
) -> Result<PathBuf, LibraryError> {
    let bytes = encode(package, 0)?;
    let path = trash_file(root, id);
    write_atomic(&path, &bytes)?;
    Ok(path)
}

pub fn read_trash_file(root: &Path, id: u64) -> Result<BlocksPackage, LibraryError> {
    match read_package(&trash_file(root, id)) {
        Ok(Some((package, _))) => Ok(package),
        Ok(None) => Err(LibraryError::NotFound),
        Err(m) => Err(LibraryError::Corrupt(format!("the Trash item {id} {m}"))),
    }
}

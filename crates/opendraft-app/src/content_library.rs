//! The library the commands and the Content Manager window share: one
//! `opendraft_content::library::Library`, opened on first use in the folder of
//! `Location::Library` and closed again when that folder changes.

use std::path::PathBuf;
use std::sync::Mutex;

use opendraft_content::library::{Library, LibraryError};

#[derive(Default)]
struct Inner {
    folder: Option<PathBuf>,
    library: Option<Library>,
    /// What opening said that the user has not been told yet.
    notices: Vec<String>,
}

#[derive(Default)]
pub struct LibraryHandle {
    inner: Mutex<Inner>,
}

impl LibraryHandle {
    /// Use the library in `folder` from now on (`None`: there is none). The
    /// library in memory is let go when the folder is not the one it is in.
    pub fn set_folder(&self, folder: Option<PathBuf>) {
        let mut inner = self.lock();
        if inner.folder != folder {
            inner.library = None;
            inner.folder = folder;
        }
    }

    /// Runs `f` on the library, opening it first if need be. An error means it
    /// could not be opened (no folder, or damaged files that were left alone).
    pub fn with<R>(&self, f: impl FnOnce(&mut Library) -> R) -> Result<R, LibraryError> {
        let mut inner = self.lock();
        if inner.library.is_none() {
            let folder = inner
                .folder
                .clone()
                .ok_or_else(|| LibraryError::Io("there is no library folder".into()))?;
            let library = Library::open(&folder)?;
            inner.notices.extend(library.notices().iter().cloned());
            inner.library = Some(library);
        }
        match inner.library.as_mut() {
            Some(library) => Ok(f(library)),
            None => Err(LibraryError::NotFound),
        }
    }

    /// What opening the library said, once: say it to the user.
    pub fn take_notices(&self) -> Vec<String> {
        std::mem::take(&mut self.lock().notices)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl crate::state::AppState {
    /// Use the folders `settings` names (or their defaults) for templates and
    /// the block library.
    pub(crate) fn use_folders_of(&mut self, settings: &crate::settings::Settings) {
        use crate::locations::Location;
        self.templates_dir = Location::Templates.resolve(settings);
        self.defaults
            .library
            .set_folder(Location::Library.resolve(settings));
    }
}

/// Seconds since 1970, for the Trash.
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_on_first_use_and_follows_the_folder() {
        let handle = LibraryHandle::default();
        assert!(handle.with(|_| ()).is_err(), "no folder yet");
        let dir = tempfile::tempdir().expect("temp dir");
        handle.set_folder(Some(dir.path().join("a")));
        assert_eq!(handle.with(|l| l.blocks().len()).ok(), Some(0));
        assert!(dir.path().join("a").join("trash").is_dir());
        handle.set_folder(Some(dir.path().join("b")));
        assert_eq!(handle.with(|l| l.blocks().len()).ok(), Some(0));
        assert!(dir.path().join("b").is_dir());
        assert!(handle.take_notices().is_empty());
    }
}

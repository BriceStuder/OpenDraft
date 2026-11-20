//! Showing a file or folder in the system's file manager, or opening a file
//! in the program the system uses for it.

use std::path::Path;

/// Open `folder` in the system's file manager.
pub fn open_folder(folder: &Path) -> std::io::Result<()> {
    open_as_double_clicked(folder)
}

/// Open the file at `path` in the program the system uses for its kind (a PDF
/// viewer, a browser for SVG...).
pub fn open_file(path: &Path) -> std::io::Result<()> {
    open_as_double_clicked(path)
}

/// What a double-click on `path` does: a folder opens in the file manager,
/// a file in its default program.
fn open_as_double_clicked(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(not(windows))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// Reveal `path` in the system's file manager.
pub fn show_in_folder(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let child = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(not(windows))]
    let child = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
    child.map(|_| ())
}

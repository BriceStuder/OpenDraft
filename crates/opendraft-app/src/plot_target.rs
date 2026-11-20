//! Where the Plot dialog's plot goes: OpenDraft's own PDF and SVG devices,
//! or one of the machine's printers.

use opendraft_core::plot_settings::{DEVICE_PDF, DEVICE_SVG};

use crate::print_backend::PrinterId;

/// Where the plot goes: one of OpenDraft's own file devices, or a printer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A PDF file, asked for when Plot is pressed.
    Pdf,
    /// An SVG file, asked for when Plot is pressed.
    Svg,
    Printer(PrinterId),
}

impl Target {
    /// The device as a page setup names it.
    pub fn device_name(&self) -> String {
        match self {
            Self::Pdf => DEVICE_PDF.to_owned(),
            Self::Svg => DEVICE_SVG.to_owned(),
            Self::Printer(printer) => printer.0.clone(),
        }
    }

    /// OpenDraft's own devices by name; a printer is found among the
    /// machine's once they are known.
    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            DEVICE_PDF => Some(Self::Pdf),
            DEVICE_SVG => Some(Self::Svg),
            _ => None,
        }
    }
}

/// The device to start on: the layout's own, else the one used last. A
/// printer is only named (to be found once the printers are known).
pub fn initial_device(layout_device: &str, last: &str) -> (Target, Option<String>) {
    for name in [layout_device, last] {
        if let Some(target) = Target::builtin(name) {
            return (target, None);
        }
        // Not a device: what a layout holds when none was chosen, or the
        // name an earlier version gave to both file devices.
        if !name.is_empty() && name != "none_device" && name != "OpenDraft PDF/SVG" {
            return (Target::Pdf, Some(name.to_owned()));
        }
    }
    (Target::Pdf, None)
}

//! How colours and by-layer attributes are written: `ByLayer`, `ByBlock` or a
//! value.

use opendraft_core::{Attr, Color};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ColorDto {
    Foreground,
    Rgb([u8; 3]),
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(super) enum AttrDto<T> {
    #[default]
    ByLayer,
    ByBlock,
    Set(T),
}

impl From<Color> for ColorDto {
    fn from(color: Color) -> Self {
        match color {
            Color::Foreground => ColorDto::Foreground,
            Color::Rgb(r, g, b) => ColorDto::Rgb([r, g, b]),
        }
    }
}

impl From<ColorDto> for Color {
    fn from(color: ColorDto) -> Self {
        match color {
            ColorDto::Foreground => Color::Foreground,
            ColorDto::Rgb([r, g, b]) => Color::Rgb(r, g, b),
        }
    }
}

pub(super) fn attr_to_dto<T, U>(attr: &Attr<T>, convert: impl Fn(&T) -> U) -> AttrDto<U> {
    match attr {
        Attr::ByLayer => AttrDto::ByLayer,
        Attr::ByBlock => AttrDto::ByBlock,
        Attr::Set(value) => AttrDto::Set(convert(value)),
    }
}

pub(super) fn attr_from_dto<T, U>(attr: AttrDto<T>, convert: impl Fn(T) -> U) -> Attr<U> {
    match attr {
        AttrDto::ByLayer => Attr::ByLayer,
        AttrDto::ByBlock => Attr::ByBlock,
        AttrDto::Set(value) => Attr::Set(convert(value)),
    }
}

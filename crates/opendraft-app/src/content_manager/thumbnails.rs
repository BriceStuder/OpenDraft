//! Pictures of blocks, drawn by the canvas's CPU rasteriser and kept as
//! textures until the block, the library or the drawing changes. A few are
//! made each frame so opening a big library does not stall the window.

use std::collections::HashMap;

use eframe::egui;
use opendraft_canvas::preview::Preview;
use opendraft_content::blocks_payload::BlockUuid;
use opendraft_core::{BlockId, Document};

use super::Env;

/// Pictures made in one frame.
const PER_FRAME: usize = 4;

/// A picture, or `None` for a block that has nothing to draw.
type Slot = Option<egui::TextureHandle>;

#[derive(Default)]
pub struct Thumbnails {
    library: HashMap<BlockUuid, Slot>,
    drawing: HashMap<String, Slot>,
    library_stamp: u64,
    drawing_revision: u64,
    made: usize,
}

impl Thumbnails {
    /// Lets go of pictures that no longer match the library or the drawing.
    pub fn keep_current(&mut self, library_stamp: u64, drawing_revision: u64) {
        if self.library_stamp != library_stamp {
            self.library.clear();
            self.library_stamp = library_stamp;
        }
        if self.drawing_revision != drawing_revision {
            self.drawing.clear();
            self.drawing_revision = drawing_revision;
        }
        self.made = 0;
    }

    /// The picture of a library block. `Err(())` while it has not been made
    /// yet this frame; `Ok(None)` when there is nothing to draw.
    pub fn library_block(
        &mut self,
        ctx: &egui::Context,
        env: &Env,
        id: BlockUuid,
    ) -> Result<Option<&egui::TextureHandle>, ()> {
        if !self.library.contains_key(&id) {
            if self.made >= PER_FRAME {
                ctx.request_repaint();
                return Err(());
            }
            self.made += 1;
            let picture = env
                .library
                .with(|lib| {
                    let (doc, block) = lib.source(id)?;
                    picture_of(doc, block)
                })
                .ok()
                .flatten();
            self.library.insert(id, upload(ctx, "cm_lib", picture));
        }
        Ok(self.library.get(&id).and_then(Option::as_ref))
    }

    /// As `library_block`, for a block of the open drawing.
    pub fn drawing_block(
        &mut self,
        ctx: &egui::Context,
        env: &Env,
        name: &str,
        id: BlockId,
    ) -> Result<Option<&egui::TextureHandle>, ()> {
        if !self.drawing.contains_key(name) {
            if self.made >= PER_FRAME {
                ctx.request_repaint();
                return Err(());
            }
            self.made += 1;
            let picture = picture_of(env.document, id);
            self.drawing
                .insert(name.to_owned(), upload(ctx, "cm_drawing", picture));
        }
        Ok(self.drawing.get(name).and_then(Option::as_ref))
    }
}

/// What the block draws, in the dark picture the drawing area has.
fn picture_of(doc: &Document, block: BlockId) -> Option<Preview> {
    crate::purge_cmd::block_picture(doc, block)
}

fn upload(ctx: &egui::Context, name: &str, picture: Option<Preview>) -> Slot {
    let p = picture?;
    let image =
        egui::ColorImage::from_rgba_unmultiplied([p.width as usize, p.height as usize], &p.rgba);
    Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
}

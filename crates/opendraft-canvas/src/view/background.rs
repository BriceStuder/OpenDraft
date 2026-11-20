//! A slot's new picture from the background (see `builder.rs`): asked for
//! when the view moves, kept when it comes back. Used for pictures kept as an
//! image when the app has shared the drawing; otherwise a new picture is made
//! here, a slice a frame after the view has stood still (`layer_paint.rs`).

use std::sync::Arc;

use opendraft_core::Document;

use super::builder::{Builder, Job};
use super::layer_paint::Frame;
use super::View;
use crate::layer_cache::{LayerKey, Render};

impl View {
    /// Keep the picture the background made for the frame's slot, if one came
    /// back that shows what `key` shows (from any view: a picture of a view
    /// nearer the one wanted than the kept one is still better), unless the
    /// picture of `key` itself is kept already.
    pub(super) fn take_built(&self, frame: &Frame<'_>, key: &LayerKey) {
        let built = match self.builder.borrow_mut().as_mut() {
            Some(builder) => builder.take(frame.slot),
            None => None,
        };
        let Some(built) = built else {
            return;
        };
        let mut cache = self.cache(frame);
        if !built.key.same_content(key) || cache.has(key) {
            return;
        }
        match built.image {
            Some(image) => {
                cache.install_image(built.key, image);
                cache.note_build_time(built.took);
            }
            // It could not be made there: make it here from now on.
            None => cache.refuse_background(built.key),
        }
    }

    /// The drawing, shared, when the picture of `key` can be made in the
    /// background: kept as an image, and shared by the app.
    pub(super) fn background(&self, frame: &Frame<'_>, key: &LayerKey) -> Option<Arc<Document>> {
        let image = self.cache_config.render == Render::Image;
        if !image || self.document_token.is_none() || self.cache(frame).refused(key) {
            return None;
        }
        // Only the drawing being painted, never another or an older one.
        let document = self.shared_document.upgrade();
        document.filter(|d| std::ptr::eq(&**d, frame.document))
    }

    /// Ask the background for the picture of `key`, unless it is asked for
    /// already; false when it cannot be made there, and is made here.
    pub(super) fn ask_background(&self, frame: &Frame<'_>, key: &LayerKey) -> bool {
        let (Some(document), Some(token)) = (self.background(frame, key), self.document_token)
        else {
            return false;
        };
        let mut builder = self.builder.borrow_mut();
        let ctx = frame.painter.ctx();
        let builder = builder.get_or_insert_with(|| Builder::new(ctx.clone()));
        if !builder.alive() {
            return false;
        }
        if builder.asked_for(frame.slot, key) {
            return true;
        }
        let plan = self.keep_plan(frame);
        builder.ask(Job {
            slot: frame.slot,
            key: key.clone(),
            document,
            token,
            camera: self.camera,
            hidden: self.hidden,
            point_style: self.point_style,
            style: frame.style.clone(),
            rect: frame.rect,
            canvas: frame.canvas,
            clip: frame.clip,
            space: frame.space,
            frozen: frame.frozen.to_vec(),
            geometry: plan.geometry,
            name: plan.name,
            tess: frame.tess.clone(),
        });
        true
    }
}

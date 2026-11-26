//! How an object is shown (its layer, colour, linetype and lineweight, and
//! whether it can be seen or picked), the queries over the objects of the
//! space being edited, and the bounds of a drawing.

use super::{Document, Object};
use crate::layer::{Color, Layer, LayerId, LayerLook, Lineweight};
use crate::layout::LayoutId;
use crate::linetype::{LinetypeId, StrokeElement};
use crate::{Bounds, EntityId};

impl Document {
    // ---- how an object is shown ----

    /// The layer an object is on. An object whose layer is missing (which
    /// valid edits cannot cause) is treated as being on the default layer.
    pub(super) fn layer_of(&self, object: &Object) -> &Layer {
        self.layer(object.attrs.layer).unwrap_or(&self.layers[0])
    }

    /// What a layer gives its objects (the default layer's for an unknown id).
    pub fn layer_look(&self, id: LayerId) -> LayerLook {
        self.layer(id).unwrap_or(&self.layers[0]).look()
    }

    pub fn color_of(&self, object: &Object) -> Color {
        let (color, outside) = (&object.attrs.color, LayerLook::OUTSIDE_BLOCK.color);
        color.resolve(&self.layer_of(object).color, &outside)
    }

    pub fn linetype_of(&self, object: &Object) -> LinetypeId {
        let (attr, outside) = (&object.attrs.linetype, LayerLook::OUTSIDE_BLOCK.linetype);
        attr.resolve(&self.layer_of(object).linetype, &outside)
    }

    /// The pattern an object is drawn with and the scale to lay it at (its own
    /// linetype scale times `LTSCALE`), or
    /// `None` for a continuous line (also for an id the table lacks, which a
    /// valid drawing has none of).
    pub fn dash_pattern(&self, object: &Object) -> Option<(&[StrokeElement], f64)> {
        let record = self.linetypes().get(self.linetype_of(object))?;
        (!record.is_continuous()).then_some((
            record.elements.as_slice(),
            object.attrs.linetype_scale * self.linetype_scale,
        ))
    }

    pub fn lineweight_of(&self, object: &Object) -> Lineweight {
        let (attr, outside) = (
            &object.attrs.lineweight,
            LayerLook::OUTSIDE_BLOCK.lineweight,
        );
        attr.resolve(&self.layer_of(object).lineweight, &outside)
    }

    /// Drawn and snapped to: on a layer that is on and not frozen, and not
    /// hidden by isolation.
    pub fn is_visible(&self, object: &Object) -> bool {
        self.layer_of(object).is_visible() && !self.is_hidden(object.id)
    }

    /// Whether isolation hides the object.
    pub fn is_hidden(&self, id: EntityId) -> bool {
        !self.hidden_objects.is_empty() && self.hidden_objects.binary_search(&id).is_ok()
    }

    /// The objects hidden by isolation, sorted.
    pub fn hidden_objects(&self) -> &[EntityId] {
        &self.hidden_objects
    }

    /// Hide exactly these objects from every query (none shows them all).
    /// Not an edit: it is neither undone nor saved.
    pub fn set_hidden_objects(&mut self, mut ids: Vec<EntityId>) {
        ids.sort_unstable();
        ids.dedup();
        self.hidden_objects = ids;
    }

    /// May be picked and edited: visible, not on a locked layer, nor on one
    /// frozen where the view is (`active_frozen`).
    pub fn is_selectable(&self, object: &Object) -> bool {
        let layer = self.layer_of(object);
        self.is_visible(object) && !layer.locked && !self.active_frozen.contains(&layer.id)
    }

    /// Visible objects of the space being edited (see `active_layout`).
    pub fn visible_objects(&self) -> impl Iterator<Item = &Object> {
        self.visible_in(self.active_layout)
            .filter(|o| !self.active_frozen.contains(&o.attrs.layer))
    }

    /// Pickable objects of the space being edited.
    pub fn selectable_objects(&self) -> impl Iterator<Item = &Object> {
        let space = self.active_layout;
        self.objects.iter().filter(move |o| {
            o.layout == space
                && self.is_selectable(o)
                && !self.active_frozen.contains(&o.attrs.layer)
        })
    }

    /// Visible objects of model space (`None`) or of a layout's paper space.
    pub fn visible_in(&self, space: Option<LayoutId>) -> impl Iterator<Item = &Object> {
        self.objects
            .iter()
            .filter(move |o| o.layout == space && self.is_visible(o))
    }

    /// Bounds of everything visible in the space being edited, or `None`
    /// when nothing is.
    pub fn bounds(&self) -> Option<Bounds> {
        Self::union_of(self.visible_objects())
    }

    /// Bounds of everything visible in model space.
    pub fn model_bounds(&self) -> Option<Bounds> {
        Self::union_of(self.visible_in(None))
    }

    /// The box round the objects that have one: construction lines and rays
    /// run off to infinity and are left out, as AutoCAD leaves them out of
    /// the extents.
    pub(super) fn union_of<'a>(objects: impl Iterator<Item = &'a Object>) -> Option<Bounds> {
        objects
            .map(Object::bounds)
            .filter(Bounds::is_finite)
            .reduce(Bounds::union)
    }

    /// Whether the space being edited holds any object.
    pub fn has_objects_here(&self) -> bool {
        // Stops at the first, where counting would walk them all.
        self.objects.iter().any(|o| o.layout == self.active_layout)
    }

    /// How many objects the space being edited holds.
    pub fn len_here(&self) -> usize {
        self.objects
            .iter()
            .filter(|o| o.layout == self.active_layout)
            .count()
    }

    /// Objects of model space only (what a DXF file holds).
    pub fn model_objects(&self) -> impl Iterator<Item = &Object> {
        self.objects.iter().filter(|o| o.layout.is_none())
    }

    /// The paper space being edited (`None`: model space).
    pub fn active_layout(&self) -> Option<LayoutId> {
        self.active_layout
    }

    /// Choose the space new objects go to and queries look at. Not an edit:
    /// it is neither undone nor saved.
    pub fn set_active_layout(&mut self, layout: Option<LayoutId>) {
        self.active_layout = layout;
    }

    /// Hide these layers from the object queries (those frozen in the
    /// viewport being worked through). Not an edit.
    pub fn set_active_frozen(&mut self, layers: Vec<LayerId>) {
        self.active_frozen = layers;
    }

    /// The layers hidden from the object queries (see `set_active_frozen`).
    pub fn active_frozen(&self) -> &[LayerId] {
        &self.active_frozen
    }

    /// The layers frozen in the Model tab only (`Layer::frozen_in_model`):
    /// the Model tab hides them, layout viewports still show them.
    pub fn frozen_in_model(&self) -> Vec<LayerId> {
        let layers = self.layers().iter().filter(|l| l.frozen_in_model);
        layers.map(|l| l.id).collect()
    }

    /// Whether objects on this layer reach a plot: not on a layer switched
    /// to No Plot, nor on Defpoints (see `Layer::plots`).
    pub fn plots(&self, layer: LayerId) -> bool {
        self.layer(layer).is_none_or(Layer::plots)
    }

    /// Ids of the objects on a layout's sheet.
    pub fn objects_of_layout(&self, layout: LayoutId) -> Vec<EntityId> {
        self.objects
            .iter()
            .filter(|o| o.layout == Some(layout))
            .map(|o| o.id)
            .collect()
    }
}

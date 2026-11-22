//! Where the VP Freeze switch acts: the viewport being worked on in a layout,
//! or the Model tab. Freezing in the Model tab is an OpenDraft extension
//! (AutoCAD has no such switch there): the layer is hidden in the Model tab,
//! while layout viewports still show it and plot it.

use opendraft_canvas::Space;
use opendraft_core::{Layer, LayerId, Viewport};

use crate::state::AppState;

/// What the VP Freeze switch freezes layers in.
#[derive(Clone, Copy, Debug)]
pub enum FreezeTarget<'a> {
    Viewport(&'a Viewport),
    Model,
}

impl FreezeTarget<'_> {
    /// The switch as `layer` shows it.
    pub fn switch(self, layer: &Layer) -> VpSwitch {
        match self {
            FreezeTarget::Viewport(v) => VpSwitch {
                frozen: v.is_frozen(layer.id),
                model: false,
            },
            FreezeTarget::Model => VpSwitch {
                frozen: layer.frozen_in_model,
                model: true,
            },
        }
    }
}

/// One layer's VP Freeze switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VpSwitch {
    pub frozen: bool,
    /// In the Model tab rather than in a viewport.
    pub model: bool,
}

impl VpSwitch {
    /// Its tooltip, in the Layers panel and the ribbon's layer list.
    pub fn tip(self) -> &'static str {
        match (self.model, self.frozen) {
            (false, true) => "Frozen in this viewport: click to thaw",
            (false, false) => "Click to freeze in this viewport only",
            (true, true) => "Frozen in the Model tab: click to thaw",
            (true, false) => "Click to freeze in the Model tab only",
        }
    }
}

impl AppState {
    /// What the VP Freeze switch acts on: the Model tab, the viewport being
    /// worked on in a layout, or nothing (a sheet with no viewport picked).
    pub fn freeze_target(&self) -> Option<FreezeTarget<'_>> {
        if self.space() == Space::Model {
            return Some(FreezeTarget::Model);
        }
        self.target_viewport_ref().map(FreezeTarget::Viewport)
    }

    /// Freeze (`freeze`) or thaw layers where the VP Freeze switch acts, one
    /// undo step.
    pub fn set_layers_in_view(&mut self, layers: &[LayerId], freeze: bool) {
        if self.space() == Space::Model {
            self.set_layers_in_model(layers, freeze);
        } else {
            self.set_layers_in_viewport(layers, freeze);
        }
    }

    /// Freeze or thaw layers in the Model tab. The current layer is left
    /// thawed, as Freeze leaves it: what is drawn on it would vanish.
    fn set_layers_in_model(&mut self, layers: &[LayerId], freeze: bool) {
        let current = self.session().document().current_layer();
        if freeze && layers.contains(&current) {
            self.message = "The current layer cannot be frozen".to_owned();
        }
        let layers = layers
            .iter()
            .copied()
            .filter(|&id| !(freeze && id == current));
        let name = if freeze {
            "Freeze layer in Model tab"
        } else {
            "Thaw layer in Model tab"
        };
        self.edit_layers(name, layers, |l| l.frozen_in_model = freeze);
    }
}

#[cfg(test)]
mod tests {
    use opendraft_core::{Entity, Point};

    use super::*;
    use crate::layers::LayerEdit;

    /// A line on a new layer, with layer 0 current again.
    fn drawing() -> (AppState, LayerId) {
        let mut state = AppState::new();
        let layer = state.new_layer();
        state.set_current_layer(layer);
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(10000.0, 5000.0),
        });
        state.set_current_layer(LayerId::DEFAULT);
        (state, layer)
    }

    fn freeze(state: &mut AppState, layers: &[LayerId], freeze: bool) {
        state.apply_layer_edit(LayerEdit::SetInViewport(layers.to_vec(), freeze));
        // What the next frame does.
        state.docs[state.active].keep_space_valid();
    }

    fn layer(state: &AppState, id: LayerId) -> Layer {
        state.session().document().layer(id).unwrap().clone()
    }

    fn pickable(state: &AppState) -> usize {
        state.session().document().selectable_objects().count()
    }

    #[test]
    fn in_the_model_tab_the_switch_freezes_there_and_undoes() {
        let (mut state, id) = drawing();
        let target = state.freeze_target().unwrap();
        assert!(matches!(target, FreezeTarget::Model));
        assert_eq!(
            target.switch(&layer(&state, id)),
            VpSwitch {
                frozen: false,
                model: true
            }
        );
        state.select_all();
        freeze(&mut state, &[id], true);
        let frozen = layer(&state, id);
        assert!(frozen.frozen_in_model && !frozen.frozen && frozen.on);
        assert_eq!(pickable(&state), 0, "hidden from picking and snapping");
        assert!(state.selection().is_empty(), "it leaves the selection");
        assert!(state.session().document().bounds().is_none(), "nor in ZE");
        state.undo();
        state.docs[state.active].keep_space_valid();
        assert!(!layer(&state, id).frozen_in_model);
        assert_eq!(pickable(&state), 1);
    }

    #[test]
    fn the_current_layer_stays_thawed_in_the_model_tab() {
        let (mut state, id) = drawing();
        freeze(&mut state, &[LayerId::DEFAULT, id], true);
        assert_eq!(state.message, "The current layer cannot be frozen");
        assert!(!layer(&state, LayerId::DEFAULT).frozen_in_model);
        assert!(layer(&state, id).frozen_in_model);
    }

    #[test]
    fn viewports_still_show_a_layer_frozen_in_the_model_tab() {
        let (mut state, id) = drawing();
        freeze(&mut state, &[id], true);
        state.new_layout();
        assert!(state.freeze_target().is_none(), "no viewport picked yet");
        state.mspace();
        let target = state.freeze_target().unwrap();
        assert!(matches!(target, FreezeTarget::Viewport(_)));
        assert!(!target.switch(&layer(&state, id)).frozen);
        state.docs[state.active].keep_space_valid();
        assert_eq!(pickable(&state), 1, "shown in the viewport");
        // There the switch acts on the viewport, not on the Model tab.
        freeze(&mut state, &[id], true);
        assert_eq!(pickable(&state), 0);
        freeze(&mut state, &[id], false);
        assert_eq!(pickable(&state), 1);
        assert!(layer(&state, id).frozen_in_model);
        state.enter_model();
        assert_eq!(pickable(&state), 0, "still frozen in the Model tab");
    }

    #[test]
    fn the_tooltips_say_where_the_switch_acts() {
        let tip = |frozen, model| VpSwitch { frozen, model }.tip();
        assert_eq!(tip(false, true), "Click to freeze in the Model tab only");
        assert_eq!(tip(true, true), "Frozen in the Model tab: click to thaw");
        assert_eq!(tip(false, false), "Click to freeze in this viewport only");
        assert_eq!(tip(true, false), "Frozen in this viewport: click to thaw");
    }
}

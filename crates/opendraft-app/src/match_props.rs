//! MATCHPROP: copy one object's properties onto others, as AutoCAD's Match
//! Properties. What a target becomes is [`matched`], kept pure so it is
//! tested without the command.

use opendraft_canvas::{select_in, Region};
use opendraft_commands::{
    Answer, Cancelled, Editor, ModifyAttributes, ModifyEntities, Options, Picked, Swept,
};
use opendraft_core::{Attributes, Ccs, Document, Entity, EntityId, MText, Object, Point};
use opendraft_ui::key;

use crate::defaults::EditorDefaults;
use crate::edges_sweep::ask_window;
use crate::layers::has_outline;
use opendraft_commands::Wording;

/// The properties that can be matched, as AutoCAD names them in "Current
/// active settings".
const NAMES: [&str; 10] = [
    "Color",
    "Layer",
    "Ltype",
    "Ltscale",
    "Lineweight",
    "Dim",
    "Text",
    "Hatch",
    "Polyline",
    "Multileader",
];
/// What is typed to pick each, one apiece.
const KEYS: [&str; 10] = ["C", "L", "LT", "LS", "LW", "D", "T", "H", "P", "M"];
const COLOR: usize = 0;
const LAYER: usize = 1;
const LTYPE: usize = 2;
const LTSCALE: usize = 3;
const LINEWEIGHT: usize = 4;
const DIM: usize = 5;
const TEXT: usize = 6;
const HATCH: usize = 7;
const POLYLINE: usize = 8;
const MULTILEADER: usize = 9;

/// Which properties MATCHPROP copies (its Settings option). All of them to
/// start with; kept for the session, not saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Matching([bool; 10]);

impl Default for Matching {
    fn default() -> Self {
        Self([true; 10])
    }
}

impl Matching {
    /// The names of the properties that are on, worded as AutoCAD's
    /// "Current active settings".
    pub fn summary(&self) -> String {
        let on: Vec<&str> = NAMES
            .iter()
            .zip(self.0)
            .filter_map(|(name, on)| on.then_some(*name))
            .collect();
        if on.is_empty() {
            "none".to_owned()
        } else {
            on.join(" ")
        }
    }

    fn has(&self, which: usize) -> bool {
        self.0[which]
    }
}

/// `target` as it would be after matching `source`: its attributes, and its
/// entity when one of the special properties changes it. `None` when nothing
/// would change.
///
/// Every object takes the colour, layer, linetype, linetype scale and
/// lineweight. Between objects of the same kind: a dimension takes the
/// dimension style (with its overrides), text the text style and height, a
/// hatch the fill and island style, a multileader the style, and a polyline
/// or rectangle the linetype generation and a polyline the global width.
pub fn matched(
    document: &Document,
    source: &Object,
    target: &Object,
    matching: &Matching,
) -> Option<(Attributes, Option<Entity>)> {
    let mut attrs = target.attrs.clone();
    if matching.has(LAYER) {
        attrs.layer = source.attrs.layer;
    }
    if matching.has(COLOR) {
        attrs.color = source.attrs.color.clone();
    }
    if matching.has(LTYPE) {
        attrs.linetype = source.attrs.linetype.clone();
    }
    if matching.has(LINEWEIGHT) {
        attrs.lineweight = source.attrs.lineweight.clone();
    }
    if matching.has(LTSCALE) {
        attrs.linetype_scale = source.attrs.linetype_scale;
    }
    if matching.has(POLYLINE) && has_outline(&source.entity) && has_outline(&target.entity) {
        attrs.continuous_generation = source.attrs.continuous_generation;
    }
    let entity = special(document, &source.entity, &target.entity, matching);
    (attrs != target.attrs || entity.is_some()).then_some((attrs, entity))
}

/// The target changed by the source's special properties, if they apply and
/// change it.
fn special(
    document: &Document,
    source: &Entity,
    target: &Entity,
    matching: &Matching,
) -> Option<Entity> {
    let next = match (source, target) {
        (Entity::Dimension(s), Entity::Dimension(t)) if matching.has(DIM) => {
            let mut d = (**t).clone();
            d.style = s.style.clone();
            Entity::Dimension(Box::new(d))
        }
        (Entity::MText(s), Entity::MText(t)) if matching.has(TEXT) => {
            Entity::MText(Box::new(MText {
                style: s.style,
                nominal_height: s.nominal_height,
                ..(**t).clone()
            }))
        }
        (Entity::Hatch(s), Entity::Hatch(t)) if matching.has(HATCH) => {
            let mut h = (**t).clone();
            h.fill = s.fill.clone();
            h.style = s.style;
            Entity::Hatch(Box::new(h))
        }
        (Entity::MLeader(s), Entity::MLeader(t)) if matching.has(MULTILEADER) => {
            let style = document.mleader_style_named(&s.style_name)?;
            Entity::MLeader(Box::new(t.restyled(style)))
        }
        (Entity::Polyline { widths: from, .. }, Entity::Polyline { .. })
            if matching.has(POLYLINE) =>
        {
            let width = global_width(from)?;
            let mut p = target.clone();
            if let Entity::Polyline { points, widths, .. } = &mut p {
                *widths = match width {
                    0.0 => Vec::new(),
                    w => vec![(w, w); points.len()],
                };
            }
            p
        }
        _ => return None,
    };
    (next != *target).then_some(next)
}

/// A polyline's one width (0 for none), or `None` when its segments differ.
fn global_width(widths: &[(f64, f64)]) -> Option<f64> {
    let first = widths.first().map_or(0.0, |w| w.0);
    widths
        .iter()
        .all(|&(a, b)| a == first && b == first)
        .then_some(first)
}

/// MATCHPROP: pick the source object (or have one selected), then each
/// object picked, or inside a crossing window or along a path dragged from
/// empty space, takes its properties, one undo step each, until Enter. The
/// Settings option chooses which properties are copied.
pub fn match_properties(ed: &mut Editor) -> Result<(), Cancelled> {
    let source = match ed.pickset() {
        [one] => *one,
        _ => loop {
            let options = Options::<Picked>::new(key!("Select source object"))
                .keyword("Settings", "S")
                .pick_locked()
                .allow_empty();
            match ed.ask_object(options)? {
                Answer::Value(Picked { id, .. }) => break id,
                Answer::Keyword(_) => settings(ed)?,
                Answer::Enter => return Ok(()),
            }
        },
    };
    say_settings(ed);
    loop {
        let options = Options::<Swept>::new(key!("Select destination object(s)"))
            .keyword("Settings", "S")
            .allow_empty();
        let targets = match ed.ask_object_or_sweep(options)? {
            Answer::Value(Swept::Object(Picked { id, .. })) => vec![id],
            Answer::Value(Swept::Missed { at, .. }) => {
                let corners = ask_window(ed, at)?;
                let twist = ed.read(|doc| doc.view_twist())?;
                let region = window_or_crossing(corners, twist);
                ed.read(move |doc| select_in(doc, &region))?
            }
            Answer::Value(Swept::Path { points, .. }) => {
                let region = Region::Fence(points);
                ed.read(move |doc| select_in(doc, &region))?
            }
            Answer::Keyword(_) => {
                settings(ed)?;
                say_settings(ed);
                continue;
            }
            Answer::Enter => return Ok(()),
        };
        for id in targets.into_iter().filter(|id| *id != source) {
            apply(ed, source, id)?;
        }
    }
}

/// A window drawn to the right of its first corner (on the screen) takes only
/// what it holds; one drawn to the left takes whatever it touches.
fn window_or_crossing(corners: Vec<Point>, twist: f64) -> Region {
    let frame = Ccs::new(Point::new(0.0, 0.0), -twist);
    let rightward = match (corners.first(), corners.get(2)) {
        (Some(first), Some(opposite)) => {
            frame.from_world(*opposite).x >= frame.from_world(*first).x
        }
        _ => false,
    };
    if rightward {
        Region::Window(corners)
    } else {
        Region::Crossing(corners)
    }
}

fn say_settings(ed: &Editor) {
    let summary = ed.defaults().matching.get().summary();
    ed.say(Wording::new(
        key!("Current active settings: %{settings}"),
        &[("settings", &summary)],
    ));
}

/// The Settings option: switch properties on and off one at a time until
/// Enter.
fn settings(ed: &Editor) -> Result<(), Cancelled> {
    loop {
        let now = ed.defaults().matching.get();
        let mut options = Options::<String>::new(key!("Switch a property on or off"));
        for (name, key) in NAMES.iter().zip(KEYS) {
            options = options.keyword(*name, key);
        }
        let Some(chosen) = ed.ask_keyword(options)? else {
            return Ok(());
        };
        let Some(which) = NAMES.iter().position(|n| *n == chosen) else {
            continue;
        };
        ed.defaults()
            .matching
            .change(|m| m.0[which] = !now.0[which]);
        say_settings(ed);
    }
}

/// Make `target` match `source`, as one undo step.
fn apply(ed: &Editor, source: EntityId, target: EntityId) -> Result<(), Cancelled> {
    let matching = ed.defaults().matching.get();
    ed.transaction("Match Properties", move |tx| {
        let doc = tx.document();
        let (Some(from), Some(to)) = (doc.object(source), doc.object(target)) else {
            return Ok(());
        };
        let Some((attrs, entity)) = matched(doc, from, to, &matching) else {
            return Ok(());
        };
        tx.perform(ModifyAttributes::new(
            "Match Properties",
            vec![(target, attrs)],
        ));
        if let Some(entity) = entity {
            tx.perform(ModifyEntities::new(
                "Match Properties",
                vec![(target, entity)],
            ));
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use opendraft_commands::ModifyAttributes;
    use opendraft_core::{Attr, Color, DimStyle, Dimension, LayerId, Lineweight, Point};

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(y: f64) -> Entity {
        Entity::Line {
            start: p(0.0, y),
            end: p(10.0, y),
        }
    }

    fn object(entity: Entity, attrs: Attributes) -> Object {
        Object::new(EntityId(1), entity, attrs, None)
    }

    fn red_dashed(layer: LayerId) -> Attributes {
        Attributes {
            color: Attr::Set(Color::Rgb(255, 0, 0)),
            linetype: Attr::Set(opendraft_core::LinetypeId::DASHED),
            lineweight: Attr::Set(Lineweight::STANDARD[3]),
            linetype_scale: 2.0,
            ..Attributes::on_layer(layer)
        }
    }

    #[test]
    fn every_object_takes_the_general_properties() {
        let doc = Document::default();
        let source = object(line(0.0), red_dashed(LayerId(7)));
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        let target = object(circle, Attributes::on_layer(LayerId::DEFAULT));
        let (attrs, entity) = matched(&doc, &source, &target, &Matching::default()).unwrap();
        assert_eq!(attrs, red_dashed(LayerId(7)));
        assert!(
            entity.is_none(),
            "a line has no special properties for a circle"
        );
        let same = object(line(5.0), red_dashed(LayerId(7)));
        assert!(
            matched(&doc, &source, &same, &Matching::default()).is_none(),
            "nothing to change"
        );
    }

    #[test]
    fn same_kinds_take_their_special_properties() {
        let doc = Document::default();
        let dim = |height: f64| {
            let style = DimStyle {
                text_height: height,
                ..DimStyle::default()
            };
            Entity::Dimension(Box::new(Dimension::aligned(
                p(0.0, 0.0),
                p(10.0, 0.0),
                p(0.0, 5.0),
                &style,
            )))
        };
        let on_zero = Attributes::on_layer(LayerId::DEFAULT);
        let source = object(dim(5.0), on_zero.clone());
        let target = object(dim(2.5), on_zero.clone());
        let (_, entity) = matched(&doc, &source, &target, &Matching::default()).unwrap();
        let Some(Entity::Dimension(d)) = entity else {
            panic!("the dimension changes");
        };
        assert_eq!(d.style.text_height, 5.0);

        let wide = |widths: Vec<(f64, f64)>| Entity::Polyline {
            points: vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0)],
            bulges: Vec::new(),
            widths,
            closed: false,
        };
        let source = object(wide(vec![(0.5, 0.5); 3]), on_zero.clone());
        let target = object(wide(Vec::new()), on_zero.clone());
        let (_, entity) = matched(&doc, &source, &target, &Matching::default()).unwrap();
        assert_eq!(entity, Some(wide(vec![(0.5, 0.5); 3])));
        let tapered = object(wide(vec![(0.0, 1.0); 3]), on_zero);
        assert!(
            matched(&doc, &tapered, &target, &Matching::default()).is_none(),
            "no one width"
        );
    }

    #[test]
    fn switched_off_properties_are_not_copied() {
        let doc = Document::default();
        let source = object(line(0.0), red_dashed(LayerId(7)));
        let target = object(line(5.0), Attributes::on_layer(LayerId::DEFAULT));
        let mut matching = Matching::default();
        matching.0[COLOR] = false;
        matching.0[LAYER] = false;
        let (attrs, _) = matched(&doc, &source, &target, &matching).unwrap();
        assert_eq!(attrs.layer, LayerId::DEFAULT);
        assert_eq!(attrs.color, target.attrs.color);
        assert_eq!(attrs.linetype_scale, 2.0, "the rest still matches");
        let none = Matching([false; 10]);
        assert!(matched(&doc, &source, &target, &none).is_none());
        assert_eq!(none.summary(), "none");
        assert!(Matching::default()
            .summary()
            .starts_with("Color Layer Ltype"));
    }

    fn red_source_and_two_plain(state: &mut AppState) -> Vec<EntityId> {
        for y in [0.0, 5.0, 10.0] {
            state.add_entity(line(y));
        }
        let ids: Vec<EntityId> = state
            .session()
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        state.perform(ModifyAttributes::new(
            "Red",
            vec![(ids[0], red_dashed(LayerId::DEFAULT))],
        ));
        ids
    }

    #[test]
    fn the_settings_option_switches_properties_for_the_command_and_after() {
        let mut state = AppState::new();
        let ids = red_source_and_two_plain(&mut state);
        state.click_select(Some(ids[0]), false);
        state.start_command("MATCHPROP", match_properties);
        state.settle_command();
        state.cmd_submit("s");
        state.settle_command();
        // Colour off, linetype off; Enter finishes the settings.
        for key in ["c", "lt", ""] {
            state.cmd_submit(key);
            state.settle_command();
        }
        let line = state.cmdline.log.last().cloned().unwrap_or_default();
        assert!(!line.contains("Color") && !line.contains("Ltype"), "{line}");
        state.answer_object(p(1.0, 0.0), Some(ids[1]), false);
        state.settle_command();
        let now = state
            .session()
            .document()
            .object(ids[1])
            .unwrap()
            .attrs
            .clone();
        assert_eq!(now.color, Attributes::on_layer(LayerId::DEFAULT).color);
        assert_eq!(now.linetype_scale, 2.0, "line type scale still matched");
        assert!(!state.defaults().matching.get().has(COLOR));
    }

    #[test]
    fn a_window_drawn_to_the_right_takes_only_what_it_holds() {
        let mut state = AppState::new();
        let ids = red_source_and_two_plain(&mut state);
        state.click_select(Some(ids[0]), false);
        state.start_command("MATCHPROP", match_properties);
        state.settle_command();
        // Right to left would touch both; left to right holds neither line
        // (they run to x = 10) until it reaches past them.
        state.answer_object(p(-5.0, 4.0), None, false);
        state.settle_command();
        state.answer_point(p(5.0, 6.0));
        state.settle_command();
        let plain = |i: usize, state: &AppState| {
            state
                .session()
                .document()
                .object(ids[i])
                .unwrap()
                .attrs
                .clone()
                == Attributes::on_layer(LayerId::DEFAULT)
        };
        assert!(plain(1, &state), "the line is only crossed, not held");
        state.answer_object(p(-5.0, 4.0), None, false);
        state.settle_command();
        state.answer_point(p(11.0, 6.0));
        state.settle_command();
        assert!(!plain(1, &state), "now the window holds it");
        assert!(plain(2, &state), "the line at y = 10 is outside");
    }

    #[test]
    fn a_crossing_window_matches_everything_it_touches() {
        let mut state = AppState::new();
        let ids = red_source_and_two_plain(&mut state);
        state.click_select(Some(ids[0]), false);
        state.start_command("MATCHPROP", match_properties);
        state.settle_command();
        // A click on nothing, then the opposite corner, around the other two.
        state.answer_object(p(-5.0, 4.0), None, false);
        state.settle_command();
        state.answer_point(p(15.0, 11.0));
        state.settle_command();
        for id in &ids[1..] {
            let now = state
                .session()
                .document()
                .object(*id)
                .unwrap()
                .attrs
                .clone();
            assert_eq!(now, red_dashed(LayerId::DEFAULT));
        }
        assert!(state.command_running(), "back at the destination prompt");
        state.undo();
        state.undo();
        let kept = state
            .session()
            .document()
            .object(ids[1])
            .unwrap()
            .attrs
            .clone();
        assert_eq!(kept, Attributes::on_layer(LayerId::DEFAULT));
    }

    #[test]
    fn the_command_matches_each_picked_object_in_its_own_step() {
        let mut state = AppState::new();
        for y in [0.0, 5.0, 10.0] {
            state.add_entity(line(y));
        }
        let ids: Vec<EntityId> = state
            .session()
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        state.perform(ModifyAttributes::new(
            "Red",
            vec![(ids[0], red_dashed(LayerId::DEFAULT))],
        ));
        let attrs =
            |state: &AppState, id| state.session().document().object(id).unwrap().attrs.clone();

        state.click_select(Some(ids[0]), false);
        state.start_command("MATCHPROP", match_properties);
        state.settle_command();
        let line = state.cmdline.log.last().cloned().unwrap_or_default();
        assert!(
            line.starts_with("Current active settings: Color Layer"),
            "{line}"
        );
        for &id in &ids[1..] {
            state.answer_object(p(1.0, 0.0), Some(id), false);
            state.settle_command();
        }
        state.answer_enter();
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(attrs(&state, ids[2]), red_dashed(LayerId::DEFAULT));
        state.undo();
        assert_eq!(
            attrs(&state, ids[2]),
            Attributes::on_layer(LayerId::DEFAULT)
        );
        assert_eq!(attrs(&state, ids[1]), red_dashed(LayerId::DEFAULT));
    }
}

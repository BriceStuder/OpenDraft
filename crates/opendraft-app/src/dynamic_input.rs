//! Cursor field state and geometry. Command execution remains in `interact`.
use eframe::egui;
use std::collections::BTreeMap;

use opendraft_commands::{DynamicDimensions, Prompt, Wants};
use opendraft_core::units::{format_angle, format_length, parse_angle, parse_length};
use opendraft_core::{AngleFormat, Point, Units};

use crate::cmdline::parse_number;
use crate::coordinates::{parse_point_with_mode, CoordinateMode, Reading};
use crate::dynamic_settings::DynamicInputSettings;

pub mod dimensions;
mod placement;
pub use placement::{avoid, cursor_clearance, Placement};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    X,
    Y,
    Coordinate,
    Distance,
    /// The whole length of a line given by its middle (where `Distance` is
    /// half of it).
    Length,
    Angle,
    Number,
    Text,
    Keyword,
}

/// What was typed into `field`, as the number the field holds: a length in
/// drawing units, an angle in degrees counted from the base angle, or a plain
/// number. The coordinate system plays no part: a field holds what is shown.
pub fn parse_field(field: Field, text: &str, reading: impl Into<Reading>) -> Option<f64> {
    let units = &reading.into().units;
    match field {
        Field::Angle => parse_angle(units, text).map(|a| units.to_display_angle(a).to_degrees()),
        Field::Number => parse_number(text),
        _ => parse_length(text),
    }
}

/// The number an angle field holds for the world direction `direction`:
/// degrees measured in the coordinate system, from the base angle, or for an
/// amount to turn by (`turn`), from the system's X axis.
fn angle_field(direction: f64, turn: bool, reading: Reading) -> f64 {
    let units = &reading.units;
    let local = reading.ccs.angle_from_world(direction);
    let from = if turn {
        units.from_display_angle(0.0)
    } else {
        0.0
    };
    units.to_display_angle(local + from).to_degrees()
}

/// The world direction of an angle field holding `degrees` (see
/// [`angle_field`]).
fn field_direction(degrees: f64, turn: bool, reading: Reading) -> f64 {
    let units = &reading.units;
    let from = if turn {
        units.from_display_angle(0.0)
    } else {
        0.0
    };
    reading
        .ccs
        .angle_to_world(units.from_display_angle(degrees.to_radians()) - from)
}

/// A field's number written the way the drawing's units write it.
fn field_text(field: Field, value: f64, units: &Units) -> String {
    match field {
        // Surveyor bearings have no sign; the others show a flipped angle as negative.
        Field::Angle if value < 0.0 && units.angle != AngleFormat::Surveyor => format!(
            "-{}",
            format_angle(units, units.from_display_angle((-value).to_radians()))
        ),
        Field::Angle => format_angle(units, units.from_display_angle(value.to_radians())),
        Field::Number => format!("{value:.4}"),
        _ => format_length(units, value),
    }
}

/// A field's text as drawn when it is not being edited. Only plain degrees
/// need their mark; the other angle formats write their own.
pub fn shown_text(field: Field, live: &str, units: &Units) -> String {
    if field == Field::Angle && units.angle == AngleFormat::DecimalDegrees {
        format!("{live}\u{b0}")
    } else {
        live.to_owned()
    }
}

#[derive(Clone, Default)]
pub struct DynamicInputState {
    pub editing: bool,
    pub active: Option<Field>,
    pub draft: String,
    pub locks: BTreeMap<Field, f64>,
    pub locked_coordinate: Option<Point>,
    pub menu: bool,
    pub selected: usize,
    pub error: Option<String>,
    pub cursor: Option<Point>,
    pub origin: Option<Point>,
    pub canvas: Option<egui::Rect>,
    pub hit_rects: Vec<egui::Rect>,
    pub placements: BTreeMap<Field, Placement>,
    pub prompt_placement: Placement,
    pub signature: String,
    pub request_focus: bool,
}

/// Whether the prompt's X and Y are the sides of a box.
fn boxed(ask: &Prompt) -> bool {
    matches!(
        ask.dynamic_dimensions,
        DynamicDimensions::Cartesian | DynamicDimensions::Centred
    )
}

/// How many times the pointer's offset from the base X and Y show: twice
/// for a box centred on the base.
fn size_factor(ask: &Prompt) -> f64 {
    if ask.dynamic_dimensions == DynamicDimensions::Centred {
        2.0
    } else {
        1.0
    }
}

impl DynamicInputState {
    pub fn clear(&mut self) {
        let (cursor, canvas) = (self.cursor, self.canvas);
        *self = Self::default();
        self.cursor = cursor;
        self.canvas = canvas;
    }

    pub fn fields(&self, ask: &Prompt, settings: &DynamicInputSettings) -> Vec<Field> {
        use Field::*;
        match ask.wants {
            Wants::Point
                if self.draft.contains([',', '<'])
                    || self.draft.starts_with(['#', '@'])
                    || self.locked_coordinate.is_some() =>
            {
                vec![Coordinate]
            }
            Wants::Point
                if ask.base.is_some()
                    && settings.dimensions()
                    && ask.dynamic_dimensions == DynamicDimensions::Polar =>
            {
                vec![Distance, Angle]
            }
            Wants::Point
                if ask.base.is_some()
                    && settings.dimensions()
                    && ask.dynamic_dimensions == DynamicDimensions::Mirrored =>
            {
                vec![Distance, Length, Angle]
            }
            Wants::Point if settings.pointer() || (settings.dimensions() && boxed(ask)) => {
                if ask.base.is_some() && !settings.pointer_cartesian && !boxed(ask) {
                    vec![Distance, Angle]
                } else {
                    vec![X, Y]
                }
            }
            Wants::Distance if settings.dimensions() => vec![Distance],
            Wants::Angle if settings.dimensions() => vec![Angle],
            Wants::Number => vec![Number],
            Wants::Text => vec![Text],
            _ => Vec::new(),
        }
    }

    pub fn begin(&mut self, field: Field) {
        if self.active != Some(field) {
            self.draft.clear();
        }
        self.active = Some(field);
        self.editing = true;
        self.request_focus = true;
        self.error = None;
    }

    pub fn cycle(
        &mut self,
        fields: &[Field],
        reverse: bool,
        reading: impl Into<Reading>,
    ) -> Result<(), &'static str> {
        let reading = reading.into();
        if fields.is_empty() {
            return Ok(());
        }
        let current = self.active.unwrap_or(fields[0]);
        if matches!(current, Field::Text | Field::Keyword) {
            return Ok(());
        }
        if !self.draft.trim().is_empty() {
            let value = parse_field(current, &self.draft, reading)
                .ok_or("Enter a finite numeric value before locking this field.")?;
            // Half the length and the whole length say the same thing: the
            // one locked last counts.
            match current {
                Field::Distance => self.locks.remove(&Field::Length),
                Field::Length => self.locks.remove(&Field::Distance),
                _ => None,
            };
            self.locks.insert(current, value);
        }
        let index = fields.iter().position(|f| *f == current).unwrap_or(0);
        let next = if reverse {
            (index + fields.len() - 1) % fields.len()
        } else {
            (index + 1) % fields.len()
        };
        self.draft.clear();
        self.begin(fields[next]);
        Ok(())
    }

    pub fn value(
        &self,
        field: Field,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        reading: impl Into<Reading>,
    ) -> Option<f64> {
        let reading = reading.into();
        if self.active == Some(field) && !self.draft.is_empty() {
            if let Some(value) = parse_field(field, &self.draft, reading) {
                return Some(value);
            }
        }
        // The half length being typed gives the whole length, and the other
        // way round.
        let other = match field {
            Field::Distance => Some((Field::Length, 0.5)),
            Field::Length => Some((Field::Distance, 2.0)),
            _ => None,
        };
        if let Some((other, factor)) = other {
            if self.active == Some(other) && !self.draft.is_empty() {
                if let Some(value) = parse_field(other, &self.draft, reading) {
                    return Some(value * factor);
                }
            }
        }
        if let Some(value) = self.locks.get(&field) {
            return Some(*value);
        }
        if let Some((other, factor)) = other {
            if let Some(value) = self.locks.get(&other) {
                return Some(value * factor);
            }
        }
        let cursor = self.cursor?;
        let base = ask.base.unwrap_or(Point::new(0.0, 0.0));
        let relative = ask.base.is_some() && !settings.pointer_absolute;
        // X and Y are the cursor's coordinates in the system, or its offset
        // from the base measured along the system's axes.
        let ccs = reading.ccs;
        let shown = if relative {
            let size = size_factor(ask);
            let offset = ccs.vector_from_world(Point::new(cursor.x - base.x, cursor.y - base.y));
            Point::new(offset.x * size, offset.y * size)
        } else {
            ccs.from_world(cursor)
        };
        match field {
            Field::X => Some(shown.x),
            Field::Y => Some(shown.y),
            Field::Distance => Some((cursor.x - base.x).hypot(cursor.y - base.y)),
            Field::Length => Some(2.0 * (cursor.x - base.x).hypot(cursor.y - base.y)),
            Field::Angle => {
                // Half a turn either way, so 270 reads as -90.
                let direction = (cursor.y - base.y).atan2(cursor.x - base.x);
                let degrees = angle_field(direction, ask.turn, reading).rem_euclid(360.0);
                Some(if degrees > 180.0 {
                    degrees - 360.0
                } else {
                    degrees
                })
            }
            _ => None,
        }
    }

    /// What the field shows: the typed or locked value, else the cursor's.
    pub fn live_text(
        &self,
        field: Field,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        reading: impl Into<Reading>,
    ) -> String {
        let reading = reading.into();
        self.value(field, ask, settings, reading)
            .map(|v| field_text(field, v, &reading.units))
            .or_else(|| {
                self.locked_coordinate
                    .filter(|_| field == Field::Coordinate)
                    .map(|p| reading.show_point(p))
            })
            .unwrap_or_default()
    }

    pub fn constrained_point(
        &self,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        reading: impl Into<Reading>,
    ) -> Option<Point> {
        let reading = reading.into();
        if self.draft.is_empty() && self.locked_coordinate.is_some() {
            return self.locked_coordinate;
        }
        let active = self.active?;
        if self.locks.is_empty() && self.draft.is_empty() {
            return None;
        }
        if self.draft.contains([',', '<']) || self.draft.starts_with(['#', '@']) {
            let origin = ask.base.or(self.origin).unwrap_or(Point::new(0.0, 0.0));
            return parse_point_with_mode(
                &self.draft,
                origin,
                CoordinateMode::DynamicPointer {
                    default_relative: ask.base.is_some() && !settings.pointer_absolute,
                },
                reading,
            );
        }
        if !self.draft.is_empty() && parse_field(active, &self.draft, reading).is_none() {
            return None;
        }
        let point = match active {
            Field::X | Field::Y => {
                let shown = Point::new(
                    self.value(Field::X, ask, settings, reading)?,
                    self.value(Field::Y, ask, settings, reading)?,
                );
                if ask.base.is_some() && !settings.pointer_absolute {
                    let base = ask.base?;
                    let size = size_factor(ask);
                    let v = reading.ccs.vector_to_world(shown);
                    Point::new(base.x + v.x / size, base.y + v.y / size)
                } else {
                    reading.ccs.to_world(shown)
                }
            }
            Field::Distance | Field::Length | Field::Angle => {
                let base = ask.base?;
                let distance = self.value(Field::Distance, ask, settings, reading)?;
                let degrees = self.value(Field::Angle, ask, settings, reading)?;
                let angle = field_direction(degrees, false, reading);
                if distance != 0.0
                    && self.cursor == Some(base)
                    && !self.locks.contains_key(&Field::Angle)
                    && active != Field::Angle
                {
                    return None;
                }
                Point::new(
                    base.x + distance * angle.cos(),
                    base.y + distance * angle.sin(),
                )
            }
            _ => return None,
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(point)
    }

    pub fn submission(
        &self,
        ask: &Prompt,
        settings: &DynamicInputSettings,
        reading: impl Into<Reading>,
    ) -> Result<String, &'static str> {
        let reading = reading.into();
        // A point goes back as its coordinates in the system, so it reads the
        // same way and the command line shows what the fields showed.
        let point_text = |p: Point| {
            let p = reading.ccs.from_world(p);
            format!("#{},{}", p.x, p.y)
        };
        if ask.wants != Wants::Point && self.draft.is_empty() {
            if let Some(value) = self.active.and_then(|f| self.locks.get(&f)) {
                // A locked angle is in degrees whatever the drawing's angle format.
                return Ok(if ask.wants == Wants::Angle {
                    format!("{value}d")
                } else {
                    value.to_string()
                });
            }
        }
        if let Some(point) = self.locked_coordinate.filter(|_| self.draft.is_empty()) {
            return Ok(point_text(point));
        }
        if self.draft.is_empty() && self.locks.is_empty() {
            return Ok(String::new());
        }
        if ask.wants == Wants::Point
            && self.active.is_some_and(|f| {
                matches!(
                    f,
                    Field::X | Field::Y | Field::Distance | Field::Length | Field::Angle
                )
            })
            && (self
                .active
                .is_some_and(|f| parse_field(f, &self.draft, reading).is_some())
                || (self.draft.is_empty() && !self.locks.is_empty()))
        {
            let point = self
                .constrained_point(ask, settings, reading)
                .ok_or("Move the cursor to set a direction first.")?;
            return Ok(point_text(point));
        }
        Ok(self.draft.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn line_locks_then_submits_once_through_shared_command_path() {
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(110.0, 200.0));
        input.begin(Field::Distance);
        input.draft = "10".into();
        input
            .cycle(&[Field::Distance, Field::Angle], false, &Units::default())
            .unwrap();
        input.draft = "90".into();
        let text = input
            .submission(&ask, &settings, &Units::default())
            .unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        assert_eq!(state.session().document().len(), 1);
        let end = state.docs[0].view.last_point().unwrap();
        assert!((end.x - 100.0).abs() < 1e-9 && (end.y - 210.0).abs() < 1e-9);
        assert!(state.docs[0].dynamic_input.locks.is_empty());
        assert!(state.cmdline.log.last().unwrap().contains("#100,210"));
    }

    #[test]
    fn invalid_submission_preserves_prompt_and_geometry() {
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        assert!(state.answer_dynamic("NaN,3").is_err());
        assert!(state.command_running());
        assert!(state.session().document().is_empty());
        state.answer_dynamic("0,0").unwrap();
        state.settle_command();
        state.answer_dynamic("10<90").unwrap();
        state.settle_command();
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn rectangle_fields_use_cartesian_locks_even_with_polar_default() {
        let mut state = AppState::new();
        state.start_command("Rectangle", crate::rectangle::draw_rectangle);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(150.0, 250.0));
        assert_eq!(input.fields(&ask, &settings), [Field::X, Field::Y]);
        input.begin(Field::X);
        input.draft = "10".into();
        input
            .cycle(&[Field::X, Field::Y], false, &Units::default())
            .unwrap();
        input.draft = "20".into();
        let text = input
            .submission(&ask, &settings, &Units::default())
            .unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        assert_eq!(
            state.docs[0].view.last_point(),
            Some(Point::new(110.0, 220.0))
        );
        assert_eq!(state.session().document().len(), 1);
        assert!(!state.command_running());
    }

    #[test]
    fn a_rectangle_from_its_centre_takes_its_full_width_and_height() {
        let mut state = AppState::new();
        let draw = crate::rect_variants::draw_rectangle_center;
        state.start_command("Rectangle", draw);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(150.0, 230.0));
        assert_eq!(input.fields(&ask, &settings), [Field::X, Field::Y]);
        let units = Units::default();
        assert_eq!(input.value(Field::X, &ask, &settings, &units), Some(100.0));
        assert_eq!(input.value(Field::Y, &ask, &settings, &units), Some(60.0));
        input.begin(Field::X);
        input.draft = "10".into();
        input.cycle(&[Field::X, Field::Y], false, &units).unwrap();
        input.draft = "20".into();
        let text = input.submission(&ask, &settings, &units).unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        let objects = state.session().document().objects();
        assert_eq!(
            objects[0].entity,
            opendraft_core::Entity::Rectangle {
                min: Point::new(95.0, 190.0),
                max: Point::new(105.0, 210.0),
            }
        );
    }

    #[test]
    fn a_line_from_its_middle_takes_half_its_length_or_on_tab_the_whole() {
        let mut state = AppState::new();
        state.start_command("Line", crate::line_modes::draw_line_middle);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let units = Units::default();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(110.0, 200.0));
        let fields = [Field::Distance, Field::Length, Field::Angle];
        assert_eq!(input.fields(&ask, &settings), fields);
        assert_eq!(
            input.value(Field::Length, &ask, &settings, &units),
            Some(20.0)
        );
        input.begin(Field::Distance);
        input.draft = "10".into();
        input.cycle(&fields, false, &units).unwrap();
        input.draft = "30".into();
        // The whole length typed last counts, not the half locked before it.
        assert_eq!(
            input.value(Field::Distance, &ask, &settings, &units),
            Some(15.0)
        );
        input.cycle(&fields, false, &units).unwrap();
        assert!(!input.locks.contains_key(&Field::Distance));
        let text = input.submission(&ask, &settings, &units).unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        assert_eq!(
            state.session().document().objects()[0].entity,
            opendraft_core::Entity::Line {
                start: Point::new(85.0, 200.0),
                end: Point::new(115.0, 200.0),
            }
        );
    }

    #[test]
    fn tab_locked_scalar_still_submits_to_the_circle_command() {
        let mut state = AppState::new();
        state.start_command("Circle", crate::circle::draw_circle);
        state.settle_command();
        state.answer_dynamic("0,0").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.begin(Field::Distance);
        input.draft = "10".into();
        input
            .cycle(&[Field::Distance], false, &Units::default())
            .unwrap();
        let text = input
            .submission(&ask, &settings, &Units::default())
            .unwrap();
        state.answer_dynamic(&text).unwrap();
        state.settle_command();
        assert!(matches!(&state.session().document().objects()[0].entity,
            opendraft_core::Entity::Circle { radius, .. } if *radius == 10.0));
    }

    /// CIRCLE waiting for its centre, with `90` typed in the X field.
    fn circle_centre_with_x_typed(cursor: Point) -> AppState {
        let mut state = AppState::new();
        state.start_command("Circle", crate::circle::draw_circle);
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(cursor);
        assert_eq!(input.fields(&ask, &settings), [Field::X, Field::Y]);
        input.begin(Field::X);
        input.draft = "90".into();
        state
    }

    fn circle_centre(state: &AppState) -> Point {
        match &state.session().document().objects()[0].entity {
            opendraft_core::Entity::Circle { center, .. } => *center,
            other => panic!("not a circle: {other:?}"),
        }
    }

    #[test]
    fn a_number_in_the_x_field_follows_a_tracking_path() {
        let mut state = circle_centre_with_x_typed(Point::new(160.0, 50.0));
        // Following the horizontal path from a circle's centre at (100, 50).
        let mut snap = opendraft_canvas::SnapPoint::new(
            Point::new(160.0, 50.0),
            opendraft_canvas::SnapKind::Tracking,
        );
        snap.guides = [
            Some(opendraft_canvas::Guide {
                origin: Point::new(100.0, 50.0),
                dir: Point::new(1.0, 0.0),
                kind: Some(opendraft_canvas::SnapKind::Center),
            }),
            None,
        ];
        state.snap_now = Some(snap);
        state.commit_dynamic_input();
        state.settle_command();
        state.answer_dynamic("10").unwrap();
        state.settle_command();
        assert_eq!(circle_centre(&state), Point::new(190.0, 50.0));
    }

    #[test]
    fn a_number_in_the_x_field_with_no_path_stays_a_coordinate() {
        let mut state = circle_centre_with_x_typed(Point::new(5.0, 7.0));
        state.commit_dynamic_input();
        state.settle_command();
        state.answer_dynamic("10").unwrap();
        state.settle_command();
        assert_eq!(circle_centre(&state), Point::new(90.0, 7.0));
    }

    #[test]
    fn undo_first_line_point_does_not_undo_an_existing_object() {
        let mut state = AppState::new();
        state.perform(opendraft_commands::AddEntity::new(
            opendraft_core::Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(1.0, 0.0),
            },
        ));
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        state.answer_dynamic("100,200").unwrap();
        state.settle_command();
        let option = state.cmd_option_for("U").unwrap();
        state.cmd_choose(option);
        state.settle_command();
        assert!(state.waiting().unwrap().base.is_none());
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn angles_past_half_a_turn_read_as_negative() {
        let units = Units::default();
        assert_eq!(field_text(Field::Angle, -90.0, &units), "-90");
        assert_eq!(parse_field(Field::Angle, "-90", &units), Some(-90.0));
        let mut state = AppState::new();
        state.start_command("Line", crate::draw::draw_line);
        state.settle_command();
        state.answer_dynamic("0,0").unwrap();
        state.settle_command();
        let ask = state.waiting().unwrap().clone();
        let settings = state.settings.dynamic_input.clone();
        let input = &mut state.docs[0].dynamic_input;
        input.cursor = Some(Point::new(0.0, -10.0));
        let value = input.value(Field::Angle, &ask, &settings, &units).unwrap();
        assert!((value + 90.0).abs() < 1e-9);
    }
}

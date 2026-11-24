//! DIMORDINATE: the X or Y coordinate of a feature, on a leader, measured in
//! the coordinate system in force (its origin and axes).

use std::sync::Arc;

use opendraft_commands::{Cancelled, Editor, PointOptions};
use opendraft_core::dimension::suggested_x_datum;
use opendraft_core::Dimension;
use opendraft_ui::key;

use crate::dimension_cmd::{place, style, Make};

pub fn dim_ordinate(ed: &mut Editor) -> Result<(), Cancelled> {
    let style = style(ed)?;
    let frame = ed.read(|doc| doc.ccs())?;
    let feature = ed.get_point(PointOptions::new(key!("Specify feature location")))?;
    let make: Make = Arc::new(move |at, tweaks| {
        let x_datum = tweaks
            .datum
            .unwrap_or_else(|| suggested_x_datum(frame.from_world(feature), frame.from_world(at)));
        let mut dimension = Dimension::ordinate_in(frame, feature, at, x_datum, &style);
        dimension.text = tweaks.text.clone();
        dimension.measurement().map(|_| dimension)
    });
    place(
        ed,
        key!("Specify leader endpoint"),
        &[("Xdatum", "X"), ("Ydatum", "Y"), ("Text", "T")],
        feature,
        make,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use opendraft_core::{Entity, Point};

    fn dimension(state: &AppState) -> Dimension {
        state
            .session()
            .document()
            .objects()
            .iter()
            .find_map(|o| match &o.entity {
                Entity::Dimension(d) => Some((**d).clone()),
                _ => None,
            })
            .unwrap()
    }

    fn run(inputs: &[&str]) -> AppState {
        let mut state = AppState::new();
        state.start_command("DIMORDINATE", dim_ordinate);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
        state
    }

    #[test]
    fn a_leader_up_measures_x_and_one_across_measures_y() {
        let up = run(&["12,3", "12,20"]);
        assert_eq!(dimension(&up).label(), "12.00");
        let across = run(&["12,3", "30,3"]);
        assert_eq!(dimension(&across).label(), "3.00");
    }

    #[test]
    fn it_measures_from_the_custom_origin_along_its_axes() {
        use opendraft_commands::SetCcs;
        use opendraft_core::{Ccs, CcsSpace};
        let mut state = AppState::new();
        // Origin at (100, 50), X axis up the world's Y.
        let frame = Ccs::new(Point::new(100.0, 50.0), std::f64::consts::FRAC_PI_2);
        state.perform(SetCcs::new(CcsSpace::Model, frame));
        state.start_command("DIMORDINATE", dim_ordinate);
        state.settle_command();
        // Typed in the system: the feature at (12, 3), the leader along its Y.
        for input in ["12,3", "30,3"] {
            state.cmd_submit(input);
            state.settle_command();
        }
        let d = dimension(&state);
        assert!(
            (d.measurement().unwrap() - 3.0).abs() < 1e-9,
            "{:?}",
            d.measurement()
        );
        let opendraft_core::DimKind::Ordinate { frame: kept, .. } = d.kind else {
            panic!("an ordinate dimension");
        };
        assert_eq!(kept, frame);
    }

    #[test]
    fn the_datum_options_override_the_direction() {
        let forced = run(&["12,3", "y", "12,20"]);
        assert_eq!(dimension(&forced).measurement(), Some(3.0));
        let forced = run(&["12,3", "x", "30,3"]);
        assert_eq!(dimension(&forced).measurement(), Some(12.0));
        assert_eq!(dimension(&forced).points()[0], Point::new(12.0, 3.0));
    }
}

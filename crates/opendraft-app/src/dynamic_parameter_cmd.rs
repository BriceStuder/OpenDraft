//! The questions BPARAMETER asks for each kind of parameter: its points, and
//! then the name, grips and value sets every kind shares.

use opendraft_commands::{Cancelled, Editor, NumberOptions, PointOptions};
use opendraft_core::block::{DynamicBlock, DynamicParameter, DynamicParameterKind, ParameterId};

use crate::dynamic_block_prompts::{ask_grips, ask_line, ask_naming, ask_value_set, Quantity, EPS};

pub(super) fn linear_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let start = ed.get_point(PointOptions::new("Specify start point"))?;
    let end = loop {
        let end = ed.get_point(PointOptions::new("Specify endpoint").base(start))?;
        if (end.x - start.x).hypot(end.y - start.y) > EPS {
            break end;
        }
        ed.say("The parameter has no length.");
    };
    let length = (end.x - start.x).hypot(end.y - start.y);
    let named = ask_naming(ed, schema, "Distance", "distance")?;
    let grips = ask_grips(ed)?;
    let value_set = ask_value_set(ed, Quantity::Distance, length)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Linear {
            start,
            end,
            value_set,
        },
    })
}

pub(super) fn rotation_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let base = ed.get_point(PointOptions::new("Specify base point"))?;
    let radius = loop {
        let radius = ed.get_number(NumberOptions::new("Specify radius of parameter"))?;
        if radius.is_finite() && radius > EPS {
            break radius;
        }
        ed.say("The radius must be above zero.");
    };
    let named = ask_naming(ed, schema, "Angle", "angle")?;
    let grips = ask_grips(ed)?;
    let value_set = ask_value_set(ed, Quantity::Angle, 0.0)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Rotation {
            base,
            radius,
            value_set,
        },
    })
}

pub(super) fn polar_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let base = ed.get_point(PointOptions::new("Specify base point"))?;
    let end = loop {
        let end = ed.get_point(PointOptions::new("Specify endpoint").base(base))?;
        if (end.x - base.x).hypot(end.y - base.y) > EPS {
            break end;
        }
        ed.say("The parameter has no length.");
    };
    let named = ask_naming(ed, schema, "Distance", "distance")?;
    let angle_label = ask_line(ed, "Enter angle label", &free_angle(schema))?;
    let grips = ask_grips(ed)?;
    let length = (end.x - base.x).hypot(end.y - base.y);
    let distance_set = ask_value_set(ed, Quantity::Distance, length)?;
    let angle_set = ask_value_set(ed, Quantity::Angle, (end.y - base.y).atan2(end.x - base.x))?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Polar {
            base,
            end,
            distance_set,
            angle_set,
            angle_label,
        },
    })
}

/// `AngleN` for the polar parameter being added: one more than the polar
/// parameters there are.
fn free_angle(schema: &DynamicBlock) -> String {
    let polar = schema.parameters.iter().filter(|p| p.is_polar()).count();
    format!("Angle{}", polar + 1)
}

pub(super) fn flip_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let base = ed.get_point(PointOptions::new("Specify base point of reflection line"))?;
    let end = loop {
        let end =
            ed.get_point(PointOptions::new("Specify endpoint of reflection line").base(base))?;
        if (end.x - base.x).hypot(end.y - base.y) > EPS {
            break end;
        }
        ed.say("The reflection line has no length.");
    };
    let named = ask_naming(ed, schema, "Flip", "flip")?;
    let grips = ask_grips(ed)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Flip { base, end },
    })
}

pub(super) fn xy_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let base = ed.get_point(PointOptions::new("Specify base point"))?;
    let end = ed.get_point(PointOptions::new("Specify opposite corner").base(base))?;
    let named = ask_naming(ed, schema, "Distance", "horizontal distance")?;
    let polar_like = schema.parameters.iter().filter(|p| p.is_xy()).count() + 1;
    let y_label = ask_line(
        ed,
        "Enter vertical distance label",
        &format!("Y Distance{polar_like}"),
    )?;
    let grips = ask_grips(ed)?;
    let x_set = ask_value_set(ed, Quantity::Distance, end.x - base.x)?;
    let y_set = ask_value_set(ed, Quantity::Distance, end.y - base.y)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Xy {
            base,
            end,
            x_set,
            y_set,
            y_label,
        },
    })
}

pub(super) fn point_parameter(
    ed: &Editor,
    schema: &DynamicBlock,
) -> Result<DynamicParameter, Cancelled> {
    let position = ed.get_point(PointOptions::new("Specify parameter location"))?;
    let named = ask_naming(ed, schema, "Position", "position")?;
    let grips = ask_grips(ed)?;
    Ok(DynamicParameter {
        id: ParameterId(schema.next_parameter_id),
        name: named.name,
        label: named.label,
        description: named.description,
        show: named.show,
        grips,
        kind: DynamicParameterKind::Point { position },
    })
}

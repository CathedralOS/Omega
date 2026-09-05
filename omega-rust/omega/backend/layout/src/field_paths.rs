//! Spelled FIELD-PATH offset resolution -- the single source of truth for
//! receiver-path offset prediction. The contained-receiver fence
//! (emission planning) and per-instance dispatch resolution (instruction
//! selection) must agree on this walk BY CONSTRUCTION: both previously
//! carried private copies with a "keep in lockstep" warning; both now call
//! here.

use crate::{DataShape, FieldLayout, LayoutPlan, MachineLayout};
use arena::HandleSpan;
use checked_trees::name::Identifier;

/// The byte offset of a spelled field path (`["sum"]`, `["p", "second"]`)
/// within `fields_span`, descending contained machines and plain data
/// records alike. `None` when any hop does not resolve.
pub fn field_path_offset(
    layouts: &LayoutPlan,
    mut fields_span: HandleSpan<FieldLayout>,
    segments: &[Identifier],
) -> Option<usize> {
    if segments.is_empty() {
        return None;
    }
    let mut offset = 0usize;
    for (position, segment) in segments.iter().enumerate() {
        let field = layouts
            .fields
            .span(fields_span)?
            .iter()
            .find(|field| field.name == *segment)?;
        offset += field.offset;
        if position + 1 < segments.len() {
            // Descend the intermediate hop -- a contained sub-machine OR a
            // plain nested record (`p: PairD`). Data descent matches the
            // backend's storage walk so this prediction stays accurate.
            fields_span = field_machine_layout(layouts, field)
                .map(|machine_layout| machine_layout.fields)
                .or_else(|| field_data_layout_fields(layouts, field))?;
        }
    }
    Some(offset)
}

/// The machine layout a field's TYPE names (a contained sub-machine field).
pub fn field_machine_layout<'plan>(
    layouts: &'plan LayoutPlan,
    field: &FieldLayout,
) -> Option<&'plan MachineLayout> {
    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| {
            machine_layout.symbol == field.type_symbol
                || machine_layout.name.as_str() == field.type_name.as_ref()
                || machine_layout
                    .attached_data
                    .as_deref()
                    .is_some_and(|attached_data| attached_data == field.type_name.as_ref())
        })
        .map(|(_, machine_layout)| machine_layout)
}

/// The field span of a plain-DATA field's layout (`p: PairD` -> `PairD`'s
/// record fields / a case-bearing shape's common fields).
pub fn field_data_layout_fields(
    layouts: &LayoutPlan,
    field: &FieldLayout,
) -> Option<HandleSpan<FieldLayout>> {
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| {
            data_layout.symbol == field.type_symbol
                || data_layout.name.as_str() == field.type_name.as_ref()
        })
        .map(|(_, data_layout)| data_layout)?;
    match &data_layout.shape {
        DataShape::Record { fields } => Some(*fields),
        DataShape::Enum { common_fields, .. } => Some(*common_fields),
    }
}

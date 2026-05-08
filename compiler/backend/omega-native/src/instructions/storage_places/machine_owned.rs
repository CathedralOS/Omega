use super::expressions::normalized_storage_expression;
use super::nested_fields::{field_layout, resolve_nested_field_layout};
use crate::layout::{FieldLayout, LayoutPlan};
use omega_typed_program::expression::Expression;

pub(in crate::instructions) fn resolve_machine_owned_place(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    expression: &Expression,
) -> Option<(usize, usize)> {
    let normalized_expression = normalized_storage_expression(expression)?;
    let Expression::Name(path) = &normalized_expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };
    let (machine_base_offset, root_field) =
        root_machine_field_layout(layouts, entry_machine, source_machine, root_name)?;
    let (field_offset, field_layout) = resolve_nested_field_layout(layouts, root_field, suffix)?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

fn root_machine_field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_for_machine(layouts, entry_machine, source_machine, root_name)
        .or_else(|| {
            layouts
                .machine_layouts
                .iter()
                .find_map(|(_, machine_layout)| {
                    root_machine_field_layout_for_machine(
                        layouts,
                        entry_machine,
                        &machine_layout.name,
                        root_name,
                    )
                })
        })
}

fn root_machine_field_layout_for_machine<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = field_layout(layouts, machine_layout.fields, root_name)?;
    Some((machine_base_offset, root_field))
}

fn machine_storage_offset(
    layouts: &LayoutPlan,
    entry_machine: &str,
    source_machine: &str,
) -> Option<usize> {
    if entry_machine == source_machine {
        return Some(0);
    }

    let entry_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == entry_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    layouts
        .fields
        .span(entry_layout.fields)?
        .iter()
        .find(|field| field.type_name == source_machine)
        .map(|field| field.offset)
}

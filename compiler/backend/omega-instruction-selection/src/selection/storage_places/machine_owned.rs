use super::expressions::{normalized_storage_expression, normalized_storage_name_path_in_table};
use super::nested_fields::resolve_nested_field_layout;
use omega_core::symbols::SymbolHandle;
use omega_layout::{FieldLayout, LayoutPlan};
use omega_typed_trees::expression::{Expression, ExpressionHandle, ExpressionTable};

pub(in crate::selection) fn resolve_machine_owned_place(
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expression: &Expression,
) -> Option<(usize, usize)> {
    let normalized_expression = normalized_storage_expression(expression)?;
    let Expression::Name(path) = &normalized_expression else {
        return None;
    };
    let (machine_base_offset, root_field, suffix) =
        root_machine_field_layout_from_path(layouts, entry_machine, source_machine, path)?;
    let (field_offset, field_layout) = resolve_nested_field_layout(layouts, root_field, suffix)?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

pub(in crate::selection) fn resolve_machine_owned_place_in_table(
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(usize, usize)> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let (machine_base_offset, root_field, suffix) =
        root_machine_field_layout_from_path(layouts, entry_machine, source_machine, &path)?;
    let (field_offset, field_layout) = resolve_nested_field_layout(layouts, root_field, suffix)?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

fn root_machine_field_layout_from_path<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    path: &'path omega_typed_trees::expression::NamePath,
) -> Option<(usize, &'layout FieldLayout, &'path [omega_typed_trees::name::ProgramName])> {
    let [root_name, suffix @ ..] = path.as_slice() else {
        return None;
    };

    if root_name.as_str() == "self" {
        let [field_name, rest @ ..] = suffix else {
            return None;
        };
        let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
        let machine_layout = layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
            .map(|(_, machine_layout)| machine_layout)?;
        let root_field = layouts
            .fields
            .span(machine_layout.fields)?
            .iter()
            .find(|field| field.name.as_str() == field_name.as_str())?;
        return Some((machine_base_offset, root_field, rest));
    }

    let root_symbol = path.head_symbol();
    let (machine_base_offset, root_field) = root_machine_field_layout(
        layouts,
        entry_machine,
        source_machine,
        root_symbol,
        root_name,
    )?;
    Some((machine_base_offset, root_field, suffix))
}

fn root_machine_field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_for_machine(
        layouts,
        entry_machine,
        source_machine,
        root_symbol,
        root_name,
    )
}

fn root_machine_field_layout_for_machine<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_in_machine(
        layouts,
        entry_machine,
        source_machine,
        root_symbol,
        root_name,
    )
    .or_else(|| root_machine_field_layout_by_symbol(layouts, entry_machine, root_symbol, root_name))
}

fn root_machine_field_layout_in_machine<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = layouts
        .fields
        .span(machine_layout.fields)?
        .iter()
        .find(|field| {
            (root_symbol.is_valid() && field.symbol == root_symbol) || field.name.as_str() == root_name
        })?;
    Some((machine_base_offset, root_field))
}

fn root_machine_field_layout_by_symbol<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &str,
) -> Option<(usize, &'plan FieldLayout)> {
    layouts
        .machine_layouts
        .iter()
        .find_map(|(_, machine_layout)| {
            let machine_base_offset =
                machine_storage_offset(layouts, entry_machine, machine_layout.symbol)?;
            let root_field = layouts
                .fields
                .span(machine_layout.fields)?
                .iter()
                .find(|field| {
                    (root_symbol.is_valid() && field.symbol == root_symbol)
                        || field.name.as_str() == root_name
                })?;
            Some((machine_base_offset, root_field))
        })
}

fn machine_storage_offset(
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
) -> Option<usize> {
    if entry_machine == source_machine {
        return Some(0);
    }

    let entry_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == entry_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    layouts
        .fields
        .span(entry_layout.fields)?
        .iter()
        .find(|field| field.type_symbol == source_machine)
        .map(|field| field.offset)
}

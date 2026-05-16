use super::expressions::{
    StorageNamePath, normalized_storage_expression, normalized_storage_name_path_in_table,
};
use super::nested_fields::resolve_nested_field_layout_with_symbols;
use omega_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable, NamePath};
use omega_checked_trees::name::ProgramName;
use omega_core::symbols::SymbolHandle;
use omega_layout::{FieldLayout, LayoutPlan};

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
    let (machine_base_offset, root_field, suffix, suffix_start_index) =
        root_machine_field_layout_from_path(layouts, entry_machine, source_machine, path)?;
    let (field_offset, field_layout) =
        resolve_nested_field_layout_with_symbols(layouts, root_field, suffix, |index| {
            path.member_symbol(suffix_start_index + index)
        })?;

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
    let (machine_base_offset, root_field, suffix, suffix_start_index) =
        root_machine_field_layout_from_table_path(layouts, entry_machine, source_machine, &path)?;
    let (field_offset, field_layout) =
        resolve_nested_field_layout_with_symbols(layouts, root_field, suffix, |index| {
            path.member_symbol(suffix_start_index + index)
        })?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

fn root_machine_field_layout_from_table_path<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    path: &'path StorageNamePath<'_>,
) -> Option<(usize, &'layout FieldLayout, &'path [ProgramName], usize)> {
    root_machine_field_layout_from_parts(
        layouts,
        entry_machine,
        source_machine,
        path.members(),
        path.head_symbol(),
        path.member_symbol(1),
    )
}

fn root_machine_field_layout_from_path<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    path: &'path NamePath,
) -> Option<(usize, &'layout FieldLayout, &'path [ProgramName], usize)> {
    root_machine_field_layout_from_parts(
        layouts,
        entry_machine,
        source_machine,
        path.members(),
        path.head_symbol(),
        path.member_symbol(1),
    )
}

fn root_machine_field_layout_from_parts<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    members: &'path [ProgramName],
    root_symbol: SymbolHandle,
    self_field_symbol: SymbolHandle,
) -> Option<(usize, &'layout FieldLayout, &'path [ProgramName], usize)> {
    let [root_name, suffix @ ..] = members else {
        return None;
    };

    if root_symbol == source_machine {
        let [field_name, rest @ ..] = suffix else {
            return None;
        };
        let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
        let machine_layout = layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
            .map(|(_, machine_layout)| machine_layout)?;
        let root_field = field_layout_by_symbol_or_name(
            layouts,
            machine_layout.fields,
            self_field_symbol,
            field_name.as_str(),
        )?;
        return Some((machine_base_offset, root_field, rest, 2));
    }

    let (machine_base_offset, root_field) = root_machine_field_layout(
        layouts,
        entry_machine,
        source_machine,
        root_symbol,
        root_name,
    )?;
    Some((machine_base_offset, root_field, suffix, 1))
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
    let root_field =
        field_layout_by_symbol_or_name(layouts, machine_layout.fields, root_symbol, root_name)?;
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
            let root_field = field_layout_by_symbol_or_name(
                layouts,
                machine_layout.fields,
                root_symbol,
                root_name,
            )?;
            Some((machine_base_offset, root_field))
        })
}

fn field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    fields: omega_core::arena::HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
    field_name: &str,
) -> Option<&'plan FieldLayout> {
    layouts.fields.span(fields)?.iter().find(|field| {
        if field_symbol.is_valid() {
            field.symbol == field_symbol
        } else {
            field.name.as_str() == field_name
        }
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

    nested_machine_storage_offset(layouts, entry_layout, source_machine, 0)
}

fn nested_machine_storage_offset(
    layouts: &LayoutPlan,
    machine_layout: &omega_layout::MachineLayout,
    target_machine: SymbolHandle,
    base_offset: usize,
) -> Option<usize> {
    let fields = layouts.fields.span(machine_layout.fields)?;

    for field in fields {
        let field_offset = base_offset + field.offset;

        let nested_machine_layout =
            field_machine_layout(layouts, field.type_symbol, &field.type_name);

        if nested_machine_layout
            .is_some_and(|nested_machine_layout| nested_machine_layout.symbol == target_machine)
        {
            return Some(field_offset);
        }

        let Some(nested_machine_layout) = nested_machine_layout else {
            continue;
        };

        if let Some(offset) = nested_machine_storage_offset(
            layouts,
            nested_machine_layout,
            target_machine,
            field_offset,
        ) {
            return Some(offset);
        }
    }

    None
}

fn field_machine_layout<'plan>(
    layouts: &'plan LayoutPlan,
    type_symbol: SymbolHandle,
    type_name: &str,
) -> Option<&'plan omega_layout::MachineLayout> {
    if let Some(machine_layout) = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == type_symbol)
        .map(|(_, machine_layout)| machine_layout)
    {
        return Some(machine_layout);
    }

    let data_name = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.symbol == type_symbol)
        .map(|(_, data_layout)| data_layout.name.as_str())
        .unwrap_or(type_name);

    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name.as_str() == data_name)
        .map(|(_, machine_layout)| machine_layout)
}

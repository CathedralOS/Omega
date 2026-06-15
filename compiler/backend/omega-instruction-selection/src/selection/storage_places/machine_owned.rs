use super::expressions::{
    StorageNamePath, normalized_storage_expression, normalized_storage_name_path_in_table,
};
use super::nested_fields::{
    NestedFieldLayoutCursor, resolve_nested_field_layout_step,
    resolve_nested_field_layout_with_pairs, resolve_nested_field_layout_with_symbols,
};
use omega_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable, NamePath};
use omega_checked_trees::name::Identifier;
use omega_core::symbols::SymbolHandle;
use omega_layout::{FieldLayout, LayoutPlan, TypeLayout, TypeLayoutDescriptor};

#[derive(Clone)]
pub(in crate::selection) struct MachineOwnedCollectionTarget {
    pub(in crate::selection) byte_offset: usize,
    pub(in crate::selection) type_descriptor: TypeLayoutDescriptor,
}

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
    let (machine_base_offset, root_field, suffix_start_index) =
        root_machine_field_layout_from_table_path(layouts, entry_machine, source_machine, &path)?;
    let suffix = path.suffix(suffix_start_index);
    let (field_offset, field_layout) =
        resolve_nested_field_layout_with_pairs(layouts, root_field, suffix.iter())?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

pub(in crate::selection) fn resolve_machine_owned_collection_in_table(
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<MachineOwnedCollectionTarget> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let (machine_base_offset, root_field, suffix_start_index) =
        root_machine_field_layout_from_table_path(layouts, entry_machine, source_machine, &path)?;
    let mut cursor = NestedFieldLayoutCursor::from_root(root_field);
    for (field_name, field_symbol, field_index, case_variant) in
        path.suffix(suffix_start_index).iter()
    {
        cursor = resolve_nested_field_layout_step(
            layouts,
            cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }

    // The root field position in the path (at `suffix_start_index - 1`) may carry an
    // array element index (e.g. `self.vals[0]` → member 1 has index `Some(0)`).
    // When the suffix above is empty (no nested members after the root field),
    // `root_machine_field_layout_from_table_path` does not traverse that index and the
    // cursor still holds the ARRAY type descriptor. Apply the element descent here so
    // the returned descriptor is the scalar element type, not the array container.
    if suffix_start_index > 0 {
        if let Some(element_index) = path.member_index(suffix_start_index - 1) {
            if let Some((element_type, length)) = cursor.type_descriptor().fixed_array() {
                if element_index < length {
                    let element_layout = TypeLayout {
                        size: cursor.layout().size / length,
                        alignment: cursor.layout().alignment,
                    };
                    cursor = NestedFieldLayoutCursor::from_indexed_fixed_array_element(
                        cursor,
                        element_index,
                        element_type,
                        element_layout,
                    );
                }
            }
        }
    }

    Some(MachineOwnedCollectionTarget {
        byte_offset: machine_base_offset + cursor.byte_offset(),
        type_descriptor: cursor.type_descriptor().clone(),
    })
}

fn root_machine_field_layout_from_table_path<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    path: &'path StorageNamePath<'_>,
) -> Option<(usize, &'layout FieldLayout, usize)> {
    path.member(0)?;

    if table_path_targets_source_machine(path, source_machine) {
        let field_name = path.member(1)?;
        let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
        if let Some(machine_layout) = layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
            .map(|(_, machine_layout)| machine_layout)
            && let Some(root_field) = field_layout_by_symbol_or_name(
                layouts,
                machine_layout.fields,
                path.member_symbol(1),
                field_name,
            )
        {
            return Some((machine_base_offset, root_field, 2));
        }

        let (machine_base_offset, root_field) = machine_field_layout_by_symbol_or_name(
            layouts,
            entry_machine,
            path.member_symbol(1),
            field_name,
        )?;
        return Some((machine_base_offset, root_field, 2));
    }

    let root_name = path.member(0)?;
    let (machine_base_offset, root_field) = root_machine_field_layout(
        layouts,
        entry_machine,
        source_machine,
        path.head_symbol(),
        root_name,
    )?;
    Some((machine_base_offset, root_field, 1))
}

fn root_machine_field_layout_from_path<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    path: &'path NamePath,
) -> Option<(usize, &'layout FieldLayout, &'path [Identifier], usize)> {
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
    members: &'path [Identifier],
    root_symbol: SymbolHandle,
    self_field_symbol: SymbolHandle,
) -> Option<(usize, &'layout FieldLayout, &'path [Identifier], usize)> {
    let [_root_name, suffix @ ..] = members else {
        return None;
    };

    if path_targets_source_machine(root_symbol, members.first(), source_machine) {
        let [field_name, rest @ ..] = suffix else {
            return None;
        };
        let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
        if let Some(machine_layout) = layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
            .map(|(_, machine_layout)| machine_layout)
            && let Some(root_field) = field_layout_by_symbol_or_name(
                layouts,
                machine_layout.fields,
                self_field_symbol,
                field_name,
            )
        {
            return Some((machine_base_offset, root_field, rest, 2));
        }

        let (machine_base_offset, root_field) = machine_field_layout_by_symbol_or_name(
            layouts,
            entry_machine,
            self_field_symbol,
            field_name,
        )?;
        return Some((machine_base_offset, root_field, rest, 2));
    }

    let (machine_base_offset, root_field) = root_machine_field_layout(
        layouts,
        entry_machine,
        source_machine,
        root_symbol,
        &members[0],
    )?;
    Some((machine_base_offset, root_field, suffix, 1))
}

fn table_path_targets_source_machine(
    path: &StorageNamePath<'_>,
    source_machine: SymbolHandle,
) -> bool {
    path.head_symbol() == source_machine
        || path.member(0).is_some_and(|name| name.as_str() == "self")
}

fn path_targets_source_machine(
    root_symbol: SymbolHandle,
    root_name: Option<&Identifier>,
    source_machine: SymbolHandle,
) -> bool {
    root_symbol == source_machine || root_name.is_some_and(|name| name.as_str() == "self")
}

fn root_machine_field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &Identifier,
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
    root_name: &Identifier,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_in_machine(
        layouts,
        entry_machine,
        source_machine,
        root_symbol,
        root_name,
    )
    .or_else(|| {
        root_machine_field_layout_by_symbol_or_name(layouts, entry_machine, root_symbol, root_name)
    })
}

fn root_machine_field_layout_in_machine<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &Identifier,
) -> Option<(usize, &'plan FieldLayout)> {
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field =
        field_layout_by_symbol_or_name(layouts, machine_layout.fields, root_symbol, root_name)?;
    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    Some((machine_base_offset, root_field))
}

fn root_machine_field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &Identifier,
) -> Option<(usize, &'plan FieldLayout)> {
    layouts
        .machine_layouts
        .iter()
        .find_map(|(_, machine_layout)| {
            let root_field = field_layout_by_symbol_or_name(
                layouts,
                machine_layout.fields,
                root_symbol,
                root_name,
            )?;
            let machine_base_offset =
                machine_storage_offset(layouts, entry_machine, machine_layout.symbol)?;
            Some((machine_base_offset, root_field))
        })
}

fn machine_field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    entry_machine: SymbolHandle,
    field_symbol: SymbolHandle,
    field_name: &Identifier,
) -> Option<(usize, &'plan FieldLayout)> {
    if field_symbol.is_valid()
        && let Some(layout) = layouts
            .machine_layouts
            .iter()
            .find_map(|(_, machine_layout)| {
                let root_field = layouts
                    .fields
                    .span(machine_layout.fields)?
                    .iter()
                    .find(|field| field.symbol == field_symbol)?;
                let machine_base_offset =
                    machine_storage_offset(layouts, entry_machine, machine_layout.symbol)?;
                Some((machine_base_offset, root_field))
            })
    {
        return Some(layout);
    }

    layouts
        .machine_layouts
        .iter()
        .find_map(|(_, machine_layout)| {
            let root_field = layouts
                .fields
                .span(machine_layout.fields)?
                .iter()
                .find(|field| field.name == *field_name)?;
            let machine_base_offset =
                machine_storage_offset(layouts, entry_machine, machine_layout.symbol)?;
            Some((machine_base_offset, root_field))
        })
}

fn field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    fields: omega_core::arena::HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
    field_name: &Identifier,
) -> Option<&'plan FieldLayout> {
    let fields = layouts.fields.span(fields)?;
    fields
        .iter()
        .find(|field| field_symbol.is_valid() && field.symbol == field_symbol)
        .or_else(|| fields.iter().find(|field| field.name == *field_name))
}

fn machine_storage_offset(
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
) -> Option<usize> {
    if entry_machine == source_machine {
        return Some(0);
    }

    let source_attached_data = source_attached_data_name(layouts, source_machine);

    // A free machine that operates on the SAME data instance as the entry machine
    // -- e.g. `machine Main::outer(&mut self)` whose `self` IS the entry's `Main`
    // data -- shares the entry machine-owned region at offset 0; it is not a nested
    // field of the entry, so the field search below would not find it. Detect this
    // by matching their attached-data type. (Embedded sub-machine instances, like a
    // `store: Store` field, have a DIFFERENT attached data and resolve via the
    // nested search.)
    if source_attached_data.is_some()
        && source_attached_data == source_attached_data_name(layouts, entry_machine)
    {
        return Some(0);
    }

    let entry_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == entry_machine)
        .map(|(_, machine_layout)| machine_layout)?;

    nested_machine_storage_offset(
        layouts,
        entry_layout,
        source_machine,
        source_attached_data,
        0,
        &mut Vec::new(),
    )
}

fn source_attached_data_name(layouts: &LayoutPlan, source_machine: SymbolHandle) -> Option<&str> {
    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        .and_then(|(_, machine_layout)| machine_layout.attached_data.as_ref())
        .map(|attached_data| attached_data.as_str())
}

fn nested_machine_storage_offset(
    layouts: &LayoutPlan,
    machine_layout: &omega_layout::MachineLayout,
    target_machine: SymbolHandle,
    target_attached_data: Option<&str>,
    base_offset: usize,
    visited: &mut Vec<SymbolHandle>,
) -> Option<usize> {
    if visited.contains(&machine_layout.symbol) {
        return None;
    }
    visited.push(machine_layout.symbol);

    let Some(fields) = layouts.fields.span(machine_layout.fields) else {
        visited.pop();
        return None;
    };

    for field in fields {
        let field_offset = base_offset + field.offset;

        if target_attached_data.is_some_and(|name| field.type_name.as_ref() == name) {
            visited.pop();
            return Some(field_offset);
        }

        let nested_machine_layout = field_machine_layout(layouts, field);

        if nested_machine_layout
            .is_some_and(|nested_machine_layout| nested_machine_layout.symbol == target_machine)
        {
            visited.pop();
            return Some(field_offset);
        }

        let Some(nested_machine_layout) = nested_machine_layout else {
            continue;
        };

        if let Some(offset) = nested_machine_storage_offset(
            layouts,
            nested_machine_layout,
            target_machine,
            target_attached_data,
            field_offset,
            visited,
        ) {
            visited.pop();
            return Some(offset);
        }
    }

    visited.pop();
    None
}

fn field_machine_layout<'plan>(
    layouts: &'plan LayoutPlan,
    field: &FieldLayout,
) -> Option<&'plan omega_layout::MachineLayout> {
    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| {
            machine_layout.symbol == field.type_symbol
                || machine_layout.name.as_str() == field.type_name.as_ref()
                || machine_layout
                    .attached_data
                    .as_ref()
                    .is_some_and(|attached_data| attached_data.as_str() == field.type_name.as_ref())
        })
        .map(|(_, machine_layout)| machine_layout)
}

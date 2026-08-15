use super::expressions::{
    StorageNamePath, normalized_storage_expression, normalized_storage_name_path_in_table,
};
use super::model::{
    RuntimeBitFieldPlace, RuntimeStoragePlace, RuntimeStoredIntegerProjection,
    RuntimeStoredIntegerSource,
};
use super::nested_fields::{
    NestedFieldLayoutCursor, resolve_nested_field_layout_step,
    resolve_nested_field_layout_with_pairs, resolve_nested_field_layout_with_symbols,
    resolve_nested_stored_integer_layout_step,
};
use omega_abstract_operations::RuntimeStorageRegion;
use omega_layout::{
    DataShape, ENUM_TAG_BYTES, FieldLayout, LayoutPlan, TypeLayout, TypeLayoutDescriptor,
};
use psi_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable, NamePath};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

#[derive(Clone)]
pub(in crate::selection) struct MachineOwnedCollectionTarget {
    pub(in crate::selection) byte_offset: usize,
    pub(in crate::selection) type_descriptor: TypeLayoutDescriptor,
    pub(in crate::selection) element_stride: Option<usize>,
}

pub(in crate::selection) fn resolve_machine_owned_place(
    layouts: &LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expression: &Expression,
) -> Option<(usize, usize)> {
    let normalized_expression = normalized_storage_expression(expression)?;
    let Expression::Name(path) = &normalized_expression else {
        return None;
    };
    let (machine_base_offset, root_field, suffix, suffix_start_index) =
        root_machine_field_layout_from_path(
            layouts,
            input,
            dispatch_index,
            entry_machine,
            source_machine,
            path,
        )?;
    let (field_offset, field_layout) =
        resolve_nested_field_layout_with_symbols(layouts, root_field, suffix, |index| {
            path.member_symbol(suffix_start_index + index)
        })?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

/// A BARE `self` GUARD subject in a machine attached to CASE-BEARING data
/// (`machine Verdict::is_yes(&self) { transition self { Verdict::Yes -> .. } }`)
/// resolves to the attached value's TAG word: `DataShape::Enum` fixes the i32
/// tag at offset 0 of every case-bearing value, so the case compare is an
/// `ENUM_TAG_BYTES` read at the machine's storage base. GUARD-COMPARE USE
/// ONLY -- a whole-value read through this place would drop common/payload
/// bytes, so it is deliberately NOT part of the general place resolver.
/// Which instance the base resolves to is the by-type walk; a receiver naming
/// a non-first same-type field is already rejected by the contained-receiver
/// blocker in emission planning.
pub(in crate::selection) fn resolve_machine_owned_self_case_tag_place_in_table(
    layouts: &LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.len() != 1 || !table_path_targets_source_machine(&path, source_machine) {
        return None;
    }
    let attached_data = source_attached_data_name(layouts, source_machine)?;
    let is_case_bearing = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name.as_str() == attached_data)
        .is_some_and(|(_, data_layout)| matches!(data_layout.shape, DataShape::Enum { .. }));
    if !is_case_bearing {
        return None;
    }
    let machine_base_offset = resolved_machine_base(
        input,
        dispatch_index,
        layouts,
        entry_machine,
        source_machine,
    )?;
    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::Machine,
        byte_offset: machine_base_offset,
        byte_count: ENUM_TAG_BYTES,
    })
}

pub(in crate::selection) fn resolve_machine_owned_place_in_table(
    layouts: &LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(usize, usize)> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let (machine_base_offset, root_field, suffix_start_index) =
        root_machine_field_layout_from_table_path(
            layouts,
            input,
            dispatch_index,
            entry_machine,
            source_machine,
            &path,
        )?;
    let suffix = path.suffix(suffix_start_index);
    let (field_offset, field_layout) =
        resolve_nested_field_layout_with_pairs(layouts, root_field, suffix.iter())?;

    Some((machine_base_offset + field_offset, field_layout.size))
}

pub(in crate::selection) fn resolve_machine_owned_bit_field_in_table(
    layouts: &LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeBitFieldPlace> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let (machine_base_offset, root_field, suffix_start_index) =
        root_machine_field_layout_from_table_path(
            layouts,
            input,
            dispatch_index,
            entry_machine,
            source_machine,
            &path,
        )?;
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
    let (containing_byte_offset, bit_field) = cursor.bit_field()?;
    Some(RuntimeBitFieldPlace {
        region: RuntimeStorageRegion::Machine,
        base_byte_offset: machine_base_offset.checked_add(containing_byte_offset)?,
        value_byte_count: cursor.layout().size,
        fragments: bit_field.fragments.clone(),
    })
}

pub(in crate::selection) fn resolve_machine_owned_stored_integer_in_table(
    layouts: &LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoredIntegerProjection> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let (machine_base_offset, root_field, suffix_start_index) =
        root_machine_field_layout_from_table_path(
            layouts,
            input,
            dispatch_index,
            entry_machine,
            source_machine,
            &path,
        )?;
    let mut cursor = NestedFieldLayoutCursor::from_root(root_field);
    for (field_name, field_symbol, field_index, case_variant) in
        path.suffix(suffix_start_index).iter()
    {
        cursor = resolve_nested_stored_integer_layout_step(
            layouts,
            cursor,
            field_name,
            field_symbol,
            field_index,
            case_variant,
        )?;
    }
    let stored = cursor.stored_integer()?;
    let stored_byte_count = usize::from(stored.stored_width_bits.checked_div(8)?);
    if stored_byte_count == 0 || stored.stored_width_bits % 8 != 0 {
        return None;
    }
    let carrier = super::descriptor_primitive_type(cursor.type_descriptor())?;
    Some(RuntimeStoredIntegerProjection {
        source: RuntimeStoredIntegerSource::Direct {
            region: RuntimeStorageRegion::Machine,
            byte_offset: machine_base_offset.checked_add(cursor.byte_offset())?,
        },
        stored_byte_count,
        carrier_byte_count: cursor.layout().size,
        interpretation: stored.interpretation,
        carrier_signed: carrier.is_signed_integer(),
        write_is_total: stored.write_is_total,
    })
}

pub(in crate::selection) fn resolve_machine_owned_collection_in_table(
    layouts: &LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<MachineOwnedCollectionTarget> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let (machine_base_offset, root_field, suffix_start_index) =
        root_machine_field_layout_from_table_path(
            layouts,
            input,
            dispatch_index,
            entry_machine,
            source_machine,
            &path,
        )?;
    let mut cursor = NestedFieldLayoutCursor::from_root(root_field);

    // The root field position in the path (at `suffix_start_index - 1`) may carry an
    // array element index (e.g. `self.vals[0]` → member 1 has index `Some(0)`).
    // `root_machine_field_layout_from_table_path` resolves the FIELD but never
    // traverses that index, so apply the element descent BEFORE the suffix walk:
    // the suffix names fields of the ELEMENT (`rows[2].data` -- `data` is a field
    // of `Row`, not of `[Row; 3]`), and the index must scale by the ELEMENT size.
    // (It used to be applied AFTER the walk, at the leaf's scale -- correct only
    // for an empty suffix; `rows[2].data` resolved to rows+2*4 instead of
    // rows+2*8, a silent wrong-element write once nested lowering engaged.)
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

    Some(MachineOwnedCollectionTarget {
        byte_offset: machine_base_offset + cursor.byte_offset(),
        type_descriptor: cursor.type_descriptor().clone(),
        element_stride: cursor.repeated_element_stride(),
    })
}

fn root_machine_field_layout_from_table_path<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    path: &'path StorageNamePath<'_>,
) -> Option<(usize, &'layout FieldLayout, usize)> {
    path.member(0)?;

    if table_path_targets_source_machine(path, source_machine) {
        let field_name = path.member(1)?;
        let machine_base_offset = resolved_machine_base(
            input,
            dispatch_index,
            layouts,
            entry_machine,
            source_machine,
        )?;
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
            input,
            dispatch_index,
            entry_machine,
            path.member_symbol(1),
            field_name,
        )?;
        return Some((machine_base_offset, root_field, 2));
    }

    let root_name = path.member(0)?;
    let (machine_base_offset, root_field) = root_machine_field_layout(
        layouts,
        input,
        dispatch_index,
        entry_machine,
        source_machine,
        path.head_symbol(),
        root_name,
    )?;
    Some((machine_base_offset, root_field, 1))
}

fn root_machine_field_layout_from_path<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    path: &'path NamePath,
) -> Option<(usize, &'layout FieldLayout, &'path [Identifier], usize)> {
    root_machine_field_layout_from_parts(
        layouts,
        input,
        dispatch_index,
        entry_machine,
        source_machine,
        path.members(),
        path.head_symbol(),
        path.member_symbol(1),
    )
}

fn root_machine_field_layout_from_parts<'path, 'layout>(
    layouts: &'layout LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
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
        let machine_base_offset = resolved_machine_base(
            input,
            dispatch_index,
            layouts,
            entry_machine,
            source_machine,
        )?;
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
            input,
            dispatch_index,
            entry_machine,
            self_field_symbol,
            field_name,
        )?;
        return Some((machine_base_offset, root_field, rest, 2));
    }

    let (machine_base_offset, root_field) = root_machine_field_layout(
        layouts,
        input,
        dispatch_index,
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
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &Identifier,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_for_machine(
        layouts,
        input,
        dispatch_index,
        entry_machine,
        source_machine,
        root_symbol,
        root_name,
    )
}

fn root_machine_field_layout_for_machine<'plan>(
    layouts: &'plan LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &Identifier,
) -> Option<(usize, &'plan FieldLayout)> {
    root_machine_field_layout_in_machine(
        layouts,
        input,
        dispatch_index,
        entry_machine,
        source_machine,
        root_symbol,
        root_name,
    )
    .or_else(|| {
        root_machine_field_layout_by_symbol_or_name(
            layouts,
            input,
            dispatch_index,
            entry_machine,
            root_symbol,
            root_name,
        )
    })
}

fn root_machine_field_layout_in_machine<'plan>(
    layouts: &'plan LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
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
    let machine_base_offset = resolved_machine_base(
        input,
        dispatch_index,
        layouts,
        entry_machine,
        source_machine,
    )?;
    Some((machine_base_offset, root_field))
}

fn root_machine_field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
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
            let machine_base_offset = resolved_machine_base(
                input,
                dispatch_index,
                layouts,
                entry_machine,
                machine_layout.symbol,
            )?;
            Some((machine_base_offset, root_field))
        })
}

fn machine_field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
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
                let machine_base_offset = resolved_machine_base(
                    input,
                    dispatch_index,
                    layouts,
                    entry_machine,
                    machine_layout.symbol,
                )?;
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
            let machine_base_offset = resolved_machine_base(
                input,
                dispatch_index,
                layouts,
                entry_machine,
                machine_layout.symbol,
            )?;
            Some((machine_base_offset, root_field))
        })
}

fn field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    fields: psi_arena::HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
    field_name: &Identifier,
) -> Option<&'plan FieldLayout> {
    let fields = layouts.fields.span(fields)?;
    fields
        .iter()
        .find(|field| field_symbol.is_valid() && field.symbol == field_symbol)
        .or_else(|| fields.iter().find(|field| field.name == *field_name))
}

/// The callee's machine-storage base: the PER-INSTANCE receiver base when
/// the dispatch context minted one (per-instance dispatch, TASKS_FS "Stolen
/// work #2"), else the historical first-type-match walk. The entry machine
/// itself never carries an override (its base is 0 either way).
fn resolved_machine_base(
    input: &crate::InstructionSelectionInput<'_>,
    dispatch_index: u32,
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    source_machine: SymbolHandle,
) -> Option<usize> {
    if entry_machine != source_machine
        && let Some(base) = crate::selection::receiver_base::receiver_base_for(
            input,
            dispatch_index,
            source_machine,
        )
    {
        return Some(base);
    }
    machine_storage_offset(layouts, entry_machine, source_machine)
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
    let offset = nested_field_span_storage_offset(
        layouts,
        machine_layout.fields,
        target_machine,
        target_attached_data,
        base_offset,
        visited,
    );
    visited.pop();
    offset
}

/// Search a FIELD SPAN (a machine's or a nested data's fields) for the target
/// machine's storage region, descending BOTH nested machine-typed fields (a
/// contained sub-machine) AND nested plain-DATA fields (`p: PairD`). The data
/// descent is what lets a NESTED receiver `self.p.a.method()` -- whose
/// intermediate `p` is a plain record with no attached machine -- resolve the
/// callee's `self` base; without it the by-type walk stopped at the first
/// machine-typed hop. Cycle guard is on machine symbols only (plain data is
/// acyclic by construction -- a by-value self-containing record has no finite
/// layout).
fn nested_field_span_storage_offset(
    layouts: &LayoutPlan,
    fields_span: psi_arena::HandleSpan<FieldLayout>,
    target_machine: SymbolHandle,
    target_attached_data: Option<&str>,
    base_offset: usize,
    visited: &mut Vec<SymbolHandle>,
) -> Option<usize> {
    let fields = layouts.fields.span(fields_span)?;

    for field in fields {
        let field_offset = base_offset + field.offset;

        if target_attached_data.is_some_and(|name| field.type_name.as_ref() == name) {
            return Some(field_offset);
        }

        if let Some(nested_machine_layout) = field_machine_layout(layouts, field) {
            if nested_machine_layout.symbol == target_machine {
                return Some(field_offset);
            }
            if let Some(offset) = nested_machine_storage_offset(
                layouts,
                nested_machine_layout,
                target_machine,
                target_attached_data,
                field_offset,
                visited,
            ) {
                return Some(offset);
            }
            continue;
        }

        if let Some(data_fields) = field_data_layout_fields(layouts, field)
            && let Some(offset) = nested_field_span_storage_offset(
                layouts,
                data_fields,
                target_machine,
                target_attached_data,
                field_offset,
                visited,
            )
        {
            return Some(offset);
        }
    }

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

/// The field span of a plain-DATA field's layout (`p: PairD` -> `PairD`'s
/// record fields, or a case-bearing shape's common fields). `None` when the
/// field is not a data type this plan lays out. Lets the storage walk descend
/// nested records to reach a machine-attached leaf.
fn field_data_layout_fields(
    layouts: &LayoutPlan,
    field: &FieldLayout,
) -> Option<psi_arena::HandleSpan<FieldLayout>> {
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

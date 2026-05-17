use crate::StateGuardOperandStorage;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::name::ProgramName;
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout};
use omega_runtime_storage::RuntimeStoragePlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedOperandLayout {
    pub storage: StateGuardOperandStorage,
    pub byte_offset: usize,
    pub layout: TypeLayout,
}

pub(super) fn resolve_guard_operand_layout(
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    entry_machine: SymbolHandle,
    source_key: StateKey,
    source_machine: SymbolHandle,
    source_dispatch_index: u32,
    statement_index: usize,
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<ResolvedOperandLayout> {
    if matches!(table.expression(expression), ExpressionNode::Call(_))
        && let Some(slot) = runtime_storage.transition_guard_result_slot(
            source_dispatch_index,
            source_key,
            statement_index,
        )
    {
        return Some(ResolvedOperandLayout {
            storage: StateGuardOperandStorage::RuntimeFrame,
            byte_offset: slot.byte_offset,
            layout: TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        });
    }

    let path = normalized_guard_name_path(table, expression)?;
    let root_symbol = path.head_symbol();
    if path.is_empty() {
        return None;
    }
    let suffix = path.suffix(1);

    if let Some(slot_layout) = runtime_frame_operand_layout(
        layouts,
        runtime_storage,
        source_key,
        source_dispatch_index,
        &path,
        suffix,
    ) {
        return Some(slot_layout);
    }

    if root_symbol == source_machine {
        if suffix.is_empty() {
            return None;
        }
        let field_symbol = path.member_symbol(1);
        let rest = path.suffix(2);
        if let Some((_, machine_layout)) = layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        {
            if let Some(machine_base_offset) =
                machine_storage_offset(layouts, entry_machine, source_machine)
            {
                if let Some(root_field) =
                    field_layout_by_symbol(layouts, machine_layout.fields, field_symbol)
                {
                    return resolve_nested_field_layout_with_symbols(layouts, root_field, rest)
                        .map(|(byte_offset, layout)| ResolvedOperandLayout {
                            storage: StateGuardOperandStorage::MachineOwned,
                            byte_offset: machine_base_offset + byte_offset,
                            layout,
                        });
                }
            }
        }

        return layouts
            .machine_layouts
            .iter()
            .find_map(|(_, candidate_layout)| {
                let candidate_base_offset =
                    machine_storage_offset(layouts, entry_machine, candidate_layout.symbol)?;
                let root_field =
                    field_layout_by_symbol(layouts, candidate_layout.fields, field_symbol)?;

                resolve_nested_field_layout_with_symbols(layouts, root_field, rest).map(
                    |(byte_offset, layout)| ResolvedOperandLayout {
                        storage: StateGuardOperandStorage::MachineOwned,
                        byte_offset: candidate_base_offset + byte_offset,
                        layout,
                    },
                )
            });
    }

    let machine_base_offset = machine_storage_offset(layouts, entry_machine, source_machine)?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field = field_layout_by_symbol(layouts, machine_layout.fields, root_symbol);

    if let Some(root_field) = root_field {
        return resolve_nested_field_layout_with_symbols(layouts, root_field, suffix).map(
            |(byte_offset, layout)| ResolvedOperandLayout {
                storage: StateGuardOperandStorage::MachineOwned,
                byte_offset: machine_base_offset + byte_offset,
                layout,
            },
        );
    }

    fallback_machine_named_path_layout(layouts, entry_machine, root_symbol, &path).map(
        |(byte_offset, layout)| ResolvedOperandLayout {
            storage: StateGuardOperandStorage::MachineOwned,
            byte_offset,
            layout,
        },
    )
}

fn runtime_frame_operand_layout(
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    source_key: StateKey,
    source_dispatch_index: u32,
    path: &NormalizedGuardNamePath<'_>,
    suffix: GuardPathSuffix<'_, '_>,
) -> Option<ResolvedOperandLayout> {
    let root_symbol = path.head_symbol();
    let slot_matches_symbol = |slot: &omega_runtime_storage::RuntimeFrameSlot| {
        root_symbol.is_valid() && slot.symbol == root_symbol
    };
    let same_state_without_segment = |slot_key: StateKey| {
        slot_key.machine == source_key.machine && slot_key.state == source_key.state
    };

    let slot = runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == source_dispatch_index
                && slot.source_key == source_key
                && slot_matches_symbol(slot))
            .then_some(slot)
        })
        .or_else(|| {
            runtime_storage.frame_slots.iter().find_map(|(_, slot)| {
                (same_state_without_segment(slot.source_key) && slot_matches_symbol(slot))
                    .then_some(slot)
            })
        })
        .or_else(|| {
            runtime_storage.frame_slots.iter().find_map(|(_, slot)| {
                (slot.source_key.machine == source_key.machine && slot_matches_symbol(slot))
                    .then_some(slot)
            })
        })?;

    let (byte_offset, layout) = if suffix.is_empty() {
        (
            slot.byte_offset,
            TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        )
    } else {
        resolve_nested_slot_layout(
            layouts,
            slot.byte_offset,
            slot.type_symbol,
            &slot.type_name,
            suffix,
        )?
    };

    Some(ResolvedOperandLayout {
        storage: StateGuardOperandStorage::RuntimeFrame,
        byte_offset,
        layout,
    })
}

fn resolve_nested_slot_layout(
    layouts: &LayoutPlan,
    root_byte_offset: usize,
    root_type_symbol: SymbolHandle,
    root_type_name: &str,
    suffix: GuardPathSuffix<'_, '_>,
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_byte_offset;
    let mut type_symbol = root_type_symbol;
    let mut type_name = root_type_name;
    let mut layout = TypeLayout {
        size: 0,
        alignment: 1,
    };

    for (segment, field_symbol, field_index) in suffix.iter() {
        let field_segment = parse_field_segment(segment, field_index)?;
        let fields = record_fields(layouts, type_symbol, type_name);
        let field = field_layout_by_symbol(layouts, fields, field_symbol)?;
        byte_offset += field.offset;
        type_symbol = field.type_symbol;
        type_name = &field.type_name;
        layout = field.layout;

        if let Some(index) = field_segment.index {
            let array = parse_array_type_name(type_name)?;
            if index >= array.length {
                return None;
            }
            let element_size = field.layout.size / array.length;
            byte_offset += element_size * index;
            type_name = array.element_type_name;
            layout = TypeLayout {
                size: element_size,
                alignment: field.layout.alignment,
            };
        }
    }

    if layout.size == 0 {
        None
    } else {
        Some((byte_offset, layout))
    }
}

fn normalized_guard_name_path<'table>(
    table: &'table ExpressionTable,
    expression: ExpressionHandle,
) -> Option<NormalizedGuardNamePath<'table>> {
    guard_path_len(table, expression).map(|_| NormalizedGuardNamePath { table, expression })
}

struct NormalizedGuardNamePath<'table> {
    table: &'table ExpressionTable,
    expression: ExpressionHandle,
}

impl<'table> NormalizedGuardNamePath<'table> {
    fn head_symbol(&self) -> SymbolHandle {
        guard_path_head_symbol(self.table, self.expression)
    }

    fn len(&self) -> usize {
        guard_path_len(self.table, self.expression).unwrap_or(0)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn member(&self, index: usize) -> Option<&ProgramName> {
        guard_path_member(self.table, self.expression, index)
    }

    fn member_symbol(&self, index: usize) -> SymbolHandle {
        guard_path_member_symbol(self.table, self.expression, index)
    }

    fn member_index(&self, index: usize) -> Option<usize> {
        guard_path_member_index(self.table, self.expression, index)
    }

    fn suffix(&self, start: usize) -> GuardPathSuffix<'_, 'table> {
        GuardPathSuffix { path: self, start }
    }
}

fn guard_path_head_symbol(table: &ExpressionTable, expression: ExpressionHandle) -> SymbolHandle {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => guard_path_head_symbol(table, *target),
        ExpressionNode::Indexed(indexed) => guard_path_head_symbol(table, indexed.collection),
        ExpressionNode::Member(member) => guard_path_head_symbol(table, member.receiver),
        ExpressionNode::Name(path) => path.head_symbol,
        _ => SymbolHandle::invalid(),
    }
}

fn guard_path_len(table: &ExpressionTable, expression: ExpressionHandle) -> Option<usize> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => guard_path_len(table, *target),
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Integer(_) = table.expression(indexed.index) else {
                return None;
            };
            guard_path_len(table, indexed.collection)
        }
        ExpressionNode::Member(member) => guard_path_len(table, member.receiver)?.checked_add(1),
        ExpressionNode::Name(path) => Some(table.name_path_members(path.members).len()),
        _ => None,
    }
}

fn guard_path_member(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    index: usize,
) -> Option<&ProgramName> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => guard_path_member(table, *target, index),
        ExpressionNode::Indexed(indexed) => guard_path_member(table, indexed.collection, index),
        ExpressionNode::Member(member) => {
            let receiver_len = guard_path_len(table, member.receiver)?;
            if index == receiver_len {
                Some(&member.member)
            } else {
                guard_path_member(table, member.receiver, index)
            }
        }
        ExpressionNode::Name(path) => table.name_path_members(path.members).get(index),
        _ => None,
    }
}

fn guard_path_member_symbol(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    index: usize,
) -> SymbolHandle {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => guard_path_member_symbol(table, *target, index),
        ExpressionNode::Indexed(indexed) => {
            guard_path_member_symbol(table, indexed.collection, index)
        }
        ExpressionNode::Member(member) => {
            let Some(receiver_len) = guard_path_len(table, member.receiver) else {
                return SymbolHandle::invalid();
            };
            if index == receiver_len {
                member.member_symbol
            } else {
                guard_path_member_symbol(table, member.receiver, index)
            }
        }
        ExpressionNode::Name(path) => borrowed_member_symbol(
            table.name_path_members(path.members),
            table.name_path_member_symbols(path.member_symbols),
            path.head_symbol,
            path.symbol,
            index,
        ),
        _ => SymbolHandle::invalid(),
    }
}

fn guard_path_member_index(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    index: usize,
) -> Option<usize> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => guard_path_member_index(table, *target, index),
        ExpressionNode::Indexed(indexed) => {
            let indexed_len = guard_path_len(table, indexed.collection)?;
            if index + 1 == indexed_len {
                let ExpressionNode::Integer(value) = table.expression(indexed.index) else {
                    return None;
                };
                usize::try_from(*value).ok()
            } else {
                guard_path_member_index(table, indexed.collection, index)
            }
        }
        ExpressionNode::Member(member) => {
            let receiver_len = guard_path_len(table, member.receiver)?;
            if index < receiver_len {
                guard_path_member_index(table, member.receiver, index)
            } else {
                None
            }
        }
        ExpressionNode::Name(_) => None,
        _ => None,
    }
}

fn borrowed_member_symbol(
    members: &[ProgramName],
    member_symbols: &[SymbolHandle],
    head_symbol: SymbolHandle,
    final_symbol: SymbolHandle,
    index: usize,
) -> SymbolHandle {
    member_symbols.get(index).copied().unwrap_or_else(|| {
        if index == 0 {
            head_symbol
        } else if index + 1 == members.len() {
            final_symbol
        } else {
            SymbolHandle::invalid()
        }
    })
}

#[derive(Clone, Copy)]
struct GuardPathSuffix<'path, 'table> {
    path: &'path NormalizedGuardNamePath<'table>,
    start: usize,
}

impl<'path, 'table> GuardPathSuffix<'path, 'table> {
    fn is_empty(self) -> bool {
        self.start >= self.path.len()
    }

    fn iter(self) -> GuardPathSuffixIter<'path, 'table> {
        GuardPathSuffixIter {
            path: self.path,
            index: self.start,
        }
    }
}

struct GuardPathSuffixIter<'path, 'table> {
    path: &'path NormalizedGuardNamePath<'table>,
    index: usize,
}

impl<'path, 'table> Iterator for GuardPathSuffixIter<'path, 'table> {
    type Item = (&'path ProgramName, SymbolHandle, Option<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        let member = self.path.member(self.index)?;
        let symbol = self.path.member_symbol(self.index);
        let field_index = self.path.member_index(self.index);
        self.index += 1;
        Some((member, symbol, field_index))
    }
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

        let nested_machine_layout = field_machine_layout(layouts, field.type_symbol);

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
) -> Option<&'plan omega_layout::MachineLayout> {
    layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == type_symbol)
        .map(|(_, machine_layout)| machine_layout)
}

fn resolve_nested_field_layout_with_symbols(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: GuardPathSuffix<'_, '_>,
) -> Option<(usize, TypeLayout)> {
    resolve_nested_field_layout_by_symbol(layouts, root_field, suffix)
}

fn resolve_nested_field_layout_by_symbol(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: GuardPathSuffix<'_, '_>,
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_symbol = root_field.type_symbol;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for (segment, field_symbol, field_index) in suffix.iter() {
        let field_segment = parse_field_segment(segment, field_index)?;
        let fields = record_fields(layouts, type_symbol, type_name);
        let field = field_layout_by_symbol(layouts, fields, field_symbol)?;
        byte_offset += field.offset;
        type_symbol = field.type_symbol;
        type_name = &field.type_name;
        layout = field.layout;

        if let Some(index) = field_segment.index {
            let array = parse_array_type_name(type_name)?;
            if index >= array.length {
                return None;
            }
            let element_layout = TypeLayout {
                size: layout.size / array.length,
                alignment: layout.alignment,
            };
            byte_offset += element_layout.size * index;
            type_symbol = field.type_symbol;
            type_name = array.element_type_name;
            layout = element_layout;
        }
    }

    Some((byte_offset, layout))
}

fn data_layout<'plan>(
    layouts: &'plan LayoutPlan,
    type_symbol: SymbolHandle,
    _type_name: &str,
) -> Option<&'plan omega_layout::DataLayout> {
    if !type_symbol.is_valid() {
        return None;
    }

    layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.symbol == type_symbol)
        .map(|(_, data_layout)| data_layout)
}

fn record_fields(
    layouts: &LayoutPlan,
    type_symbol: SymbolHandle,
    type_name: &str,
) -> HandleSpan<FieldLayout> {
    if let Some(data_layout) = data_layout(layouts, type_symbol, type_name) {
        let DataShape::Record { fields } = &data_layout.shape else {
            return HandleSpan::empty();
        };
        return *fields;
    }

    layouts
        .machine_layouts
        .iter()
        .find_map(|(_, machine_layout)| {
            (type_symbol.is_valid() && machine_layout.symbol == type_symbol)
                .then_some(machine_layout.fields)
        })
        .unwrap_or_else(HandleSpan::empty)
}

struct FieldSegment {
    index: Option<usize>,
}

fn parse_field_segment(segment: &str, explicit_index: Option<usize>) -> Option<FieldSegment> {
    if explicit_index.is_some() {
        return Some(FieldSegment {
            index: explicit_index,
        });
    }

    let Some((_field_name, index_suffix)) = segment.split_once('[') else {
        return Some(FieldSegment { index: None });
    };
    let index = index_suffix.strip_suffix(']')?.parse::<usize>().ok()?;
    Some(FieldSegment { index: Some(index) })
}

struct ArrayTypeName<'name> {
    element_type_name: &'name str,
    length: usize,
}

fn parse_array_type_name(type_name: &str) -> Option<ArrayTypeName<'_>> {
    let inner = type_name.strip_prefix('[')?.strip_suffix(']')?;
    let (element_type_name, length) = inner.split_once(';')?;
    Some(ArrayTypeName {
        element_type_name: element_type_name.trim(),
        length: length.trim().parse::<usize>().ok()?,
    })
}

fn field_layout_by_symbol<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
) -> Option<&'plan FieldLayout> {
    field_symbol
        .is_valid()
        .then(|| {
            layouts
                .fields
                .span(fields)?
                .iter()
                .find(|field| field.symbol == field_symbol)
        })
        .flatten()
}

fn fallback_machine_named_path_layout(
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    path: &NormalizedGuardNamePath<'_>,
) -> Option<(usize, TypeLayout)> {
    let mut root_index = 0;
    if root_symbol.is_valid()
        && layouts
            .machine_layouts
            .iter()
            .any(|(_, machine_layout)| machine_layout.symbol == root_symbol)
    {
        root_index = 1;
    }
    path.member(root_index)?;
    let suffix = path.suffix(root_index + 1);
    let root_field_symbol = path.member_symbol(root_index);

    layouts
        .machine_layouts
        .iter()
        .find_map(|(_, machine_layout)| {
            let machine_base_offset =
                machine_storage_offset(layouts, entry_machine, machine_layout.symbol)?;
            let root_field =
                field_layout_by_symbol(layouts, machine_layout.fields, root_field_symbol)?;
            let (byte_offset, layout) =
                resolve_nested_field_layout_with_symbols(layouts, root_field, suffix)?;
            Some((machine_base_offset + byte_offset, layout))
        })
}

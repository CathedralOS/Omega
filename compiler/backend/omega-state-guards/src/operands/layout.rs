use crate::StateGuardOperandStorage;
use omega_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableIndexedExpression,
    TableMemberExpression,
};
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
    let root_name = path.first()?.clone();
    let suffix = path.members().get(1..).unwrap_or(&[]);

    if let Some(slot_layout) = runtime_frame_operand_layout(
        layouts,
        runtime_storage,
        source_key,
        source_dispatch_index,
        root_symbol,
        suffix,
    ) {
        return Some(slot_layout);
    }

    if root_symbol == source_machine {
        let [field_name, rest @ ..] = suffix else {
            return None;
        };
        let field_symbol = path.member_symbol(1);
        if let Some((_, machine_layout)) = layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        {
            if let Some(machine_base_offset) =
                machine_storage_offset(layouts, entry_machine, source_machine)
            {
                if let Some(root_field) = field_layout_by_symbol_or_name(
                    layouts,
                    machine_layout.fields,
                    field_symbol,
                    field_name.as_str(),
                ) {
                    return resolve_nested_field_layout_with_symbols(
                        layouts, root_field, rest, &path, 2,
                    )
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
                let root_field = field_layout_by_symbol_or_name(
                    layouts,
                    candidate_layout.fields,
                    field_symbol,
                    field_name.as_str(),
                )?;

                resolve_nested_field_layout_with_symbols(layouts, root_field, rest, &path, 2).map(
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
    let root_field = field_layout_by_symbol_or_name(
        layouts,
        machine_layout.fields,
        root_symbol,
        root_name.as_str(),
    );

    if let Some(root_field) = root_field {
        return resolve_nested_field_layout_with_symbols(layouts, root_field, suffix, &path, 1)
            .map(|(byte_offset, layout)| ResolvedOperandLayout {
                storage: StateGuardOperandStorage::MachineOwned,
                byte_offset: machine_base_offset + byte_offset,
                layout,
            });
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
    root_symbol: SymbolHandle,
    suffix: &[ProgramName],
) -> Option<ResolvedOperandLayout> {
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
    suffix: &[ProgramName],
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_byte_offset;
    let mut type_symbol = root_type_symbol;
    let mut type_name = root_type_name;
    let mut layout = TypeLayout {
        size: 0,
        alignment: 1,
    };

    for segment in suffix {
        let field_segment = parse_field_segment(segment)?;
        let fields = record_fields(layouts, type_symbol, type_name)?;
        let field = field_layout(layouts, fields, field_segment.name)?;
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
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => normalized_guard_name_path(table, *target),
        ExpressionNode::Indexed(indexed) => indexed_expression_path_in_table(table, indexed),
        ExpressionNode::Member(member) => member_expression_path_in_table(table, member),
        ExpressionNode::Name(path) => Some(NormalizedGuardNamePath::borrowed(
            table.name_path_members(path.members),
            path.head_symbol,
            path.symbol,
        )),
        _ => None,
    }
}

fn member_expression_path_in_table<'table>(
    table: &'table ExpressionTable,
    member: &TableMemberExpression,
) -> Option<NormalizedGuardNamePath<'table>> {
    let path = match table.expression(member.receiver) {
        ExpressionNode::Name(path) => NormalizedGuardNamePath::borrowed(
            table.name_path_members(path.members),
            path.head_symbol,
            path.symbol,
        ),
        ExpressionNode::Indexed(indexed) => indexed_expression_path_in_table(table, indexed)?,
        ExpressionNode::Member(inner_member) => {
            member_expression_path_in_table(table, inner_member)?
        }
        ExpressionNode::Mutable(target) => normalized_guard_name_path(table, *target)?,
        _ => return None,
    };
    let (mut members, mut member_symbols, head_symbol, _) = path.into_owned_parts();
    members.push(member.member.clone());
    member_symbols.push(member.member_symbol);
    Some(NormalizedGuardNamePath::owned(
        members,
        member_symbols,
        head_symbol,
        member.member_symbol,
    ))
}

fn indexed_expression_path_in_table<'table>(
    table: &'table ExpressionTable,
    indexed: &TableIndexedExpression,
) -> Option<NormalizedGuardNamePath<'table>> {
    let ExpressionNode::Integer(index) = table.expression(indexed.index) else {
        return None;
    };
    let path = match table.expression(indexed.collection) {
        ExpressionNode::Name(path) => NormalizedGuardNamePath::borrowed(
            table.name_path_members(path.members),
            path.head_symbol,
            path.symbol,
        ),
        ExpressionNode::Indexed(inner_indexed) => {
            indexed_expression_path_in_table(table, inner_indexed)?
        }
        _ => return None,
    };
    let (mut members, member_symbols, head_symbol, final_symbol) = path.into_owned_parts();
    let last_segment = members.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(NormalizedGuardNamePath::owned(
        members,
        member_symbols,
        head_symbol,
        final_symbol,
    ))
}

enum NormalizedGuardNamePath<'table> {
    Borrowed {
        members: &'table [ProgramName],
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    },
    Owned {
        members: Vec<ProgramName>,
        member_symbols: Vec<SymbolHandle>,
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    },
}

impl<'table> NormalizedGuardNamePath<'table> {
    fn borrowed(
        members: &'table [ProgramName],
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    ) -> Self {
        Self::Borrowed {
            members,
            head_symbol,
            final_symbol,
        }
    }

    fn owned(
        members: Vec<ProgramName>,
        member_symbols: Vec<SymbolHandle>,
        head_symbol: SymbolHandle,
        final_symbol: SymbolHandle,
    ) -> Self {
        Self::Owned {
            members,
            member_symbols,
            head_symbol,
            final_symbol,
        }
    }

    fn head_symbol(&self) -> SymbolHandle {
        match self {
            Self::Borrowed { head_symbol, .. } | Self::Owned { head_symbol, .. } => *head_symbol,
        }
    }

    fn members(&self) -> &[ProgramName] {
        match self {
            Self::Borrowed { members, .. } => members,
            Self::Owned { members, .. } => members,
        }
    }

    fn first(&self) -> Option<&ProgramName> {
        self.members().first()
    }

    fn member_symbol(&self, index: usize) -> SymbolHandle {
        match self {
            Self::Borrowed {
                members,
                head_symbol,
                final_symbol,
                ..
            } => {
                if index == 0 {
                    *head_symbol
                } else if index + 1 == members.len() {
                    *final_symbol
                } else {
                    SymbolHandle::invalid()
                }
            }
            Self::Owned { member_symbols, .. } => member_symbols
                .get(index)
                .copied()
                .unwrap_or_else(SymbolHandle::invalid),
        }
    }

    fn into_owned_parts(
        self,
    ) -> (
        Vec<ProgramName>,
        Vec<SymbolHandle>,
        SymbolHandle,
        SymbolHandle,
    ) {
        match self {
            Self::Borrowed {
                members,
                head_symbol,
                final_symbol,
                ..
            } => {
                let mut member_symbols = vec![SymbolHandle::invalid(); members.len()];
                if let Some(root_symbol) = member_symbols.first_mut() {
                    *root_symbol = head_symbol;
                }
                if members.len() > 1
                    && let Some(last_symbol) = member_symbols.last_mut()
                {
                    *last_symbol = final_symbol;
                }
                (members.to_vec(), member_symbols, head_symbol, final_symbol)
            }
            Self::Owned {
                members,
                member_symbols,
                head_symbol,
                final_symbol,
                ..
            } => (members, member_symbols, head_symbol, final_symbol),
        }
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

fn resolve_nested_field_layout_with_symbols(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[ProgramName],
    path: &NormalizedGuardNamePath<'_>,
    suffix_start_index: usize,
) -> Option<(usize, TypeLayout)> {
    resolve_nested_field_layout_by_symbol(layouts, root_field, suffix, |offset, _| {
        path.member_symbol(suffix_start_index + offset)
    })
}

fn resolve_nested_field_layout_by_symbol(
    layouts: &LayoutPlan,
    root_field: &FieldLayout,
    suffix: &[ProgramName],
    mut field_symbol_at: impl FnMut(usize, &FieldSegment<'_>) -> SymbolHandle,
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_field.offset;
    let mut type_symbol = root_field.type_symbol;
    let mut type_name = root_field.type_name.as_str();
    let mut layout = root_field.layout;

    for (offset, segment) in suffix.iter().enumerate() {
        let field_segment = parse_field_segment(segment)?;
        let fields = record_fields(layouts, type_symbol, type_name)?;
        let field_symbol = field_symbol_at(offset, &field_segment);
        let field =
            field_layout_by_symbol_or_name(layouts, fields, field_symbol, field_segment.name)?;
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
) -> Option<HandleSpan<FieldLayout>> {
    if let Some(data_layout) = data_layout(layouts, type_symbol, type_name) {
        let DataShape::Record { fields } = &data_layout.shape else {
            return None;
        };
        return Some(*fields);
    }

    layouts
        .machine_layouts
        .iter()
        .find_map(|(_, machine_layout)| {
            ((type_symbol.is_valid() && machine_layout.symbol == type_symbol)
                || machine_layout.name.as_str() == type_name)
                .then_some(machine_layout.fields)
        })
}

struct FieldSegment<'name> {
    name: &'name str,
    index: Option<usize>,
}

fn parse_field_segment(segment: &str) -> Option<FieldSegment<'_>> {
    let Some((field_name, index_suffix)) = segment.split_once('[') else {
        return Some(FieldSegment {
            name: segment,
            index: None,
        });
    };
    let index = index_suffix.strip_suffix(']')?.parse::<usize>().ok()?;
    Some(FieldSegment {
        name: field_name,
        index: Some(index),
    })
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

fn field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_name: &str,
) -> Option<&'plan FieldLayout> {
    layouts
        .fields
        .span(fields)?
        .iter()
        .find(|field| field.name.as_str() == field_name)
}

fn field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
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

fn fallback_machine_named_path_layout(
    layouts: &LayoutPlan,
    entry_machine: SymbolHandle,
    root_symbol: SymbolHandle,
    path: &NormalizedGuardNamePath<'_>,
) -> Option<(usize, TypeLayout)> {
    let path_members = path.members();
    let mut root_index = 0;
    let mut segments = path_members;
    if root_symbol.is_valid()
        && layouts
            .machine_layouts
            .iter()
            .any(|(_, machine_layout)| machine_layout.symbol == root_symbol)
    {
        segments = segments.get(1..)?;
        root_index = 1;
    }
    let [root_name, suffix @ ..] = segments else {
        return None;
    };
    let root_field_symbol = path.member_symbol(root_index);

    layouts
        .machine_layouts
        .iter()
        .find_map(|(_, machine_layout)| {
            let machine_base_offset =
                machine_storage_offset(layouts, entry_machine, machine_layout.symbol)?;
            let root_field = field_layout_by_symbol_or_name(
                layouts,
                machine_layout.fields,
                root_field_symbol,
                root_name.as_str(),
            )?;
            let (byte_offset, layout) = resolve_nested_field_layout_with_symbols(
                layouts,
                root_field,
                suffix,
                path,
                root_index + 1,
            )?;
            Some((machine_base_offset + byte_offset, layout))
        })
}

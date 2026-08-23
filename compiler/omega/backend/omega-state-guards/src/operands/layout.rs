use crate::StateGuardOperandStorage;
use omega_control_flow::StateKey;
use omega_layout::{DataShape, FieldLayout, LayoutPlan, TypeLayout, TypeLayoutDescriptor};
use omega_runtime_storage::RuntimeStoragePlan;
use psi_arena::HandleSpan;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedOperandLayout {
    pub storage: StateGuardOperandStorage,
    pub byte_offset: usize,
    pub layout: TypeLayout,
}

pub(super) fn resolve_guard_operand_layout(
    layouts: &LayoutPlan,
    runtime_storage: &RuntimeStoragePlan,
    receiver_bases: &[Option<usize>],
    state_contexts: &[u32],
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
    let root_name = path.member(0)?;
    let suffix = path.suffix(1);

    if let Some(slot_layout) = runtime_frame_operand_layout(
        layouts,
        runtime_storage,
        state_contexts,
        source_key,
        source_dispatch_index,
        &path,
        suffix,
    ) {
        return Some(slot_layout);
    }

    if path_targets_source_machine(&path, source_machine) {
        if suffix.is_empty() {
            return None;
        }
        let field_symbol = path.member_symbol(1);
        let field_name = path.member(1)?;
        // A constant array index on the root machine field (`self.cells[1].value`)
        // is carried out of band as `member_index(1)`; it must be folded into the
        // root field before the rest of the path resolves, or every `self.arr[i].f`
        // guard reads element 0.
        let root_field_index = path.member_index(1);
        let rest = path.suffix(2);
        if let Some((_, machine_layout)) = layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        {
            if let Some(machine_base_offset) =
                per_instance_base(receiver_bases, source_dispatch_index)
                    .or_else(|| machine_storage_offset(layouts, entry_machine, source_machine))
            {
                if let Some(root_field) = field_layout_by_symbol_or_name(
                    layouts,
                    machine_layout.fields,
                    field_symbol,
                    field_name,
                ) {
                    let indexed_root;
                    let root_field = match root_field_index {
                        Some(index) => {
                            indexed_root = indexed_root_field(root_field, index)?;
                            &indexed_root
                        }
                        None => root_field,
                    };
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
                let root_field = field_layout_by_symbol_or_name(
                    layouts,
                    candidate_layout.fields,
                    field_symbol,
                    field_name,
                )?;
                let indexed_root;
                let root_field = match root_field_index {
                    Some(index) => {
                        indexed_root = indexed_root_field(root_field, index)?;
                        &indexed_root
                    }
                    None => root_field,
                };

                resolve_nested_field_layout_with_symbols(layouts, root_field, rest).map(
                    |(byte_offset, layout)| ResolvedOperandLayout {
                        storage: StateGuardOperandStorage::MachineOwned,
                        byte_offset: candidate_base_offset + byte_offset,
                        layout,
                    },
                )
            });
    }

    let machine_base_offset = per_instance_base(receiver_bases, source_dispatch_index)
        .or_else(|| machine_storage_offset(layouts, entry_machine, source_machine))?;
    let machine_layout = layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.symbol == source_machine)
        .map(|(_, machine_layout)| machine_layout)?;
    let root_field =
        field_layout_by_symbol_or_name(layouts, machine_layout.fields, root_symbol, root_name);

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
    state_contexts: &[u32],
    source_key: StateKey,
    source_dispatch_index: u32,
    path: &NormalizedGuardNamePath<'_>,
    suffix: GuardPathSuffix<'_, '_>,
) -> Option<ResolvedOperandLayout> {
    let root_symbol = path.head_symbol();
    let root_name = path.member(0)?;
    let root_index = path.member_index(0);
    let slot_matches_symbol = |slot: &omega_runtime_storage::RuntimeFrameSlot| {
        (root_symbol.is_valid() && slot.symbol == root_symbol) || slot.name == *root_name
    };
    let same_state_without_segment = |slot_key: StateKey| {
        slot_key.machine == source_key.machine && slot_key.state == source_key.state
    };
    // The fallback tiers relax segment/state matching, and each tier must
    // PREFER its own call context before crossing one: a machine expanded at
    // two call sites has two full slot regions keyed by the same
    // (machine, state) symbols, and a bare first-in-arena match is the FIRST
    // expansion's region. A tail-segment guard in the second expansion then
    // reads the first call's stale `i`/`path.len`, exits its loop at
    // iteration 0, and the walk sees an empty path (the repeated dir-walk
    // `"/*"`-pattern miscompile). Cross-context stays as the LAST resort of
    // each tier -- a caller's guard reading a straight-line callee's terminal
    // result slot has no same-context candidate at all (the value-return
    // keystone family), and dropping those resolutions exits the program at
    // the wrong state.
    let context_of = |dispatch_index: u32| -> u32 {
        state_contexts
            .get(dispatch_index as usize)
            .copied()
            .unwrap_or(0)
    };
    let source_context = context_of(source_dispatch_index);

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
                (context_of(slot.dispatch_index) == source_context
                    && same_state_without_segment(slot.source_key)
                    && slot_matches_symbol(slot))
                .then_some(slot)
            })
        })
        .or_else(|| {
            runtime_storage.frame_slots.iter().find_map(|(_, slot)| {
                (same_state_without_segment(slot.source_key) && slot_matches_symbol(slot))
                    .then_some(slot)
            })
        })
        .or_else(|| {
            runtime_storage.frame_slots.iter().find_map(|(_, slot)| {
                (context_of(slot.dispatch_index) == source_context
                    && slot.source_key.machine == source_key.machine
                    && slot_matches_symbol(slot))
                .then_some(slot)
            })
        })
        .or_else(|| {
            runtime_storage.frame_slots.iter().find_map(|(_, slot)| {
                (slot.source_key.machine == source_key.machine && slot_matches_symbol(slot))
                    .then_some(slot)
            })
        })?;

    let root_layout = TypeLayout {
        size: slot.byte_size,
        alignment: slot.alignment,
    };
    let (root_byte_offset, root_type_symbol, root_layout) = if let Some(index) = root_index {
        let Some((element_type, length)) = slot.type_descriptor.fixed_array() else {
            // Slice-like root indexing must stay on the runtime-expression path.
            return None;
        };
        if index >= length {
            return None;
        }
        let element_size = root_layout.size / length;
        (
            slot.byte_offset + (element_size * index),
            element_type.storage_symbol(),
            TypeLayout {
                size: element_size,
                alignment: root_layout.alignment,
            },
        )
    } else {
        (slot.byte_offset, slot.type_symbol, root_layout)
    };

    let (byte_offset, layout) = if suffix.is_empty() {
        // A `&scalar`-typed local (`let v: &f32 = ...`, the §5b recast
        // carrier): the slot is pointer-wide, but reads THROUGH the view
        // compare at the REFEREE's width/kind -- an 8-byte read of the
        // 4-byte pattern compared as f64 was the recast pin's read-side
        // divergence. Aggregate referees keep the slot layout (their reads
        // resolve through suffixes).
        (
            root_byte_offset,
            reference_scalar_referee_layout(&slot.type_descriptor, root_layout)
                .unwrap_or(root_layout),
        )
    } else if reference_named_record_referee(&slot.type_descriptor) {
        // A member read THROUGH a `&Record` reference slot cannot lower as a
        // flat place clause: the slot may hold a REAL POINTER (a wide
        // referee's borrow-recast let, a reference param) and the clause
        // model has no dereference -- `slot + field` read the walk guard's
        // `d.number_of_pages` from stale frame bytes. Refuse the whole
        // conjunction; the expression-guard path lowers the read with the
        // deref-vs-content decision (the referee-size rule).
        return None;
    } else if let Some(member_layout) =
        runtime_slice_descriptor_member_layout(slot, root_byte_offset, root_layout, suffix)
    {
        member_layout
    } else {
        resolve_nested_slot_layout(layouts, root_byte_offset, root_type_symbol, suffix)?
    };

    Some(ResolvedOperandLayout {
        storage: StateGuardOperandStorage::RuntimeFrame,
        byte_offset,
        layout,
    })
}

fn runtime_slice_descriptor_member_layout(
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    root_byte_offset: usize,
    root_layout: TypeLayout,
    suffix: GuardPathSuffix<'_, '_>,
) -> Option<(usize, TypeLayout)> {
    if !is_slice_descriptor(&slot.type_descriptor) || root_layout.size % 2 != 0 {
        return None;
    }

    let mut segments = suffix.iter();
    let (member, _, _) = segments.next()?;
    if segments.next().is_some() || member.as_str() != "len" {
        return None;
    }

    let pointer_size = root_layout.size / 2;
    Some((
        root_byte_offset.checked_add(pointer_size)?,
        TypeLayout {
            size: pointer_size,
            alignment: root_layout.alignment,
        },
    ))
}

/// The scalar REFEREE layout of a `Reference`-descriptored slot (`&f32` ->
/// 4 bytes), peeling `Constrained` shells on both sides. `None` for
/// non-references and aggregate referees.
fn reference_scalar_referee_layout(
    descriptor: &TypeLayoutDescriptor,
    root_layout: TypeLayout,
) -> Option<TypeLayout> {
    let mut descriptor = descriptor;
    while let TypeLayoutDescriptor::Constrained { base_type, .. } = descriptor {
        descriptor = base_type.as_ref();
    }
    let TypeLayoutDescriptor::Reference { referee, .. } = descriptor else {
        return None;
    };
    let mut referee = referee.as_ref();
    while let TypeLayoutDescriptor::Constrained { base_type, .. } = referee {
        referee = base_type.as_ref();
    }
    let TypeLayoutDescriptor::Named { name, .. } = referee else {
        return None;
    };
    let size =
        psi_checked_trees::types::PrimitiveType::from_name(name.as_str())?.scalar_byte_size()?;
    Some(TypeLayout {
        size,
        alignment: root_layout.alignment,
    })
}

/// Whether the slot's descriptor is a reference to a NAMED RECORD (peeling
/// `Constrained` on both sides; primitives excluded -- their referee reads
/// resolve at the scalar width above, and records are where member suffixes
/// need the dereference decision).
fn reference_named_record_referee(descriptor: &TypeLayoutDescriptor) -> bool {
    let mut descriptor = descriptor;
    while let TypeLayoutDescriptor::Constrained { base_type, .. } = descriptor {
        descriptor = base_type.as_ref();
    }
    let TypeLayoutDescriptor::Reference { referee, .. } = descriptor else {
        return false;
    };
    let mut referee = referee.as_ref();
    while let TypeLayoutDescriptor::Constrained { base_type, .. } = referee {
        referee = base_type.as_ref();
    }
    match referee {
        TypeLayoutDescriptor::Named { name, .. } => {
            psi_checked_trees::types::PrimitiveType::from_name(name.as_str()).is_none()
        }
        _ => false,
    }
}

fn is_slice_descriptor(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Reference { referee, .. } => {
            // A domain-constrained slice view (`&[u8] in Path`) is
            // `Reference { Constrained { Slice } }`: the constraint is a
            // compile-time fact that does NOT change the `{ptr, len}` storage
            // shape (see omega-layout `builder.rs`, which unwraps `Constrained`
            // past the referee before deciding sized-ness). Peel any `Constrained`
            // wrappers off the referee before checking for the Slice -- otherwise
            // a domained-slice param's `.len` guard operand fails to resolve to
            // storage and the guard is refused (silently-dropped-guard backstop),
            // even though a plain `&[T]` param's `.len` resolves fine.
            let mut referee = referee.as_ref();
            while let TypeLayoutDescriptor::Constrained { base_type, .. } = referee {
                referee = base_type.as_ref();
            }
            matches!(referee, TypeLayoutDescriptor::Slice { .. })
        }
        TypeLayoutDescriptor::Constrained { base_type, .. } => is_slice_descriptor(base_type),
        _ => false,
    }
}

fn descriptor_is_bounded_byte_buffer(descriptor: &TypeLayoutDescriptor) -> bool {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            descriptor_is_bounded_byte_buffer(base_type)
        }
        TypeLayoutDescriptor::BoundedByteBuffer { .. } => true,
        _ => false,
    }
}

fn resolve_nested_slot_layout(
    layouts: &LayoutPlan,
    root_byte_offset: usize,
    root_type_symbol: SymbolHandle,
    suffix: GuardPathSuffix<'_, '_>,
) -> Option<(usize, TypeLayout)> {
    let mut byte_offset = root_byte_offset;
    let mut type_symbol = root_type_symbol;
    let mut layout = TypeLayout {
        size: 0,
        alignment: 1,
    };

    for (segment, field_symbol, field_index) in suffix.iter() {
        let field_segment = parse_field_segment(segment, field_index)?;
        let field = data_member_field_layout(layouts, type_symbol, field_symbol, segment)?;
        byte_offset += field.offset;
        type_symbol = field.type_symbol;
        layout = field.layout;

        if let Some(index) = field_segment.index {
            let (element_type, length) = field.type_descriptor.fixed_array()?;
            if index >= length {
                return None;
            }
            let element_size = field.layout.size / length;
            byte_offset += element_size * index;
            type_symbol = element_type.storage_symbol();
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

    fn member(&self, index: usize) -> Option<&Identifier> {
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
        ExpressionNode::Borrow(target) => guard_path_head_symbol(table, target.target),
        ExpressionNode::Indexed(indexed) => guard_path_head_symbol(table, indexed.collection),
        ExpressionNode::Member(member) => guard_path_head_symbol(table, member.receiver),
        ExpressionNode::Name(path) => path.head_symbol,
        _ => SymbolHandle::invalid(),
    }
}

fn guard_path_len(table: &ExpressionTable, expression: ExpressionHandle) -> Option<usize> {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => guard_path_len(table, target.target),
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Integer(_) = table.expression(indexed.index) else {
                return None;
            };
            // A stacked constant index (`grid[0][1]`) cannot be flattened
            // into this one-index-per-name-member path. `member_index` would
            // otherwise retain only the outermost index and silently read the
            // transposed/wrong element. Refuse the static clause so the
            // canonical expression/place guard path retains both levels.
            let mut collection = indexed.collection;
            while let ExpressionNode::Borrow(inner) = table.expression(collection) {
                collection = inner.target;
            }
            if matches!(table.expression(collection), ExpressionNode::Indexed(_)) {
                return None;
            }
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
) -> Option<&Identifier> {
    match table.expression(expression) {
        ExpressionNode::Borrow(target) => guard_path_member(table, target.target, index),
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
        ExpressionNode::Borrow(target) => guard_path_member_symbol(table, target.target, index),
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
        ExpressionNode::Borrow(target) => guard_path_member_index(table, target.target, index),
        ExpressionNode::Indexed(indexed) => {
            let indexed_len = guard_path_len(table, indexed.collection)?;
            if index + 1 == indexed_len {
                let ExpressionNode::Integer(value) = table.expression(indexed.index) else {
                    return None;
                };
                value
                    .value_i64()
                    .and_then(|value| usize::try_from(value).ok())
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
    members: &[Identifier],
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
    type Item = (&'path Identifier, SymbolHandle, Option<usize>);

    fn next(&mut self) -> Option<Self::Item> {
        let member = self.path.member(self.index)?;
        let symbol = self.path.member_symbol(self.index);
        let field_index = self.path.member_index(self.index);
        self.index += 1;
        Some((member, symbol, field_index))
    }
}

/// The per-instance receiver base for this dispatch case, when the pipeline
/// resolved one (BackendPlan::receiver_bases; per-instance dispatch). Guard
/// operands MUST read the same instance the body writes -- this site was the
/// phase-3 probe's wrong-resolution hole (native 3 vs interp 70).
fn per_instance_base(receiver_bases: &[Option<usize>], dispatch_index: u32) -> Option<usize> {
    receiver_bases
        .get(dispatch_index as usize)
        .copied()
        .flatten()
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

/// Fold a constant array index into a root machine field (`cells` in
/// `self.cells[1].value`): advance the offset by `index * element_size` and narrow
/// the type to the element. The suffix walk handles indices on later segments, but
/// the root field is consumed before the suffix, so its index must be applied here.
fn indexed_root_field(root_field: &FieldLayout, index: usize) -> Option<FieldLayout> {
    let (element_type, length) = root_field.type_descriptor.fixed_array()?;
    if index >= length {
        return None;
    }
    let element_size = root_field.layout.size / length;
    Some(FieldLayout {
        symbol: root_field.symbol,
        name: root_field.name.clone(),
        offset: root_field.offset + element_size * index,
        type_symbol: element_type.storage_symbol(),
        type_name: root_field.type_name.clone(),
        type_descriptor: element_type.clone(),
        layout: TypeLayout {
            size: element_size,
            alignment: root_field.layout.alignment,
        },
    })
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
    let mut layout = root_field.layout;
    let mut type_descriptor = root_field.type_descriptor.clone();

    for (segment, field_symbol, field_index) in suffix.iter() {
        // A `[u8;N]` carrier (BoundedByteBuffer) has no real `len` field; its
        // length word lives at the carrier's own base offset (size 4), matching
        // the instruction-selection resolver. Recognize `.len` BEFORE the
        // record-field lookup, which would otherwise return None and leave the
        // guard operand with `storage = Unknown` -- a silently-dropped guard.
        if segment.as_str() == "len" && descriptor_is_bounded_byte_buffer(&type_descriptor) {
            return Some((
                byte_offset,
                TypeLayout {
                    size: 4,
                    alignment: 4,
                },
            ));
        }
        let field_segment = parse_field_segment(segment, field_index)?;
        let field = data_member_field_layout(layouts, type_symbol, field_symbol, segment)?;
        byte_offset += field.offset;
        type_symbol = field.type_symbol;
        layout = field.layout;
        type_descriptor = field.type_descriptor.clone();

        if let Some(index) = field_segment.index {
            let (element_type, length) = field.type_descriptor.fixed_array()?;
            if index >= length {
                return None;
            }
            let element_layout = TypeLayout {
                size: layout.size / length,
                alignment: layout.alignment,
            };
            byte_offset += element_layout.size * index;
            type_symbol = element_type.storage_symbol();
            layout = element_layout;
        }
    }

    Some((byte_offset, layout))
}

fn data_layout<'plan>(
    layouts: &'plan LayoutPlan,
    type_symbol: SymbolHandle,
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

fn record_fields(layouts: &LayoutPlan, type_symbol: SymbolHandle) -> HandleSpan<FieldLayout> {
    if let Some(data_layout) = data_layout(layouts, type_symbol) {
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

/// Resolve a member of `type_symbol` covering BOTH data shapes: record fields,
/// and members of case-bearing data -- COMMON fields (mixed shapes, readable
/// without case knowledge) plus CASE PAYLOAD fields (searched across every
/// variant's payload span). All offsets are absolute within the value, so the
/// caller's offset arithmetic is shape-independent.
fn data_member_field_layout<'plan>(
    layouts: &'plan LayoutPlan,
    type_symbol: SymbolHandle,
    field_symbol: SymbolHandle,
    field_name: &Identifier,
) -> Option<&'plan FieldLayout> {
    if let Some(data_layout) = data_layout(layouts, type_symbol)
        && let DataShape::Enum {
            common_fields,
            variants,
        } = &data_layout.shape
    {
        return field_layout_by_symbol_or_name(layouts, *common_fields, field_symbol, field_name)
            .or_else(|| {
                layouts
                    .variants
                    .span_or_empty(*variants)
                    .iter()
                    .find_map(|variant| {
                        field_layout_by_symbol_or_name(
                            layouts,
                            variant.fields,
                            field_symbol,
                            field_name,
                        )
                    })
            });
    }

    field_layout_by_symbol_or_name(
        layouts,
        record_fields(layouts, type_symbol),
        field_symbol,
        field_name,
    )
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

fn field_layout_by_symbol_or_name<'plan>(
    layouts: &'plan LayoutPlan,
    fields: HandleSpan<FieldLayout>,
    field_symbol: SymbolHandle,
    field_name: &Identifier,
) -> Option<&'plan FieldLayout> {
    let fields = layouts.fields.span(fields)?;
    fields
        .iter()
        .find(|field| field_symbol.is_valid() && field.symbol == field_symbol)
        .or_else(|| fields.iter().find(|field| field.name == *field_name))
}

fn path_targets_source_machine(
    path: &NormalizedGuardNamePath<'_>,
    source_machine: SymbolHandle,
) -> bool {
    path.head_symbol() == source_machine
        || path.member(0).is_some_and(|name| name.as_str() == "self")
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
    let root_field_name = path.member(root_index)?;

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
                root_field_name,
            )?;
            let (byte_offset, layout) =
                resolve_nested_field_layout_with_symbols(layouts, root_field, suffix)?;
            Some((machine_base_offset + byte_offset, layout))
        })
}

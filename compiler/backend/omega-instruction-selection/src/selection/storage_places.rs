mod expressions;
mod machine_owned;
mod model;
mod nested_fields;
mod static_values;

pub(super) use expressions::indexed_expression_path;
pub(super) use machine_owned::{
    resolve_machine_owned_collection_in_table, resolve_machine_owned_place,
    resolve_machine_owned_place_in_table,
};
pub(super) use model::{
    RuntimeFrameBaseIndexedTarget, RuntimeFrameFixedIndexedTarget, RuntimeFrameIndexedTarget,
    RuntimeStoragePlace,
};
use omega_abstract_operations::RuntimeStorageRegion;
pub(super) use static_values::{
    clamp_runtime_case_comparison_operands, clamp_runtime_case_comparison_operands_in_table,
    enum_variant_value, enum_variant_value_in_table, static_integer_value,
    static_integer_value_in_table,
};

use crate::InstructionSelectionInput;
use expressions::{StorageNamePath, normalized_storage_name_path_in_table};
use nested_fields::{
    NestedFieldLayoutCursor, resolve_nested_field_layout_step,
    resolve_nested_field_layout_with_pairs,
};
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_checked_trees::name::Identifier;
use omega_checked_trees::types::PrimitiveType;
use omega_control_flow::StateKey;
use omega_core::symbols::{BuiltinType, SymbolHandle};
use omega_layout::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use omega_state_calls::StateCallRole;

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

fn runtime_slice_descriptor_member_place(
    input: &InstructionSelectionInput<'_>,
    root_offset: usize,
    byte_size: usize,
    member_name: Option<&str>,
    member_count: usize,
) -> Option<RuntimeStoragePlace> {
    let descriptor = input.runtime_abi.slice_descriptor();
    if byte_size != descriptor.total_size() || member_count != 1 {
        return None;
    }

    match member_name {
        Some("len") => Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: root_offset.checked_add(descriptor.len_offset())?,
            byte_count: descriptor.len_size(),
        }),
        _ => None,
    }
}

pub(super) fn resolve_runtime_storage_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    _source_machine: &str,
    _source_state: &str,
    expression: &Expression,
) -> Option<RuntimeStoragePlace> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_place_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

pub(super) fn resolve_runtime_assignment_value_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::AssignmentValue,
        None,
    )
}

pub(super) fn resolve_runtime_assignment_value_call_result_place_by_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::AssignmentValue,
        Some(call_ordinal),
    )
}

pub(super) fn resolve_runtime_call_argument_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::CallArgument,
        None,
    )
}

pub(super) fn resolve_runtime_call_argument_call_result_place_by_ordinal(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::CallArgument,
        Some(call_ordinal),
    )
}

pub(super) fn resolve_runtime_transition_guard_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::TransitionGuard,
        None,
    )
    .or_else(|| {
        input
            .runtime_storage
            .transition_guard_result_slot(dispatch_index, source_key, statement_index)
            .map(|slot| RuntimeStoragePlace {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: slot.byte_offset,
                byte_count: slot.byte_size,
            })
    })
}

pub(super) fn resolve_runtime_transition_argument_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::TransitionArgument,
        None,
    )
}

fn resolve_runtime_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: Option<usize>,
) -> Option<RuntimeStoragePlace> {
    let slot = if let Some(call_ordinal) = call_ordinal {
        input.runtime_storage.call_result_slot_by_ordinal(
            dispatch_index,
            source_key,
            statement_index,
            role,
            call_ordinal,
        )
    } else {
        input
            .runtime_storage
            .call_result_slot(dispatch_index, source_key, statement_index, role)
    }?;
    let target_key = match slot.kind {
        omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult { target_key, .. } => {
            target_key
        }
        _ => return None,
    };
    let state_call = if let Some(call_ordinal) = call_ordinal {
        input
            .state_calls
            .calls_for_statement(source_key, statement_index)
            .find(|state_call| state_call.role == role && state_call.call_ordinal == call_ordinal)
    } else {
        input
            .state_calls
            .call_for_role(source_key, statement_index, role)
    }?;
    if state_call.target_key != target_key {
        return None;
    }
    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: slot.byte_offset,
        byte_count: slot.byte_size,
    })
}

pub(super) fn resolve_runtime_storage_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    if let Some(place) = resolve_runtime_fixed_indexed_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(place);
    }

    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let suffix = path.suffix(1);
    if let Some(slot) =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })
        .or_else(|| {
            latest_dispatch_frame_slot(input, dispatch_index, |slot| {
                slot_matches_table_path(slot, &path)
            })
        })
    {
        // A root segment that carries an element index (`items[0].value` where
        // `items` is the matched frame slot) only resolves to a static place when
        // the slot stores the array INLINE — and the inline cases were already
        // handled by the index-aware resolver above. When the slot holds a slice
        // DESCRIPTOR the element lives behind the descriptor's data pointer, so
        // no static frame place exists; the fall-through below used to DROP the
        // index and alias the descriptor slot's own bytes as the element (a
        // threaded `items[0].value` argument received the data pointer's low
        // bytes). Refuse so callers fall back to descriptor-aware indexed
        // strategies. Index 0 on an inline array keeps the legacy direct place
        // (offset arithmetic is identical with the index dropped).
        if let Some(root_index) = path.member_index(0)
            && (root_index != 0 || !runtime_frame_slot_is_inline_fixed_array_storage(input, slot))
        {
            return None;
        }

        if let Some(place) = runtime_slice_descriptor_member_place(
            input,
            slot.byte_offset,
            slot.byte_size,
            suffix.iter().next().map(|(name, _, _)| name.as_str()),
            suffix.iter().count(),
        ) {
            return Some(place);
        }

        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: slot.byte_offset,
            type_symbol: slot.type_symbol,
            type_name: slot.type_name.clone(),
            type_descriptor: slot.type_descriptor.clone(),
            layout: TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        };
        let (byte_offset, layout) =
            resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?;

        return Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset,
            byte_count: layout.size,
        });
    }

    resolve_machine_owned_place_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    )
    .map(|(byte_offset, byte_count)| RuntimeStoragePlace {
        region: RuntimeStorageRegion::Machine,
        byte_offset,
        byte_count,
    })
}

/// Fold `<receiver>.len` to its STATIC element count when the receiver is (an
/// alias of) a FIXED ARRAY. A fixed array stored inline has no runtime length
/// field -- its length is a layout constant -- so a `.len` read on it (or on a
/// full `as_slice()` view of it) cannot resolve to a storage place; it folds to
/// an immediate instead.
///
/// The motivating shape is an inline-leaf VALUE-call arm guard `s.len > 0`
/// where the callee param `s` was substituted (through the branch's argument
/// bindings) to a caller local `let s = self.arr.as_slice()` that runtime
/// storage ELIDED (the local is only used as a call argument, so it has no
/// frame slot and therefore no slice descriptor to read a length from). The
/// receiver is traced through such unmaterialized local aliases back to the
/// fixed array they view.
pub(super) fn static_fixed_array_len_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    let expression = peel_mutable_in_table(expressions, expression);
    let ExpressionNode::Member(member) = expressions.expression(expression) else {
        return None;
    };
    if member.member.as_str() != "len" {
        return None;
    }
    let length = fixed_array_length_of_receiver_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        member.receiver,
        0,
    )?;
    i64::try_from(length).ok()
}

fn peel_mutable_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner) => peel_mutable_in_table(expressions, *inner),
        _ => expression,
    }
}

/// How many unmaterialized local-alias hops (`let s = t;` / `let t = self.arr
/// .as_slice();`) the fixed-array `.len` fold will trace before giving up.
const FIXED_ARRAY_ALIAS_TRACE_DEPTH_LIMIT: usize = 4;

/// The static length of the fixed array a receiver expression views, if any:
/// a machine-owned field (`self.arr`, `self.arr.as_slice()` -- path
/// normalization peels full slice views), a frame-slot field path, or an
/// unmaterialized local alias traced through its initializer.
fn fixed_array_length_of_receiver_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
    depth: usize,
) -> Option<usize> {
    if depth > FIXED_ARRAY_ALIAS_TRACE_DEPTH_LIMIT {
        return None;
    }

    if let Some(target) = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        receiver,
    ) && let Some((_, length)) = target.type_descriptor.fixed_array()
    {
        return Some(length);
    }

    let path = normalized_storage_name_path_in_table(expressions, receiver)?;
    if path.is_empty() {
        return None;
    }

    if let Some(slot) =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })
    {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: slot.byte_offset,
            type_symbol: slot.type_symbol,
            type_name: slot.type_name.clone(),
            type_descriptor: slot.type_descriptor.clone(),
            layout: TypeLayout {
                size: slot.byte_size,
                alignment: slot.alignment,
            },
        };
        let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
        for (field_name, field_symbol, field_index) in path.suffix(1).iter() {
            cursor = resolve_nested_field_layout_step(
                &input.layouts,
                cursor,
                field_name,
                field_symbol,
                field_index,
            )?;
        }
        return cursor
            .type_descriptor()
            .fixed_array()
            .map(|(_, length)| length);
    }

    // An ELIDED local (no frame slot): trace its declared initializer. Only a
    // bare single-name path can be a local.
    if path.len() != 1 {
        return None;
    }
    let initializer =
        state_local_initializer(input, source_key, path.head_symbol(), path.member(0)?)?;
    fixed_array_length_of_receiver_in_table(
        input,
        dispatch_index,
        source_key,
        &input.program.expression_table,
        initializer,
        depth + 1,
    )
}

/// Fold an ELIDED local (a single-name path with no frame slot, folded into
/// its initializer by storage planning) to its STATIC initializer value -- a
/// boolean or integer literal, possibly through `mut` wrappers or a chain of
/// such locals. The motivating shape is an inline-leaf VALUE-call arm guard
/// substituted with a caller local used only as the call argument (`let flag:
/// bool = true; self.out = self.pick(flag)` -- the arm guard reads `flag`,
/// which has no storage to compare against). Elision implies the local folds
/// into its initializer everywhere (a mutated or `&mut`-escaped local gets a
/// frame slot), so the fold is sound.
pub(super) fn static_elided_local_value_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<i64> {
    static_elided_local_value_traced(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
        0,
    )
}

fn static_elided_local_value_traced(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    depth: usize,
) -> Option<i64> {
    if depth > FIXED_ARRAY_ALIAS_TRACE_DEPTH_LIMIT {
        return None;
    }
    let expression = peel_mutable_in_table(expressions, expression);
    match expressions.expression(expression) {
        ExpressionNode::Boolean(value) => return Some(i64::from(*value)),
        ExpressionNode::Integer(value) => return Some(*value),
        ExpressionNode::Name(_) => {}
        _ => return None,
    }

    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.len() != 1 {
        return None;
    }
    // A MATERIALIZED local has runtime storage and may be mutated after its
    // initializer; never fold it here (the place resolvers read the slot).
    if find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
    .is_some()
    {
        return None;
    }
    let initializer =
        state_local_initializer(input, source_key, path.head_symbol(), path.member(0)?)?;
    static_elided_local_value_traced(
        input,
        dispatch_index,
        source_key,
        &input.program.expression_table,
        initializer,
        depth + 1,
    )
}

/// The declared initializer of the local named by `symbol`/`name` in
/// `source_key`'s state body, read from the checked program statements.
fn state_local_initializer(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<ExpressionHandle> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let omega_checked_trees::statement::StatementNode::LocalData(local_data) = statement
            else {
                return None;
            };
            let matches_symbol =
                symbol.is_valid() && local_data.symbol.is_valid() && local_data.symbol == symbol;
            (matches_symbol || local_data.name == *name).then_some(local_data.initial_value)
        })
        .filter(|initializer| initializer.is_valid())
}

/// Best-effort signedness of the integer place named by `expression`. Resolves
/// the frame-slot field path to its leaf primitive type. Returns `None` when the
/// type cannot be determined here (non-frame places, non-primitive leaves), in
/// which case callers fall back to the signed form. Used to pick signed vs
/// unsigned division/shift encodings.
pub(super) fn resolve_runtime_storage_is_signed_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<bool> {
    // A numeric `as` cast operand has the signedness of the cast's TARGET type
    // (`(x as u32) % k` must pick the unsigned modulo regardless of `x`'s own
    // type) -- without this, the place resolution below fails on the Cast node
    // and the caller falls back to the signed encoding.
    if let ExpressionNode::Cast(cast) = expressions.expression(expression) {
        let target = expressions
            .name_path_members(cast.target_type)
            .last()
            .and_then(|name| PrimitiveType::from_name(name.as_str()))?;
        return Some(target.is_signed_integer());
    }
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    descriptor_primitive_is_signed(&descriptor)
}

/// Resolve the leaf primitive type of a runtime storage target (a `data` field
/// or frame slot, possibly through nested fields). Used to pick float vs integer
/// codegen for a binary write.
pub(super) fn resolve_runtime_storage_primitive_type_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    let descriptor = resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    descriptor_primitive_type(&descriptor)
}

/// The arithmetic domain (`T in Wrapping/Saturating/Trapping`, decision 17) of a
/// storage PLACE — read from the `Constrained` wrapper the layout builder records
/// on the slot/field descriptor. Used at the binary-write site to decide whether
/// the target wants the plain wrapping op (Exact/Wrapping) or saturating/trapping
/// overflow handling. Defaults to `Exact` for unconstrained or unresolved places.
pub(super) fn resolve_runtime_storage_arithmetic_domain_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> omega_core::arithmetic::ArithmeticDomain {
    // A domain `as` cast (`x as u8 in Saturating`, decision 17 S2) re-tags the
    // value's domain explicitly, overriding the operand's own type.
    if let ExpressionNode::Cast(cast) = expressions.expression(expression) {
        return cast.domain;
    }
    resolve_runtime_storage_leaf_descriptor_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )
    .map(|descriptor| descriptor.arithmetic_domain())
    .unwrap_or(omega_core::arithmetic::ArithmeticDomain::Exact)
}

/// The scalar primitive type of a VALUE/source expression, for codegen
/// classification (float-vs-integer, byte width). The single funnel every
/// binary-write / convert producer should use so they all agree: a storage PLACE
/// of any shape resolves to its leaf type; a LITERAL classifies from its node (a
/// float literal is `f64` -- the default float width; an integer literal `i64`;
/// a boolean `bool`). Returns `None` for non-scalar / unresolved expressions.
/// (A `Cast` value resolves via its own selection, so it is not classified here.)
pub(super) fn classify_scalar_value_type_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    if let Some(primitive) = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(primitive);
    }
    match expressions.expression(expression) {
        ExpressionNode::Float(_) => Some(PrimitiveType::F64),
        ExpressionNode::Integer(_) => Some(PrimitiveType::I64),
        ExpressionNode::Boolean(_) => Some(PrimitiveType::Bool),
        // An arithmetic sub-expression (a folded `let c = a + b` inlined into a
        // later cast `c as i32`) has the type of its operands. Classify from a
        // resolvable operand: a float operand makes the whole binary float (its
        // width is the float operand's), otherwise the operand's integer type. This
        // lets a convert see through a binary source to pick single vs double
        // precision and the source width.
        ExpressionNode::Binary(binary) => {
            let left = classify_scalar_value_type_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.left,
            );
            let right = classify_scalar_value_type_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                binary.right,
            );
            combine_binary_operand_scalar_types(left, right)
        }
        // A nested cast (`(self.src as f64) as i32` after `wide` is folded into
        // the outer cast) has the type of its TARGET. Without this, the outer
        // cast's source-width re-derivation returned None and the entire write
        // was silently dropped (the f32->f64-local->i32 miscompile). The
        // `as`-value resolves via its own Convert selection; here we only need
        // its result type so a CONSUMING cast can size its source.
        ExpressionNode::Cast(cast) => expressions
            .name_path_members(cast.target_type)
            .last()
            .and_then(|name| PrimitiveType::from_name(name.as_str())),
        _ => None,
    }
}

/// Combine two operands' classified scalar types into the binary's result type.
/// Prefer a place-resolved operand (its width is exact) over a bare literal
/// (which defaults to the widest f64/i64); a float on either side makes the
/// result float. Shared by `classify_scalar_value_type_in_table`'s `Binary`
/// arm and the value-operand width helper so they agree on ONE answer (the
/// scalar-width-rederivation smell — see wiki/architecture).
pub(super) fn combine_binary_operand_scalar_types(
    left: Option<PrimitiveType>,
    right: Option<PrimitiveType>,
) -> Option<PrimitiveType> {
    match (left, right) {
        (Some(l), Some(r)) if l.accepts_float_literal() && !r.accepts_float_literal() => Some(l),
        (Some(l), Some(r)) if r.accepts_float_literal() && !l.accepts_float_literal() => Some(r),
        (Some(l), Some(r)) => {
            // Same float-ness: take the narrower (place-resolved) width.
            if scalar_primitive_rank(r) < scalar_primitive_rank(l) {
                Some(r)
            } else {
                Some(l)
            }
        }
        (l, r) => l.or(r),
    }
}

/// A coarse width rank for picking the narrower of two same-float-ness primitives
/// when classifying a binary's type from its operands (a 4-byte f32/i32 ranks below
/// an 8-byte f64/i64; a literal that resolved to the widest default loses to a
/// place-resolved narrower operand).
fn scalar_primitive_rank(primitive: PrimitiveType) -> u8 {
    match primitive {
        PrimitiveType::Bool
        | PrimitiveType::I8
        | PrimitiveType::U8
        | PrimitiveType::I16
        | PrimitiveType::U16 => 0,
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => 1,
        PrimitiveType::F64
        | PrimitiveType::I64
        | PrimitiveType::U64
        | PrimitiveType::Usize
        | PrimitiveType::Isize
        | PrimitiveType::String => 2,
    }
}

/// Non-table counterpart of [`resolve_runtime_storage_primitive_type_in_table`]:
/// resolve the leaf primitive type of a runtime storage target given as a resolved
/// `Expression`.
pub(super) fn resolve_runtime_storage_primitive_type(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<PrimitiveType> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_primitive_type_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

/// Non-table sibling of [`resolve_runtime_storage_arithmetic_domain_in_table`]
/// (decision 17): the arithmetic domain of a storage target reached through the
/// older `&Expression` path.
pub(super) fn resolve_runtime_storage_arithmetic_domain(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> omega_core::arithmetic::ArithmeticDomain {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_arithmetic_domain_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

/// Non-table sibling of [`resolve_runtime_storage_is_signed_in_table`]: the
/// signedness of a storage target reached through the `&Expression` path.
pub(super) fn resolve_runtime_storage_is_signed(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<bool> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        source_key,
        &delegated_expressions,
        delegated_expression,
    )
}

/// Walk a runtime storage target (frame slot or machine-owned `data` field, plus
/// any nested-field suffix) to its leaf [`TypeLayoutDescriptor`].
fn resolve_runtime_storage_leaf_descriptor_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TypeLayoutDescriptor> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let suffix = path.suffix(1);
    let slot = find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
    .or_else(|| {
        latest_dispatch_frame_slot(input, dispatch_index, |slot| {
            slot_matches_table_path(slot, &path)
        })
    });

    let Some(slot) = slot else {
        // Not a frame slot: most `data` fields are machine-owned. Resolve the
        // leaf type descriptor through that path instead.
        let collection = resolve_machine_owned_collection_in_table(
            &input.layouts,
            input.entry_key.machine,
            source_key.machine,
            expressions,
            expression,
        )?;
        return Some(collection.type_descriptor.clone());
    };

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        type_descriptor: slot.type_descriptor.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };

    let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
    for (field_name, field_symbol, field_index) in suffix.iter() {
        cursor =
            resolve_nested_field_layout_step(&input.layouts, cursor, field_name, field_symbol, field_index)?;
    }
    Some(cursor.type_descriptor().clone())
}

fn descriptor_primitive_is_signed(descriptor: &TypeLayoutDescriptor) -> Option<bool> {
    Some(descriptor_primitive_type(descriptor)?.is_signed_integer())
}

pub(super) fn descriptor_primitive_type(descriptor: &TypeLayoutDescriptor) -> Option<PrimitiveType> {
    match descriptor {
        TypeLayoutDescriptor::Named { name, .. } => PrimitiveType::from_name(name),
        TypeLayoutDescriptor::Constrained { base_type, .. } => descriptor_primitive_type(base_type),
        _ => None,
    }
}

pub(super) fn resolve_fixed_array_length_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    if let Some(slot) = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        if let Some((_, length)) = slot.type_descriptor.fixed_array() {
            return Some(length);
        }

        let path = normalized_storage_name_path_in_table(expressions, expression)?;
        if path.len() > 1 {
            let root_field = FieldLayout {
                symbol: slot.symbol,
                name: slot.name.clone(),
                offset: 0,
                type_symbol: slot.type_descriptor.storage_symbol(),
                type_name: "".into(),
                type_descriptor: slot.type_descriptor.clone(),
                layout: TypeLayout {
                    size: slot.byte_size,
                    alignment: slot.alignment,
                },
            };
            let mut cursor = NestedFieldLayoutCursor::from_root(&root_field);
            for (field_name, field_symbol, field_index) in path.suffix(1).iter() {
                cursor = resolve_nested_field_layout_step(
                    &input.layouts,
                    cursor,
                    field_name,
                    field_symbol,
                    field_index,
                )?;
            }
            let (_, length) = cursor.type_descriptor().fixed_array()?;
            return Some(length);
        }
    }

    if let Some(indexed) = indexed_target_path_in_table(expressions, expression) {
        let collection_slot = runtime_frame_slot_for_expression_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            indexed.collection,
        )?;
        let element_descriptor = inline_fixed_array_element_type(&collection_slot.type_descriptor)?;
        let element_layout = descriptor_layout(input, element_descriptor);
        let root_field = FieldLayout {
            symbol: collection_slot.symbol,
            name: collection_slot.name.clone(),
            offset: 0,
            type_symbol: element_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: element_descriptor.clone(),
            layout: element_layout,
        };
        let cursor = resolve_indexed_target_suffix_cursor_in_table(
            &input.layouts,
            NestedFieldLayoutCursor::from_root(&root_field),
            expressions,
            indexed.suffix_root,
        )?;
        let (_, length) = cursor.type_descriptor().fixed_array()?;
        return Some(length);
    }

    let collection = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    )?;
    let (_, length) = collection.type_descriptor.fixed_array()?;
    Some(length)
}

pub(super) fn resolve_fixed_array_length(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<usize> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_fixed_array_length_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

pub(super) fn resolve_runtime_frame_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_indexed_target_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

pub(super) fn resolve_runtime_frame_base_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameBaseIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_base_indexed_target_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

pub(super) fn resolve_runtime_frame_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    // Dynamic indexing for inline frame arrays needs a base-offset lowering path rather than the
    // descriptor-based slice/view path used by runtime frame indexed targets.
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }
    let descriptor_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame
        || index_place.region != RuntimeStorageRegion::RuntimeFrame
    {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        indexed.suffix_root,
    )?;

    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        index_offset: index_place.byte_offset,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

/// The primitive type of a frame-indexed target's FIELD (`items[i].name`),
/// for guard/operand classification: the plain leaf-descriptor resolver walks
/// NAME paths and cannot see through an Index node, so slice-indexed places
/// resolve their suffix cursor against the collection slot's element type
/// here instead.
pub(super) fn resolve_runtime_frame_indexed_primitive_type_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<PrimitiveType> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let cursor = NestedFieldLayoutCursor::from_root(&root_field);
    let cursor = resolve_indexed_target_suffix_cursor_in_table(
        &input.layouts,
        cursor,
        expressions,
        indexed.suffix_root,
    )?;
    descriptor_primitive_type(cursor.type_descriptor())
}

pub(super) fn resolve_runtime_frame_indexed_target_near_slot_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    preferred_descriptor_offset: usize,
) -> Option<RuntimeFrameIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_path = normalized_storage_name_path_in_table(expressions, indexed.collection)?;
    let collection_slot = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.byte_offset == preferred_descriptor_offset
                && slot_matches_table_path(slot, &collection_path))
            .then_some(slot)
        })?;
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }

    let index_place = resolve_runtime_storage_place_near_frame_offset_in_table(
        input,
        dispatch_index,
        expressions,
        indexed.index,
        collection_slot.byte_offset,
    )
    .or_else(|| {
        resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            collection_slot.source_key,
            expressions,
            indexed.index,
        )
    })?;
    if index_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        indexed.suffix_root,
    )?;

    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: collection_slot.byte_offset,
        index_offset: index_place.byte_offset,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

fn resolve_runtime_storage_place_near_frame_offset_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    minimum_byte_offset: usize,
) -> Option<RuntimeStoragePlace> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let slot = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.byte_offset >= minimum_byte_offset
                && slot_matches_table_path(slot, &path))
            .then_some(slot)
        })
        .min_by_key(|slot| slot.byte_offset)?;

    let suffix = path.suffix(1);
    if let Some(place) = runtime_slice_descriptor_member_place(
        input,
        slot.byte_offset,
        slot.byte_size,
        suffix.iter().next().map(|(name, _, _)| name.as_str()),
        suffix.iter().count(),
    ) {
        return Some(place);
    }

    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        type_descriptor: slot.type_descriptor.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) =
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset,
        byte_count: layout.size,
    })
}

pub(super) fn resolve_runtime_frame_base_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameBaseIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let element_descriptor = inline_fixed_array_element_type(&collection_slot.type_descriptor)?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if index_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        indexed.suffix_root,
    )?;

    Some(RuntimeFrameBaseIndexedTarget {
        base_byte_offset: collection_slot.byte_offset,
        index_offset: index_place.byte_offset,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMachineIndexedTarget {
    pub(super) base_byte_offset: usize,
    pub(super) index_region: RuntimeStorageRegion,
    pub(super) index_offset: usize,
    pub(super) element_byte_size: usize,
    pub(super) field_byte_offset: usize,
    pub(super) byte_count: usize,
}

pub(super) fn resolve_runtime_machine_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeMachineIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if !matches!(
        index_place.region,
        RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine
    ) {
        return None;
    }

    let element_descriptor = collection.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        indexed.suffix_root,
    )?;

    Some(RuntimeMachineIndexedTarget {
        base_byte_offset: collection.byte_offset,
        index_region: index_place.region,
        index_offset: index_place.byte_offset,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

pub(super) fn resolve_runtime_machine_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeMachineIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_machine_indexed_target_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

pub(super) fn resolve_runtime_pointee_slot_offset(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimePointeeTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_pointee_slot_offset_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

pub(super) fn resolve_runtime_pointee_slot_offset_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeTarget> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    if slot.byte_size != input.runtime_abi.pointer_size {
        return None;
    }
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    let suffix = path.suffix(1);
    let (field_byte_offset, field_layout) = if path.len() <= 1 {
        (0, pointee_layout)
    } else {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: pointee_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: pointee_descriptor.clone(),
            layout: pointee_layout,
        };
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?
    };
    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: slot.byte_offset,
        field_byte_offset,
        pointee_byte_size: field_layout.size,
    })
}

pub(super) fn resolve_runtime_pointee_fixed_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeTarget> {
    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    let collection_descriptor = slot.type_descriptor.reference_referee()?;
    let element_descriptor = collection_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_index = usize::try_from(fixed.index).ok()?;
    let element_offset = element_index.checked_mul(element_layout.size)?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
    )?;

    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: pointee_pointer_offset(input, place)?,
        field_byte_offset: element_offset.checked_add(field_byte_offset)?,
        pointee_byte_size: field_layout.size,
    })
}

pub(super) fn resolve_runtime_pointee_fixed_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimePointeeTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_pointee_fixed_indexed_target_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

fn pointee_pointer_offset(
    input: &InstructionSelectionInput<'_>,
    place: RuntimeStoragePlace,
) -> Option<usize> {
    if place.byte_count == input.runtime_abi.pointer_size {
        return Some(place.byte_offset);
    }

    let descriptor = input.runtime_abi.slice_descriptor();
    if place.byte_count == descriptor.total_size() {
        return place.byte_offset.checked_add(descriptor.ptr_offset());
    }

    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimePointeeTarget {
    pub(super) pointer_byte_offset: usize,
    pub(super) field_byte_offset: usize,
    pub(super) pointee_byte_size: usize,
}

fn slot_matches_table_path(
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    path: &StorageNamePath<'_>,
) -> bool {
    slot_matches_root(slot.symbol, path.head_symbol())
        || path.member(0).is_some_and(|name| *name == slot.name)
}

fn slot_matches_root(slot_symbol: SymbolHandle, root_symbol: SymbolHandle) -> bool {
    slot_symbol.is_valid() && root_symbol.is_valid() && slot_symbol == root_symbol
}

/// Byte size of one element of the slice (or fixed array) named by `expression`,
/// from the resolved frame slot's element type. Used to scale subslice pointer
/// arithmetic on a runtime slice descriptor.
pub(super) fn resolve_slice_element_byte_size_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    let slot =
        runtime_frame_slot_for_expression_in_table(input, dispatch_index, source_key, expressions, expression)?;
    let element_descriptor = slot.type_descriptor.element_type()?;
    Some(descriptor_layout(input, element_descriptor).size)
}

fn runtime_frame_slot_for_expression_in_table<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;

    find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
    .or_else(|| {
        latest_dispatch_frame_slot(input, dispatch_index, |slot| {
            slot_matches_table_path(slot, &path)
        })
    })
}

fn find_runtime_frame_slot_for_path<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    matches_path: impl Fn(&omega_runtime_storage::RuntimeFrameSlot) -> bool,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && matches_path(slot))
            .then_some(slot)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .find_map(|(_, slot)| {
                    (slot.dispatch_index == dispatch_index
                        && state_key_matches_statement_source(slot.source_key, source_key)
                        && matches_path(slot))
                    .then_some(slot)
                })
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index <= dispatch_index
                        && slot.source_key == source_key
                        && matches_path(slot))
                    .then_some(slot)
                })
                .max_by_key(|slot| slot.dispatch_index)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index <= dispatch_index
                        && state_key_matches_statement_source(slot.source_key, source_key)
                        && matches_path(slot))
                    .then_some(slot)
                })
                .max_by_key(|slot| slot.dispatch_index)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .find_map(|(_, slot)| {
                    (slot.source_key == source_key && matches_path(slot)).then_some(slot)
                })
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .find_map(|(_, slot)| {
                    (state_key_matches_statement_source(slot.source_key, source_key)
                        && matches_path(slot))
                    .then_some(slot)
                })
        })
        .or_else(|| {
            // Last resort: a slot owned by an OUTER call scope whose source_key
            // does not match the querying source. This happens when a caller's
            // `&mut` parameter is aliased into a callee arm AND the caller body
            // was split into dispatch segments by a dispatched (looping) call --
            // the arm's write resolves against the leaf source, which never
            // matches the caller-parameter slot under any source-scoped clause
            // above. The path match is symbol-specific for a resolved parameter
            // reference, so it identifies exactly that one slot; take the nearest
            // dispatch at or before this one (segments of one control state share
            // the slot).
            input
                .runtime_storage
                .frame_slots
                .iter()
                .filter_map(|(_, slot)| {
                    (slot.dispatch_index <= dispatch_index && matches_path(slot)).then_some(slot)
                })
                .max_by_key(|slot| slot.dispatch_index)
        })
}

fn latest_dispatch_frame_slot<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    matches_path: impl Fn(&omega_runtime_storage::RuntimeFrameSlot) -> bool,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .fold(None, |matched_slot, (_, slot)| {
            if slot.dispatch_index == dispatch_index && matches_path(slot) {
                Some(slot)
            } else {
                matched_slot
            }
        })
}

fn resolve_runtime_fixed_indexed_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let index = usize::try_from(fixed.index).ok()?;
    if let Some(slot) = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    ) {
        let element_descriptor = inline_fixed_array_element_type(&slot.type_descriptor)?;
        let element_layout = descriptor_layout(input, element_descriptor);
        let element_offset = index.checked_mul(element_layout.size)?;
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: element_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: element_descriptor.clone(),
            layout: element_layout,
        };
        let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
            input,
            &root_field,
            expressions,
            fixed.suffix_root,
        )?;

        return Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot
                .byte_offset
                .checked_add(element_offset)?
                .checked_add(field_byte_offset)?,
            byte_count: field_layout.size,
        });
    }

    let collection = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        fixed.collection,
    )?;
    let element_descriptor = inline_fixed_array_element_type(&collection.type_descriptor)?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_offset = index.checked_mul(element_layout.size)?;
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
    )?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::Machine,
        byte_offset: collection
            .byte_offset
            .checked_add(element_offset)?
            .checked_add(field_byte_offset)?,
        byte_count: field_layout.size,
    })
}

pub(super) fn resolve_runtime_frame_fixed_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameFixedIndexedTarget> {
    if let Some(target) = resolve_runtime_frame_fixed_indexed_storage_path_target_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(target);
    }

    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }
    let descriptor_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_index = usize::try_from(fixed.index).ok()?;
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
    )?;

    Some(RuntimeFrameFixedIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        element_index,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

fn resolve_runtime_frame_fixed_indexed_storage_path_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameFixedIndexedTarget> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    let element_index = path
        .member_index(0)
        .or_else(|| root_member_fixed_index(path.member(0)?))?;
    let collection_slot =
        find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
            slot_matches_table_path(slot, &path)
        })?;
    if runtime_frame_slot_is_inline_fixed_array_storage(input, collection_slot) {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) =
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, path.suffix(1).iter())?;

    Some(RuntimeFrameFixedIndexedTarget {
        descriptor_offset: collection_slot.byte_offset,
        element_index,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

fn root_member_fixed_index(member: &omega_checked_trees::name::Identifier) -> Option<usize> {
    let (_, suffix) = member.as_str().rsplit_once('[')?;
    suffix.strip_suffix(']')?.parse().ok()
}

pub(super) fn resolve_runtime_frame_fixed_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameFixedIndexedTarget> {
    let mut delegated_expressions = ExpressionTable::default();
    let delegated_expression = delegated_expressions.insert_tree(expression);
    resolve_runtime_frame_fixed_indexed_target_in_table(input, dispatch_index, source_key, &delegated_expressions, delegated_expression)
}

#[derive(Debug, Clone, Copy)]
struct TableFixedIndexedTargetPath {
    collection: ExpressionHandle,
    index: i64,
    suffix_root: ExpressionHandle,
}

#[derive(Debug, Clone, Copy)]
struct TableIndexedTargetPath {
    collection: ExpressionHandle,
    index: ExpressionHandle,
    suffix_root: ExpressionHandle,
}

fn fixed_indexed_target_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TableFixedIndexedTargetPath> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => fixed_indexed_target_path_in_table(table, *target),
        ExpressionNode::Member(member) => {
            let path = fixed_indexed_target_path_in_table(table, member.receiver)?;
            Some(TableFixedIndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
            })
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(path) = fixed_indexed_target_path_in_table(table, indexed.collection) {
                return Some(TableFixedIndexedTargetPath {
                    collection: path.collection,
                    index: path.index,
                    suffix_root: expression,
                });
            }
            let ExpressionNode::Integer(index) = table.expression(indexed.index) else {
                return None;
            };
            Some(TableFixedIndexedTargetPath {
                collection: indexed.collection,
                index: *index,
                suffix_root: expression,
            })
        }
        _ => None,
    }
}

fn indexed_target_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TableIndexedTargetPath> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => indexed_target_path_in_table(table, *target),
        ExpressionNode::Member(member) => {
            let path = indexed_target_path_in_table(table, member.receiver)?;
            Some(TableIndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
            })
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(path) = indexed_target_path_in_table(table, indexed.collection) {
                return Some(TableIndexedTargetPath {
                    collection: path.collection,
                    index: path.index,
                    suffix_root: expression,
                });
            }
            Some(TableIndexedTargetPath {
                collection: indexed.collection,
                index: indexed.index,
                suffix_root: expression,
            })
        }
        _ => None,
    }
}

fn resolve_indexed_target_suffix_layout_in_table(
    input: &InstructionSelectionInput<'_>,
    root_field: &FieldLayout,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(usize, TypeLayout)> {
    let cursor = NestedFieldLayoutCursor::from_root(root_field);
    let cursor = resolve_indexed_target_suffix_cursor_in_table(
        &input.layouts,
        cursor,
        expressions,
        expression,
    )?;
    Some((cursor.byte_offset(), cursor.layout()))
}

fn resolve_indexed_target_suffix_cursor_in_table<'layout>(
    layouts: &'layout omega_layout::LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(target) => {
            resolve_indexed_target_suffix_cursor_in_table(layouts, cursor, expressions, *target)
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(collection_cursor) = resolve_indexed_target_suffix_cursor_in_table(
                layouts,
                cursor,
                expressions,
                indexed.collection,
            ) else {
                return Some(cursor);
            };

            let ExpressionNode::Integer(index) = expressions.expression(indexed.index) else {
                return None;
            };
            apply_fixed_array_index_to_cursor(collection_cursor, usize::try_from(*index).ok()?)
        }
        ExpressionNode::Member(member) => {
            let cursor = resolve_indexed_target_suffix_cursor_in_table(
                layouts,
                cursor,
                expressions,
                member.receiver,
            )?;
            resolve_nested_field_layout_step(
                layouts,
                cursor,
                &member.member,
                member.member_symbol,
                None,
            )
        }
        _ => None,
    }
}

fn apply_fixed_array_index_to_cursor<'layout>(
    cursor: NestedFieldLayoutCursor<'layout>,
    index: usize,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    let (element_type, length) = cursor.type_descriptor().fixed_array()?;
    if index >= length {
        return None;
    }

    let element_layout = TypeLayout {
        size: cursor.layout().size / length,
        alignment: cursor.layout().alignment,
    };

    Some(NestedFieldLayoutCursor::from_indexed_fixed_array_element(
        cursor,
        index,
        element_type,
        element_layout,
    ))
}

fn descriptor_layout(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> TypeLayout {
    match descriptor {
        TypeLayoutDescriptor::Reference { .. } => {
            return TypeLayout {
                size: input.runtime_abi.pointer_size,
                alignment: input.runtime_abi.pointer_alignment,
            };
        }
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            return descriptor_layout(input, base_type);
        }
        TypeLayoutDescriptor::FixedArray {
            element_type,
            length,
        } => {
            let element = descriptor_layout(input, element_type);
            return TypeLayout {
                size: element.size.saturating_mul(*length),
                alignment: element.alignment,
            };
        }
        TypeLayoutDescriptor::Slice { .. } => {
            let descriptor = input.runtime_abi.slice_descriptor();
            return TypeLayout {
                size: descriptor.total_size(),
                alignment: descriptor.align(),
            };
        }
        TypeLayoutDescriptor::DynamicTrait { .. } => {
            let descriptor = input.runtime_abi.slice_descriptor();
            return TypeLayout {
                size: descriptor.total_size(),
                alignment: descriptor.align(),
            };
        }
        TypeLayoutDescriptor::Named { symbol, name } => {
            let type_symbol = *symbol;
            if let Some(primitive_type) = PrimitiveType::from_name(name) {
                return primitive_layout(input, primitive_type);
            }

            if let Some(layout) = builtin_type_layout(input, type_symbol) {
                return layout;
            }

            if type_symbol.is_valid() {
                if let Some(layout) = input
                    .layouts
                    .data_layouts
                    .iter()
                    .find(|(_, layout)| layout.symbol == type_symbol)
                    .map(|(_, layout)| layout.layout)
                {
                    return layout;
                }

                if let Some(layout) = input
                    .layouts
                    .machine_layouts
                    .iter()
                    .find(|(_, layout)| layout.symbol == type_symbol)
                    .map(|(_, layout)| layout.layout)
                {
                    return layout;
                }
            }
        }
        TypeLayoutDescriptor::Unit => {}
    }

    TypeLayout::default()
}

fn inline_fixed_array_element_type(
    descriptor: &TypeLayoutDescriptor,
) -> Option<&TypeLayoutDescriptor> {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type, .. } => {
            inline_fixed_array_element_type(base_type)
        }
        TypeLayoutDescriptor::FixedArray { element_type, .. } => Some(element_type),
        _ => None,
    }
}

fn runtime_frame_slot_is_inline_fixed_array_storage(
    input: &InstructionSelectionInput<'_>,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
) -> bool {
    // A direct (non-reference) fixed array is stored inline when the slot holds
    // the whole array rather than a slice descriptor. Compare against the array's
    // own layout size rather than the descriptor size: a previous heuristic keyed
    // off `byte_size != slice_descriptor_size`, which misclassified inline arrays
    // that happen to be exactly the descriptor size (e.g. `[T; 2]` with 8-byte T)
    // as descriptors, dereferencing the inline element data as a wild pointer.
    if inline_fixed_array_element_type(&slot.type_descriptor).is_none() {
        return false;
    }
    slot.byte_size == descriptor_layout(input, &slot.type_descriptor).size
}

fn builtin_type_layout(
    input: &InstructionSelectionInput<'_>,
    type_symbol: SymbolHandle,
) -> Option<TypeLayout> {
    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::UInt) {
        return Some(TypeLayout {
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        });
    }

    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::Int) {
        return Some(TypeLayout {
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        });
    }

    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::Real) {
        return Some(TypeLayout {
            size: 8,
            alignment: 8,
        });
    }

    None
}

fn primitive_layout(
    input: &InstructionSelectionInput<'_>,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    match primitive_type {
        PrimitiveType::Bool | PrimitiveType::I8 | PrimitiveType::U8 => TypeLayout {
            size: 1,
            alignment: 1,
        },
        PrimitiveType::I16 | PrimitiveType::U16 => TypeLayout {
            size: 2,
            alignment: 2,
        },
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => TypeLayout {
            size: 4,
            alignment: 4,
        },
        PrimitiveType::F64 | PrimitiveType::I64 | PrimitiveType::U64 => TypeLayout {
            size: 8,
            alignment: 8,
        },
        PrimitiveType::Usize | PrimitiveType::Isize => TypeLayout {
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        },
        PrimitiveType::String => {
            let descriptor = input.runtime_abi.text_descriptor();
            TypeLayout {
                size: descriptor.total_size(),
                alignment: descriptor.align(),
            }
        }
    }
}

mod fixed_array_slices;
pub(crate) mod mutation;
mod slice_descriptors;
mod static_values;
mod storage_copy;
mod string_values;
mod subslice_copy;

use super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
};
use super::super::lookups::state_mutation_for_statement;
use super::text_writes::runtime_text_builder_write_in_table_emit;
use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::storage_places::{
    resolve_runtime_frame_base_double_indexed_source_in_table,
    resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_machine_double_indexed_source_in_table,
    resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_double_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::{
    Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction,
    SelectedInstructionKind,
};
use omega_control_flow::StateKey;
use omega_layout::{DataShape, ENUM_TAG_BYTES};
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use psi_arena::Arena;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableIndexedExpression,
    TableMemberExpression,
};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;
pub(crate) use static_values::RuntimeStaticValues;
use static_values::invalidate_runtime_static_value_in_table;

pub(in crate::selection::runtime_dispatch) use mutation::{
    resolve_runtime_text_equals_operand_in_table, select_runtime_convert_mutation_write_in_table,
    signedness_adjusted_operator, signedness_adjusted_operator_for_operands,
};
pub(in crate::selection) use mutation::{
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
    select_runtime_frame_slot_value_write_in_table_with_call_ordinal,
    select_runtime_frame_slot_value_write_in_table_with_source_anchor,
};
pub(in crate::selection) use slice_descriptors::emit_runtime_frame_slot_slice_descriptor_write_in_table;
pub(super) use storage_copy::{
    runtime_storage_copy, runtime_storage_copy_in_table, runtime_storage_fixed_indexed_source_copy,
    runtime_storage_fixed_indexed_source_copy_in_table,
    runtime_storage_indexed_source_copy_in_table, runtime_storage_indirect_copy_in_table,
    runtime_stored_integer_projection_copy_in_table,
};
pub(in crate::selection) use string_values::emit_runtime_frame_slot_text_comparison_write_in_table;

#[derive(Default)]
pub(crate) struct RuntimeStorageWriteScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
}

impl RuntimeStorageWriteScratch {
    pub(crate) fn clear(&mut self) {
        self.expressions.clear();
        self.mutable_expressions.clear();
        self.resolved_segment_expressions.clear();
    }
}

/// True when this statement has a GUARDED assignment-value leaf expansion — a
/// value-call assigned to the mutation target whose callee transitions to
/// constant-returning leaf STATES (`self.f = self.g()` where `g` does `transition
/// c { .. -> a() .. -> b() }`). In that case the leaf expansions already write the
/// GUARDED result into the call-result slot AND copy it to the mutation target
/// (`select_runtime_leaf_assignment_value_target_copy`), so the Mutation op's own
/// storage-write is redundant AND WRONG: it re-materializes the callee's FIRST leaf
/// terminal as a CONSTANT and copies it unconditionally, overwriting the guarded
/// value (TASKS_FS.md deep-fix bug #2). A SINGLE-terminal value-call (`exists`,
/// terminal-value completion) produces no guarded leaf here, so its mutation write
/// stays — that path is correct and must not be skipped.
fn statement_has_guarded_assignment_value_leaf(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .any(|expansion| {
            expansion.dispatch_index == dispatch_index
                && super::state_key_matches_statement_source(expansion.source_key, source_key)
                && expansion.statement_index == statement_index
                && expansion.role == omega_state_calls::StateCallRole::AssignmentValue
                && expansion.target_value.is_valid()
                && expansion.guard_kind != omega_state_guards::StateGuardKind::Always
        })
}

pub(super) fn select_runtime_storage_write_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    scratch: &mut RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::Mutation { .. } => {
            // Deep-fix bug #2: a BARE value-call assigned to the target
            // (`self.f = self.g()`) whose callee transitions to constant leaf STATES
            // is already delivered with the correct GUARDED result by the leaf
            // expansions' target copy. This Mutation storage-write would otherwise
            // re-materialize the callee's FIRST leaf terminal as a constant and
            // overwrite it (TASKS_FS.md bug #2). Skip ONLY for a bare call: when the
            // call is a sub-expression (`self.f = self.g() + 1`, `max(x, self.g())`)
            // the surrounding operator's binary-write path resolves the call operand
            // to its result slot and must still run — mirrors the bare-call guard on
            // the existing call-result copy shortcut (mutation.rs ~L1155).
            if matches!(
                state_mutation_for_statement(
                    input,
                    operation.source_key,
                    operation.statement_index,
                ),
                Some(m) if matches!(
                    input.state_storage.expressions.expression(m.value),
                    ExpressionNode::Call(call) if call.receiver.is_valid()
                )
            ) && statement_has_guarded_assignment_value_leaf(
                input,
                dispatch_index,
                operation.source_key,
                operation.statement_index,
            ) {
                return;
            }
        }
        RuntimeDispatchBodyOperationKind::StateCallResult {
            role,
            call_ordinal,
            target_key,
            value,
            ..
        } => {
            mutation::select_runtime_state_call_result_write(
                input,
                dispatch_index,
                operation.source_key,
                operation.statement_index,
                *role,
                *call_ordinal,
                *target_key,
                *value,
                aliases,
                alias_expressions,
                static_values,
                scratch,
                runtime_value_operands,
                selected_instructions,
            );
            return;
        }
        _ => return,
    }
    let Some(mutation) =
        state_mutation_for_statement(input, operation.source_key, operation.statement_index)
    else {
        return;
    };

    if aliases.is_empty() {
        if select_runtime_storage_mutation_write_in_table_with_scratch(
            input,
            dispatch_index,
            mutation.source_key,
            mutation.statement_index,
            mutation.target,
            mutation.value,
            static_values,
            scratch,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
    } else {
        scratch.expressions.clear();
        let expressions = &mut scratch.expressions;
        let copied_aliases =
            RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases, expressions);
        let target = expressions.copy_from(&input.state_storage.expressions, mutation.target);
        let value = expressions.copy_from(&input.state_storage.expressions, mutation.value);
        let resolved_target = resolve_runtime_alias_binding_handle(
            target,
            mutation.source_key,
            copied_aliases.bindings(),
            expressions,
        );
        let resolved_value = resolve_runtime_alias_binding_handle(
            value,
            mutation.source_key,
            copied_aliases.bindings(),
            expressions,
        );
        if select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
            input,
            dispatch_index,
            mutation.source_key,
            resolved_target.source_key,
            resolved_value.source_key,
            mutation.statement_index,
            &expressions,
            resolved_target.expression,
            resolved_value.expression,
            copied_aliases.bindings(),
            static_values,
            &mut scratch.mutable_expressions,
            &mut scratch.resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
    }

    let (source_machine, source_state) = state_names(input, mutation.source_key);
    let target = input.state_storage.expressions.to_tree(mutation.target);
    let value = input.state_storage.expressions.to_tree(mutation.value);
    mutation::select_runtime_mutation_writes(
        input,
        dispatch_index,
        mutation.source_key,
        mutation.source_key,
        &source_machine,
        &source_state,
        mutation.statement_index,
        &target,
        &value,
        aliases,
        alias_expressions,
        static_values,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn select_runtime_storage_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
    scratch: &mut RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        source_key,
        source_key,
        statement_index,
        &input.state_storage.expressions,
        target,
        value,
        &[],
        static_values,
        &mut scratch.mutable_expressions,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut RuntimeStaticValues,
    mutable_expressions: &mut ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if matches!(
        expressions.expression(value),
        ExpressionNode::StructLiteral(_) | ExpressionNode::ArrayLiteral(_)
    ) {
        let source_expressions = expressions;
        mutable_expressions.clear();
        let expressions = mutable_expressions;
        let copied_aliases =
            RuntimeAliasBuffer::copy_from_bindings(source_expressions, aliases, expressions);
        let target = expressions.copy_from(source_expressions, target);
        let value = expressions.copy_from(source_expressions, value);
        return select_runtime_storage_resolved_mutation_write_in_mutable_table(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            value_source_key,
            statement_index,
            expressions,
            target,
            value,
            copied_aliases.bindings(),
            resolved_segment_expressions,
            static_values,
            runtime_value_operands,
            selected_instructions,
        );
    }

    select_runtime_storage_resolved_scalar_mutation_write_in_table(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        aliases,
        resolved_segment_expressions,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_resolved_mutation_write_in_mutable_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &mut ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    // An ARRAY literal (`[Room { label: "x" }, ..]` or `[1, 2, 3]`) writes
    // element by element through a LITERAL-indexed target (`target[i]`): each
    // element value recurses here, so struct-literal elements expand into
    // their per-field member writes (String descriptor writes included) and
    // scalar elements ride the ordinary static-write path. The literal index
    // resolves to a static frame place (slot offset + element stride), the
    // same fixed-indexed resolution guards already use. Before this arm,
    // array-literal initializers fell through to the scalar path, selected
    // NOTHING, and every element frame slot silently stayed zeroed natively.
    if let ExpressionNode::ArrayLiteral(elements) = *expressions.expression(value) {
        let mut emitted = false;
        let mut any_element_failed = false;
        for offset in 0..elements.count() {
            let element = expressions.expression_handle_at_offset(elements, offset);
            let element_index = expressions.insert(ExpressionNode::Integer(
                psi_numerics::literals::IntegerLiteral::from_value(i64::from(offset)),
            ));
            let element_target =
                expressions.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection: target,
                    index: element_index,
                }));
            // A state call nested in an array literal has already run and
            // materialized its exact occurrence-ranked result slot before the
            // LocalStorage operation reaches this decomposition. Copy that
            // result into the element's resolved destination. Treating the
            // Call as an ordinary scalar element cannot recover its storage
            // place and silently leaves the element at ZII (notably
            // `[convert(0)]` under an expected result-domain-qualified array).
            // Builtin calls remain expression operators and take the ordinary
            // element lowering below.
            let call_result_copy = match expressions.expression(element) {
                ExpressionNode::Call(call)
                    if mutation::builtin_runtime_call_operator_in_table(input, call).is_none() =>
                {
                    mutation::resolve_runtime_table_call_result_source_place(
                        input,
                        dispatch_index,
                        value_source_key,
                        statement_index,
                        expressions,
                        value,
                        element,
                        call,
                        None,
                    )
                    .and_then(|source| {
                        let target = resolve_runtime_storage_place_in_table(
                            input,
                            dispatch_index,
                            target_source_key,
                            expressions,
                            element_target,
                        )?;
                        (source.byte_count == target.byte_count && target.byte_count > 0).then(
                            || {
                                crate::selection::runtime_dispatch::copy_places_direct(
                                    source.region,
                                    source.byte_offset,
                                    target.region,
                                    target.byte_offset,
                                    target.byte_count,
                                )
                            },
                        )
                    })
                }
                _ => None,
            };
            let element_emitted = if let Some(kind) = call_result_copy {
                invalidate_runtime_static_value_in_table(
                    static_values,
                    expressions,
                    element_target,
                );
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_key: operation_source_key,
                    source_statement: statement_index,
                });
                true
            } else {
                select_runtime_storage_resolved_mutation_write_in_mutable_table(
                    input,
                    dispatch_index,
                    operation_source_key,
                    target_source_key,
                    value_source_key,
                    statement_index,
                    expressions,
                    element_target,
                    element,
                    aliases,
                    resolved_segment_expressions,
                    static_values,
                    runtime_value_operands,
                    selected_instructions,
                )
            };
            any_element_failed |= !element_emitted;
            emitted |= element_emitted;
        }
        // PARTIAL construction (some elements landed, this one didn't) is a
        // silent ZII element at runtime -- poison so emission planning
        // rejects it loudly. A FULLY-unserved literal stays a plain `false`:
        // the caller may still serve the whole value by another strategy.
        // Gated on NO aliases: a call-SUBSTITUTED literal (a value-call
        // terminal spliced through the caller) legitimately defers computed
        // members to the call's own delivery machinery, so partiality at this
        // level is not final there (constructor_computed_field's `a: n/100`);
        // the call-terminal position has its own poison in the branch
        // cascade. A SITE-SPELLED literal has no later server.
        if emitted && any_element_failed && aliases.is_empty() {
            push_unlowered_literal_field_poison(
                operation_source_key,
                statement_index,
                selected_instructions,
            );
        }
        return emitted;
    }

    if let ExpressionNode::StructLiteral(struct_literal) = expressions.expression(value).clone() {
        // Construction replaces the WHOLE value. Zero the complete resolved
        // target first so fields omitted from the literal regain their ZII
        // representation instead of retaining bytes from a prior value. This
        // is observable for repeated OpenOptions assignments: `{ read: true }`
        // must clear an earlier `{ write: true, create: true }` before the
        // target encoder reads it. Named fields overwrite the zeroes below.
        let mut emitted = zero_struct_target(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            statement_index,
            expressions,
            target,
            static_values,
            selected_instructions,
        );
        let mut any_field_failed = false;
        // Constructing a CASE (`Command::Move { steps: 70 }`) writes the i32
        // tag prefix before the payload fields. The payload fields then write
        // through the same member path as record fields (their offsets are
        // absolute within the enum value).
        if let Some(case_name) = &struct_literal.case_name {
            emitted |= select_runtime_case_tag_write_in_table(
                input,
                dispatch_index,
                operation_source_key,
                target_source_key,
                statement_index,
                expressions,
                target,
                &struct_literal.type_name,
                case_name,
                static_values,
                selected_instructions,
            );
            // MIXED shapes: case construction replaces the WHOLE value, so
            // every common field the literal does not name resets to ZERO
            // (frozen decision 7's construction rule; ZII keeps that valid).
            // The zeroes ride the ordinary member-write path below so static
            // folds of the member stay coherent. Named common fields are
            // written by the literal-field loop like any other member.
            for (field_name, zero_value) in unnamed_common_field_zero_writes(
                input,
                expressions,
                &struct_literal.type_name,
                struct_literal.fields,
            ) {
                let field_target =
                    expressions.insert(ExpressionNode::Member(TableMemberExpression {
                        receiver: target,
                        member_symbol: SymbolHandle::invalid(),
                        member: field_name,
                        case_variant: None,
                    }));
                let zero_emitted = select_runtime_storage_resolved_mutation_write_in_mutable_table(
                    input,
                    dispatch_index,
                    operation_source_key,
                    target_source_key,
                    value_source_key,
                    statement_index,
                    expressions,
                    field_target,
                    zero_value,
                    aliases,
                    resolved_segment_expressions,
                    static_values,
                    runtime_value_operands,
                    selected_instructions,
                );
                any_field_failed |= !zero_emitted;
                emitted |= zero_emitted;
            }
        }
        for offset in 0..struct_literal.fields.count() {
            let field = expressions
                .struct_field_at_offset(struct_literal.fields, offset)
                .clone();
            // A case construction's PAYLOAD fields must carry the constructed
            // variant, exactly as the destructure/read path tags them -- so the
            // write resolves the same variant-specific offset the read does.
            // Without this, a payload field whose NAME is shared with another
            // variant (e.g. `amount` in both `Deposit(amount)` and
            // `Transfer(to, amount)`) resolves to the FIRST variant's same-named
            // field and clobbers a sibling payload slot (silent miscompile).
            // Common fields (shared across variants) stay untagged.
            let case_variant = struct_literal.case_name.as_ref().and_then(|case_name| {
                case_payload_field_variant_tag(
                    input,
                    &struct_literal.type_name,
                    case_name,
                    &field.name,
                )
            });
            let field_target = expressions.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: target,
                member_symbol: SymbolHandle::invalid(),
                member: field.name,
                case_variant,
            }));
            let field_emitted = select_runtime_storage_resolved_mutation_write_in_mutable_table(
                input,
                dispatch_index,
                operation_source_key,
                target_source_key,
                value_source_key,
                statement_index,
                expressions,
                field_target,
                field.value,
                aliases,
                resolved_segment_expressions,
                static_values,
                runtime_value_operands,
                selected_instructions,
            );
            any_field_failed |= !field_emitted;
            emitted |= field_emitted;
        }
        // PARTIAL construction (the tag and/or sibling fields landed, this
        // field didn't) is a silent ZII field at runtime -- the field-store
        // texteq face (`self.stored = Msg::Pong { y: 5, z: self.name ==
        // "omega" }` ran native 72 / interp 70). Poison so emission planning
        // rejects it with the bind-to-a-`let` diagnostic. A FULLY-unserved
        // literal stays a plain `false`: the caller may still serve the whole
        // value by another strategy.
        // Same no-aliases gate as the array arm: call-substituted literals
        // defer computed members to the call's delivery machinery (partial
        // here is not final); site-spelled literals have no later server.
        if emitted && any_field_failed && aliases.is_empty() {
            push_unlowered_literal_field_poison(
                operation_source_key,
                statement_index,
                selected_instructions,
            );
        }
        return emitted;
    }

    select_runtime_storage_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        aliases,
        resolved_segment_expressions,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_resolved_scalar_mutation_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    select_runtime_storage_resolved_scalar_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        aliases,
        resolved_segment_expressions,
        static_values,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_storage_resolved_scalar_mutation_write_in_table_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    aliases: &[RuntimeAliasBinding],
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let value_tree = expressions.to_tree(value);
    let simplified_value_tree = mutation::simplify_runtime_expression_with_state_locals(
        input,
        value_source_key,
        statement_index,
        &value_tree,
    );
    if !aliases.is_empty() || simplified_value_tree != value_tree {
        let target_tree = expressions.to_tree(target);
        let (source_machine, source_state) = state_names(input, operation_source_key);
        let selected_before = selected_instructions.len();
        mutation::select_runtime_resolved_target_value_source_mutation_writes(
            input,
            dispatch_index,
            operation_source_key,
            target_source_key,
            value_source_key,
            &source_machine,
            &source_state,
            statement_index,
            &target_tree,
            &simplified_value_tree,
            aliases,
            expressions,
            static_values,
            resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        );
        if selected_instructions.len() > selected_before {
            return true;
        }
    }

    // Atomic load/store wrappers are semantic carriers, not transparent
    // arithmetic. Select them before static folding or ordinary place copies
    // can erase their ordering commitment.
    if let Some(kind) = mutation::select_runtime_atomic_load_or_store_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    // A literal write records the target as a known constant for later folds.
    if let Some(kind) = mutation::select_runtime_static_mutation_write_in_table(
        input,
        dispatch_index,
        target_source_key,
        statement_index,
        expressions,
        target,
        value,
        static_values,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    // A numeric `as` cast loads the source, converts between int/float
    // representations, and stores the result.
    if let Some(kind) = mutation::select_runtime_convert_mutation_write_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    if let Some(kind) = mutation::select_runtime_stored_integer_mutation_write_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        static_values,
        runtime_value_operands,
    ) {
        invalidate_runtime_static_value_in_table(static_values, expressions, target);
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    // Copies move a runtime value into the target. Whatever constant the target
    // previously folded to is now wrong, so forget it: a later read of the same
    // place in this state must come from live storage. Without this, a chain
    // like `v = 5; v = src; w = v + 1;` would fold the stale `v == 5` and
    // compute the wrong `w`.
    if let Some(kind) = storage_copy::runtime_stored_integer_projection_copy_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        expressions,
        target,
        value,
        runtime_value_operands,
    )
    .or_else(|| {
        subslice_copy::runtime_fixed_array_subslice_indexed_source_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        storage_copy::runtime_storage_indexed_source_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        storage_copy::runtime_storage_fixed_indexed_source_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        storage_copy::runtime_storage_indirect_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    })
    .or_else(|| {
        storage_copy::runtime_storage_copy_in_table(
            input,
            dispatch_index,
            target_source_key,
            value_source_key,
            expressions,
            target,
            value,
        )
    }) {
        invalidate_runtime_static_value_in_table(static_values, expressions, target);
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    // String writes target a String descriptor, not an integer-foldable place.
    if let Some(kind) = string_values::select_runtime_string_mutation_write_in_table(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        statement_index,
        expressions,
        target,
        value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    // Binary read-modify-write resolves its operands against the pre-write
    // static state (preserving a first read's fold), then invalidates the target
    // itself.
    if let Some(kind) = mutation::select_runtime_binary_mutation_write_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        statement_index,
        expressions,
        target,
        value,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        return true;
    }

    let target_tree = expressions.to_tree(target);
    let selected_before = selected_instructions.len();
    let (source_machine, source_state) = state_names(input, operation_source_key);
    mutation::select_runtime_resolved_target_value_source_mutation_writes(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        value_source_key,
        &source_machine,
        &source_state,
        statement_index,
        &target_tree,
        &simplified_value_tree,
        aliases,
        expressions,
        static_values,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
    if selected_instructions.len() > selected_before {
        return true;
    }

    resolved_segment_expressions.clear();
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(expressions, aliases, resolved_segment_expressions);
    if runtime_text_builder_write_in_table_emit(
        input,
        dispatch_index,
        operation_source_key,
        target_source_key,
        statement_index,
        expressions,
        target,
        resolved_segment_expressions,
        &|expressions, expression| {
            resolve_runtime_alias_binding_handle(
                expression,
                operation_source_key,
                copied_aliases.bindings(),
                expressions,
            )
            .expression
        },
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation_source_key,
                source_statement: statement_index,
            });
        },
    ) {
        return true;
    }

    false
}

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (Identifier, Identifier) {
    input.control_flow.state_names_by_key_cloned(key)
}

/// Push the `UnloweredCaseLiteralField` POISON for a literal decomposition
/// whose construction went PARTIAL: siblings (and/or the case tag) landed but
/// one member's write emitted nothing, so at runtime the member silently reads
/// ZII 0 (first the cast-in-payload face, then text-equality payloads, on two
/// separate cascades). Emission planning rejects the marker with the
/// bind-to-a-`let` diagnostic; the marker is zero bytes and the partial writes
/// never encode. Mirrors the branch-side construction cascade's poison
/// (branches/mutation.rs).
fn push_unlowered_literal_field_poison(
    operation_source_key: StateKey,
    statement_index: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering:
                omega_abstract_operations::StateGuardLowering::UnloweredCaseLiteralField,
            operator: omega_abstract_operations::StateGuardOperator::Equal,
            storage_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
            byte_offset: 0,
            byte_size: 0,
            expected_value: 0,
            has_storage: false,
            is_float: false,
        },
        source_key: operation_source_key,
        source_statement: statement_index,
    });
}

/// Zero an aggregate target in machine/frame storage, retaining any deref or
/// runtime-index path needed to reach the aggregate.
/// Struct/case construction is whole-value replacement, and omitted fields are
/// represented by zero. Chunked scalar writes keep the operation target-neutral
/// while covering arbitrary record sizes and padding.
#[allow(clippy::too_many_arguments)]
fn zero_struct_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    static_values: &mut RuntimeStaticValues,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some((place, byte_count)) = resolve_struct_target_place(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) else {
        return false;
    };
    if byte_count == 0 {
        return false;
    }

    invalidate_runtime_static_value_in_table(static_values, expressions, target);
    let mut zeroed = 0usize;
    while zeroed < byte_count {
        let step = match byte_count - zeroed {
            remaining if remaining >= 8 => 8,
            remaining if remaining >= 4 => 4,
            remaining if remaining >= 2 => 2,
            _ => 1,
        };
        let chunk_target = place
            .with_step(PlaceStep::ConstOffset(zeroed))
            .expect("a zero-fill offset merges into the place's trailing const step");
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WritePlaceInteger {
                target: chunk_target,
                value: 0,
                byte_size: step,
            },
            source_key: operation_source_key,
            source_statement: statement_index,
        });
        zeroed += step;
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn resolve_struct_target_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> Option<(Place, usize)> {
    if let Some(indexed) = resolve_runtime_pointee_double_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        return Some((indexed.place()?, indexed.byte_count));
    }

    if let Some(indexed) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let place = Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            indexed.descriptor_offset,
        )
        .with_step(PlaceStep::Deref)?
        .with_step(PlaceStep::ScaledIndex {
            index_region: indexed.index_region,
            index_offset: indexed.index_offset,
            index_byte_size: indexed.index_byte_size,
            element_byte_size: indexed.element_byte_size,
        })?
        .with_step(PlaceStep::ConstOffset(indexed.field_byte_offset))?;
        return Some((place, indexed.byte_count));
    }

    if let Some(indexed) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let place = Place::at(RuntimeStorageRegion::RuntimeFrame, indexed.base_byte_offset)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
            })?
            .with_step(PlaceStep::ConstOffset(indexed.field_byte_offset))?;
        return Some((place, indexed.byte_count));
    }

    if let Some(indexed) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let place = Place::at(RuntimeStorageRegion::Machine, indexed.base_byte_offset)
            .with_step(PlaceStep::ScaledIndex {
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
            })?
            .with_step(PlaceStep::ConstOffset(indexed.field_byte_offset))?;
        return Some((place, indexed.byte_count));
    }

    if let Some(indexed) = resolve_runtime_frame_base_double_indexed_source_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let place = Place::at(RuntimeStorageRegion::RuntimeFrame, indexed.base_byte_offset)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: indexed.outer_index_offset,
                index_byte_size: indexed.outer_index_byte_size,
                element_byte_size: indexed.outer_stride,
            })?
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: indexed.inner_index_offset,
                index_byte_size: indexed.inner_index_byte_size,
                element_byte_size: indexed.inner_stride,
            })?
            .with_step(PlaceStep::ConstOffset(indexed.field_byte_offset))?;
        return Some((place, indexed.byte_count));
    }

    if let Some(indexed) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let place = Place::at(RuntimeStorageRegion::Machine, indexed.base_byte_offset)
            .with_step(PlaceStep::ScaledIndex {
                index_region: indexed.outer_index_region,
                index_offset: indexed.outer_index_offset,
                index_byte_size: indexed.outer_index_byte_size,
                element_byte_size: indexed.outer_stride,
            })?
            .with_step(PlaceStep::ScaledIndex {
                index_region: indexed.inner_index_region,
                index_offset: indexed.inner_index_offset,
                index_byte_size: indexed.inner_index_byte_size,
                element_byte_size: indexed.inner_stride,
            })?
            .with_step(PlaceStep::ConstOffset(indexed.field_byte_offset))?;
        return Some((place, indexed.byte_count));
    }

    if let Some(indexed) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(field_byte_offset) = indexed.pointee_field_byte_offset()
    {
        let place = Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            indexed.descriptor_offset,
        )
        .with_step(PlaceStep::Deref)?
        .with_step(PlaceStep::ConstOffset(field_byte_offset))?;
        return Some((place, indexed.byte_count));
    }

    if let Some(pointee) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let place = Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointee.pointer_byte_offset,
        )
        .with_step(PlaceStep::Deref)?
        .with_step(PlaceStep::ConstOffset(pointee.field_byte_offset))?;
        return Some((place, pointee.pointee_byte_size));
    }

    if let Some(pointee) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) {
        let place = Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointee.pointer_byte_offset,
        )
        .with_step(PlaceStep::Deref)?
        .with_step(PlaceStep::ConstOffset(pointee.field_byte_offset))?;
        return Some((place, pointee.pointee_byte_size));
    }

    let direct = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    Some((
        Place::at(direct.region, direct.byte_offset),
        direct.byte_count,
    ))
}

/// Write the i32 CASE TAG of a case construction (`target = Type::Case { .. }`)
/// into the tag prefix (offset 0) of the enum-shaped target place. The payload
/// field writes are emitted separately by the struct-literal decomposition; a
/// payload-less brace construction still gets its tag from here.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn select_runtime_case_tag_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation_source_key: StateKey,
    target_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    type_name: &Identifier,
    case_name: &Identifier,
    static_values: &mut RuntimeStaticValues,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(tag) = case_tag_value(&input.layouts, type_name, case_name) else {
        return false;
    };
    let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) else {
        return false;
    };

    // A fresh case construction replaces the whole value: any constant the
    // target previously folded to (e.g. a prior bare-case assignment) is stale.
    invalidate_runtime_static_value_in_table(static_values, expressions, target);
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_integer_direct(
            place.region,
            place.byte_offset,
            tag,
            ENUM_TAG_BYTES,
        ),
        source_key: operation_source_key,
        source_statement: statement_index,
    });
    true
}

/// The tag (variant ordinal) of `Type::Case` in the layout plan, by name.
fn case_tag_value(
    layouts: &omega_layout::LayoutPlan,
    type_name: &Identifier,
    case_name: &Identifier,
) -> Option<i64> {
    let data_layout = layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { variants, .. } = &data_layout.shape else {
        return None;
    };
    layouts
        .variants
        .span_or_empty(*variants)
        .iter()
        .position(|variant| variant.name == *case_name)
        .and_then(|index| i64::try_from(index).ok())
}

/// The `case_variant` tag for a case-construction field write: `Some(case_name)`
/// when `field_name` is one of the case's PAYLOAD fields (so it resolves to that
/// variant's field), `None` when it is a COMMON field shared across variants (or
/// the type is not an enum). Mirrors the destructure/read path's tagging so a
/// field-store construction writes at the same offset the read later resolves.
pub(in crate::selection::runtime_dispatch) fn case_payload_field_variant_tag(
    input: &InstructionSelectionInput<'_>,
    type_name: &Identifier,
    case_name: &Identifier,
    field_name: &Identifier,
) -> Option<Identifier> {
    let data_layout = input
        .layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .map(|(_, data_layout)| data_layout)?;
    let DataShape::Enum { common_fields, .. } = &data_layout.shape else {
        return None;
    };
    let is_common_field = input
        .layouts
        .fields
        .span_or_empty(*common_fields)
        .iter()
        .any(|field| field.name == *field_name);
    if is_common_field {
        None
    } else {
        Some(case_name.clone())
    }
}

/// MIXED shapes: the common fields a case literal does NOT name, paired with a
/// type-shaped ZERO expression (construction zero-initializes them; validation
/// restricts mixed common fields to scalar primitives, so one literal each).
/// Pure sums and record types yield nothing.
fn unnamed_common_field_zero_writes(
    input: &InstructionSelectionInput<'_>,
    expressions: &mut ExpressionTable,
    type_name: &Identifier,
    literal_fields: psi_arena::HandleSpan<psi_checked_trees::expression::TableStructLiteralField>,
) -> Vec<(Identifier, ExpressionHandle)> {
    let Some(data_layout) = input
        .layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == *type_name)
        .map(|(_, data_layout)| data_layout)
    else {
        return Vec::new();
    };
    let DataShape::Enum { common_fields, .. } = &data_layout.shape else {
        return Vec::new();
    };

    let named: Vec<Identifier> = (0..literal_fields.count())
        .map(|offset| {
            expressions
                .struct_field_at_offset(literal_fields, offset)
                .name
                .clone()
        })
        .collect();

    let mut zero_writes = Vec::new();
    for field in input.layouts.fields.span_or_empty(*common_fields) {
        if named.iter().any(|name| *name == field.name) {
            continue;
        }
        // Float zeroes must ride the float write path (same zero bits, but
        // the operand classification differs); everything else is integer-
        // shaped (bool included).
        let zero_value = match field.type_name.as_ref() {
            "f32" | "f64" => expressions.insert(ExpressionNode::Float(
                psi_checked_trees::expression::FloatLiteral::new(0.0),
            )),
            _ => expressions.insert(ExpressionNode::Integer(
                psi_numerics::literals::IntegerLiteral::zero(),
            )),
        };
        zero_writes.push((field.name.clone(), zero_value));
    }
    zero_writes
}

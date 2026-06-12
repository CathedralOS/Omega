mod fixed_array_slices;
mod mutation;
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
use crate::selection::storage_places::resolve_runtime_storage_place_in_table;
use omega_abstract_operations::{
    RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
};
use omega_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableIndexedExpression,
    TableMemberExpression,
};
use omega_checked_trees::name::Identifier;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_layout::{DataShape, ENUM_TAG_BYTES};
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
pub(crate) use static_values::RuntimeStaticValues;
use static_values::invalidate_runtime_static_value_in_table;

pub(in crate::selection) use mutation::{
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
    select_runtime_frame_slot_value_write_in_table_with_source_anchor,
};
pub(in crate::selection::runtime_dispatch) use mutation::{
    signedness_adjusted_operator, signedness_adjusted_operator_for_operands,
};
pub(in crate::selection) use slice_descriptors::emit_runtime_frame_slot_slice_descriptor_write_in_table;
pub(super) use storage_copy::{
    runtime_storage_copy, runtime_storage_copy_in_table, runtime_storage_fixed_indexed_source_copy,
    runtime_storage_fixed_indexed_source_copy_in_table,
    runtime_storage_indexed_source_copy_in_table, runtime_storage_indirect_copy_in_table,
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
        RuntimeDispatchBodyOperationKind::Mutation { .. } => {}
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
        for offset in 0..elements.count() {
            let element = expressions.expression_handle_at_offset(elements, offset);
            let element_index = expressions.insert(ExpressionNode::Integer(i64::from(offset)));
            let element_target =
                expressions.insert(ExpressionNode::Indexed(TableIndexedExpression {
                    collection: target,
                    index: element_index,
                }));
            emitted |= select_runtime_storage_resolved_mutation_write_in_mutable_table(
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
            );
        }
        return emitted;
    }

    if let ExpressionNode::StructLiteral(struct_literal) = expressions.expression(value).clone() {
        let mut emitted = false;
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
                    }));
                emitted |= select_runtime_storage_resolved_mutation_write_in_mutable_table(
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
            }
        }
        for offset in 0..struct_literal.fields.count() {
            let field = expressions
                .struct_field_at_offset(struct_literal.fields, offset)
                .clone();
            let field_target = expressions.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: target,
                member_symbol: SymbolHandle::invalid(),
                member: field.name,
            }));
            emitted |= select_runtime_storage_resolved_mutation_write_in_mutable_table(
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

    // Copies move a runtime value into the target. Whatever constant the target
    // previously folded to is now wrong, so forget it: a later read of the same
    // place in this state must come from live storage. Without this, a chain
    // like `v = 5; v = src; w = v + 1;` would fold the stale `v == 5` and
    // compute the wrong `w`.
    if let Some(kind) = subslice_copy::runtime_fixed_array_subslice_indexed_source_copy_in_table(
        input,
        dispatch_index,
        target_source_key,
        value_source_key,
        expressions,
        target,
        value,
    )
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
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: place.region,
            byte_offset: place.byte_offset,
            byte_size: ENUM_TAG_BYTES,
            value: tag,
        },
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

/// MIXED shapes: the common fields a case literal does NOT name, paired with a
/// type-shaped ZERO expression (construction zero-initializes them; validation
/// restricts mixed common fields to scalar primitives, so one literal each).
/// Pure sums and record types yield nothing.
fn unnamed_common_field_zero_writes(
    input: &InstructionSelectionInput<'_>,
    expressions: &mut ExpressionTable,
    type_name: &Identifier,
    literal_fields: omega_core::arena::HandleSpan<
        omega_checked_trees::expression::TableStructLiteralField,
    >,
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
                omega_checked_trees::expression::FloatLiteral::new(0.0),
            )),
            _ => expressions.insert(ExpressionNode::Integer(0)),
        };
        zero_writes.push((field.name.clone(), zero_value));
    }
    zero_writes
}

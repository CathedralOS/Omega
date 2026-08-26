use crate::InstructionSelectionInput;
use crate::{
    derive_boundary_entry_slice_descriptor_footprint, derive_boundary_entry_storage,
    derive_boundary_exit,
};
use omega_control_flow::StateKey;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_state_calls::StateCallRole;
use omega_state_values::{StateValueRole, simplify_state_expression_for_role};
use psi_arena::{Arena, PagedSlice};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::types::PrimitiveType;

mod argument_materialization;
mod branches;
mod edges;
mod guards;
mod operation_aliases;
mod text_writes;
pub(crate) mod writes;

use super::host_operations::{
    runtime_string_descriptor_place, runtime_text_literal_write_for_host_call, select_host_call,
    system_v_record_descriptor_shape,
};
use super::instruction_sink::SelectedInstructionSink;
use super::lookups::host_call_for_statement;
use super::storage_places::{
    classify_scalar_value_type_in_table, resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_storage_place_in_table,
};
use crate::selection::bindings::{RuntimeAliasBuffer, RuntimeAliasResolutionContext};
pub(super) use branches::{
    BranchPreludeSelectionScratch, select_runtime_branch_preludes_for_operation,
};
use branches::{
    LeafBranchSelectionScratch, leaf_expansions_defer_to_local_initializer,
    select_runtime_leaf_branch_expansions_for_operation,
    select_runtime_straight_line_branch_expansions_for_operation,
};
use edges::select_runtime_dispatch_edge;
use omega_abstract_operations::{
    InstructionOperand, RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction,
    SelectedInstructionKind,
};
use omega_calling_conventions::{
    CallSignature, CallingPolicy, MachineRegister, StateFootprintEvidence, SystemVEightbyteClass,
    ValidatedBoundaryEntryPlan, ValueClass, ValueLocation, ValueShape,
    evaluate_ordinary_boundary_entry_plan, validate_boundary_entry_plan,
};
use omega_layout::{DataShape, TypeLayoutDescriptor};
use operation_aliases::bind_runtime_operation_aliases;
use writes::select_runtime_storage_write_for_operation;
pub(crate) use writes::{RuntimeStaticValues, RuntimeStorageWriteScratch};

pub(super) use branches::{
    StraightLineBranchSelectionScratch, select_assignment_value_call_result_local_copy,
    select_runtime_straight_line_nested_branch_expansions_for_operation,
};
pub(in crate::selection) use writes::emit_runtime_frame_slot_slice_descriptor_write_in_table;
pub(in crate::selection) use writes::runtime_frame_slot_target_expression;
pub(in crate::selection) use writes::select_runtime_frame_slot_value_write_in_table;
pub(in crate::selection) use writes::select_runtime_storage_resolved_mutation_write_in_table_with_scratch;

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

/// The rung-2 migration constructor: a DIRECT (const-path) place-pair copy.
/// Every retired `CopyRuntimeStorage` producer routes here -- addressing in
/// the Place operands, relocations patched by each place's own region.
pub(crate) fn copy_places_direct(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target: omega_abstract_operations::Place::at(target_region, target_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// A pointee place: deref the frame-resident pointer slot at
/// `pointer_byte_offset`, then walk to `field_byte_offset`. Three steps,
/// always within `PLACE_MAX_STEPS`.
fn pointee_place(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(RuntimeStorageRegion::RuntimeFrame, pointer_byte_offset)
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a pointee place is three steps, within PLACE_MAX_STEPS")
}

/// Rung 2c-ii: the retired to-pointee copy -- a direct source into
/// `*(frame[pointer_byte_offset]) + field_byte_offset`.
pub(crate) fn copy_places_to_pointee(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target: pointee_place(pointer_byte_offset, field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-ii: the retired from-pointee copy -- a pointee source into a
/// direct target place.
pub(crate) fn copy_places_from_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(pointer_byte_offset, field_byte_offset),
        target: omega_abstract_operations::Place::at(target_region, target_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Copy between two fields reached through frame-held pointer slots.
pub(crate) fn copy_places_pointee_pair(
    source_pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    target_pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(source_pointer_byte_offset, source_field_byte_offset),
        target: pointee_place(target_pointer_byte_offset, target_field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-iv: a FIXED-indexed element read folds to a pure deref place --
/// the compile-time index scales into the constant displacement
/// (`*(frame[descriptor]) + index*size + field`), the same shape as a
/// pointee read. The retired ToFrame/ToStorage variant split collapses:
/// the target region rides the place.
pub(crate) fn copy_places_from_fixed_indexed(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(
            descriptor_offset,
            element_index * element_byte_size + field_byte_offset,
        ),
        target: omega_abstract_operations::Place::at(target_region, target_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// A RUNTIME-indexed element place: deref the frame-resident descriptor,
/// scale the index from its retained storage region, and walk to the field.
/// Four steps -- the PLACE_MAX_STEPS shape (a zero field offset merges away).
pub(crate) fn indexed_place(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(RuntimeStorageRegion::RuntimeFrame, descriptor_offset)
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("an indexed place is four steps, within PLACE_MAX_STEPS")
}

/// Rung 2c-v: the retired runtime-indexed element READ -- the target region
/// rides the place (the ToFrame/ToStorage split collapses).
pub(crate) fn copy_places_from_indexed(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: indexed_place(
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        target: omega_abstract_operations::Place::at(target_region, target_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-v: the retired runtime-indexed element WRITE (`exits[i] = e`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_to_indexed(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target: indexed_place(
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// A MACHINE inline-array element place (no deref -- machine statics), the
/// index slot in ITS OWN region (the cross-region index the materializer
/// serves with r11's own base).
pub(crate) fn machine_indexed_place(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(RuntimeStorageRegion::Machine, base_byte_offset)
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a machine-indexed place is three steps, within PLACE_MAX_STEPS")
}

pub(crate) fn frame_base_indexed_place(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    frame_base_indexed_place_with_index_region(
        base_byte_offset,
        RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn frame_base_indexed_place_with_index_region(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(RuntimeStorageRegion::RuntimeFrame, base_byte_offset)
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a frame-base-indexed place is three steps, within PLACE_MAX_STEPS")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_to_frame_base_indexed(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a frame-base-indexed place is three steps, within PLACE_MAX_STEPS"),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-x: an inline 2D-array element path (`arr[i][j].field`, no
/// deref): `[Const(base), SI(outer), SI(inner), Const(field)]`.
pub(crate) fn double_indexed_place(
    region: RuntimeStorageRegion,
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(region, base_byte_offset)
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: outer_index_region,
            index_offset: outer_index_offset,
            index_byte_size: outer_index_byte_size,
            element_byte_size: outer_stride,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: inner_index_region,
                index_offset: inner_index_offset,
                index_byte_size: inner_index_byte_size,
                element_byte_size: inner_stride,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a double-indexed place is four steps, within PLACE_MAX_STEPS")
}

/// Rung 2c-x: the machine inline 2D-array element READ.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_from_machine_double_indexed(
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: double_indexed_place(
            RuntimeStorageRegion::Machine,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        target: omega_abstract_operations::Place::at(target_region, target_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Binary rung 2b: the place-shaped binary write constructors. The shaped
/// forms are Exact-only (matching the retired kinds' field sets); the
/// direct form carries the full float/domain/signedness triple.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_binary_direct(
    region: RuntimeStorageRegion,
    byte_offset: usize,
    byte_size: usize,
    left: omega_abstract_operations::AbstractValueOperandHandle,
    operator: omega_abstract_operations::StateGuardOperator,
    right: omega_abstract_operations::AbstractValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceBinary {
        target: omega_abstract_operations::Place::at(region, byte_offset),
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    }
}

/// Materialize one computed scalar host-call argument into its reserved frame
/// slot. Nested value calls need separate call sequencing rather than a
/// scratch write and remain outside this path.
pub(in crate::selection) fn select_computed_host_argument_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    value: ExpressionHandle,
    target_offset: usize,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<(SelectedInstructionKind, usize)> {
    let byte_size =
        computed_host_argument_byte_size(input, dispatch_index, source_key, expressions, value)?;
    let static_values = writes::RuntimeStaticValues::default();
    let instruction = match expressions.expression(value) {
        ExpressionNode::Binary(_) | ExpressionNode::Call(_) => {
            writes::mutation::select_runtime_storage_binary_write_in_table(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                RuntimeStorageRegion::RuntimeFrame,
                target_offset,
                byte_size,
                value,
                &static_values,
                runtime_value_operands,
            )?
        }
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => {
            let target_primitive = input.program.primitive_type_reference(cast.target_type)?;
            writes::mutation::build_runtime_convert_write(
                input,
                dispatch_index,
                source_key,
                statement_index,
                expressions,
                RuntimeStorageRegion::RuntimeFrame,
                target_offset,
                None,
                target_primitive,
                cast.value,
                cast.domain,
                &static_values,
                runtime_value_operands,
            )?
        }
        ExpressionNode::Indexed(_) => {
            if let Some(place) = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                value,
            )
            .filter(|place| place.byte_count == byte_size)
            {
                copy_places_direct(
                    place.region,
                    place.byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    target_offset,
                    byte_size,
                )
            } else {
                let indexed = resolve_runtime_frame_indexed_target_in_table(
                    input,
                    dispatch_index,
                    source_key,
                    expressions,
                    value,
                )?;
                if indexed.byte_count != byte_size {
                    return None;
                }
                copy_places_from_indexed(
                    indexed.descriptor_offset,
                    indexed.index_region,
                    indexed.index_offset,
                    indexed.index_byte_size,
                    indexed.element_byte_size,
                    indexed.field_byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    target_offset,
                    byte_size,
                )
            }
        }
        _ => return None,
    };
    Some((instruction, byte_size))
}

pub(in crate::selection) fn computed_host_argument_byte_size(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    value: ExpressionHandle,
) -> Option<usize> {
    match expressions.expression(value) {
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                psi_checked_trees::expression::BinaryOperator::Equal
                    | psi_checked_trees::expression::BinaryOperator::NotEqual
                    | psi_checked_trees::expression::BinaryOperator::Less
                    | psi_checked_trees::expression::BinaryOperator::LessOrEqual
                    | psi_checked_trees::expression::BinaryOperator::Greater
                    | psi_checked_trees::expression::BinaryOperator::GreaterOrEqual
            ) =>
        {
            Some(1)
        }
        ExpressionNode::Binary(binary) => Some(writes::mutation::binary_value_operand_byte_width(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.left,
            binary.right,
        )),
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => input
            .program
            .primitive_type_reference(cast.target_type)
            .and_then(|primitive| primitive.scalar_byte_size()),
        ExpressionNode::Call(call) => {
            let (left, right) = computed_host_builtin_operands(input, expressions, call)?;
            Some(writes::mutation::binary_value_operand_byte_width(
                input,
                dispatch_index,
                source_key,
                expressions,
                left,
                right,
            ))
        }
        ExpressionNode::Indexed(_) => resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            value,
        )
        .map(|place| place.byte_count)
        .or_else(|| {
            resolve_runtime_frame_indexed_target_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                value,
            )
            .map(|indexed| indexed.byte_count)
        })
        .filter(|byte_count| matches!(byte_count, 1 | 2 | 4 | 8)),
        _ => None,
    }
}

pub(in crate::selection) fn computed_host_argument_is_float(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    value: ExpressionHandle,
) -> bool {
    match expressions.expression(value) {
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                psi_checked_trees::expression::BinaryOperator::Equal
                    | psi_checked_trees::expression::BinaryOperator::NotEqual
                    | psi_checked_trees::expression::BinaryOperator::Less
                    | psi_checked_trees::expression::BinaryOperator::LessOrEqual
                    | psi_checked_trees::expression::BinaryOperator::Greater
                    | psi_checked_trees::expression::BinaryOperator::GreaterOrEqual
            ) =>
        {
            false
        }
        ExpressionNode::Binary(_) | ExpressionNode::Indexed(_) => matches!(
            classify_scalar_value_type_in_table(
                input,
                dispatch_index,
                source_key,
                expressions,
                value,
            ),
            Some(PrimitiveType::F32 | PrimitiveType::F64)
        ),
        ExpressionNode::Cast(cast) if !cast.form.is_recast() => input
            .program
            .primitive_type_reference(cast.target_type)
            .is_some_and(|primitive| matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)),
        ExpressionNode::Call(call) => computed_host_builtin_operands(input, expressions, call)
            .is_some_and(|(left, right)| {
                [left, right].into_iter().any(|operand| {
                    matches!(
                        classify_scalar_value_type_in_table(
                            input,
                            dispatch_index,
                            source_key,
                            expressions,
                            operand,
                        ),
                        Some(PrimitiveType::F32 | PrimitiveType::F64)
                    )
                })
            }),
        _ => false,
    }
}

fn computed_host_builtin_operands(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    call: &psi_checked_trees::expression::TableCallExpression,
) -> Option<(ExpressionHandle, ExpressionHandle)> {
    if call.receiver.is_valid() {
        return None;
    }
    let symbols = &input.program.symbols;
    let is_binary = [
        psi_symbols::BuiltinFunction::Max,
        psi_symbols::BuiltinFunction::Min,
    ]
    .into_iter()
    .any(|builtin| symbols.builtin_function_symbol(builtin) == Some(call.target_symbol));
    if is_binary && call.arguments.count() == 2 {
        return Some((
            expressions.expression_handle_at_offset(call.arguments, 0),
            expressions.expression_handle_at_offset(call.arguments, 1),
        ));
    }
    let is_unary_float = [
        psi_symbols::BuiltinFunction::Sqrt,
        psi_symbols::BuiltinFunction::FloatIsNan,
        psi_symbols::BuiltinFunction::FloatIsFinite,
        psi_symbols::BuiltinFunction::FloatIsInfinite,
        psi_symbols::BuiltinFunction::FloatIsNormal,
        psi_symbols::BuiltinFunction::FloatIsSubnormal,
        psi_symbols::BuiltinFunction::FloatClassifyF32,
        psi_symbols::BuiltinFunction::FloatClassifyF64,
    ]
    .into_iter()
    .any(|builtin| symbols.builtin_function_symbol(builtin) == Some(call.target_symbol));
    if is_unary_float && call.arguments.count() == 1 {
        let operand = expressions.expression_handle_at_offset(call.arguments, 0);
        return Some((operand, operand));
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_binary_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: omega_abstract_operations::AbstractValueOperandHandle,
    operator: omega_abstract_operations::StateGuardOperator,
    right: omega_abstract_operations::AbstractValueOperandHandle,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceBinary {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointer_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a pointee place is three steps, within PLACE_MAX_STEPS"),
        byte_size,
        left,
        operator,
        right,
        is_float: false,
        domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
        target_signed: false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_binary_frame_indexed(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: omega_abstract_operations::AbstractValueOperandHandle,
    operator: omega_abstract_operations::StateGuardOperator,
    right: omega_abstract_operations::AbstractValueOperandHandle,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceBinary {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            descriptor_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a frame-indexed place is four steps, within PLACE_MAX_STEPS"),
        byte_size,
        left,
        operator,
        right,
        is_float: false,
        domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
        target_signed: false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_binary_base_indexed(
    region: RuntimeStorageRegion,
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: omega_abstract_operations::AbstractValueOperandHandle,
    operator: omega_abstract_operations::StateGuardOperator,
    right: omega_abstract_operations::AbstractValueOperandHandle,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceBinary {
        target: omega_abstract_operations::Place::at(region, base_byte_offset)
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a base-indexed place is three steps, within PLACE_MAX_STEPS"),
        byte_size,
        left,
        operator,
        right,
        is_float: false,
        domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
        target_signed: false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_binary_double_indexed(
    region: RuntimeStorageRegion,
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: omega_abstract_operations::AbstractValueOperandHandle,
    operator: omega_abstract_operations::StateGuardOperator,
    right: omega_abstract_operations::AbstractValueOperandHandle,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceBinary {
        target: double_indexed_place(
            region,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        byte_size,
        left,
        operator,
        right,
        is_float: false,
        domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
        target_signed: false,
    }
}

/// Write rung 2b: the place-shaped integer write constructors -- the seven
/// Write*Integer kinds collapse onto WritePlaceInteger through these.
pub(crate) fn write_place_integer_direct(
    region: RuntimeStorageRegion,
    byte_offset: usize,
    value: i64,
    byte_size: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceInteger {
        target: omega_abstract_operations::Place::at(region, byte_offset),
        value,
        byte_size,
    }
}

pub(crate) fn write_place_integer_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    value: i64,
    byte_size: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceInteger {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointer_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a pointee place is three steps, within PLACE_MAX_STEPS"),
        value,
        byte_size,
    }
}

/// Frame descriptor deref + frame index (`slice[i] = v`).
pub(crate) fn write_place_integer_frame_indexed(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    value: i64,
    byte_size: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceInteger {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            descriptor_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a frame-indexed place is four steps, within PLACE_MAX_STEPS"),
        value,
        byte_size,
    }
}

/// Text rung 2b: the place-string constructor family (the five retired
/// Write*String spellings as places).
pub(crate) fn write_place_string_direct(
    region: RuntimeStorageRegion,
    byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceString {
        target: omega_abstract_operations::Place::at(region, byte_offset),
        data,
        byte_length,
    }
}

pub(crate) fn write_place_string_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceString {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointer_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a pointee place is three steps, within PLACE_MAX_STEPS"),
        data,
        byte_length,
    }
}

pub(crate) fn write_place_string_frame_indexed(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceString {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            descriptor_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a frame-indexed place is four steps, within PLACE_MAX_STEPS"),
        data,
        byte_length,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_string_frame_base_indexed(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    write_place_string_frame_base_indexed_with_index_region(
        base_byte_offset,
        RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        data,
        byte_length,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_string_frame_base_indexed_with_index_region(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceString {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a frame-base-indexed place is three steps, within PLACE_MAX_STEPS"),
        data,
        byte_length,
    }
}

pub(crate) fn write_place_string_machine_indexed(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceString {
        target: machine_indexed_place(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        data,
        byte_length,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_string_machine_double_indexed(
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    let target =
        omega_abstract_operations::Place::at(RuntimeStorageRegion::Machine, base_byte_offset)
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: outer_index_region,
                index_offset: outer_index_offset,
                index_byte_size: outer_index_byte_size,
                element_byte_size: outer_stride,
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                    index_region: inner_index_region,
                    index_offset: inner_index_offset,
                    index_byte_size: inner_index_byte_size,
                    element_byte_size: inner_stride,
                })
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a machine-double-indexed place is four steps, within PLACE_MAX_STEPS");
    SelectedInstructionKind::WritePlaceString {
        target,
        data,
        byte_length,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_string_frame_base_double_indexed(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    data: omega_abstract_operations::AbstractDataObjectHandle,
    byte_length: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceString {
        target: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
            RuntimeStorageRegion::RuntimeFrame,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            RuntimeStorageRegion::RuntimeFrame,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        data,
        byte_length,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_bounded_buffer_frame_base_double_indexed(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    literal: std::sync::Arc<[u8]>,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceBoundedBuffer {
        target: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
            RuntimeStorageRegion::RuntimeFrame,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            RuntimeStorageRegion::RuntimeFrame,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        literal,
    }
}

/// Task #132: the text-crossing constructors (the nine retired
/// Materialize/AppendStored/AppendLiteral spellings as places).
pub(crate) fn text_place_direct(
    region: RuntimeStorageRegion,
    byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(region, byte_offset)
}

pub(crate) fn text_place_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(RuntimeStorageRegion::RuntimeFrame, pointer_byte_offset)
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a pointee place is three steps, within PLACE_MAX_STEPS")
}

pub(crate) fn text_place_frame_indexed(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> omega_abstract_operations::Place {
    omega_abstract_operations::Place::at(RuntimeStorageRegion::RuntimeFrame, descriptor_offset)
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a frame-indexed place is four steps, within PLACE_MAX_STEPS")
}

/// Task #131 (guards consume Places): the direct-place compare
/// constructors (the retired storage-compare spellings as places).
pub(crate) fn compare_places_direct(
    left_region: RuntimeStorageRegion,
    left_offset: usize,
    right_region: RuntimeStorageRegion,
    right_offset: usize,
    byte_size: usize,
    operator: omega_abstract_operations::StateGuardOperator,
    is_float: bool,
) -> SelectedInstructionKind {
    SelectedInstructionKind::ComparePlaces {
        left: omega_abstract_operations::Place::at(left_region, left_offset),
        right: omega_abstract_operations::Place::at(right_region, right_offset),
        byte_size,
        operator,
        is_float,
    }
}

pub(crate) fn compare_place_value_direct(
    region: RuntimeStorageRegion,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    operator: omega_abstract_operations::StateGuardOperator,
) -> SelectedInstructionKind {
    SelectedInstructionKind::ComparePlaceValue {
        place: omega_abstract_operations::Place::at(region, byte_offset),
        byte_size,
        expected_value,
        operator,
    }
}

/// Task #131: the place-address constructor family (the six retired
/// Write*AddressToRuntimeFrame spellings as places).
pub(crate) fn write_place_address_direct(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target_offset,
    }
}

pub(crate) fn emit_local_dynamic_conformance_descriptor(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target_slot: &omega_runtime_storage::RuntimeFrameSlot,
    expressions: &ExpressionTable,
    initializer: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(selection) = input
        .control_flow
        .semantics
        .facts
        .dynamic_conformances
        .for_binding(source_key.machine, source_key.state, target_slot.symbol)
    else {
        return false;
    };
    if selection.statement_index != statement_index
        || selection.source_path.last() != Some(&selection.source_name)
    {
        return true;
    }
    let Some(conformance) = selection.conformance else {
        return true;
    };
    let recast = match expressions.expression(initializer) {
        ExpressionNode::Borrow(inner) => inner.target,
        ExpressionNode::Cast(_) => initializer,
        _ => return true,
    };
    let ExpressionNode::Cast(cast) = expressions.expression(recast) else {
        return true;
    };
    let Some(source_place) =
        crate::selection::storage_places::resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            cast.value,
        )
    else {
        return true;
    };
    let Some(table_object) = input
        .data
        .dynamic_conformance_table_object(selection.target_trait, conformance)
    else {
        return true;
    };
    let mut tables = input
        .data
        .dynamic_conformance_tables
        .iter()
        .filter_map(|(_, table)| (table.object == table_object).then_some(table));
    let Some(table) = tables.next() else {
        return true;
    };
    if tables.next().is_some()
        || table.rows.len() != selection.rows.len()
        || table
            .rows
            .iter()
            .zip(&selection.rows)
            .any(|(physical, checked)| {
                physical.requirement_identity.as_ref() != checked.requirement_identity
                    || physical.realization_identity.as_ref() != checked.realization_identity
                    || physical.realization.machine != checked.realization_machine
                    || physical.realization.state != checked.realization_state
            })
    {
        return true;
    }
    let abi = input.runtime_abi.dynamic_trait_descriptor();
    if target_slot.byte_size != abi.total_size()
        || target_slot.alignment < abi.align()
        || !matches!(
            &target_slot.type_descriptor,
            omega_layout::TypeLayoutDescriptor::Reference { referee, is_mutable: false }
                if matches!(
                    referee.as_ref(),
                    omega_layout::TypeLayoutDescriptor::DynamicTrait { symbol, .. }
                        if *symbol == selection.target_trait
                )
        )
    {
        return true;
    }
    let Some(instance_offset) = target_slot.byte_offset.checked_add(abi.instance_offset()) else {
        return true;
    };
    let Some(table_offset) = target_slot.byte_offset.checked_add(abi.table_offset()) else {
        return true;
    };
    selected_instructions.push(SelectedInstruction {
        kind: write_place_address_direct(
            source_place.region,
            source_place.byte_offset,
            instance_offset,
        ),
        source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteDataAddressToRuntimeFrame {
            data: table_object,
            target_offset: table_offset,
        },
        source_key,
        source_statement: statement_index,
    });
    true
}

pub(crate) fn write_place_address_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointer_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a pointee place is three steps, within PLACE_MAX_STEPS"),
        target_offset,
    }
}

/// The retired FIXED-index shape: the constant element index folds into the
/// post-deref const offset, so the place is pointee-shaped.
pub(crate) fn write_place_address_fixed_indexed(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    // Layout-derived constants cannot overflow usize in a real program; the
    // encoder's disp32 fence still refuses any displacement past i32.
    let displacement = element_index
        .checked_mul(element_byte_size)
        .and_then(|scaled| scaled.checked_add(field_byte_offset))
        .expect("fixed indexed address offset overflows usize");
    write_place_address_pointee(descriptor_offset, displacement, target_offset)
}

pub(crate) fn write_place_address_frame_indexed_deref(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            descriptor_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("an indexed-deref place is four steps, within PLACE_MAX_STEPS"),
        target_offset,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_address_base_indexed_with_index_region(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a base-indexed place is three steps, within PLACE_MAX_STEPS"),
        target_offset,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_address_base_double_indexed(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: RuntimeStorageRegion::RuntimeFrame,
            index_offset: outer_index_offset,
            index_byte_size: outer_index_byte_size,
            element_byte_size: outer_stride,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: inner_index_offset,
                index_byte_size: inner_index_byte_size,
                element_byte_size: inner_stride,
            })
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a base-double-indexed place is four steps, within PLACE_MAX_STEPS"),
        target_offset,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_address_machine_double_indexed(
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: double_indexed_place(
            RuntimeStorageRegion::Machine,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        target_offset,
    }
}

pub(crate) fn write_place_address_region_indexed(
    base_region: RuntimeStorageRegion,
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: omega_abstract_operations::Place::at(base_region, base_byte_offset)
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a region-indexed place is three steps, within PLACE_MAX_STEPS"),
        target_offset,
    }
}

pub(crate) fn write_place_address_machine_indexed(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceAddress {
        source: machine_indexed_place(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        target_offset,
    }
}

/// Text rung 2b: the retained pointee convenience constructor. Direct and
/// indexed carrier writes now carry their canonical Place directly.
pub(crate) fn write_place_bounded_buffer_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: std::sync::Arc<[u8]>,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceBoundedBuffer {
        target: omega_abstract_operations::Place::at(
            RuntimeStorageRegion::RuntimeFrame,
            pointer_byte_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                field_byte_offset,
            ))
        })
        .expect("a pointee place is three steps, within PLACE_MAX_STEPS"),
        literal,
    }
}

/// No-deref inline array element (`arr[i] = v`; frame or machine root).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_integer_base_indexed(
    region: RuntimeStorageRegion,
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    value: i64,
    byte_size: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceInteger {
        target: omega_abstract_operations::Place::at(region, base_byte_offset)
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    field_byte_offset,
                ))
            })
            .expect("a base-indexed place is three steps, within PLACE_MAX_STEPS"),
        value,
        byte_size,
    }
}

/// Frame- or machine-resident 2D inline array element (`grid[i][j] = v`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_place_integer_double_indexed(
    region: RuntimeStorageRegion,
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    value: i64,
    byte_size: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::WritePlaceInteger {
        target: double_indexed_place(
            region,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        value,
        byte_size,
    }
}

/// Rung 2c-x: the machine inline 2D-array element WRITE.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_to_machine_double_indexed(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target: double_indexed_place(
            RuntimeStorageRegion::Machine,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-x: `arr[i] = arr[j]` on machine inline arrays -- one runtime
/// index each side.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_machine_indexed_pair(
    source_base_byte_offset: usize,
    source_index_region: RuntimeStorageRegion,
    source_index_offset: usize,
    source_index_byte_size: usize,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_index_region: RuntimeStorageRegion,
    target_index_offset: usize,
    target_index_byte_size: usize,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: machine_indexed_place(
            source_base_byte_offset,
            source_index_region,
            source_index_offset,
            source_index_byte_size,
            source_element_byte_size,
            source_field_byte_offset,
        ),
        target: machine_indexed_place(
            target_base_byte_offset,
            target_index_region,
            target_index_offset,
            target_index_byte_size,
            target_element_byte_size,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_machine_indexed_to_pointee(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: machine_indexed_place(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            source_field_byte_offset,
        ),
        target: pointee_place(pointer_byte_offset, target_field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_pointee_to_machine_indexed(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(pointer_byte_offset, source_field_byte_offset),
        target: machine_indexed_place(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// `arr[i] = arr[j]` on frame-inline arrays. Frame-held index slots share the
/// runtime frame; machine-held index slots share one machine root.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_frame_base_indexed_pair(
    source_base_byte_offset: usize,
    source_index_region: RuntimeStorageRegion,
    source_index_offset: usize,
    source_index_byte_size: usize,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_index_region: RuntimeStorageRegion,
    target_index_offset: usize,
    target_index_byte_size: usize,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: frame_base_indexed_place_with_index_region(
            source_base_byte_offset,
            source_index_region,
            source_index_offset,
            source_index_byte_size,
            source_element_byte_size,
            source_field_byte_offset,
        ),
        target: frame_base_indexed_place_with_index_region(
            target_base_byte_offset,
            target_index_region,
            target_index_offset,
            target_index_byte_size,
            target_element_byte_size,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// `target[j] = source[i]` across one machine-inline and one frame-inline
/// array. Each runtime index retains its own storage region.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_cross_region_indexed_pair(
    source_region: RuntimeStorageRegion,
    source_base_byte_offset: usize,
    source_index_region: RuntimeStorageRegion,
    source_index_offset: usize,
    source_index_byte_size: usize,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_base_byte_offset: usize,
    target_index_region: RuntimeStorageRegion,
    target_index_offset: usize,
    target_index_byte_size: usize,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_base_byte_offset)
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: source_index_region,
                index_offset: source_index_offset,
                index_byte_size: source_index_byte_size,
                element_byte_size: source_element_byte_size,
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    source_field_byte_offset,
                ))
            })
            .expect("a cross-region indexed source is within PLACE_MAX_STEPS"),
        target: omega_abstract_operations::Place::at(target_region, target_base_byte_offset)
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: target_index_region,
                index_offset: target_index_offset,
                index_byte_size: target_index_byte_size,
                element_byte_size: target_element_byte_size,
            })
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    target_field_byte_offset,
                ))
            })
            .expect("a cross-region indexed target is within PLACE_MAX_STEPS"),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// `target[a][b] = source[i][j]` across one machine-inline and one
/// frame-inline 2D array. All four runtime indices retain their storage region.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_cross_region_double_indexed_pair(
    source_region: RuntimeStorageRegion,
    source_base_byte_offset: usize,
    source_outer_index_region: RuntimeStorageRegion,
    source_outer_index_offset: usize,
    source_outer_index_byte_size: usize,
    source_outer_stride: usize,
    source_inner_index_region: RuntimeStorageRegion,
    source_inner_index_offset: usize,
    source_inner_index_byte_size: usize,
    source_inner_stride: usize,
    source_field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_base_byte_offset: usize,
    target_outer_index_region: RuntimeStorageRegion,
    target_outer_index_offset: usize,
    target_outer_index_byte_size: usize,
    target_outer_stride: usize,
    target_inner_index_region: RuntimeStorageRegion,
    target_inner_index_offset: usize,
    target_inner_index_byte_size: usize,
    target_inner_stride: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: double_indexed_place(
            source_region,
            source_base_byte_offset,
            source_outer_index_region,
            source_outer_index_offset,
            source_outer_index_byte_size,
            source_outer_stride,
            source_inner_index_region,
            source_inner_index_offset,
            source_inner_index_byte_size,
            source_inner_stride,
            source_field_byte_offset,
        ),
        target: double_indexed_place(
            target_region,
            target_base_byte_offset,
            target_outer_index_region,
            target_outer_index_offset,
            target_outer_index_byte_size,
            target_outer_stride,
            target_inner_index_region,
            target_inner_index_offset,
            target_inner_index_byte_size,
            target_inner_stride,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-x: a frame-inline 2D-array element read with independently placed
/// runtime indices.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_from_frame_base_double_indexed(
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        target: omega_abstract_operations::Place::at(target_region, target_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// A direct storage value copied into a frame-inline 2D-array element with
/// independently placed runtime indices.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_to_frame_base_double_indexed(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// `grid[a][b] = grid[i][j]` on frame-inline 2D arrays. Both collections and
/// frame-held index slots share the runtime frame; machine-held index slots
/// share one machine root.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_frame_base_double_indexed_pair(
    source_base_byte_offset: usize,
    source_outer_index_region: RuntimeStorageRegion,
    source_outer_index_offset: usize,
    source_outer_index_byte_size: usize,
    source_outer_stride: usize,
    source_inner_index_region: RuntimeStorageRegion,
    source_inner_index_offset: usize,
    source_inner_index_byte_size: usize,
    source_inner_stride: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_outer_index_region: RuntimeStorageRegion,
    target_outer_index_offset: usize,
    target_outer_index_byte_size: usize,
    target_outer_stride: usize,
    target_inner_index_region: RuntimeStorageRegion,
    target_inner_index_offset: usize,
    target_inner_index_byte_size: usize,
    target_inner_stride: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            source_base_byte_offset,
            source_outer_index_region,
            source_outer_index_offset,
            source_outer_index_byte_size,
            source_outer_stride,
            source_inner_index_region,
            source_inner_index_offset,
            source_inner_index_byte_size,
            source_inner_stride,
            source_field_byte_offset,
        ),
        target: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            target_base_byte_offset,
            target_outer_index_region,
            target_outer_index_offset,
            target_outer_index_byte_size,
            target_outer_stride,
            target_inner_index_region,
            target_inner_index_offset,
            target_inner_index_byte_size,
            target_inner_stride,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_frame_base_double_indexed_to_pointee(
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            source_field_byte_offset,
        ),
        target: pointee_place(pointer_byte_offset, target_field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_pointee_to_frame_base_double_indexed(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(pointer_byte_offset, source_field_byte_offset),
        target: double_indexed_place(
            RuntimeStorageRegion::RuntimeFrame,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// `grid[a][b] = grid[i][j]` on machine-rooted inline 2D arrays. Each
/// runtime index retains its own frame-or-machine storage region.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_machine_double_indexed_pair(
    source_base_byte_offset: usize,
    source_outer_index_region: RuntimeStorageRegion,
    source_outer_index_offset: usize,
    source_outer_index_byte_size: usize,
    source_outer_stride: usize,
    source_inner_index_region: RuntimeStorageRegion,
    source_inner_index_offset: usize,
    source_inner_index_byte_size: usize,
    source_inner_stride: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_outer_index_region: RuntimeStorageRegion,
    target_outer_index_offset: usize,
    target_outer_index_byte_size: usize,
    target_outer_stride: usize,
    target_inner_index_region: RuntimeStorageRegion,
    target_inner_index_offset: usize,
    target_inner_index_byte_size: usize,
    target_inner_stride: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: double_indexed_place(
            RuntimeStorageRegion::Machine,
            source_base_byte_offset,
            source_outer_index_region,
            source_outer_index_offset,
            source_outer_index_byte_size,
            source_outer_stride,
            source_inner_index_region,
            source_inner_index_offset,
            source_inner_index_byte_size,
            source_inner_stride,
            source_field_byte_offset,
        ),
        target: double_indexed_place(
            RuntimeStorageRegion::Machine,
            target_base_byte_offset,
            target_outer_index_region,
            target_outer_index_offset,
            target_outer_index_byte_size,
            target_outer_stride,
            target_inner_index_region,
            target_inner_index_offset,
            target_inner_index_byte_size,
            target_inner_stride,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_machine_double_indexed_to_pointee(
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: double_indexed_place(
            RuntimeStorageRegion::Machine,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            source_field_byte_offset,
        ),
        target: pointee_place(pointer_byte_offset, target_field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_pointee_to_machine_double_indexed(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    base_byte_offset: usize,
    outer_index_region: RuntimeStorageRegion,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_region: RuntimeStorageRegion,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(pointer_byte_offset, source_field_byte_offset),
        target: double_indexed_place(
            RuntimeStorageRegion::Machine,
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-vii: the retired machine inline-array element READ.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_from_machine_indexed(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: machine_indexed_place(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        target: omega_abstract_operations::Place::at(target_region, target_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_frame_base_indexed_to_pointee(
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: frame_base_indexed_place_with_index_region(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            source_field_byte_offset,
        ),
        target: pointee_place(pointer_byte_offset, target_field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_pointee_to_frame_base_indexed(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(pointer_byte_offset, source_field_byte_offset),
        target: frame_base_indexed_place_with_index_region(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            target_field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-vii: the retired machine inline-array element WRITE.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_to_machine_indexed(
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: omega_abstract_operations::Place::at(source_region, source_offset),
        target: machine_indexed_place(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        ),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-v: the runtime-indexed read landing THROUGH a pointer slot.
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_indexed_to_pointee(
    descriptor_offset: usize,
    index_region: RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: indexed_place(
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            source_field_byte_offset,
        ),
        target: pointee_place(pointer_byte_offset, target_field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

/// Rung 2c-iv: the fixed-indexed read landing THROUGH a pointer slot --
/// both sides deref (the PointeePair shape).
#[allow(clippy::too_many_arguments)]
pub(crate) fn copy_places_fixed_indexed_to_pointee(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> SelectedInstructionKind {
    SelectedInstructionKind::CopyPlaces {
        source: pointee_place(
            descriptor_offset,
            element_index * element_byte_size + source_field_byte_offset,
        ),
        target: pointee_place(pointer_byte_offset, target_field_byte_offset),
        byte_count,
        role: omega_abstract_operations::CopyPlacesRole::Ordinary,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn select_runtime_unaliased_storage_mutation_write_with_scratch(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target: ExpressionHandle,
    value: ExpressionHandle,
    static_values: &mut writes::RuntimeStaticValues,
    scratch: &mut RuntimeStorageWriteScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    writes::select_runtime_storage_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        statement_index,
        target,
        value,
        static_values,
        scratch,
        runtime_value_operands,
        selected_instructions,
    )
}

/// Emit the ENTRY PROLOGUE's argument unmarshal: one register store per declared
/// entry parameter, mapping the parameter's frame slot (in declaration = offset
/// order) through the target's normalized native `CallPlan`. This is the
/// calling plan's INBOUND direction -- how a UEFI
/// `main(image_handle, system_table)` receives the firmware handoff. Emitted
/// FIRST at the entry (before the field-default writes) because the argument
/// registers are volatile. `&mut self` takes no frame slot (the machine's static
/// storage), so declared non-self parameters map 1:1 to the argument registers.
/// Register and incoming-stack placements both flow from the plan; target
/// encoders alone account for their entry-frame bias.
pub(super) fn select_entry_argument_register_writes(
    input: &InstructionSelectionInput<'_>,
    selected_instructions: &mut SelectedInstructionSink,
    boundary_footprints: &mut omega_abstract_operations::BoundaryFootprintPlan,
) -> Option<ValidatedBoundaryEntryPlan> {
    // THE BYTES HANDOFF -- `run(&self, args: &[u8])`: when the entry's sole
    // declared parameter is a byte slice, the prologue SPILLS the platform's
    // four argument registers into the reserved spill region and binds `args`
    // as a {ptr -> spill, len 32} view over them. The handoff is FOREIGN BYTES;
    // the program casts/mints what it trusts (UEFI: bytes 0..8 = ImageHandle
    // from RCX, 8..16 = SystemTable* from RDX).
    if input.runtime_storage.entry_argument_spill_size > 0 {
        let spill_base = input.runtime_storage.entry_argument_spill_base;
        let descriptor_offset = input
            .runtime_storage
            .frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::Parameter
                ) && slot.source_key == input.entry_key)
                    .then_some(slot.byte_offset)
            });
        let Some(descriptor_offset) = descriptor_offset else {
            return None;
        };
        let destinations = (0..4)
            .map(|index| (spill_base + index * 8, ValueShape::integer(8, 8)))
            .collect::<Vec<_>>();
        let selected =
            select_normalized_entry_argument_writes(input, &destinations, selected_instructions);
        let descriptor_footprint =
            derive_boundary_entry_slice_descriptor_footprint(&selected.boundary)
                .expect("bytes-handoff descriptor must fit the validated entry state ceiling");
        retain_boundary_footprint(
            boundary_footprints,
            &selected.boundary,
            omega_abstract_operations::BoundaryFootprintFragmentOrigin::EntryStorage,
            selected.footprint,
        );
        retain_boundary_footprint(
            boundary_footprints,
            &selected.boundary,
            omega_abstract_operations::BoundaryFootprintFragmentOrigin::EntrySliceDescriptor,
            descriptor_footprint,
        );
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor {
                descriptor_offset,
                spill_offset: spill_base,
                byte_length: input.runtime_storage.entry_argument_spill_size,
            },
            source_key: input.entry_key,
            source_statement: 0,
        });
        return Some(selected.boundary);
    }

    // TYPED entry parameters: each declared non-self parameter receives its
    // argument register directly (`main(handle: addr, table: addr)`).
    let mut parameter_slots: Vec<(usize, usize, ValueShape)> = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| {
            // EXACT key match (segment included): case-payload bindings are
            // Parameter slots in LATER SEGMENTS of the entry state and are NOT
            // platform entry arguments.
            (matches!(
                slot.kind,
                omega_runtime_storage::RuntimeFrameSlotKind::Parameter
            ) && slot.source_key == input.entry_key)
                .then(|| {
                    entry_slot_value_shape(input, slot)
                        .map(|shape| (slot.byte_offset, slot.byte_size, shape))
                })
                .flatten()
        })
        .collect();
    let declared_parameter_count = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter(|(_, slot)| {
            matches!(
                slot.kind,
                omega_runtime_storage::RuntimeFrameSlotKind::Parameter
            ) && slot.source_key == input.entry_key
        })
        .count();
    if parameter_slots.len() != declared_parameter_count {
        // Aggregates and descriptor-shaped arguments need source-policy ABI
        // classification before this lowering can populate them honestly.
        return None;
    }
    parameter_slots.sort_unstable_by_key(|(byte_offset, _, _)| *byte_offset);

    // THE STRUCT-SHAPED HANDOFF (ladder step 3, boundary machines): a BOUNDARY
    // entry whose sole declared parameter is a multi-word struct receives the
    // platform's argument registers SPREAD across its 8-byte chunks --
    // `boundary machine Main::main(&self, handoff: EfiHandoff)` binds RCX to
    // handoff.handle (+0) and RDX to handoff.table (+8). This is the boundary
    // contract's shape-over-arrival-bytes, NOT general MS-x64 struct passing
    // (which passes large aggregates by pointer; there is no caller here --
    // the platform hands registers, the declaration shapes them). Keep this
    // exceptional contract on the Microsoft-x64/UEFI family; ordinary SysV and
    // AAPCS64 boundary entries must follow their native aggregate ABI.
    let entry_is_boundary = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == input.entry_key.machine)
        .is_some_and(|machine| machine.supply_mode.is_boundary_declaration());
    if entry_is_boundary
        && input.entry_boundary_plan.is_none()
        && entry_calling_policy(input) == CallingPolicy::MicrosoftX64
        && let [(byte_offset, byte_size, shape)] = parameter_slots.as_slice()
        && matches!(shape.class, ValueClass::Integer)
        && *byte_size > 8
        && *byte_size % 8 == 0
        && *byte_size <= 32
    {
        let destinations = (0..(*byte_size / 8))
            .map(|index| (*byte_offset + index * 8, ValueShape::integer(8, 8)))
            .collect::<Vec<_>>();
        let selected =
            select_normalized_entry_argument_writes(input, &destinations, selected_instructions);
        retain_boundary_footprint(
            boundary_footprints,
            &selected.boundary,
            omega_abstract_operations::BoundaryFootprintFragmentOrigin::EntryStorage,
            selected.footprint,
        );
        return Some(selected.boundary);
    }

    let destinations = parameter_slots
        .into_iter()
        .map(|(byte_offset, _, shape)| (byte_offset, shape))
        .collect::<Vec<_>>();
    // Native aggregate policies consume the evaluator's direct, whole-stack,
    // or indirect placement. Other policies retain the previous fail-closed
    // no-prologue behavior; the explicit Microsoft boundary-handoff special
    // case above remains separate.
    if destinations
        .iter()
        .any(|(_, shape)| matches!(shape.class, ValueClass::Integer) && shape.byte_size > 8)
        && !matches!(
            entry_calling_policy(input),
            CallingPolicy::Aapcs64 | CallingPolicy::MicrosoftX64 | CallingPolicy::SystemVAMD64
        )
    {
        return None;
    }
    let selected =
        select_normalized_entry_argument_writes(input, &destinations, selected_instructions);
    retain_boundary_footprint(
        boundary_footprints,
        &selected.boundary,
        omega_abstract_operations::BoundaryFootprintFragmentOrigin::EntryStorage,
        selected.footprint,
    );
    Some(selected.boundary)
}

fn retain_boundary_footprint(
    plan: &mut omega_abstract_operations::BoundaryFootprintPlan,
    boundary: &ValidatedBoundaryEntryPlan,
    origin: omega_abstract_operations::BoundaryFootprintFragmentOrigin,
    evidence: StateFootprintEvidence,
) {
    plan.retain_validated_fragment(
        boundary,
        omega_abstract_operations::BoundaryFootprintFragment { origin, evidence },
    )
    .expect("retained entry footprint must name and fit one validated boundary contract");
}

struct SelectedNormalizedEntryStorage {
    boundary: ValidatedBoundaryEntryPlan,
    footprint: StateFootprintEvidence,
}

fn select_normalized_entry_argument_writes(
    input: &InstructionSelectionInput<'_>,
    destinations: &[(usize, ValueShape)],
    selected_instructions: &mut SelectedInstructionSink,
) -> SelectedNormalizedEntryStorage {
    let result = normalized_entry_result_shape(input);
    let indirect_result = normalized_entry_indirect_result_shape(input);
    let signature = CallSignature {
        parameters: destinations.iter().map(|(_, shape)| *shape).collect(),
        result,
    };
    let boundary = match input.entry_boundary_plan {
        Some(plan) => validate_boundary_entry_plan(plan.clone(), &signature)
            .expect("selected target entry plan must match its source continuation signature"),
        None if input.freestanding => {
            omega_calling_conventions::evaluate_freestanding_program_entry_plan(
                entry_calling_policy(input),
                &signature,
            )
            .expect(
                "freestanding runtime entry signature must have a normalized boundary entry plan",
            )
        }
        None => evaluate_ordinary_boundary_entry_plan(entry_calling_policy(input), &signature)
            .expect("runtime entry signature must have a normalized boundary entry plan"),
    };
    let indirect_result_pointer_byte_offset = indirect_result.map(|_| {
        assert_eq!(
            input.runtime_storage.entry_indirect_result_pointer_size, 8,
            "large native entry result must reserve its destination pointer"
        );
        input.runtime_storage.entry_indirect_result_pointer_base
    });
    let entry_storage = derive_boundary_entry_storage(
        boundary.plan(),
        destinations,
        result,
        indirect_result_pointer_byte_offset,
    )
    .expect("runtime entry must lower from its validated boundary plan");
    let crate::DerivedBoundaryEntryStorage {
        writes, footprint, ..
    } = entry_storage;
    for kind in writes {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: input.entry_key,
            source_statement: 0,
        });
    }
    SelectedNormalizedEntryStorage {
        boundary,
        footprint,
    }
}

pub(super) fn normalized_entry_indirect_result_shape(
    input: &InstructionSelectionInput<'_>,
) -> Option<ValueShape> {
    let shape = normalized_entry_result_shape(input)?;
    let is_indirect = match CallingPolicy::native_for_target(input.target) {
        CallingPolicy::MicrosoftX64 => !matches!(shape.byte_size, 1 | 2 | 4 | 8),
        CallingPolicy::Aapcs64 | CallingPolicy::SystemVAMD64 => shape.byte_size > 16,
        _ => false,
    };
    (matches!(shape.class, ValueClass::Integer) && is_indirect).then_some(shape)
}

fn normalized_entry_result_shape(input: &InstructionSelectionInput<'_>) -> Option<ValueShape> {
    if let Some(shape) = normalized_entry_record_result_shape(input) {
        return Some(shape);
    }
    let primitive = normalized_entry_scalar_result_primitive(input)?;
    let byte_size = u16::try_from(primitive.scalar_byte_size()?).ok()?;
    Some(match primitive {
        PrimitiveType::F32 | PrimitiveType::F64 => ValueShape::float(byte_size),
        _ => ValueShape::integer(byte_size, byte_size.max(1)),
    })
}

pub(super) fn normalized_entry_record_result_placement(
    input: &InstructionSelectionInput<'_>,
) -> Option<(ValueShape, Vec<ValueLocation>)> {
    let shape = normalized_entry_record_result_shape(input)?;
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(input.target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(shape),
        },
    )
    .ok()?;
    let locations = derive_boundary_exit(boundary.plan(), &[], Some(shape))
        .ok()?
        .result_locations;
    Some((shape, locations))
}

fn normalized_entry_record_result_shape(
    input: &InstructionSelectionInput<'_>,
) -> Option<ValueShape> {
    let policy = CallingPolicy::native_for_target(input.target);
    if !matches!(
        policy,
        CallingPolicy::Aapcs64 | CallingPolicy::MicrosoftX64 | CallingPolicy::SystemVAMD64
    ) {
        return None;
    }
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == input.entry_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == input.entry_key.state)?;
    let result_symbol = input.program.type_reference_symbol(state.return_type);
    let data_layout = input
        .layouts
        .data_layouts
        .iter()
        .find(|(_, layout)| layout.symbol == result_symbol)
        .map(|(_, layout)| layout)?;
    let DataShape::Record { fields } = data_layout.shape else {
        return None;
    };
    let byte_size = u16::try_from(data_layout.layout.size).ok()?;
    let alignment = u16::try_from(data_layout.layout.alignment).ok()?;

    // Microsoft x64 returns only 1-, 2-, 4-, and 8-byte records directly in
    // RAX. Every other record width uses its distinct hidden-RCX convention.
    if policy == CallingPolicy::MicrosoftX64 {
        return Some(ValueShape::integer(byte_size, alignment));
    }

    if policy == CallingPolicy::Aapcs64 {
        return flat_homogeneous_float_aggregate_shape(input, fields, data_layout.layout)
            .or_else(|| Some(ValueShape::integer(byte_size, alignment)));
    }

    if byte_size > 16 {
        return Some(ValueShape::integer(byte_size, alignment));
    }

    if let Some(shape) = flat_homogeneous_float_aggregate_shape(input, fields, data_layout.layout) {
        return Some(shape);
    }

    let descriptor = TypeLayoutDescriptor::Named {
        symbol: data_layout.symbol,
        name: data_layout.name.clone(),
    };
    if let Some((_, _, sse_eightbytes)) = system_v_record_descriptor_shape(input, &descriptor)
        && sse_eightbytes != 0
    {
        if byte_size <= 8 {
            return Some(ValueShape::float(byte_size));
        }
        let class = |mask| {
            if sse_eightbytes & mask == 0 {
                SystemVEightbyteClass::Integer
            } else {
                SystemVEightbyteClass::Sse
            }
        };
        return Some(ValueShape::system_v_aggregate(
            byte_size,
            alignment,
            class(0b01),
            class(0b10),
        ));
    }

    Some(ValueShape::integer(byte_size, alignment))
}

/// Select the process-entry integer result register through the same
/// normalized native call-plan evaluator used by inbound arguments. The
/// operation carries this identity onward; ISA encoders do not choose it.
pub(super) fn normalized_entry_integer_result_register(
    input: &InstructionSelectionInput<'_>,
) -> MachineRegister {
    let signature = CallSignature {
        parameters: Vec::new(),
        result: Some(ValueShape::integer(4, 4)),
    };
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(input.target),
        &signature,
    )
    .expect("runtime entry result must have a normalized boundary entry plan");
    let result = derive_boundary_exit(boundary.plan(), &[], Some(ValueShape::integer(4, 4)))
        .expect("integer-result boundary exit");
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: 4,
        },
    ] = result.result_locations.as_slice()
    else {
        panic!("integer-result call plan must select one complete register");
    };
    *register
}

/// Select a scalar runtime terminal's result register from the entry state's
/// actual primitive return type. Float terminals must preserve their class so
/// Microsoft x64/SysV use XMM0 and AAPCS64 uses V0 rather than collapsing a
/// same-width value into the integer result bank.
pub(super) fn normalized_entry_scalar_result_register(
    input: &InstructionSelectionInput<'_>,
    byte_size: usize,
) -> Option<MachineRegister> {
    let primitive = normalized_entry_scalar_result_primitive(input)?;
    let byte_size = u16::try_from(byte_size).ok()?;
    let shape = match primitive {
        PrimitiveType::F32 | PrimitiveType::F64 => ValueShape::float(byte_size),
        _ => ValueShape::integer(byte_size, byte_size.max(1)),
    };
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(input.target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(shape),
        },
    )
    .ok()?;
    let exit = derive_boundary_exit(boundary.plan(), &[], Some(shape)).ok()?;
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: placed_byte_size,
        },
    ] = exit.result_locations.as_slice()
    else {
        return None;
    };
    (*placed_byte_size == byte_size).then_some(*register)
}

pub(super) fn normalized_entry_scalar_result_primitive(
    input: &InstructionSelectionInput<'_>,
) -> Option<PrimitiveType> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == input.entry_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == input.entry_key.state)?;
    input.program.primitive_type_reference(state.return_type)
}

/// Return the normalized register and declared width for an integer primitive
/// entry result. Constant terminals use this instead of the legacy fixed i32
/// shape so narrow and pointer-width returns select their actual ABI value.
pub(super) fn normalized_entry_integer_result_placement(
    input: &InstructionSelectionInput<'_>,
) -> Option<(MachineRegister, usize)> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == input.entry_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == input.entry_key.state)?;
    let primitive = input.program.primitive_type_reference(state.return_type)?;
    if matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64) {
        return None;
    }
    let byte_size = primitive.scalar_byte_size()?;
    let shape = ValueShape::integer(
        u16::try_from(byte_size).ok()?,
        u16::try_from(byte_size.max(1)).ok()?,
    );
    let boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(input.target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(shape),
        },
    )
    .ok()?;
    let exit = derive_boundary_exit(boundary.plan(), &[], Some(shape)).ok()?;
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size: placed_byte_size,
        },
    ] = exit.result_locations.as_slice()
    else {
        return None;
    };
    (usize::from(*placed_byte_size) == byte_size).then_some((*register, byte_size))
}

fn entry_slot_value_shape(
    input: &InstructionSelectionInput<'_>,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
) -> Option<ValueShape> {
    let byte_size = u16::try_from(slot.byte_size).ok()?;
    let alignment = u16::try_from(slot.alignment).ok()?;
    if slot.type_descriptor.reference_referee().is_some() {
        return Some(ValueShape::integer(byte_size, alignment));
    }
    if let Some(primitive) = PrimitiveType::from_name(slot.type_name.as_ref()) {
        return match primitive {
            PrimitiveType::F32 | PrimitiveType::F64 => Some(ValueShape::float(byte_size)),
            _ => Some(ValueShape::integer(byte_size, alignment)),
        };
    }

    let data_layout = input
        .layouts
        .data_layouts
        .iter()
        .find(|(_, layout)| {
            layout.symbol == slot.type_symbol || layout.name.as_str() == slot.type_name.as_ref()
        })
        .map(|(_, layout)| layout)?;
    let DataShape::Record { fields } = data_layout.shape else {
        return None;
    };

    let policy = entry_calling_policy(input);
    if (policy == CallingPolicy::Aapcs64
        || (policy == CallingPolicy::SystemVAMD64 && data_layout.layout.size <= 16))
        && let Some(shape) =
            flat_homogeneous_float_aggregate_shape(input, fields, data_layout.layout)
    {
        return Some(shape);
    }

    if policy == CallingPolicy::SystemVAMD64
        && let Some((_, _, sse_eightbytes)) =
            system_v_record_descriptor_shape(input, &slot.type_descriptor)
        && sse_eightbytes != 0
    {
        if byte_size <= 8 {
            return Some(ValueShape::float(byte_size));
        }
        let class = |mask| {
            if sse_eightbytes & mask == 0 {
                SystemVEightbyteClass::Integer
            } else {
                SystemVEightbyteClass::Sse
            }
        };
        return Some(ValueShape::system_v_aggregate(
            byte_size,
            alignment,
            class(0b01),
            class(0b10),
        ));
    }

    // Small records classify as one integer value. The boundary handoff's
    // explicitly-shaped, multiword carrier is admitted as an integer aggregate
    // here so the caller below can split it into platform-arrival words; this
    // remains distinct from general C aggregate classification.
    let entry_is_boundary = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == input.entry_key.machine)
        .is_some_and(|machine| machine.supply_mode.is_boundary_declaration());
    if byte_size <= 8
        || policy == CallingPolicy::Aapcs64
        || policy == CallingPolicy::MicrosoftX64
        || policy == CallingPolicy::SystemVAMD64
        || (entry_is_boundary && byte_size <= 32 && byte_size.is_multiple_of(8))
    {
        return Some(ValueShape::integer(byte_size, alignment));
    }
    None
}

fn entry_calling_policy(input: &InstructionSelectionInput<'_>) -> CallingPolicy {
    input.entry_boundary_plan.map_or_else(
        || CallingPolicy::native_for_target(input.target),
        |plan| plan.call.policy,
    )
}

fn flat_homogeneous_float_aggregate_shape(
    input: &InstructionSelectionInput<'_>,
    fields: psi_arena::HandleSpan<omega_layout::FieldLayout>,
    layout: omega_layout::TypeLayout,
) -> Option<ValueShape> {
    let fields = input.layouts.fields.span(fields)?;
    let members = u8::try_from(fields.len()).ok()?;
    if !(1..=4).contains(&members) {
        return None;
    }
    let member_size = fields.first().and_then(|field| {
        match PrimitiveType::from_name(field.type_name.as_ref())? {
            PrimitiveType::F32 => Some(4usize),
            PrimitiveType::F64 => Some(8usize),
            _ => None,
        }
    })?;
    if fields.iter().enumerate().any(|(index, field)| {
        field.offset != index * member_size
            || field.layout.size != member_size
            || PrimitiveType::from_name(field.type_name.as_ref()).and_then(|primitive| {
                matches!(primitive, PrimitiveType::F32 | PrimitiveType::F64)
                    .then(|| primitive.scalar_byte_size())
                    .flatten()
            }) != Some(member_size)
    }) || layout.size != member_size * fields.len()
        || layout.alignment != member_size
    {
        return None;
    }
    Some(ValueShape::homogeneous_float_aggregate(
        u16::try_from(member_size).ok()?,
        members,
    ))
}

pub(super) fn select_runtime_dispatch_loop_instructions(
    input: &InstructionSelectionInput<'_>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index: input.runtime_dispatch_loop.entry_dispatch_index,
            terminal_dispatch_index: input.runtime_dispatch_loop.terminal_dispatch_index,
        },
        source_key: input.entry_key,
        source_statement: 0,
    });

    let mut runtime_aliases = RuntimeAliasBuffer::with_capacity(input.state_calls.arguments.len());
    let mut runtime_alias_expressions =
        ExpressionTable::with_expression_capacity(input.state_calls.arguments.len());
    let mut local_initializer_expressions = ExpressionTable::with_expression_capacity(
        input.state_calls.arguments.len().saturating_add(4),
    );
    let mut local_initializer_mutable_expressions = ExpressionTable::with_expression_capacity(4);
    let mut local_initializer_segment_expressions = ExpressionTable::with_expression_capacity(4);
    let mut runtime_static_values =
        writes::RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    let mut runtime_storage_write_scratch = RuntimeStorageWriteScratch::default();
    let mut prelude_expansion_cursor = 0usize;
    let mut leaf_expansion_cursor = 0usize;
    let mut straight_line_expansion_cursor = 0usize;
    let mut prelude_selection_scratch = BranchPreludeSelectionScratch::default();
    let mut leaf_selection_scratch = LeafBranchSelectionScratch::default();
    let mut straight_line_selection_scratch = StraightLineBranchSelectionScratch::default();

    for (dispatch_case_index, (_, dispatch_case)) in
        input.runtime_dispatch_loop.cases.iter().enumerate()
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EnterDispatchCase {
                dispatch_index: dispatch_case.dispatch_index,
            },
            source_key: dispatch_case.key,
            source_statement: 0,
        });

        if let Some(runtime_body) = input
            .runtime_bodies
            .bodies
            .storage_slice()
            .get(dispatch_case_index)
            .filter(|body| body.dispatch_index == dispatch_case.dispatch_index)
            .or_else(|| {
                input
                    .runtime_bodies
                    .bodies
                    .iter()
                    .find(|(_, body)| body.dispatch_index == dispatch_case.dispatch_index)
                    .map(|(_, body)| body)
            })
            && let Some(operations) = input
                .runtime_bodies
                .operations
                .paged_span(runtime_body.operations)
        {
            runtime_aliases.clear();
            runtime_alias_expressions.clear();
            runtime_static_values.clear();
            runtime_storage_write_scratch.clear();

            // `let v = self.f(...)` call-result selections deferred past the
            // callee's spliced effect operations; each waits here for the
            // statement's own LocalStorage operation (see the deferral note at
            // the leaf emission below).
            let mut deferred_leaf_operations: Vec<DeferredLeafOperation> = Vec::new();

            for (operation_index, operation) in operations.iter().enumerate() {
                let (call_ordinal, call_target, target_entry) = match &operation.kind {
                    RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                        call_ordinal,
                        target_key,
                        ..
                    }
                    | RuntimeDispatchBodyOperationKind::InlineStateCall {
                        call_ordinal,
                        target_key,
                        ..
                    }
                    | RuntimeDispatchBodyOperationKind::StateCall {
                        call_ordinal,
                        target_key,
                        ..
                    } => (
                        Some(Some(*call_ordinal)),
                        Some(target_key.state),
                        Some(*target_key),
                    ),
                    RuntimeDispatchBodyOperationKind::StateCallResult {
                        call_ordinal,
                        target_key,
                        ..
                    } => (Some(Some(*call_ordinal)), Some(target_key.state), None),
                    RuntimeDispatchBodyOperationKind::HostCall { call_ordinal } => {
                        (Some(Some(*call_ordinal)), None, None)
                    }
                    _ => (None, None, None),
                };
                selected_instructions.begin_permission_site(
                    operation.source_key,
                    operation.statement_index,
                    call_ordinal,
                    call_target,
                );
                if let Some(target_key) = target_entry {
                    selected_instructions.include_state_entry_permission_events(target_key);
                }
                bind_runtime_operation_aliases(
                    input,
                    operation,
                    &mut runtime_aliases,
                    &mut runtime_alias_expressions,
                );
                if let RuntimeDispatchBodyOperationKind::DynamicStateCall { call_ordinal, .. } =
                    operation.kind
                {
                    super::dynamic_calls::select_dynamic_state_call(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        call_ordinal,
                        operands,
                        selected_instructions,
                    );
                    continue;
                }

                // The synthesized wire encoder/decoder calls surface as
                // unresolved state calls (no real machine exists for
                // `Schema::encode` / `Schema::decode`); lower them
                // into their append/read sequences before the state-call
                // machinery skips them.
                if super::wire_encode::select_wire_encode_call(
                    input,
                    dispatch_case.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    selected_instructions,
                ) {
                    // The synthesized wire call mutates its &mut arguments
                    // OUTSIDE the assignment machinery that feeds
                    // static_values: drop every recorded constant so later
                    // reads come from live storage. Skipping this folded
                    // `let m = d.x == 3` after a DECODE to the pre-decode
                    // constant (the wire-wide decode-then-let-compare
                    // divergence) -- plain machine calls invalidate through
                    // their own selection; the wire branches were the only
                    // call shapes that did not.
                    runtime_static_values.clear();
                    continue;
                }
                if super::wire_decode::select_wire_decode_call(
                    input,
                    dispatch_case.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    selected_instructions,
                ) {
                    // See the wire-encode arm above: decode writes the
                    // value's fields, the read cursor, and the verdict
                    // through &mut arguments this walk cannot see.
                    runtime_static_values.clear();
                    continue;
                }

                if matches!(
                    operation.kind,
                    RuntimeDispatchBodyOperationKind::LocalStorage { .. }
                ) {
                    // A deferred call-result selection lands HERE, timed so the
                    // callee's terminal-value slot is live when the copy fires.
                    //
                    // Case A (caller-owned LocalStorage): the statement's own
                    // `LocalStorage { source_key=caller, stmt=N }` arrives.
                    // The deferred op fires BEFORE the local initializer write,
                    // so the expansion writes into the call-result slot BEFORE
                    // the local-initializer copy reads it.
                    //
                    // Case B (callee-body LocalStorage, last one): the callee's
                    // spliced `LocalStorage { source_key=callee_target_key }` is
                    // the last such op for this call.  The deferred op fires
                    // AFTER the local initializer write so the callee-frame slot
                    // (e.g. `rr`) is written before the expansion reads it.
                    while let Some(deferred_index) =
                        deferred_leaf_operations.iter().position(|deferred| {
                            deferred.operation.source_key == operation.source_key
                                && deferred.operation.statement_index == operation.statement_index
                        })
                    {
                        let deferred = deferred_leaf_operations.remove(deferred_index);
                        if deferred.defer_prelude {
                            select_runtime_branch_preludes_for_operation(
                                input,
                                dispatch_case.dispatch_index,
                                &deferred.operation,
                                &mut prelude_expansion_cursor,
                                &mut prelude_selection_scratch,
                                operands,
                                runtime_value_operands,
                                selected_instructions,
                            );
                        }
                        select_runtime_straight_line_branch_expansions_for_operation(
                            input,
                            dispatch_case.dispatch_index,
                            &deferred.operation,
                            &mut straight_line_expansion_cursor,
                            &mut straight_line_selection_scratch,
                            operands,
                            runtime_value_operands,
                            selected_instructions,
                        );
                        select_runtime_leaf_branch_expansions_for_operation(
                            input,
                            dispatch_case.dispatch_index,
                            &deferred.operation,
                            &mut leaf_expansion_cursor,
                            &mut leaf_selection_scratch,
                            runtime_value_operands,
                            selected_instructions,
                        );
                    }

                    // A local initialized by a HOST CALL (`let n = self.host.write(
                    // fd, bytes)`, the ergonomic-wrapper shape) must emit the CALL
                    // itself — its result operand (argument[0], the synthesized local
                    // Name from collection) writes into the local's slot. Routing it
                    // through the value-only local-initializer write and `continue`-ing
                    // would SILENTLY DROP it (file created, never written). This is the
                    // local-target sibling of the field-target host-call emission below
                    // (~line 631). host_call_for_statement is None for a non-host-call
                    // `let`, so those keep the value-write path.
                    if let Some(host_call) = host_call_for_statement(
                        input,
                        operation.source_key,
                        operation.statement_index,
                    ) {
                        let alias_bindings = runtime_aliases.bindings();
                        let alias_context =
                            (!alias_bindings.is_empty()).then_some(RuntimeAliasResolutionContext {
                                aliases: alias_bindings,
                                alias_expressions: &runtime_alias_expressions,
                            });
                        select_host_call(
                            input,
                            host_call,
                            Some(dispatch_case.dispatch_index),
                            alias_context,
                            operands,
                            runtime_value_operands,
                            selected_instructions,
                        );
                    } else {
                        select_runtime_dispatch_local_initializer_write(
                            input,
                            dispatch_case.dispatch_index,
                            operation.source_key,
                            operation.statement_index,
                            runtime_aliases.bindings(),
                            &runtime_alias_expressions,
                            &mut local_initializer_expressions,
                            &mut local_initializer_mutable_expressions,
                            &mut local_initializer_segment_expressions,
                            &mut runtime_static_values,
                            runtime_value_operands,
                            selected_instructions,
                        );
                    }

                    // Case B: fire AFTER the local initializer write so the
                    // callee's frame slot is populated before the expansion reads it.
                    fire_ready_deferred_leaf_expansions(
                        input,
                        dispatch_case.dispatch_index,
                        operation,
                        operation_index,
                        &operations,
                        dispatch_case.key,
                        &mut deferred_leaf_operations,
                        &mut prelude_expansion_cursor,
                        &mut prelude_selection_scratch,
                        &mut straight_line_expansion_cursor,
                        &mut straight_line_selection_scratch,
                        &mut leaf_expansion_cursor,
                        &mut leaf_selection_scratch,
                        operands,
                        runtime_value_operands,
                        selected_instructions,
                    );

                    continue;
                }

                select_runtime_storage_write_for_operation(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                    runtime_aliases.bindings(),
                    &runtime_alias_expressions,
                    &mut runtime_static_values,
                    &mut runtime_storage_write_scratch,
                    runtime_value_operands,
                    selected_instructions,
                );

                // A deferred value-call leaf whose callee's LAST body op is a
                // MUTATION (`machine make(alive) { self.flag = alive;
                // transition self.flag {..} }`) fires HERE, after the spliced
                // field write above -- the Mutation analogue of the Case-B
                // LocalStorage/HostCall firing blocks. Without it the inline
                // guard was emitted at the StateCall, BEFORE the write, and
                // read the field's ZII zero (wrong arm, silently).
                if matches!(
                    operation.kind,
                    RuntimeDispatchBodyOperationKind::Mutation { .. }
                ) {
                    fire_ready_deferred_leaf_expansions(
                        input,
                        dispatch_case.dispatch_index,
                        operation,
                        operation_index,
                        &operations,
                        dispatch_case.key,
                        &mut deferred_leaf_operations,
                        &mut prelude_expansion_cursor,
                        &mut prelude_selection_scratch,
                        &mut straight_line_expansion_cursor,
                        &mut straight_line_selection_scratch,
                        &mut leaf_expansion_cursor,
                        &mut leaf_selection_scratch,
                        operands,
                        runtime_value_operands,
                        selected_instructions,
                    );
                }

                // The splice lays a `let v = self.f(...)` out as
                // [StateCall, ...callee effect ops..., LocalStorage], so the
                // call-result value selection emitted at the StateCall would
                // read the callee's PRE-mutation state. When the statement's
                // only leaf role is AssignmentValue and a LocalStorage
                // operation follows in this body (either the caller's own
                // LocalStorage for this statement, OR the callee's spliced
                // LocalStorage ops from an inlined callee that has internal
                // `let` bindings), defer the selection to after those ops.
                let is_host_call_argument =
                    matches!(
                        operation.kind,
                        RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                            role: StateCallRole::CallArgument,
                            ..
                        } | RuntimeDispatchBodyOperationKind::InlineStateCall {
                            role: StateCallRole::CallArgument,
                            ..
                        } | RuntimeDispatchBodyOperationKind::StateCall {
                            role: StateCallRole::CallArgument,
                            ..
                        }
                    ) && operations.iter().skip(operation_index + 1).any(|later| {
                        matches!(
                            later.kind,
                            RuntimeDispatchBodyOperationKind::HostCall { .. }
                        ) && state_key_matches_statement_source(
                            later.source_key,
                            operation.source_key,
                        ) && later.statement_index == operation.statement_index
                    });
                let leaf_defers_to_local_initializer = leaf_expansions_defer_to_local_initializer(
                    input,
                    dispatch_case.dispatch_index,
                    operation,
                ) || is_host_call_argument;
                let has_caller_local = operations.iter().skip(operation_index + 1).any(|later| {
                    matches!(
                        later.kind,
                        RuntimeDispatchBodyOperationKind::LocalStorage { .. }
                    ) && later.source_key == operation.source_key
                        && later.statement_index == operation.statement_index
                });
                // Every effect in the contiguous nested splice must precede
                // the call's branching expansion, including effects owned by
                // a nested callee rather than the direct target. The runtime
                // dispatch body's source key marks the splice boundary.
                let has_spliced_effect = state_call_target_key(operation).is_some()
                    && operations
                        .iter()
                        .skip(operation_index + 1)
                        .take_while(|later| {
                            later.source_key != operation.source_key
                                && later.source_key != dispatch_case.key
                        })
                        .any(|later| {
                            matches!(
                                later.kind,
                                RuntimeDispatchBodyOperationKind::LocalStorage { .. }
                                    | RuntimeDispatchBodyOperationKind::HostCall { .. }
                                    | RuntimeDispatchBodyOperationKind::Mutation { .. }
                            )
                        });
                let defers_to_local_initializer =
                    has_spliced_effect || (leaf_defers_to_local_initializer && has_caller_local);
                if defers_to_local_initializer {
                    // Assignment/transition-result and host-argument leaves already
                    // had their preludes emitted at the call site before nested-splice
                    // ordering was introduced. Preserve that declaration-time capture
                    // while deferring only the newly covered statement-call prelude.
                    let defer_prelude = has_spliced_effect
                        && !leaf_defers_to_local_initializer
                        && !is_host_call_argument;
                    if !defer_prelude {
                        select_runtime_branch_preludes_for_operation(
                            input,
                            dispatch_case.dispatch_index,
                            operation,
                            &mut prelude_expansion_cursor,
                            &mut prelude_selection_scratch,
                            operands,
                            runtime_value_operands,
                            selected_instructions,
                        );
                    }
                    // The callee's ARM effects defer WITH the leaf: its branch
                    // prelude, straight-line statements, and terminal expansion
                    // all run after the callee's entry body. Emitting any of that
                    // bundle here, before the entry's spliced effect ops, reverses
                    // source order. In particular, a nested branching prelude can
                    // mutate a field before the parent entry mutation that should
                    // precede it. They fire together at the fire sites below.
                    deferred_leaf_operations.push(DeferredLeafOperation {
                        operation: operation.clone(),
                        defer_prelude,
                    });
                } else {
                    select_runtime_branch_preludes_for_operation(
                        input,
                        dispatch_case.dispatch_index,
                        operation,
                        &mut prelude_expansion_cursor,
                        &mut prelude_selection_scratch,
                        operands,
                        runtime_value_operands,
                        selected_instructions,
                    );
                    select_runtime_straight_line_branch_expansions_for_operation(
                        input,
                        dispatch_case.dispatch_index,
                        operation,
                        &mut straight_line_expansion_cursor,
                        &mut straight_line_selection_scratch,
                        operands,
                        runtime_value_operands,
                        selected_instructions,
                    );
                    select_runtime_leaf_branch_expansions_for_operation(
                        input,
                        dispatch_case.dispatch_index,
                        operation,
                        &mut leaf_expansion_cursor,
                        &mut leaf_selection_scratch,
                        runtime_value_operands,
                        selected_instructions,
                    );
                }

                if matches!(
                    operation.kind,
                    RuntimeDispatchBodyOperationKind::MachineHalt
                ) {
                    selected_instructions.push(SelectedInstruction {
                        kind: SelectedInstructionKind::MachineHalt,
                        source_key: operation.source_key,
                        source_statement: operation.statement_index,
                    });
                }

                if let RuntimeDispatchBodyOperationKind::MemoryFence(kind) = &operation.kind {
                    selected_instructions.push(SelectedInstruction {
                        kind: SelectedInstructionKind::MemoryFence(*kind),
                        source_key: operation.source_key,
                        source_statement: operation.statement_index,
                    });
                }

                if let RuntimeDispatchBodyOperationKind::InterruptControl(kind) = &operation.kind {
                    selected_instructions.push(SelectedInstruction {
                        kind: SelectedInstructionKind::InterruptControl(*kind),
                        source_key: operation.source_key,
                        source_statement: operation.statement_index,
                    });
                }

                if matches!(
                    operation.kind,
                    RuntimeDispatchBodyOperationKind::FlagsRestore
                ) && let Some(source_expression) = super::lookups::asm_flags_restore_source(
                    input,
                    operation.source_key,
                    operation.statement_index,
                ) {
                    let scratch = writes::RuntimeStaticValues::with_capacity(0);
                    if let Some(source) = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_calls.expressions,
                        source_expression,
                        &scratch,
                        runtime_value_operands,
                    ) {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::FlagsRestore { source },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if matches!(operation.kind, RuntimeDispatchBodyOperationKind::MsrWrite)
                    && let Some((index_expr, value_expr)) = super::lookups::asm_msr_write_operands(
                        input,
                        operation.source_key,
                        operation.statement_index,
                    )
                {
                    let scratch = writes::RuntimeStaticValues::with_capacity(0);
                    let index = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_calls.expressions,
                        index_expr,
                        &scratch,
                        runtime_value_operands,
                    );
                    let value = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_calls.expressions,
                        value_expr,
                        &scratch,
                        runtime_value_operands,
                    );
                    if let (Some(index), Some(value)) = (index, value) {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::MsrWrite { index, value },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if matches!(operation.kind, RuntimeDispatchBodyOperationKind::MsrRead)
                    && let Some((index_expr, dest_expr)) = super::lookups::asm_msr_read_operands(
                        input,
                        operation.source_key,
                        operation.statement_index,
                    )
                {
                    let scratch = writes::RuntimeStaticValues::with_capacity(0);
                    let index = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_storage.expressions,
                        index_expr,
                        &scratch,
                        runtime_value_operands,
                    );
                    let dest =
                        crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                            input,
                            dispatch_case.dispatch_index,
                            operation.source_key,
                            &input.state_storage.expressions,
                            dest_expr,
                        );
                    if let (Some(index), Some(dest)) = (index, dest) {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::MsrRead {
                                index,
                                dest_region: dest.region,
                                dest_byte_offset: dest.byte_offset,
                            },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if let RuntimeDispatchBodyOperationKind::ControlRegisterWrite(register) =
                    operation.kind
                    && let Some((_, source_expression)) =
                        super::lookups::asm_control_register_write_source(
                            input,
                            operation.source_key,
                            operation.statement_index,
                        )
                {
                    let scratch = writes::RuntimeStaticValues::with_capacity(0);
                    if let Some(source) = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_calls.expressions,
                        source_expression,
                        &scratch,
                        runtime_value_operands,
                    ) {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::ControlRegisterWrite {
                                register,
                                source,
                            },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if let RuntimeDispatchBodyOperationKind::ControlRegisterRead(register) =
                    operation.kind
                    && let Some((_, dest_expr)) =
                        super::lookups::asm_control_register_read_destination(
                            input,
                            operation.source_key,
                            operation.statement_index,
                        )
                {
                    let dest =
                        crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                            input,
                            dispatch_case.dispatch_index,
                            operation.source_key,
                            &input.state_storage.expressions,
                            dest_expr,
                        );
                    if let Some(dest) = dest {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::ControlRegisterRead {
                                register,
                                dest_region: dest.region,
                                dest_byte_offset: dest.byte_offset,
                            },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if matches!(operation.kind, RuntimeDispatchBodyOperationKind::PortWrite)
                    && let Some((port_expr, value_expr)) = super::lookups::asm_port_write_operands(
                        input,
                        operation.source_key,
                        operation.statement_index,
                    )
                {
                    let scratch = writes::RuntimeStaticValues::with_capacity(0);
                    let port = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_calls.expressions,
                        port_expr,
                        &scratch,
                        runtime_value_operands,
                    );
                    let value = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_calls.expressions,
                        value_expr,
                        &scratch,
                        runtime_value_operands,
                    );
                    if let (Some(port), Some(value)) = (port, value) {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::PortWrite { port, value },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if matches!(operation.kind, RuntimeDispatchBodyOperationKind::PortRead)
                    && let Some((port_expr, dest_expr)) = super::lookups::asm_port_read_operands(
                        input,
                        operation.source_key,
                        operation.statement_index,
                    )
                {
                    let scratch = writes::RuntimeStaticValues::with_capacity(0);
                    let port = writes::mutation::resolve_runtime_value_operand_in_table(
                        input,
                        dispatch_case.dispatch_index,
                        operation.source_key,
                        operation.statement_index,
                        &input.state_storage.expressions,
                        port_expr,
                        &scratch,
                        runtime_value_operands,
                    );
                    let dest =
                        crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                            input,
                            dispatch_case.dispatch_index,
                            operation.source_key,
                            &input.state_storage.expressions,
                            dest_expr,
                        );
                    if let (Some(port), Some(dest)) = (port, dest) {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::PortRead {
                                port,
                                dest_region: dest.region,
                                dest_byte_offset: dest.byte_offset,
                            },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if matches!(
                    operation.kind,
                    RuntimeDispatchBodyOperationKind::FlagsSnapshot
                ) && let Some(dest_expression) = super::lookups::asm_flags_snapshot_destination(
                    input,
                    operation.source_key,
                    operation.statement_index,
                ) {
                    let dest =
                        crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                            input,
                            dispatch_case.dispatch_index,
                            operation.source_key,
                            &input.state_storage.expressions,
                            dest_expression,
                        );
                    if let Some(dest) = dest {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::FlagsSnapshot {
                                dest_region: dest.region,
                                dest_byte_offset: dest.byte_offset,
                            },
                            source_key: operation.source_key,
                            source_statement: operation.statement_index,
                        });
                    }
                }

                if matches!(
                    operation.kind,
                    RuntimeDispatchBodyOperationKind::HostCall { .. }
                ) && let Some(host_call) =
                    host_call_for_statement(input, operation.source_key, operation.statement_index)
                {
                    let alias_bindings = runtime_aliases.bindings();
                    let alias_context =
                        (!alias_bindings.is_empty()).then_some(RuntimeAliasResolutionContext {
                            aliases: alias_bindings,
                            alias_expressions: &runtime_alias_expressions,
                        });

                    if runtime_string_descriptor_place(
                        input,
                        host_call,
                        Some(dispatch_case.dispatch_index),
                        alias_context,
                    )
                    .is_none()
                        && let Some(literal_write) =
                            runtime_text_literal_write_for_host_call(input, host_call)
                    {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::WriteRuntimeTextLiteral {
                                buffer: literal_write.buffer,
                                literal: literal_write.literal,
                            },
                            source_key: host_call.source_key,
                            source_statement: host_call.statement_index,
                        });
                    }
                    select_host_call(
                        input,
                        host_call,
                        Some(dispatch_case.dispatch_index),
                        alias_context,
                        operands,
                        runtime_value_operands,
                        selected_instructions,
                    );

                    // Deep-fix bug #1: fire a deferred outer-value-call leaf whose
                    // guard SUBJECT is this callee's host-call result (e.g. `rc`)
                    // HERE -- after select_host_call stored the result into the
                    // callee slot. The HostCall analogue of the Case-B LocalStorage
                    // firing above; without it the guard deferred by the Case-B
                    // HostCall test would never fire. Fire only once no further
                    // callee-body host calls / locals follow for this same callee,
                    // so the guard reads the FINAL store.
                    fire_ready_deferred_leaf_expansions(
                        input,
                        dispatch_case.dispatch_index,
                        operation,
                        operation_index,
                        &operations,
                        dispatch_case.key,
                        &mut deferred_leaf_operations,
                        &mut prelude_expansion_cursor,
                        &mut prelude_selection_scratch,
                        &mut straight_line_expansion_cursor,
                        &mut straight_line_selection_scratch,
                        &mut leaf_expansion_cursor,
                        &mut leaf_selection_scratch,
                        operands,
                        runtime_value_operands,
                        selected_instructions,
                    );
                }
            }
            selected_instructions.end_permission_site();
        }

        let case_edges = input.runtime_dispatch_loop.edges.span(dispatch_case.edges);
        if let Some(edges) = case_edges {
            for edge in edges {
                selected_instructions.begin_permission_site(
                    dispatch_case.key,
                    edge.statement_index,
                    Some(None),
                    match edge.target {
                        omega_state_graph::RuntimeTransitionTarget::State { key, .. } => {
                            Some(key.state)
                        }
                        _ => None,
                    },
                );
                if let omega_state_graph::RuntimeTransitionTarget::State { key, .. } = edge.target {
                    selected_instructions.include_state_entry_permission_events(key);
                }
                select_runtime_dispatch_edge(
                    input,
                    edge,
                    dispatch_case.key,
                    dispatch_case.dispatch_index,
                    runtime_aliases.bindings(),
                    &runtime_alias_expressions,
                    runtime_value_operands,
                    selected_instructions,
                );
                selected_instructions.end_permission_site();
            }
        }
        if case_edges.map_or(true, <[_]>::is_empty) {
            // An edgeless case terminates NATURALLY: exit 0, matching the
            // interpreter oracle (see the terminate-edge zeroing in
            // edges.rs -- same rule, this is the empty-case flavor).
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteReturnRegisterInteger {
                    register: normalized_entry_integer_result_register(input),
                    byte_size: 4,
                    value: 0,
                },
                source_key: dispatch_case.key,
                source_statement: 0,
            });
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_key: dispatch_case.key,
                source_statement: 0,
            });
        }

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::LeaveDispatchCase,
            source_key: dispatch_case.key,
            source_statement: 0,
        });
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::LeaveDispatchLoop,
        source_key: input.entry_key,
        source_statement: 0,
    });
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_dispatch_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    aliases: &[crate::selection::bindings::RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    expressions: &mut ExpressionTable,
    mutable_expressions: &mut ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    static_values: &mut writes::RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && state_key_matches_statement_source(slot.source_key, source_key)
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
    else {
        return;
    };

    expressions.clear();
    let Some(initializer) =
        local_initializer_handle(input, expressions, source_key, statement_index)
    else {
        return;
    };
    // A result-slot copy is the complete initializer only when the authored
    // initializer itself is the value call. For `let x = f() + 63`, the slot
    // is merely an operand and the full expression still has to be lowered;
    // treating every AssignmentValue result as the whole initializer silently
    // dropped the surrounding arithmetic.
    let initializer_is_direct_call =
        matches!(expressions.expression(initializer), ExpressionNode::Call(_));
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases, expressions);
    let resolved_initializer = crate::selection::bindings::resolve_runtime_alias_binding_handle(
        initializer,
        source_key,
        copied_aliases.bindings(),
        expressions,
    );
    let resolved_initializer_source_key = resolved_initializer.source_key;
    // State calls have already been sequenced and their values materialized in
    // ordinal result slots before this write. Keep the authored call node in a
    // compound initializer so table operand resolution can consume that slot.
    // Re-simplifying `let s = self.classify(x) + 67` can inline `classify` to
    // its terminal expression after scheduling, severing the AssignmentValue
    // call identity and either re-evaluating the callee body or dropping the
    // local write entirely. Bare calls still take the direct-copy path below.
    let initializer_has_assignment_value_call = initializer_statement_has_assignment_value_call(
        input
            .state_calls
            .calls_for_statement(source_key, statement_index)
            .map(|call| call.role),
    );
    let preserve_authored_initializer = initializer_has_assignment_value_call
        || expression_contains_carried_float_provider_plan(
            input,
            resolved_initializer_source_key,
            statement_index,
            expressions,
            resolved_initializer.expression,
        )
        || expression_contains_value_cast(expressions, resolved_initializer.expression)
        || expression_contains_runtime_float_builtin(
            input,
            expressions,
            resolved_initializer.expression,
        );
    let resolved_initializer = if preserve_authored_initializer {
        // Provider-selected float expressions and compiler-owned unary float
        // builtins must consume their authored checked expression. Simplifying
        // a prior local Name into its initializer moves nested float work
        // across statement identity, where exact provider/policy evidence
        // cannot follow. Provider-free constant primitive arithmetic remains
        // eligible for the shared float-semantics simplifier.
        resolved_initializer.expression
    } else {
        simplify_runtime_local_initializer_handle(
            input,
            expressions,
            source_key,
            statement_index,
            resolved_initializer.expression,
        )
        .unwrap_or(resolved_initializer.expression)
    };
    if emit_local_dynamic_conformance_descriptor(
        input,
        dispatch_index,
        source_key,
        statement_index,
        slot,
        expressions,
        resolved_initializer,
        selected_instructions,
    ) {
        return;
    }
    // §5b recast initializer (`let v: &f32 = &self.bits as &f32`): the view
    // is ADDRESS IDENTITY, and a reference-typed let materializes as a
    // pointee-VALUE copy -- so the judged recast strips to its source place
    // here and the write below copies the SOURCE's bytes at the source's
    // width (the stated type drives READS through the view; the guards
    // operand layout reads the referee width). Without the strip every
    // write arm misses the Cast node and the slot stays ZII (the pinned
    // native divergence).
    // RUNG B interior recast (`&self.buf[4] as &u32`): the source place is a
    // 1-byte element, but the view copies the STATED size from its address
    // -- emit the byte copy directly (the judged class guarantees the
    // region holds the footprint). Same-width recasts fall through to the
    // ordinary strip.
    let recast_initializer = match expressions.expression(resolved_initializer) {
        // `&mut x as &mut T` parses as Mutable(Cast(..)); shared recasts are
        // bare Cast nodes. Normalize only for recast recognition so ordinary
        // mutable borrows retain their existing initializer lowering.
        psi_checked_trees::expression::ExpressionNode::Borrow(inner)
            if matches!(
                expressions.expression(inner.target),
                psi_checked_trees::expression::ExpressionNode::Cast(cast)
                    if cast.form.is_recast()
            ) =>
        {
            inner.target
        }
        _ => resolved_initializer,
    };
    if let psi_checked_trees::expression::ExpressionNode::Cast(cast) =
        expressions.expression(recast_initializer)
        && cast.form.is_recast()
    {
        if writes::emit_runtime_frame_slot_slice_descriptor_write_in_table(
            input,
            dispatch_index,
            resolved_initializer_source_key,
            statement_index,
            expressions,
            slot,
            recast_initializer,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
        // A MUTABLE recast must retain ADDRESS identity even when its referee
        // fits in a word. Shared scalar recasts deliberately content-spill for
        // flat reads; doing that here would make `view = value` dereference the
        // copied bits as a pointer. Materialize the backing place address.
        if cast.form == psi_language_core::CastForm::RecastMutable {
            if let Some(place) =
                crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                    input,
                    dispatch_index,
                    resolved_initializer_source_key,
                    expressions,
                    cast.value,
                )
            {
                selected_instructions.push(SelectedInstruction {
                    kind: write_place_address_direct(
                        place.region,
                        place.byte_offset,
                        slot.byte_offset,
                    ),
                    source_key,
                    source_statement: statement_index,
                });
                return;
            }
            if let Some(indexed) =
                crate::selection::storage_places::resolve_runtime_machine_indexed_target_in_table(
                    input,
                    dispatch_index,
                    resolved_initializer_source_key,
                    expressions,
                    cast.value,
                )
            {
                selected_instructions.push(SelectedInstruction {
                    kind: write_place_address_machine_indexed(
                        indexed.base_byte_offset,
                        indexed.index_region,
                        indexed.index_offset,
                        indexed.index_byte_size,
                        indexed.element_byte_size,
                        indexed.field_byte_offset,
                        slot.byte_offset,
                    ),
                    source_key,
                    source_statement: statement_index,
                });
                return;
            }
        }
        let target_size = recast_target_byte_size(input, cast.target_type);
        let source = cast.value;
        if let Some(size) = target_size
            && let Some(place) =
                crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                    input,
                    dispatch_index,
                    resolved_initializer_source_key,
                    expressions,
                    source,
                )
            && size != place.byte_count
        {
            // A wide named-record reference follows the same address model
            // at a constant offset as at a runtime offset below. Copying the
            // record bytes into a referee-sized slot is inconsistent with the
            // read side, which correctly treats that slot as pointer-bearing;
            // its first field then becomes a bogus address. Preserve the
            // backing-place identity instead.
            let kind = if size > input.runtime_abi.pointer_size {
                crate::selection::runtime_dispatch::write_place_address_direct(
                    place.region,
                    place.byte_offset,
                    slot.byte_offset,
                )
            } else {
                crate::selection::runtime_dispatch::copy_places_direct(
                    place.region,
                    place.byte_offset,
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    size,
                )
            };
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
            return;
        }
        // RUNG C1: a RUNTIME offset (`&self.buf[k] as &u32`) -- the source
        // address is base + k (byte elements), and the view copies the
        // STATED size from it. The judged class guarantees the footprint
        // (high(k) + size <= N via the R1 interval machinery).
        //
        // The REFEREE-SIZE rule splits the lowering (the same rule the read
        // side applies): a stated referee wider than a pointer cannot
        // content-spill -- reads deref the slot -- so the slot receives the
        // ELEMENT ADDRESS (`&self.map_buf[k] as &EfiMemoryDescriptor`, the M2
        // walk). At or under pointer width the slot stays a content copy
        // (the pinned `&u32`/`&f32` shape) and reads stay flat.
        if let Some(size) = target_size
            && let Some(indexed) =
                crate::selection::storage_places::resolve_runtime_machine_indexed_target_in_table(
                    input,
                    dispatch_index,
                    resolved_initializer_source_key,
                    expressions,
                    source,
                )
        {
            let kind = if size > input.runtime_abi.pointer_size {
                write_place_address_machine_indexed(
                    indexed.base_byte_offset,
                    indexed.index_region,
                    indexed.index_offset,
                    indexed.index_byte_size,
                    indexed.element_byte_size,
                    indexed.field_byte_offset,
                    slot.byte_offset,
                )
            } else {
                crate::selection::runtime_dispatch::copy_places_from_machine_indexed(
                    indexed.base_byte_offset,
                    indexed.index_region,
                    indexed.index_offset,
                    indexed.index_byte_size,
                    indexed.element_byte_size,
                    indexed.field_byte_offset,
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    size,
                )
            };
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
            return;
        }
    }
    // A value-call initializer has already materialized its result while the
    // call expansion ran above. Preserve that single-evaluation boundary
    // before attempting to lower the copied initializer expression itself:
    // resolving a bare scalar callee body here can rebind its parameters to
    // unrelated caller-frame operands and overwrite the correct result.
    if initializer_is_direct_call
        && copy_assignment_value_call_result_into_local(
            input,
            dispatch_index,
            source_key,
            statement_index,
            slot,
            selected_instructions,
        )
    {
        return;
    }

    let resolved_initializer = strip_recast_initializer(expressions, resolved_initializer);
    let wrote_slice = writes::emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        dispatch_index,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer,
        runtime_value_operands,
        selected_instructions,
    );
    if wrote_slice {
        return;
    }
    let wrote_text_comparison = writes::emit_runtime_frame_slot_text_comparison_write_in_table(
        input,
        dispatch_index,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer,
        runtime_value_operands,
        selected_instructions,
    );
    if wrote_text_comparison {
        return;
    }
    // An inlined callee's provider call can contain caller-owned operands after
    // alias substitution while the checked provider/policy evidence still
    // belongs to the authored callee statement. Keep those identities split.
    // This matters especially for generated unary calls (`F32::classify`),
    // whose zero source span cannot recover the authored state on its own.
    if preserve_authored_initializer
        && resolved_initializer_source_key != source_key
        && matches!(
            expressions.expression(resolved_initializer),
            ExpressionNode::Call(call)
                if writes::mutation::builtin_runtime_unary_call_operator_in_table(input, call)
                    .is_some()
        )
        && let Some(kind) =
            writes::mutation::select_runtime_storage_binary_write_in_table_with_evidence_source_key(
                input,
                dispatch_index,
                resolved_initializer_source_key,
                source_key,
                statement_index,
                expressions,
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
                resolved_initializer,
                static_values,
                runtime_value_operands,
            )
    {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key,
            source_statement: statement_index,
        });
        return;
    }
    let direct_kind = writes::select_runtime_frame_slot_value_write_in_table(
        input,
        dispatch_index,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer,
        static_values,
        runtime_value_operands,
    );
    if let Some(kind) = direct_kind {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key,
            source_statement: statement_index,
        });
        return;
    }

    let target = writes::runtime_frame_slot_target_expression(expressions, slot);
    let _ = writes::select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        source_key,
        resolved_initializer_source_key,
        statement_index,
        expressions,
        target,
        resolved_initializer,
        &[],
        static_values,
        mutable_expressions,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    );
}

fn initializer_statement_has_assignment_value_call(
    roles: impl IntoIterator<Item = StateCallRole>,
) -> bool {
    roles
        .into_iter()
        .any(|role| role == StateCallRole::AssignmentValue)
}

#[cfg(test)]
mod local_initializer_call_tests {
    use super::{StateCallRole, initializer_statement_has_assignment_value_call};

    #[test]
    fn only_assignment_value_calls_preserve_authored_local_initializers() {
        assert!(initializer_statement_has_assignment_value_call([
            StateCallRole::AssignmentValue,
        ]));
        assert!(!initializer_statement_has_assignment_value_call([
            StateCallRole::Statement,
            StateCallRole::CallArgument,
            StateCallRole::TransitionArgument,
        ]));
    }
}

fn expression_contains_runtime_float_builtin(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            expression_contains_runtime_float_builtin(input, expressions, binary.left)
                || expression_contains_runtime_float_builtin(input, expressions, binary.right)
        }
        ExpressionNode::Atomic(atomic) => {
            expression_contains_runtime_float_builtin(input, expressions, atomic.value)
                || expression_contains_runtime_float_builtin(input, expressions, atomic.result)
        }
        ExpressionNode::ArrayLiteral(values) => expressions
            .expression_handles(*values)
            .iter()
            .any(|value| expression_contains_runtime_float_builtin(input, expressions, *value)),
        ExpressionNode::Cast(cast) => {
            expression_contains_runtime_float_builtin(input, expressions, cast.value)
        }
        ExpressionNode::Call(call) => {
            crate::selection::runtime_dispatch::writes::mutation::builtin_runtime_unary_call_operator_in_table(
                input, call,
            )
            .is_some()
                || crate::selection::runtime_dispatch::writes::mutation::builtin_runtime_binary_float_call_operator_in_table(
                    input, call,
                )
                .is_some()
                || crate::selection::runtime_dispatch::writes::mutation::builtin_runtime_ternary_float_call_operator_in_table(
                    input, call,
                )
                .is_some()
                || (call.receiver.is_valid()
                    && expression_contains_runtime_float_builtin(
                        input,
                        expressions,
                        call.receiver,
                    ))
                || expressions.expression_handles(call.arguments).iter().any(|argument| {
                    expression_contains_runtime_float_builtin(input, expressions, *argument)
                })
        }
        ExpressionNode::Borrow(inner) => {
            expression_contains_runtime_float_builtin(input, expressions, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_contains_runtime_float_builtin(input, expressions, indexed.collection)
                || expression_contains_runtime_float_builtin(input, expressions, indexed.index)
        }
        ExpressionNode::Member(member) => {
            expression_contains_runtime_float_builtin(input, expressions, member.receiver)
        }
        ExpressionNode::Range(range) => {
            (range.start.is_valid()
                && expression_contains_runtime_float_builtin(input, expressions, range.start))
                || (range.end.is_valid()
                    && expression_contains_runtime_float_builtin(input, expressions, range.end))
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| {
                expression_contains_runtime_float_builtin(input, expressions, field.value)
            }),
        ExpressionNode::Unary(unary) => {
            expression_contains_runtime_float_builtin(input, expressions, unary.operand)
        }
        _ => false,
    }
}

fn expression_contains_carried_float_provider_plan(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    if !matches!(
        super::lookups::carried_float_provider_plan(
            input,
            source_key,
            statement_index,
            expressions,
            expression,
        ),
        super::lookups::CarriedFloatProviderPlan::Missing
    ) {
        return true;
    }

    let nested = |expression| {
        expression_contains_carried_float_provider_plan(
            input,
            source_key,
            statement_index,
            expressions,
            expression,
        )
    };
    match expressions.expression(expression) {
        ExpressionNode::Binary(binary) => nested(binary.left) || nested(binary.right),
        ExpressionNode::Atomic(atomic) => nested(atomic.value) || nested(atomic.result),
        ExpressionNode::ArrayLiteral(values) => expressions
            .expression_handles(*values)
            .iter()
            .copied()
            .any(nested),
        ExpressionNode::Cast(cast) => nested(cast.value),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && nested(call.receiver))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(nested)
        }
        ExpressionNode::Borrow(inner) => nested(inner.target),
        ExpressionNode::Indexed(indexed) => nested(indexed.collection) || nested(indexed.index),
        ExpressionNode::Member(member) => nested(member.receiver),
        ExpressionNode::Range(range) => {
            (range.start.is_valid() && nested(range.start))
                || (range.end.is_valid() && nested(range.end))
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| nested(field.value)),
        ExpressionNode::Unary(unary) => nested(unary.operand),
        _ => false,
    }
}

fn expression_contains_value_cast(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> bool {
    let nested = |expression| expression_contains_value_cast(expressions, expression);
    match expressions.expression(expression) {
        ExpressionNode::Cast(cast) => !cast.form.is_recast() || nested(cast.value),
        ExpressionNode::Binary(binary) => nested(binary.left) || nested(binary.right),
        ExpressionNode::Atomic(atomic) => nested(atomic.value) || nested(atomic.result),
        ExpressionNode::ArrayLiteral(values) => expressions
            .expression_handles(*values)
            .iter()
            .copied()
            .any(nested),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && nested(call.receiver))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .copied()
                    .any(nested)
        }
        ExpressionNode::Borrow(inner) => nested(inner.target),
        ExpressionNode::Indexed(indexed) => nested(indexed.collection) || nested(indexed.index),
        ExpressionNode::Member(member) => nested(member.receiver),
        ExpressionNode::Range(range) => {
            (range.start.is_valid() && nested(range.start))
                || (range.end.is_valid() && nested(range.end))
        }
        ExpressionNode::StructLiteral(struct_literal) => expressions
            .struct_fields(struct_literal.fields)
            .iter()
            .any(|field| nested(field.value)),
        ExpressionNode::Unary(unary) => nested(unary.operand),
        _ => false,
    }
}

fn copy_assignment_value_call_result_into_local(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    local_source_key: StateKey,
    statement_index: usize,
    local_slot: &omega_runtime_storage::RuntimeFrameSlot,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(call_result_slot) = input.runtime_storage.call_result_slot(
        dispatch_index,
        local_source_key,
        statement_index,
        StateCallRole::AssignmentValue,
    ) else {
        return false;
    };
    if call_result_slot.byte_size != local_slot.byte_size || local_slot.byte_size == 0 {
        return false;
    }

    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::copy_places_direct(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            call_result_slot.byte_offset,
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            local_slot.byte_offset,
            local_slot.byte_size,
        ),
        source_key: local_source_key,
        source_statement: statement_index,
    });
    true
}

fn simplify_runtime_local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
    expression: ExpressionHandle,
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
    // The expression IS the LocalData's initializer, so its role is the
    // assignment VALUE (the role used to be behaviorally inert -- call-locals
    // are preserved for every role -- but it now selects the DESTINATION
    // landing: the local's declared type is where this value's constants
    // land, CM2).
    let authored = expressions.to_tree(expression);
    let simplified = simplify_state_expression_for_role(
        input.program,
        machine,
        state,
        statement_index,
        StateValueRole::AssignmentValue,
        &authored,
    );
    // Keep the copied checked-tree node when simplification is a no-op. Its
    // authored operator spans are the exact bridge back to provider/policy
    // evidence; reinserting an identical detached tree would erase them.
    Some(if simplified == authored {
        expression
    } else {
        expressions.insert_tree(&simplified)
    })
}

fn local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
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
    let statement = input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let StatementNode::LocalData(local_data) = statement else {
        return None;
    };
    local_data
        .initial_value
        .is_valid()
        .then(|| table.copy_from(&input.program.expression_table, local_data.initial_value))
}

/// Strips a judged §5b recast (`Cast` with a recast form) to its source
/// place. Value casts and every other shape pass through untouched.
fn strip_recast_initializer(
    expressions: &ExpressionTable,
    initializer: ExpressionHandle,
) -> ExpressionHandle {
    match expressions.expression(initializer) {
        psi_checked_trees::expression::ExpressionNode::Cast(cast) if cast.form.is_recast() => {
            cast.value
        }
        psi_checked_trees::expression::ExpressionNode::Borrow(inner) => {
            match expressions.expression(inner.target) {
                psi_checked_trees::expression::ExpressionNode::Cast(cast)
                    if cast.form.is_recast() =>
                {
                    cast.value
                }
                _ => initializer,
            }
        }
        _ => initializer,
    }
}

pub(in crate::selection) fn recast_target_byte_size(
    input: &InstructionSelectionInput<'_>,
    target: psi_checked_trees::types::TypeReferenceHandle,
) -> Option<usize> {
    omega_layout::layout_type_reference(input.program, input.target, target)
        .ok()
        .map(|layout| layout.size)
}

pub(in crate::selection) fn recast_slice_element_count(
    input: &InstructionSelectionInput<'_>,
    target: psi_checked_trees::types::TypeReferenceHandle,
    source_byte_count: usize,
) -> Option<usize> {
    let psi_checked_trees::types::TypeReferenceNode::Slice { element_type } =
        input.program.type_reference_table.type_reference(target)
    else {
        return None;
    };
    let element =
        omega_layout::layout_type_reference(input.program, input.target, *element_type).ok()?;
    (element.size > 0 && source_byte_count % element.size == 0)
        .then_some(source_byte_count / element.size)
}

/// Extract the callee `target_key` from a StateCall-family operation, or
/// `None` if the operation is not a state call.
/// Fires every deferred value-call leaf whose callee has NO further body ops
/// in its contiguous spliced run -- the shared tail of the three fire sites
/// (after a spliced LocalStorage initializer, a spliced Mutation field write,
/// and a spliced HostCall result store).
///
/// A call's splice ends when its CALLER source key resumes. Every operation
/// between the deferred StateCall and that boundary belongs to the call,
/// including operations from nested statement callees. Matching only the
/// outer target key fired a return after the outer callee's last direct write
/// but before a nested helper's write (`forward` returned PRE-`capture`
/// state). The deferred operation is already live only after its StateCall has
/// been visited, so any current foreign-key operation before caller resumption
/// is inside that contiguous splice. Fires branch prelude, straight-line arm
/// statements, then leaf expansions, in reverse-index order so removal doesn't
/// shift earlier indices.
///
/// Keep this the ONLY copy: the three sites drifted twice before extraction
/// (faces #4 and #5 each patched four hand-copied scans in lockstep).
#[allow(clippy::too_many_arguments)]
fn fire_ready_deferred_leaf_expansions(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    operation_index: usize,
    operations: &PagedSlice<'_, RuntimeDispatchBodyOperation>,
    dispatch_body_key: StateKey,
    deferred_leaf_operations: &mut Vec<DeferredLeafOperation>,
    prelude_expansion_cursor: &mut usize,
    prelude_selection_scratch: &mut BranchPreludeSelectionScratch,
    straight_line_expansion_cursor: &mut usize,
    straight_line_selection_scratch: &mut StraightLineBranchSelectionScratch,
    leaf_expansion_cursor: &mut usize,
    leaf_selection_scratch: &mut LeafBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let deferred_indices_to_fire: Vec<usize> = deferred_leaf_operations
        .iter()
        .enumerate()
        .filter_map(|(deferred_index, deferred)| {
            state_call_target_key(&deferred.operation)?;
            if operation.source_key == deferred.operation.source_key {
                return None;
            }
            let has_more = operations
                .iter()
                .skip(operation_index + 1)
                .take_while(|later| {
                    later.source_key != deferred.operation.source_key
                        && later.source_key != dispatch_body_key
                })
                .next()
                .is_some();
            if has_more { None } else { Some(deferred_index) }
        })
        .collect();
    for deferred_index in deferred_indices_to_fire.into_iter().rev() {
        let deferred = deferred_leaf_operations.remove(deferred_index);
        if deferred.defer_prelude {
            select_runtime_branch_preludes_for_operation(
                input,
                dispatch_index,
                &deferred.operation,
                prelude_expansion_cursor,
                prelude_selection_scratch,
                operands,
                runtime_value_operands,
                selected_instructions,
            );
        }
        select_runtime_straight_line_branch_expansions_for_operation(
            input,
            dispatch_index,
            &deferred.operation,
            straight_line_expansion_cursor,
            straight_line_selection_scratch,
            operands,
            runtime_value_operands,
            selected_instructions,
        );
        select_runtime_leaf_branch_expansions_for_operation(
            input,
            dispatch_index,
            &deferred.operation,
            leaf_expansion_cursor,
            leaf_selection_scratch,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

#[derive(Clone)]
struct DeferredLeafOperation {
    operation: RuntimeDispatchBodyOperation,
    defer_prelude: bool,
}

fn state_call_target_key(operation: &RuntimeDispatchBodyOperation) -> Option<StateKey> {
    match operation.kind {
        RuntimeDispatchBodyOperationKind::StateCall { target_key, .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { target_key, .. }
        | RuntimeDispatchBodyOperationKind::InlineLeafStateCall { target_key, .. } => {
            Some(target_key)
        }
        _ => None,
    }
}

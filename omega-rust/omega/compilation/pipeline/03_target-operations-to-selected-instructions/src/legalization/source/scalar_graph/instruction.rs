//! `project` turns one optimization node into its legalized scalar
//! instruction: short operation kinds are projected here and the long ones
//! by `value_instructions`, `storage_instructions`, `call_instructions` and
//! `scalar_instructions`.

use super::{AbstractOperationPlan, Error, PsiOptimizationUnit, TargetOperationPlan};
use abstract_operations::AbstractOperation;
use legalized_operations::{
    LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedValueDefinition,
};
mod call_instructions;
mod scalar_instructions;
mod storage_instructions;

use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use semantic_vocabulary::IntegerValue;
pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    optimized: &optimization_unit::PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
    targeted: &target_operations::TargetUnitOperation,
) -> Result<LegalizedScalarInstruction, LegalizationError> {
    let (operation, result) = scalar_graph_input::instruction(node).ok_or(Error::custody())?;
    // Installed and ordinary calls already share the target producer's call
    // row. Preserve that origin directly instead of manufacturing a replacement
    // source node and recursively projecting it as an ordinary call.
    let kind = if matches!(
        targeted,
        target_operations::TargetUnitOperation::Call { .. }
    ) {
        call_instructions::project_call(optimized, unit, targeted, native, plan)?
    } else if let target_operations::TargetUnitOperation::PlacedScalarAccess {
        boundary,
        realization,
        location,
        interpretation,
        operand,
        ..
    } = targeted
    {
        let root = optimized
            .arrival_parameter(location.place)
            .map(|parameter| parameter.structural_type)
            .or_else(|| {
                scalar_graph_input::structural_case::source_result(optimized, location.place)
                    .ok()
                    .map(|(_, result)| result.structural_type)
            })
            .ok_or(Error::custody())?;
        let geometry =
            crate::structural_inputs::structural_reference_input::reference_source_geometry(
                root,
                &location.path,
                unit.structural_types.as_slice(),
            )
            .ok_or(Error::custody())?;
        let indices = crate::legalization::runtime_indices::operands(
            optimized,
            unit,
            operation,
            &geometry.elements,
        )?;
        LegalizedScalarInstructionKind::PlacedScalarAccess {
            boundary: *boundary,
            realization: *realization,
            location: location.clone(),
            interpretation: interpretation.clone(),
            indices,
            operand: operand.map(|source| source.source_value()),
        }
    } else {
        match &node.operation {
            AbstractOperation::SelectImplementation { table, .. } => {
                LegalizedScalarInstructionKind::SelectImplementation { table: *table }
            }
            AbstractOperation::AtomicEvent {
                event: abstract_operations::AbstractAtomicEvent::Fence { ordering },
                ..
            } => LegalizedScalarInstructionKind::AtomicFence {
                ordering: *ordering,
            },
            AbstractOperation::AtomicEvent { event, .. } => {
                let access = scalar_graph_input::atomic_access::access(
                    event,
                    optimized,
                    &unit.structural_types,
                )
                .ok_or(Error::custody())?;
                LegalizedScalarInstructionKind::AtomicAccess {
                    location: access.location,
                    field: access.field,
                    scalar_type: access.scalar_type,
                    field_byte_offset: access.field_byte_offset,
                    event: access.event,
                }
            }
            AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
                let target_operations::TargetUnitOperation::StructuralByteSequenceFieldLength {
                    source,
                    field,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                    indices: scalar_graph_input::structural_fields::byte_index_operands(
                        optimized, unit, operation, source, *field,
                    )?,
                    source: source.clone(),
                    field: *field,
                }
            }
            AbstractOperation::StructuralScalarFieldRead { .. } => {
                let target_operations::TargetUnitOperation::StructuralScalarFieldRead {
                    source,
                    case,
                    field,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    source: source.clone(),
                    case: *case,
                    field: *field,
                }
            }
            AbstractOperation::IntegerBitwiseAnd { left, right, .. } => {
                LegalizedScalarInstructionKind::BitwiseAnd {
                    left: *left,
                    right: *right,
                }
            }
            AbstractOperation::IntegerBitwiseOr { left, right, .. } => {
                LegalizedScalarInstructionKind::BitwiseOr {
                    left: *left,
                    right: *right,
                }
            }
            AbstractOperation::IntegerBitwiseXor { left, right, .. } => {
                LegalizedScalarInstructionKind::BitwiseXor {
                    left: *left,
                    right: *right,
                }
            }
            AbstractOperation::IntegerBitwiseNot { operand, .. } => {
                LegalizedScalarInstructionKind::BitwiseNot { operand: *operand }
            }
            AbstractOperation::FloatResultPolicy { format, policy, .. } => {
                LegalizedScalarInstructionKind::FloatResultPolicy {
                    format: *format,
                    policy: policy.clone(),
                }
            }
            AbstractOperation::ScalarRepresentationView {
                operand,
                source_type,
                ..
            } => LegalizedScalarInstructionKind::ScalarRepresentationView {
                operand: *operand,
                source_type: *source_type,
            },
            AbstractOperation::IeeeFloatCompare {
                comparison,
                format,
                left,
                right,
                ..
            } => LegalizedScalarInstructionKind::IeeeFloatCompare {
                comparison: *comparison,
                format: *format,
                left: *left,
                right: *right,
            },
            AbstractOperation::EstablishPlacedReference {
                psi_operation,
                result,
                source,
                correspondence,
            } => LegalizedScalarInstructionKind::EstablishPlacedReference {
                result: result.clone(),
                source: source.clone(),
                correspondence: correspondence.clone(),
                indices: crate::legalization::runtime_indices::reference_source(
                    optimized,
                    unit,
                    *psi_operation,
                    source,
                )?,
                shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape(),
            },
            AbstractOperation::EstablishReference {
                psi_operation,
                result,
                source,
                storage,
                ..
            } => LegalizedScalarInstructionKind::EstablishReference {
                result: result.clone(),
                source: source.clone(),
                storage: *storage,
                indices: crate::legalization::runtime_indices::reference_source(
                    optimized,
                    unit,
                    *psi_operation,
                    source,
                )?,
                shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape(),
            },
            AbstractOperation::ReleaseReference { source, path, .. } => {
                LegalizedScalarInstructionKind::ReleaseReference {
                    source: *source,
                    path: path.clone(),
                }
            }
            AbstractOperation::EstablishZeroedStructural { result, .. } => {
                LegalizedScalarInstructionKind::EstablishZeroedStructural {
                    result: result.clone(),
                    shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?
                        .shape(),
                }
            }
            AbstractOperation::EstablishStructuralLocal { result, .. } => {
                LegalizedScalarInstructionKind::EstablishStructuralLocal {
                    result: result.clone(),
                    shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?
                        .shape(),
                }
            }
            AbstractOperation::EstablishStructuralArray {
                result, elements, ..
            } => LegalizedScalarInstructionKind::EstablishStructuralArray {
                result: result.clone(),
                elements: elements.clone(),
                shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?.shape(),
            },
            AbstractOperation::EstablishRecord { result, fields, .. } => {
                LegalizedScalarInstructionKind::EstablishRecord {
                    result: result.clone(),
                    fields: fields.clone(),
                    shape: scalar_graph_input::aggregate_results::home_layout(result, plan)?
                        .shape(),
                }
            }
            AbstractOperation::EstablishScalarArray {
                result, elements, ..
            } => LegalizedScalarInstructionKind::EstablishScalarArray {
                result: result.clone(),
                elements: elements.clone(),
                shape: scalar_graph_input::scalar_arrays::shape(result, plan)?.2,
            },
            AbstractOperation::EstablishCase {
                result,
                result_case,
                fields,
                ..
            } => LegalizedScalarInstructionKind::EstablishCase {
                result: result.clone(),
                result_case: *result_case,
                fields: fields.clone(),
                layout: scalar_graph_input::aggregate_results::sum_layout(result, plan)?,
            },
            AbstractOperation::EstablishPrimitiveLocal { result, value, .. } => {
                LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
                    result: result.clone(),
                    value: *value,
                    shape: scalar_graph_input::scalar_shape(value.scalar_type)
                        .ok_or(Error::custody())?,
                }
            }
            AbstractOperation::PrimitiveLocalStore {
                destination, value, ..
            } => LegalizedScalarInstructionKind::PrimitiveLocalStore {
                destination: *destination,
                value: *value,
            },
            AbstractOperation::StructuralCaseMembership { .. } => {
                storage_instructions::project_structural_case_membership(
                    node, optimized, plan, unit,
                )?
            }
            AbstractOperation::StructuralLeafCopy { .. } => {
                storage_instructions::project_structural_leaf_copy(node, optimized, plan, unit)?
            }
            abstract_operations::move_or_store_structural_field_pattern!() => {
                storage_instructions::project_borrowed_window(
                    node, optimized, plan, unit, targeted,
                )?
            }
            AbstractOperation::EstablishTrivialAffineLocal {
                place,
                structural_type,
                ..
            } => LegalizedScalarInstructionKind::EstablishTrivialAffineLocal {
                place: *place,
                structural_type: structural_type.clone(),
            },
            AbstractOperation::PrimitiveScalarRead { .. } => {
                storage_instructions::project_primitive_scalar_read(node, optimized, unit)?
            }
            // An evaluated normalized foreign row precedes hosted settlement
            // probing: the row carries its own provider execution and is never a
            // hosted realization.
            AbstractOperation::BoundaryCall { .. }
                if scalar_graph_input::normalized_foreign::row(
                    native,
                    optimized.machine,
                    operation,
                )?
                .is_some() =>
            {
                call_instructions::project_normalized_foreign_call(
                    node, optimized, native, plan, unit, operation,
                )?
            }
            // Inline float arithmetic carries only the replayed operation,
            // format and SSA operands; its row was rejoined by input validation.
            AbstractOperation::BoundaryCall {
                boundary,
                result,
                arguments,
                ..
            } if scalar_graph_input::ieee_float_arithmetic::row(
                native,
                optimized.machine,
                operation,
            )?
            .is_some() =>
            {
                if let Some(target_operations::TargetUnitOperation::IeeeFloatClassify {
                    boundary: row_boundary,
                    realization,
                    result_home,
                    ..
                }) = scalar_graph_input::ieee_float_arithmetic::row(
                    native,
                    optimized.machine,
                    operation,
                )? {
                    let abstract_operations::AbstractBoundaryResult::Structural(result) = result
                    else {
                        return Err(Error::custody());
                    };
                    if *row_boundary != *boundary || arguments.len() != 1 {
                        return Err(Error::custody());
                    }
                    LegalizedScalarInstructionKind::IeeeFloatClassify {
                        boundary: *boundary,
                        format: realization.format,
                        argument: arguments[0],
                        result: result.clone(),
                        layout: result_home.layout.sum().ok_or(Error::custody())?.clone(),
                    }
                } else {
                    let Some(target_operations::TargetUnitOperation::NearestIeeeFloatArithmetic {
                        boundary: row_boundary,
                        realization,
                        ..
                    }) = scalar_graph_input::ieee_float_arithmetic::row(
                        native,
                        optimized.machine,
                        operation,
                    )?
                    else {
                        return Err(Error::custody());
                    };
                    if arguments.len() != realization.operation.operand_count()
                        || row_boundary != boundary
                    {
                        return Err(Error::custody());
                    }
                    LegalizedScalarInstructionKind::NearestIeeeFloatArithmetic {
                        boundary: *boundary,
                        realization: *realization,
                        arguments: arguments.clone(),
                    }
                }
            }
            AbstractOperation::BoundaryCall {
                boundary,
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } if matches!(
                scalar_graph_input::hosted_realization(native, optimized.machine, operation)?,
                target_operations::BoundaryRealization::HostedReadByte(_)
            ) =>
            {
                LegalizedScalarInstructionKind::HostedReadByte {
                    boundary: *boundary,
                    result: result.clone(),
                    layout: scalar_graph_input::read_byte::layout(result, plan)?,
                }
            }
            AbstractOperation::BoundaryCall { .. } => {
                call_instructions::project_boundary_call(node, optimized, native, operation)?
            }
            AbstractOperation::WriteOnlyPrimitiveStore { .. } => {
                storage_instructions::project_write_only_primitive_store(node, optimized, unit)?
            }
            AbstractOperation::StructuralScalarFieldStore { .. } => {
                storage_instructions::project_structural_scalar_field_store(
                    node, optimized, unit, targeted,
                )?
            }
            AbstractOperation::EstablishByteSequenceLiteral {
                place,
                structural_type,
                bytes,
                ..
            } => LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                destination: *place,
                structural_type: structural_type.clone(),
                bytes: bytes.clone(),
            },
            AbstractOperation::ByteSequenceSubslice { .. } => {
                storage_instructions::project_byte_sequence_subslice(node, optimized, unit)?
            }
            AbstractOperation::StructuralByteSequenceFieldByteStore { .. } => {
                storage_instructions::project_structural_byte_sequence_field_byte_store(
                    node, optimized, unit, targeted,
                )?
            }
            AbstractOperation::StructuralByteSequenceFieldRead { .. } => {
                storage_instructions::project_structural_byte_sequence_field_read(
                    node, optimized, unit,
                )?
            }
            AbstractOperation::EstablishByteSequenceConcatenation { .. } => {
                storage_instructions::project_byte_sequence_concatenation(node, optimized, unit)?
            }
            AbstractOperation::ByteSequenceStore { .. } => {
                storage_instructions::project_byte_sequence_store(node, optimized, unit, targeted)?
            }
            AbstractOperation::StructuralByteSequenceFieldStore { .. } => {
                storage_instructions::project_structural_byte_sequence_field_store(
                    node, optimized, unit, targeted,
                )?
            }
            AbstractOperation::StructuralByteViewFieldStore { field, source, .. } => {
                let target_operations::TargetUnitOperation::StructuralByteViewFieldStore {
                    destination,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
                    indices: scalar_graph_input::structural_fields::view_slot_index_operands(
                        optimized,
                        unit,
                        operation,
                        destination,
                        *field,
                    )?,
                    destination: destination.clone(),
                    field: *field,
                    source: *source,
                }
            }
            AbstractOperation::ByteSequenceWrite { .. } => {
                storage_instructions::project_byte_sequence_write(node, optimized, unit)?
            }
            AbstractOperation::ByteSequenceRead { .. } => {
                storage_instructions::project_byte_sequence_read(node, optimized, unit)?
            }
            AbstractOperation::ByteSequencePredicate {
                operand, predicate, ..
            } => LegalizedScalarInstructionKind::ByteSequencePredicate {
                operand: operand.clone(),
                predicate: *predicate,
            },
            AbstractOperation::ByteSequenceEqual { left, right, .. } => {
                LegalizedScalarInstructionKind::ByteSequenceEqual {
                    left: left.clone(),
                    right: right.clone(),
                }
            }
            AbstractOperation::ByteSequenceLength { source, .. } => {
                let (geometry, carrier) =
                    crate::legalization::scalar_graph_input::element_views::byte_length_geometry(
                        optimized,
                        source,
                        &unit.structural_types,
                    )
                    .ok_or(LegalizationError::custody())?;
                LegalizedScalarInstructionKind::ByteSequenceLength {
                    source: source.clone(),
                    indices: crate::legalization::runtime_indices::operands(
                        optimized,
                        unit,
                        node.operation
                            .psi_operation()
                            .ok_or(LegalizationError::custody())?,
                        &geometry.elements,
                    )?,
                    length_byte_offset: if matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    ) {
                        8
                    } else {
                        0
                    },
                }
            }
            AbstractOperation::ElementViewLength { source, .. } => {
                let geometry =
                    crate::legalization::scalar_graph_input::element_views::length_geometry(
                        optimized,
                        source,
                        &unit.structural_types,
                    )
                    .ok_or(LegalizationError::custody())?;
                LegalizedScalarInstructionKind::ElementViewLength {
                    source: source.clone(),
                    indices: crate::legalization::runtime_indices::operands(
                        optimized,
                        unit,
                        node.operation
                            .psi_operation()
                            .ok_or(LegalizationError::custody())?,
                        &geometry.elements,
                    )?,
                    length_byte_offset: 8,
                }
            }
            AbstractOperation::ElementViewRead { .. } => {
                storage_instructions::project_element_view_read(node, optimized, unit)?
            }
            AbstractOperation::ElementViewWrite { .. } => {
                storage_instructions::project_element_view_write(node, optimized, unit)?
            }
            AbstractOperation::ElementViewSubslice { .. } => {
                storage_instructions::project_element_view_subslice(node, optimized, unit)?
            }
            AbstractOperation::EstablishElementView {
                psi_operation,
                result,
                destination,
                source,
                element,
                region,
            } => {
                let accepted_fact = match region {
                    terminal_psi::ElementViewRegion::WholeCollection => None,
                    terminal_psi::ElementViewRegion::RepresentationBytes { obligation, .. } => {
                        Some(
                            unit.accepted_obligation_facts
                                .iter()
                                .find(|fact| {
                                    fact.machine == optimized.machine
                                        && fact.operation == *psi_operation
                                        && fact.obligation == *obligation
                                })
                                .ok_or(Error::custody())?
                                .identity,
                        )
                    }
                };
                LegalizedScalarInstructionKind::EstablishElementView {
                    indices: crate::legalization::runtime_indices::collection_view(
                        optimized,
                        unit,
                        *psi_operation,
                        source,
                    )?,
                    result: result.clone(),
                    destination: *destination,
                    source: source.clone(),
                    element: *element,
                    region: *region,
                    accepted_fact,
                }
            }
            AbstractOperation::BooleanNot { operand, .. } => {
                LegalizedScalarInstructionKind::BooleanNot { operand: *operand }
            }
            AbstractOperation::IntegerWiden {
                operand,
                source_type,
                ..
            } => LegalizedScalarInstructionKind::IntegerWiden {
                operand: *operand,
                source_type: *source_type,
            },
            AbstractOperation::IntegerExactCast { .. } => {
                scalar_instructions::project_integer_exact_cast(node, optimized, unit)?
            }
            AbstractOperation::IntegerConstant { value, .. } => {
                LegalizedScalarInstructionKind::Constant(*value)
            }
            AbstractOperation::IeeeFloatConstant { value, .. } => {
                let bits = match value {
                    semantic_vocabulary::IeeeFloatValue::Binary32(bits) => u128::from(*bits),
                    semantic_vocabulary::IeeeFloatValue::Binary64(bits) => u128::from(*bits),
                };
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(bits))
            }
            AbstractOperation::BooleanConstant { value, .. } => {
                LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(u128::from(*value)))
            }
            AbstractOperation::TrappingInteger { .. } => {
                scalar_instructions::project_trapping_integer(node, optimized)?
            }
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                scalar_type,
                arithmetic,
                left,
                right,
                ..
            } => scalar_instructions::project_integer_arithmetic(
                scalar_instructions::IntegerArithmeticNode {
                    psi_operation: *psi_operation,
                    scalar_type: *scalar_type,
                    arithmetic: *arithmetic,
                    left: *left,
                    right: *right,
                },
                optimized,
                unit,
            )?,
            AbstractOperation::IntegerShift { .. } => {
                scalar_instructions::project_shift(node, optimized, unit)?
            }
            AbstractOperation::BooleanEqual { .. }
            | abstract_operations::integer_equal_less_pair_pattern!(..)
            | AbstractOperation::IntegerLessOrEqual { .. } => {
                scalar_instructions::project_boolean_equal(node, optimized)?
            }
            _ => return Err(Error::custody()),
        }
    };
    Ok(LegalizedScalarInstruction {
        operation,
        result: result.map(|value| LegalizedValueDefinition {
            value,
            scalar_type: node.definitions[0].scalar_type,
            definition_site: node.definitions[0].site,
        }),
        kind,
        fuel: node.fuel.clone(),
        effect: node.effect,
        ownership: node.ownership.clone(),
    })
}

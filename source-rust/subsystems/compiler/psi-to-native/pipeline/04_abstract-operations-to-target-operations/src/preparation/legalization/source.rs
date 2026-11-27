/*
We project lowered target functions and their instruction/terminator graphs
into canonical `LegalizedScalarFunction` rosters (`derive_target_function_rosters`)
for consumption by instruction selection.
*/

use crate::preparation::LegalizationError;
use target_operations::TargetOperationPlan;

pub(crate) mod custody {
    use crate::preparation::{LegalizationError, LegalizationError as Error};
    use abstract_operations::AbstractOperationPlan;
    use optimization_unit::PsiOptimizationUnit;
    use target_operations::TargetOperationPlan;

    pub(crate) fn validate_source_custody(
        target: &TargetOperationPlan,
        abstract_plan: &AbstractOperationPlan,
        unit: &PsiOptimizationUnit,
    ) -> Result<(), LegalizationError> {
        if target.psi != abstract_plan.psi
            || target.psi != unit.psi
            || target.entry != abstract_plan.entry
            || target.entry != unit.entry
            || target.functions.len() != abstract_plan.functions.len()
            || target.functions.len() != unit.functions.len()
        {
            return Err(Error::custody());
        }
        Ok(())
    }
}

pub(crate) mod scalar_graph {
    use crate::preparation::{LegalizationError, LegalizationError as Error};
    use legalized_operations::LegalizedScalarParameter;
    use target_operations::TargetOperationPlan;

    pub(crate) mod instruction {
        use super::Error;
        use crate::preparation::LegalizationError;
        use legalized_operations::{
            LegalizedScalarInstruction, LegalizedScalarInstructionKind, LegalizedValueDefinition,
        };

        pub(crate) mod call_instructions {
            use super::super::{Error, TargetOperationPlan};
            use crate::preparation::LegalizationError;
            use crate::preparation::legalization::scalar_graph_input::normalized_foreign::native_callbacks_at;
            use legalized_operations::{
                LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarInstructionKind,
            };
            use semantic_vocabulary::OperationId;

            pub(crate) fn project_call(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::Call {
                    psi_operation,
                    origin,
                    target,
                    call_plan,
                    result,
                    scalar_arguments,
                    arguments,
                    access_bindings,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                let mut lowered = scalar_arguments
                    .iter()
                    .map(LegalizedScalarArgument::from)
                    .collect::<Vec<_>>();
                for argument in arguments {
                    lowered.push(LegalizedScalarArgument::Structural {
                        indices:
                            crate::preparation::legalization::runtime_indices::retained_operands(
                                facts,
                                machine,
                                *psi_operation,
                                &argument.indices,
                            )?,
                        semantic: terminal_psi::StructuralArgument {
                            place: argument.place,
                            path: argument.path.clone(),
                            access: argument.access,
                        },
                        target: argument.clone(),
                    });
                }
                let structural_result = match result {
                    target_operations::TargetCallResult::Structural { result, .. } => {
                        Some(result.clone())
                    }
                    _ => None,
                };
                let mut descriptor_origins = match result {
                    target_operations::TargetCallResult::Structural {
                        descriptor_origins, ..
                    } => descriptor_origins.clone(),
                    _ => Vec::new(),
                };
                if let target_operations::TargetCallResult::Structural {
                    reference_results, ..
                } = result
                {
                    descriptor_origins.extend(reference_results.iter().flat_map(|reference| {
                        reference.origins.iter().map(|origin| {
                            let Some((root, path, exact, indirect)) = origin.storage_envelope()
                            else {
                                return legalized_operations::LegalizedDescriptorOrigin::Opaque;
                            };
                            legalized_operations::LegalizedDescriptorOrigin::Argument {
                                source: terminal_psi::StructuralArgument {
                                    place: root,
                                    path,
                                    access: terminal_psi::StructuralAccess::SharedBorrow,
                                },
                                exact,
                                indirect,
                            }
                        })
                    }));
                }
                Ok(LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
                    descriptor_origins,
                    access_bindings: access_bindings.clone(),
                    target: *target,
                    arguments: lowered,
                    structural_result,
                    result_placement: call_plan.result.clone(),
                    call_plan: call_plan.clone(),
                    source: origin.clone(),
                    claim_transfers: claim_transfers.clone(),
                    requirement_obligations: requirement_obligations.clone(),
                    crash_continuations: crash_continuations.clone(),
                }))
            }

            pub(crate) fn project_normalized_foreign_call(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                native: &TargetOperationPlan,
                operation: OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::NormalizedForeignCall {
                    psi_operation,
                    boundary,
                    provider_execution,
                    binding,
                    scalar_arguments,
                    structural_arguments,
                    result,
                    descriptor_input,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if *psi_operation != operation {
                    return Err(Error::custody());
                }
                let callbacks =
                    native_callbacks_at(native, machine, operation, &binding.boundary_entry_plan)?
                        .into_iter()
                        .cloned()
                        .collect();
                Ok(LegalizedScalarInstructionKind::NormalizedForeignCall(
                    legalized_operations::LegalizedNormalizedForeignCall {
                        boundary: *boundary,
                        provider_execution: *provider_execution,
                        binding: binding.clone(),
                        scalar_arguments: scalar_arguments.clone(),
                        structural_indices: structural_arguments
                            .iter()
                            .map(|argument| {
                                crate::preparation::legalization::runtime_indices::retained_operands(
                                    facts,
                                    machine,
                                    operation,
                                    &argument.indices,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                        structural_arguments: structural_arguments.clone(),
                        result: result.clone(),
                        callbacks,
                        descriptor_input: *descriptor_input,
                    },
                ))
            }
        }

        pub(crate) mod storage_instructions {
            use super::super::Error;
            use crate::preparation::LegalizationError;
            use legalized_operations::LegalizedScalarInstructionKind;
            use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};

            /// A tag test reads its source, path, tag geometry and selector
            /// operands from the retained target row.
            pub(crate) fn project_structural_case_membership(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralCaseMembership {
                    psi_operation,
                    result,
                    source,
                    path,
                    case,
                    case_tag,
                    tag_byte_offset,
                    indices,
                    pointer_slots,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if result.scalar_type != ScalarType::Boolean
                    || pointer_slots.iter().any(|slot| {
                        slot.scalar_descriptor
                            || slot.index_start > slot.index_end
                            || usize::try_from(slot.index_end)
                                .ok()
                                .is_none_or(|end| end > indices.len())
                    })
                {
                    return Err(Error::custody());
                }
                Ok(LegalizedScalarInstructionKind::StructuralCaseMembership {
                    source: *source,
                    path: path.clone(),
                    case: *case,
                    case_tag: *case_tag,
                    tag_byte_offset: *tag_byte_offset,
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                    pointer_slots: pointer_slots.clone(),
                })
            }

            /// Every leaf copy (a moved field, a whole value, a transformed
            /// value or an empty-run boundary release) keeps its result home,
            /// source path, offset and selector operands on the target row.
            pub(crate) fn project_structural_leaf_copy(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralLeafCopy {
                    psi_operation,
                    result_home,
                    source,
                    path,
                    byte_offset,
                    indices,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                let Some((operation, result)) = result_home.operation_result() else {
                    return Err(Error::custody());
                };
                if operation != *psi_operation {
                    return Err(Error::custody());
                }
                Ok(LegalizedScalarInstructionKind::StructuralLeafCopy {
                    result: result.clone(),
                    source: *source,
                    path: path.clone(),
                    byte_offset: *byte_offset,
                    shape: result_home.layout.shape(),
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            /// A field store keeps its window, offset, value type and selector
            /// operands on the target row; its shape comes from the shared
            /// type catalog, not from the value's earlier definition.
            pub(crate) fn project_structural_field_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                types: &crate::preparation::native_geometry::NativeGeometry<'_>,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StoreStructuralField {
                    psi_operation,
                    destination,
                    path,
                    field,
                    value,
                    value_type,
                    byte_offset,
                    indices,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(LegalizedScalarInstructionKind::StoreStructuralField {
                    destination: destination.clone(),
                    path: path.clone(),
                    field: *field,
                    value: *value,
                    byte_offset: *byte_offset,
                    shape: crate::preparation::structural_inputs::shape(*value_type, types)
                        .ok_or(Error::custody())?,
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            /// Length observations use the source, offset and selectors retained
            /// by target lowering.
            pub(crate) fn project_sequence_length(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                operation: semantic_vocabulary::OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{
                    TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation,
                };

                if let TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation,
                    source,
                    field,
                    indices,
                    ..
                } = targeted
                {
                    if *psi_operation != operation {
                        return Err(Error::custody());
                    }
                    return Ok(
                        LegalizedScalarInstructionKind::StructuralByteSequenceFieldLength {
                            source: source.clone(),
                            field: *field,
                            indices: crate::preparation::legalization::runtime_indices::retained_operands(
                                facts, machine, operation, indices,
                            )?,
                        },
                    );
                }
                let TargetUnitOperation::ScalarDefinition {
                    result_home,
                    expression:
                        TargetScalarExpression::Integer {
                            scalar_type,
                            expression,
                        },
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if result_home.defining_operation != operation
                    || result_home.scalar_type != ScalarType::Integer(*scalar_type)
                    || scalar_type.sign() != IntegerSign::Unsigned
                    || scalar_type.bits() != Some(64)
                {
                    return Err(Error::custody());
                }

                let retained_indices =
                    |indices: &[target_operations::TargetStructuralRuntimeIndex]| {
                        crate::preparation::legalization::runtime_indices::retained_operands(
                            facts, machine, operation, indices,
                        )
                    };
                Ok(match expression {
                    TargetIntegerExpression::Immediate {
                        source_value,
                        value: IntegerValue::Unsigned(value),
                    } if *source_value == result_home.source_value
                        && u64::try_from(*value).is_ok() =>
                    {
                        LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(*value))
                    }
                    TargetIntegerExpression::ByteSequenceLength {
                        psi_operation,
                        source_value,
                        source,
                        indices,
                        length_byte_offset,
                    } if *psi_operation == operation
                        && *source_value == result_home.source_value
                        && matches!(*length_byte_offset, 0 | 8) =>
                    {
                        LegalizedScalarInstructionKind::ByteSequenceLength {
                            source: source.clone(),
                            indices: retained_indices(indices)?,
                            length_byte_offset: *length_byte_offset,
                        }
                    }
                    TargetIntegerExpression::ElementViewLength {
                        psi_operation,
                        source_value,
                        source,
                        indices,
                        length_byte_offset,
                    } if *psi_operation == operation
                        && *source_value == result_home.source_value
                        && matches!(*length_byte_offset, 0 | 8) =>
                    {
                        LegalizedScalarInstructionKind::ElementViewLength {
                            source: source.clone(),
                            indices: retained_indices(indices)?,
                            length_byte_offset: *length_byte_offset,
                        }
                    }
                    _ => return Err(Error::custody()),
                })
            }

            pub(crate) fn project_primitive_scalar_read(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation,
                    source,
                    path,
                    indices,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                // The producer retained the selector's scalar source, stride,
                // extent and proof key.
                Ok(LegalizedScalarInstructionKind::PrimitiveScalarRead {
                    source: *source,
                    path: path.clone(),
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            pub(crate) fn project_write_only_primitive_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::WriteOnlyPrimitiveStore {
                    psi_operation,
                    destination,
                    path,
                    source,
                    byte_offset,
                    byte_size,
                    indices,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                    destination: destination.clone(),
                    path: path.clone(),
                    value: abstract_operations::AbstractResult {
                        value: source.source_value(),
                        scalar_type: source.scalar_type(),
                    },
                    byte_offset: *byte_offset,
                    byte_size: u8::try_from(*byte_size)
                        .map_err(|cause| Error::custody_with_cause(cause))?,
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            pub(crate) fn project_structural_scalar_field_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralScalarFieldStore {
                    psi_operation,
                    destination,
                    path,
                    field,
                    field_byte_offset,
                    field_byte_size,
                    source,
                    indices,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                    destination: destination.clone(),
                    path: path.clone(),
                    field: *field,
                    value: abstract_operations::AbstractResult {
                        value: source.source_value(),
                        scalar_type: source.scalar_type(),
                    },
                    byte_offset: *field_byte_offset,
                    byte_size: *field_byte_size,
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                })
            }

            pub(crate) fn project_subslice(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{TargetByteView, TargetElementView, TargetUnitOperation};
                let (result, psi_operation, place, source, indices, start, end, length, obligation) =
                    match targeted {
                        TargetUnitOperation::ByteSequenceSubslice {
                            result,
                            view:
                                TargetByteView::Subslice {
                                    psi_operation,
                                    place,
                                    source,
                                    indices,
                                    start,
                                    end,
                                    length,
                                    obligation,
                                },
                        }
                        | TargetUnitOperation::ElementViewSubslice {
                            result,
                            view:
                                TargetElementView::Subslice {
                                    psi_operation,
                                    place,
                                    source,
                                    indices,
                                    start,
                                    end,
                                    length,
                                    obligation,
                                },
                        } => (
                            result,
                            *psi_operation,
                            *place,
                            source,
                            indices,
                            start.operand_value().ok_or(Error::custody())?,
                            end.operand_value().ok_or(Error::custody())?,
                            *length,
                            *obligation,
                        ),
                        _ => return Err(Error::custody()),
                    };
                if place != result.place {
                    return Err(Error::custody());
                }
                let indices = crate::preparation::legalization::runtime_indices::retained_operands(
                    facts,
                    machine,
                    psi_operation,
                    indices,
                )?;
                let accepted_fact = crate::preparation::legalization::obligations::retained_fact(
                    facts,
                    machine,
                    psi_operation,
                    obligation,
                )?;
                let kind = if matches!(targeted, TargetUnitOperation::ElementViewSubslice { .. }) {
                    LegalizedScalarInstructionKind::ElementViewSubslice {
                        result: result.clone(),
                        source: source.clone(),
                        indices,
                        start,
                        end,
                        length,
                        obligation,
                        accepted_fact,
                    }
                } else {
                    LegalizedScalarInstructionKind::ByteSequenceSubslice {
                        result: result.clone(),
                        source: source.clone(),
                        indices,
                        start,
                        end,
                        length,
                        obligation,
                        accepted_fact,
                    }
                };
                Ok(kind)
            }

            pub(crate) fn project_established_view(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{
                    TargetElementView, TargetElementViewRegion, TargetUnitOperation,
                };
                let TargetUnitOperation::EstablishElementView {
                    result,
                    view:
                        TargetElementView::Established {
                            psi_operation,
                            place,
                            structural_type,
                            source_argument,
                            element,
                            indices,
                            region,
                            ..
                        },
                } = targeted
                else {
                    return Err(Error::custody());
                };
                if *place != result.place || *structural_type != result.structural_type {
                    return Err(Error::custody());
                }
                // The source argument and endpoint identities belong to this
                // view, alongside its physical backing recipe. Never recover
                // them from the body that happened to produce the view.
                let region = match region {
                    TargetElementViewRegion::WholeCollection => {
                        terminal_psi::ElementViewRegion::WholeCollection
                    }
                    TargetElementViewRegion::WholeRepresentation {
                        source_length,
                        obligation,
                    } => terminal_psi::ElementViewRegion::WholeRepresentation {
                        source_length: *source_length,
                        obligation: *obligation,
                    },
                    TargetElementViewRegion::RepresentationBytes {
                        start,
                        end,
                        obligation,
                    } => terminal_psi::ElementViewRegion::RepresentationBytes {
                        start: start.operand_value().ok_or(Error::custody())?,
                        end: end.operand_value().ok_or(Error::custody())?,
                        obligation: *obligation,
                    },
                };
                let accepted_fact = match region {
                    terminal_psi::ElementViewRegion::WholeCollection => None,
                    terminal_psi::ElementViewRegion::WholeRepresentation { obligation, .. }
                    | terminal_psi::ElementViewRegion::RepresentationBytes { obligation, .. } => {
                        Some(
                            crate::preparation::legalization::obligations::retained_fact(
                                facts,
                                machine,
                                *psi_operation,
                                obligation,
                            )?,
                        )
                    }
                };
                Ok(LegalizedScalarInstructionKind::EstablishElementView {
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts,
                        machine,
                        *psi_operation,
                        indices,
                    )?,
                    result: result.clone(),
                    destination: *place,
                    source: source_argument.clone(),
                    element: *element,
                    region,
                    accepted_fact,
                })
            }

            /// The target row owns the replacement and each selector used to
            /// reach its destination.
            pub(crate) fn project_byte_view_store(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralByteViewFieldStore {
                    psi_operation,
                    destination,
                    field,
                    source,
                    indices,
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(
                    LegalizedScalarInstructionKind::StructuralByteViewFieldStore {
                        destination: destination.clone(),
                        field: *field,
                        source: *source,
                        indices:
                            crate::preparation::legalization::runtime_indices::retained_operands(
                                facts,
                                machine,
                                *psi_operation,
                                indices,
                            )?,
                    },
                )
            }

            pub(crate) fn project_structural_byte_sequence_field_read(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                let target_operations::TargetUnitOperation::StructuralByteSequenceFieldRead {
                    psi_operation,
                    source,
                    field,
                    indices,
                    index,
                    length,
                    obligation,
                    ..
                } = targeted
                else {
                    return Err(Error::custody());
                };
                Ok(
                    LegalizedScalarInstructionKind::StructuralByteSequenceFieldRead {
                        source: source.clone(),
                        field: *field,
                        indices:
                            crate::preparation::legalization::runtime_indices::retained_operands(
                                facts,
                                machine,
                                *psi_operation,
                                indices,
                            )?,
                        index: index.source_value(),
                        length: *length,
                        obligation: *obligation,
                        accepted_fact:
                            crate::preparation::legalization::obligations::retained_fact(
                                facts,
                                machine,
                                *psi_operation,
                                *obligation,
                            )?,
                    },
                )
            }

            /// Checked byte reads and writes carry their scalar operands and
            /// bounds keys in target lowering.
            pub(crate) fn project_byte_access(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                operation: semantic_vocabulary::OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
                use target_operations::{
                    TargetIntegerExpression, TargetScalarExpression, TargetUnitOperation,
                };

                let accepted = |obligation| {
                    crate::preparation::legalization::obligations::retained_fact(
                        facts, machine, operation, obligation,
                    )
                };
                match targeted {
                    TargetUnitOperation::ScalarDefinition {
                        result_home,
                        expression:
                            TargetScalarExpression::Integer {
                                scalar_type,
                                expression:
                                    TargetIntegerExpression::ByteSequenceRead {
                                        psi_operation,
                                        source_value,
                                        source,
                                        index,
                                        length,
                                        obligation,
                                        ..
                                    },
                            },
                    } if *psi_operation == operation
                        && result_home.defining_operation == operation
                        && result_home.source_value == *source_value
                        && result_home.scalar_type == ScalarType::Integer(*scalar_type)
                        && scalar_type.sign() == IntegerSign::Unsigned
                        && scalar_type.bits() == Some(8) =>
                    {
                        Ok(LegalizedScalarInstructionKind::ByteSequenceRead {
                            source: *source,
                            index: index.operand_value().ok_or(Error::custody())?,
                            length: *length,
                            obligation: *obligation,
                            accepted_fact: accepted(*obligation)?,
                        })
                    }
                    TargetUnitOperation::ByteSequenceWrite {
                        psi_operation,
                        destination,
                        index,
                        value,
                        length,
                        obligation,
                        ..
                    } if *psi_operation == operation => {
                        Ok(LegalizedScalarInstructionKind::ByteSequenceWrite {
                            destination: *destination,
                            index: index.source_value(),
                            value: value.source_value(),
                            length: *length,
                            obligation: *obligation,
                            accepted_fact: accepted(*obligation)?,
                        })
                    }
                    _ => Err(Error::custody()),
                }
            }

            pub(crate) fn project_element_view_read(
                machine: semantic_vocabulary::MachineId,
                facts: &[optimization_unit::AcceptedObligationFact],
                operation: semantic_vocabulary::OperationId,
                targeted: &target_operations::TargetUnitOperation,
            ) -> Result<Option<LegalizedScalarInstructionKind>, LegalizationError> {
                use target_operations::{
                    TargetBooleanExpression, TargetIntegerExpression, TargetScalarExpression,
                    TargetUnitOperation,
                };
                use terminal_psi::StructuralPathSegment;

                let TargetUnitOperation::ScalarDefinition {
                    result_home,
                    expression,
                } = targeted
                else {
                    return Ok(None);
                };
                let (owner, result, source, indices, index, length, obligation, path) =
                    match expression {
                        TargetScalarExpression::Integer {
                            expression:
                                TargetIntegerExpression::ElementViewRead {
                                    psi_operation,
                                    source_value,
                                    source,
                                    indices,
                                    index,
                                    length,
                                    obligation,
                                    path,
                                    ..
                                },
                            ..
                        }
                        | TargetScalarExpression::Boolean(
                            TargetBooleanExpression::ElementViewRead {
                                psi_operation,
                                source_value,
                                source,
                                indices,
                                index,
                                length,
                                obligation,
                                path,
                                ..
                            },
                        )
                        | TargetScalarExpression::IeeeFloatElementViewRead {
                            psi_operation,
                            source_value,
                            source,
                            indices,
                            index,
                            length,
                            obligation,
                            path,
                            ..
                        } => (
                            psi_operation,
                            source_value,
                            source,
                            indices,
                            index,
                            length,
                            obligation,
                            path,
                        ),
                        _ => return Ok(None),
                    };
                // A checked by-value read may choose a runtime element of the
                // slice, but the scalar leaf inside that element is static.
                if *owner != operation
                    || result_home.defining_operation != operation
                    || result_home.source_value != *result
                    || result_home.scalar_type != expression.scalar_type()
                    || !path.iter().all(|segment| {
                        matches!(
                            segment,
                            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                        )
                    })
                {
                    return Err(Error::custody());
                }
                Ok(Some(LegalizedScalarInstructionKind::ElementViewRead {
                    source: source.clone(),
                    index: index.operand_value().ok_or(Error::custody())?,
                    length: *length,
                    obligation: *obligation,
                    accepted_fact: crate::preparation::legalization::obligations::retained_fact(
                        facts,
                        machine,
                        operation,
                        *obligation,
                    )?,
                    path: path.clone(),
                    indices: crate::preparation::legalization::runtime_indices::retained_operands(
                        facts, machine, operation, indices,
                    )?,
                }))
            }

            #[cfg(test)]
            mod retained_tests {
                use super::{project_primitive_scalar_read, project_structural_scalar_field_store};
                use semantic_vocabulary::{
                    MachineId, ObligationId, OperationId, PlaceId, StructuralFieldId,
                    StructuralTypeId, ValueId,
                };
                use target_operations::{
                    TargetStructuralRuntimeIndex, TargetUnitOperation,
                    TargetUnitScalarArgumentSource,
                };

                #[test]
                fn scalar_storage_operands_need_only_target_rows_and_admitted_facts() {
                    let machine = MachineId::new(1).unwrap();
                    let operation = OperationId::new(2).unwrap();
                    let obligation = ObligationId::new(3).unwrap();
                    let index_value = ValueId::new(4).unwrap();
                    let scalar_type = target_operations::selector_carrier_type();
                    let index = TargetStructuralRuntimeIndex {
                        operand: TargetUnitScalarArgumentSource::Parameter {
                            parameter_index: 0,
                            source_value: index_value,
                            scalar_type,
                        },
                        stride: 24,
                        extent: Some(7),
                        obligation,
                    };
                    let fact = optimization_unit::AcceptedObligationFact::new(
                        terminal_psi::TerminalPsiIdentity {
                            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes(
                                [5; 32],
                            ),
                        },
                        [6; 32],
                        machine,
                        optimization_unit::PsiProvenance::Operation(operation),
                        obligation,
                        vec![7],
                    );
                    let place = PlaceId::new(1).unwrap();
                    let path = vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: index_value,
                        obligation,
                    }];
                    let read = TargetUnitOperation::PrimitiveScalarRead {
                        psi_operation: operation,
                        result: abstract_operations::AbstractResult {
                            value: ValueId::new(5).unwrap(),
                            scalar_type,
                        },
                        source: place,
                        path: path.clone(),
                        indices: vec![index.clone()],
                    };
                    let parameter = terminal_psi::StructuralParameterDeclaration {
                        place,
                        position: 0,
                        structural_type: StructuralTypeId::new(1).unwrap(),
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    };
                    let value = TargetUnitScalarArgumentSource::Parameter {
                        parameter_index: 1,
                        source_value: ValueId::new(6).unwrap(),
                        scalar_type,
                    };
                    let store = TargetUnitOperation::StructuralScalarFieldStore {
                        psi_operation: operation,
                        destination:
                            abstract_operations::StructuralScalarStoreDestination::Parameter(
                                parameter,
                            ),
                        path: path.clone(),
                        field: StructuralFieldId::new(1).unwrap(),
                        destination_storage:
                            target_operations::TargetStructuralArgumentSource::BlockParameter {
                                block: semantic_vocabulary::BlockId::new(1).unwrap(),
                                place,
                            },
                        field_byte_offset: 16,
                        field_byte_size: 8,
                        source: value,
                        indices: vec![index],
                    };
                    let lowered_read =
                        project_primitive_scalar_read(machine, std::slice::from_ref(&fact), &read)
                            .unwrap();
                    let legalized_operations::LegalizedScalarInstructionKind::PrimitiveScalarRead {
                        source,
                        path: actual_path,
                        indices,
                    } = lowered_read
                    else {
                        panic!("read");
                    };
                    assert_eq!((source, actual_path), (place, path.clone()));
                    assert_eq!(
                        (
                            indices[0].operand.value,
                            indices[0].stride,
                            indices[0].extent,
                            indices[0].accepted_fact
                        ),
                        (index_value, 24, Some(7), fact.identity)
                    );
                    let lowered_store = project_structural_scalar_field_store(
                        machine,
                        std::slice::from_ref(&fact),
                        &store,
                    )
                    .unwrap();
                    let legalized_operations::LegalizedScalarInstructionKind::StructuralScalarFieldStore { value: actual_value, byte_offset, byte_size, indices, .. } = lowered_store else { panic!("store"); };
                    assert_eq!(
                        (
                            actual_value.value,
                            actual_value.scalar_type,
                            byte_offset,
                            byte_size
                        ),
                        (value.source_value(), scalar_type, 16, 8)
                    );
                    assert_eq!(indices[0].accepted_fact, fact.identity);
                    assert!(project_primitive_scalar_read(machine, &[], &read).is_err());
                    assert!(project_structural_scalar_field_store(machine, &[], &store).is_err());
                    assert!(
                        project_primitive_scalar_read(
                            MachineId::new(9).unwrap(),
                            std::slice::from_ref(&fact),
                            &read
                        )
                        .is_err()
                    );
                    assert!(
                        project_structural_scalar_field_store(
                            MachineId::new(9).unwrap(),
                            &[fact],
                            &store
                        )
                        .is_err()
                    );
                }
            }
        }

        pub(crate) fn project_reference(
            machine: semantic_vocabulary::MachineId,
            facts: &[optimization_unit::AcceptedObligationFact],
            targeted: &target_operations::TargetUnitOperation,
        ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
            use target_operations::TargetUnitOperation as Target;
            let (operation, home, indices) = match targeted {
                Target::EstablishReference {
                    psi_operation,
                    result_home,
                    indices,
                    ..
                }
                | Target::EstablishAccessReference {
                    psi_operation,
                    result_home,
                    indices,
                    ..
                } => (*psi_operation, result_home, indices),
                _ => return Err(Error::custody()),
            };
            let (_, result) = home
                .operation_result()
                .filter(|(defining, _)| *defining == operation)
                .ok_or(Error::custody())?;
            let indices = crate::preparation::legalization::runtime_indices::retained_operands(
                facts, machine, operation, indices,
            )?;
            let shape = home.layout.shape();
            // Formation already owns its result contract and selector geometry.
            Ok(match targeted {
                Target::EstablishReference {
                    source, storage, ..
                } => LegalizedScalarInstructionKind::EstablishReference {
                    result: result.clone(),
                    source: source.clone(),
                    storage: *storage,
                    indices,
                    shape,
                },
                Target::EstablishAccessReference {
                    source,
                    correspondence,
                    ..
                } => LegalizedScalarInstructionKind::EstablishAccessReference {
                    result: result.clone(),
                    source: source.clone(),
                    correspondence: correspondence.clone(),
                    indices,
                    shape,
                },
                _ => return Err(Error::custody()),
            })
        }

        /// A fixed-array reference into a view keeps its result home, the
        /// measured length and the proof that the length equals the extent.
        pub(crate) fn project_fixed_array(
            machine: semantic_vocabulary::MachineId,
            facts: &[optimization_unit::AcceptedObligationFact],
            targeted: &target_operations::TargetUnitOperation,
        ) -> Result<LegalizedScalarInstructionKind, LegalizationError> {
            let target_operations::TargetUnitOperation::ElementViewFixedArray {
                psi_operation,
                result_home,
                source,
                indices,
                length,
                extent,
                obligation,
            } = targeted
            else {
                return Err(Error::custody());
            };
            let (_, result) = result_home
                .operation_result()
                .filter(|(defining, _)| *defining == *psi_operation)
                .ok_or(Error::custody())?;
            let indices = crate::preparation::legalization::runtime_indices::retained_operands(
                facts,
                machine,
                *psi_operation,
                indices,
            )?;
            let accepted_fact = crate::preparation::legalization::obligations::retained_fact(
                facts,
                machine,
                *psi_operation,
                *obligation,
            )?;
            Ok(LegalizedScalarInstructionKind::ElementViewFixedArray {
                result: result.clone(),
                source: source.clone(),
                indices,
                length: *length,
                extent: *extent,
                obligation: *obligation,
                accepted_fact,
                shape: result_home.layout.shape(),
            })
        }

        pub(crate) use crate::preparation::legalization_instruction::project;

        /// The instruction owns its already-retained execution and custody annotations.
        pub(crate) fn with_metadata(
            source: &target_operations::TargetInstructionSource,
            kind: LegalizedScalarInstructionKind,
            types: &crate::preparation::native_geometry::ModuleTypes,
        ) -> Result<LegalizedScalarInstruction, Error> {
            Ok(LegalizedScalarInstruction {
                operation: source.operation,
                result: source
                    .result
                    .map(|result| {
                        Ok::<_, Error>(LegalizedValueDefinition {
                            value: result.value,
                            scalar_type: types
                                .scalar(result.scalar_type)
                                .ok_or_else(Error::custody)?,
                            definition_site: result.site,
                        })
                    })
                    .transpose()?,
                kind,
                work: source.metadata.work.clone(),
                effect: source.metadata.effect,
                ownership: source.metadata.ownership.clone(),
            })
        }
    }

    /// Parameter sites are ordinal in the retained function ABI, not body-derived.
    pub(crate) fn parameters(
        target: &target_operations::TargetFunction,
        types: &crate::preparation::native_geometry::ModuleTypes,
    ) -> Result<Vec<LegalizedScalarParameter>, LegalizationError> {
        target
            .graph
            .scalar_parameters
            .iter()
            .enumerate()
            .map(|(position, parameter)| {
                Ok(LegalizedScalarParameter {
                    value: parameter.value,
                    scalar_type: types
                        .scalar(parameter.scalar_type)
                        .ok_or_else(Error::custody)?,
                    definition_site: optimization_unit::ValueDefinitionSite::FunctionParameter(
                        u32::try_from(position)
                            .map_err(|cause| Error::custody_with_cause(cause))?,
                    ),
                    placement: parameter.placement.clone(),
                })
            })
            .collect()
    }
}

pub(crate) struct SourceFunctionRosters {
    pub scalar_functions: Vec<legalized_operations::LegalizedScalarFunction>,
}

pub(crate) use custody::validate_source_custody;

/// Ordinary legalization consumes only target execution and immutable judgments.
pub(crate) fn derive_target_function_rosters(
    target: &TargetOperationPlan,
    evidence: crate::preparation::PreparationEvidence<'_>,
    types: &crate::preparation::native_geometry::ModuleTypes,
) -> Result<SourceFunctionRosters, LegalizationError> {
    let geometries = crate::preparation::native_geometry::NativeGeometries::new(
        target.functions.iter().map(|function| {
            (
                function.graph.structural_types.as_ref(),
                function.graph.structural_types.owned_sequence_bounds(),
            )
        }),
        target.target,
    )
    .map_err(|cause| LegalizationError::custody_with_cause(cause))?;
    let mut scalar_functions = Vec::with_capacity(target.functions.len());
    for function in &target.functions {
        let geometry = geometries
            .get(
                &function.graph.structural_types,
                function.graph.structural_types.owned_sequence_bounds(),
            )
            .ok_or_else(LegalizationError::custody)?;
        let mut blocks = Vec::with_capacity(function.graph.blocks.len());
        for block in &function.graph.blocks {
            let mut instructions = Vec::with_capacity(block.operations.len());
            for row in &block.operations {
                if matches!(
                    row.kind,
                    target_operations::TargetUnitOperation::CleanupCall { .. }
                ) {
                    continue;
                }
                instructions.push(
                    scalar_graph::instruction::project(
                        function.machine,
                        evidence.accepted_obligation_facts,
                        types,
                        target,
                        geometry,
                        row,
                    )
                    .map_err(|error| {
                        error.attributed(
                            function.machine,
                            row.source.as_ref().map(|source| source.operation),
                        )
                    })?,
                );
            }
            let parameters = block
                .parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| {
                    Ok(legalized_operations::LegalizedValueDefinition {
                        value: parameter.value,
                        scalar_type: types
                            .scalar(parameter.scalar_type)
                            .ok_or_else(LegalizationError::custody)?,
                        definition_site: optimization_unit::ValueDefinitionSite::BlockParameter {
                            block: block.block,
                            position: u32::try_from(position)
                                .map_err(|cause| LegalizationError::custody_with_cause(cause))?,
                        },
                    })
                })
                .collect::<Result<_, LegalizationError>>()?;
            blocks.push(legalized_operations::LegalizedScalarBlock {
                id: block.block,
                parameters,
                structural_parameters: block.structural_parameters.clone().into(),
                instructions,
                terminator: crate::preparation::legalization_control::project(
                    function.machine,
                    block,
                    function
                        .structural_contract
                        .as_ref()
                        .map(|contract| contract.parameters.as_slice())
                        .unwrap_or(&[]),
                    evidence.accepted_obligation_facts,
                    types,
                )?,
            });
        }
        scalar_functions.push(legalized_operations::LegalizedScalarFunction {
            machine: function.machine,
            attachment: function.attachment,
            provenance: function.provenance.clone(),
            call_plan: function.graph.call_plan.clone(),
            parameters: scalar_graph::parameters(function, types)?,
            structural: crate::preparation::legalization::scalar_graph_input::structural_contract(
                function,
            )?,
            entry_block: function.graph.entry,
            blocks,
        });
    }
    Ok(SourceFunctionRosters { scalar_functions })
}

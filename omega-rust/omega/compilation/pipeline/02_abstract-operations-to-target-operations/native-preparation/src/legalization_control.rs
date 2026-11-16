//! Select control exits from their retained target rows, not earlier bodies.
//!
//! Source correspondence belongs to the independent audit. This projection only
//! needs the function's storage declarations and admitted immutable proof facts.

use crate::LegalizationError;
use legalized_operations::{
    LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarReturn,
    LegalizedScalarReturnValue, LegalizedScalarSuccessor, LegalizedScalarTerminator,
    LegalizedStructuralCaseSource,
};
use optimization_unit::{AcceptedObligationFact, PsiProvenance};
use semantic_vocabulary::{EdgeId, MachineId};
use target_operations::{
    TargetControlBlock, TargetControlTerminator, TargetStructuralHomeOrigin,
    TargetStructuralReturnSource, TargetUnitOperation,
};

fn home_source(origin: &TargetStructuralHomeOrigin) -> LegalizedStructuralCaseSource {
    match origin {
        TargetStructuralHomeOrigin::OperationResult { operation, result } => {
            LegalizedStructuralCaseSource::OperationResult {
                operation: *operation,
                result: result.clone(),
            }
        }
        TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
            LegalizedStructuralCaseSource::BlockParameter {
                block: *block,
                declaration: declaration.clone(),
            }
        }
    }
}

fn bindings(
    machine: MachineId,
    edge: EdgeId,
    semantic: &[abstract_operations::AbstractStructuralBinding],
    indices: &[Vec<target_operations::TargetStructuralRuntimeIndex>],
    facts: &[AcceptedObligationFact],
) -> Result<Vec<legalized_operations::LegalizedStructuralBinding>, LegalizationError> {
    if semantic.len() != indices.len() {
        return Err(LegalizationError::custody());
    }
    semantic
        .iter()
        .zip(indices)
        .map(|(semantic, indices)| {
            Ok(legalized_operations::LegalizedStructuralBinding {
                semantic: semantic.clone(),
                indices: crate::legalization::runtime_indices::retained_operands(
                    facts,
                    machine,
                    PsiProvenance::Edge(edge),
                    indices,
                )?,
            })
        })
        .collect()
}

fn successor(
    machine: MachineId,
    edge: &target_operations::TargetControlSuccessor,
    facts: &[AcceptedObligationFact],
) -> Result<LegalizedScalarSuccessor, LegalizationError> {
    Ok(LegalizedScalarSuccessor {
        edge: edge.psi_edge,
        target: edge.target,
        bindings: edge.bindings.clone(),
        structural_bindings: bindings(
            machine,
            edge.psi_edge,
            &edge.structural_bindings,
            &edge.structural_indices,
            facts,
        )?,
        work: edge.work.clone(),
        cleanup_calls: Vec::new(),
    })
}

fn structural_arguments(
    machine: MachineId,
    edge: EdgeId,
    arguments: &[target_operations::TargetStructuralArgument],
    facts: &[AcceptedObligationFact],
) -> Result<Vec<LegalizedScalarArgument>, LegalizationError> {
    arguments
        .iter()
        .map(|target| {
            Ok(LegalizedScalarArgument::Structural {
                semantic: terminal_psi::StructuralArgument {
                    place: target.place,
                    path: target.path.clone(),
                    access: target.access,
                },
                target: target.clone(),
                indices: crate::legalization::runtime_indices::retained_operands(
                    facts,
                    machine,
                    PsiProvenance::Edge(edge),
                    &target.indices,
                )?,
            })
        })
        .collect()
}

pub(crate) fn project(
    machine: MachineId,
    block: &TargetControlBlock,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    facts: &[AcceptedObligationFact],
    types: &legalized_operations::RetainedIntegerTypes,
) -> Result<LegalizedScalarTerminator, LegalizationError> {
    let metadata = &block.terminator_metadata;
    let returned = |edge, value| {
        LegalizedScalarTerminator::Return(LegalizedScalarReturn {
            edge,
            value,
            work: metadata.work.clone(),
            effect: metadata.effect,
            ownership: metadata.ownership.clone(),
        })
    };
    let mut result = match &block.terminator {
        TargetControlTerminator::Return { psi_edge, .. } => {
            returned(*psi_edge, LegalizedScalarReturnValue::Unit)
        }
        TargetControlTerminator::ReturnScalar {
            psi_edge,
            source_value,
            expression,
            ..
        } => returned(
            *psi_edge,
            LegalizedScalarReturnValue::Value {
                value: *source_value,
                scalar_type: types
                    .scalar(expression.scalar_type())
                    .ok_or_else(LegalizationError::custody)?,
            },
        ),
        TargetControlTerminator::ReturnStructural {
            psi_edge, source, ..
        } => {
            let value = match source {
                TargetStructuralReturnSource::SharedDescriptor { place } => {
                    LegalizedScalarReturnValue::SharedDescriptor { place: *place }
                }
                TargetStructuralReturnSource::Parameter(parameter) => {
                    LegalizedScalarReturnValue::StructuralParameter {
                        place: parameter.place,
                    }
                }
                TargetStructuralReturnSource::Home(home) => {
                    LegalizedScalarReturnValue::Structural {
                        source: home_source(&home.origin),
                    }
                }
            };
            returned(*psi_edge, value)
        }
        TargetControlTerminator::Jump { successor: edge } => LegalizedScalarTerminator::Jump {
            successor: successor(machine, edge, facts)?,
            effect: metadata.effect,
            ownership: metadata.ownership.clone(),
        },
        TargetControlTerminator::Conditional {
            condition_source,
            when_true,
            when_false,
            ..
        } => LegalizedScalarTerminator::Conditional {
            condition: *condition_source,
            when_true: successor(machine, when_true, facts)?,
            when_false: successor(machine, when_false, facts)?,
            effect: metadata.effect,
            ownership: metadata.ownership.clone(),
        },
        TargetControlTerminator::EliminateEmpty {
            psi_edge,
            source,
            path,
        } => LegalizedScalarTerminator::EliminateEmpty {
            psi_edge: *psi_edge,
            source: *source,
            path: path.clone(),
            work: metadata.work.clone(),
            effect: metadata.effect,
            ownership: metadata.ownership.clone(),
        },
        TargetControlTerminator::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => LegalizedScalarTerminator::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
            work: metadata.work.clone(),
            effect: metadata.effect,
            ownership: metadata.ownership.clone(),
        },
        TargetControlTerminator::ExternalCompletion {
            psi_edge,
            boundary,
            kind,
            binding,
            arguments,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
        } => LegalizedScalarTerminator::ExternalCompletion {
            psi_edge: *psi_edge,
            boundary: *boundary,
            kind: *kind,
            arguments: arguments.clone(),
            binding: binding.clone(),
            structural_arguments: structural_arguments.clone(),
            requirement_obligations: requirement_obligations.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
            work: metadata.work.clone(),
            effect: metadata.effect,
            ownership: metadata.ownership.clone(),
        },
        TargetControlTerminator::InvokeNoNormal {
            psi_edge,
            callee,
            call_plan,
            scalar_arguments,
            arguments,
            access_bindings,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            declared_result,
        } => {
            let mut actuals = scalar_arguments
                .iter()
                .map(|argument| LegalizedScalarArgument::Scalar {
                    source: argument.source.source_value(),
                    placement: argument.placement.clone(),
                })
                .collect::<Vec<_>>();
            actuals.extend(structural_arguments(machine, *psi_edge, arguments, facts)?);
            let call = LegalizedScalarCall {
                descriptor_origins: Vec::new(),
                access_bindings: access_bindings.clone(),
                source: target_operations::NativeCallOrigin::Authored,
                target: target_operations::NativeCallTarget::Machine(*callee),
                call_plan: call_plan.clone(),
                arguments: actuals,
                result_placement: call_plan.result.clone(),
                structural_result: None,
                claim_transfers: claim_transfers.clone(),
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: crash_continuations.clone(),
            };
            call.validate_no_normal_shape()
                .map_err(|_| LegalizationError::custody())?;
            call.validate_source(&metadata.ownership)
                .map_err(|_| LegalizationError::custody())?;
            LegalizedScalarTerminator::InvokeNoNormal {
                psi_edge: *psi_edge,
                call,
                declared_result: declared_result.clone(),
                work: metadata.work.clone(),
                effect: metadata.effect,
                ownership: metadata.ownership.clone(),
            }
        }
        TargetControlTerminator::StructuralCase {
            psi_block,
            source,
            path,
            indices,
            byte_offset,
            layout,
            pointer_slots,
            region_byte_size,
            cases,
        } => {
            // Scalar-descriptor chases require a different lowering.
            if pointer_slots.iter().any(|slot| slot.scalar_descriptor)
                || byte_offset
                    .checked_add(layout.shape.byte_size)
                    .is_none_or(|end| end > *region_byte_size)
                || cases.len() != layout.cases.len()
            {
                return Err(LegalizationError::custody());
            }
            // The checked block owns the discriminator's read before any case edge.
            let indices = crate::legalization::runtime_indices::retained_operands(
                facts,
                machine,
                terminal_psi::ExecutionSite::Block(*psi_block),
                indices,
            )?;
            let selected_source = match source {
                target_operations::TargetStructuralCaseSource::Home(home) => {
                    home_source(&home.origin)
                }
                target_operations::TargetStructuralCaseSource::Parameter { parameter, .. }
                | target_operations::TargetStructuralCaseSource::BorrowedParameter {
                    parameter,
                    ..
                } => {
                    let declaration = parameters
                        .iter()
                        .find(|declaration| declaration.place == parameter.place)
                        .ok_or(LegalizationError::custody())?
                        .clone();
                    if matches!(
                        source,
                        target_operations::TargetStructuralCaseSource::Parameter { .. }
                    ) {
                        LegalizedStructuralCaseSource::Parameter { declaration }
                    } else {
                        LegalizedStructuralCaseSource::BorrowedParameter { declaration }
                    }
                }
                target_operations::TargetStructuralCaseSource::BorrowedBlockParameter {
                    parameter,
                    ..
                } => LegalizedStructuralCaseSource::BorrowedParameter {
                    declaration: parameter.clone(),
                },
            };
            let cases = cases
                .iter()
                .map(|case| {
                    Ok(legalized_operations::LegalizedStructuralCaseSuccessor {
                        edge: case.psi_edge,
                        target: case.target,
                        case: case.case,
                        case_tag: case.case_tag,
                        payloads: case
                            .payloads
                            .iter()
                            .enumerate()
                            .map(|(position, payload)| {
                                Ok(legalized_operations::LegalizedStructuralCasePayload {
                                    field: payload.field,
                                    field_byte_offset: payload.field_byte_offset,
                                    parameter: legalized_operations::LegalizedValueDefinition {
                                        value: payload.parameter.value,
                                        scalar_type: types
                                            .scalar(payload.parameter.scalar_type)
                                            .ok_or_else(LegalizationError::custody)?,
                                        definition_site:
                                            optimization_unit::ValueDefinitionSite::BlockParameter {
                                                block: case.target,
                                                position: u32::try_from(position)
                                                    .map_err(|_| LegalizationError::custody())?,
                                            },
                                    },
                                })
                            })
                            .collect::<Result<Vec<_>, LegalizationError>>()?,
                        structural_bindings: bindings(
                            machine,
                            case.psi_edge,
                            &case.structural_bindings,
                            &case.structural_indices,
                            facts,
                        )?,
                        cleanup_actions: case.cleanup_actions.clone(),
                        cleanup_calls: Vec::new(),
                        work: case.work.clone(),
                    })
                })
                .collect::<Result<Vec<_>, LegalizationError>>()?;
            LegalizedScalarTerminator::StructuralCase {
                psi_block: *psi_block,
                source: selected_source,
                path: path.clone(),
                indices,
                root_shape: source.layout().shape(),
                byte_offset: *byte_offset,
                layout: layout.clone(),
                pointer_slots: pointer_slots.clone(),
                region_byte_size: *region_byte_size,
                cases,
                effect: metadata.effect,
                ownership: metadata.ownership.clone(),
            }
        }
    };
    let first_cleanup = block
        .operations
        .iter()
        .position(|operation| matches!(&operation.kind, TargetUnitOperation::CleanupCall { .. }))
        .unwrap_or(block.operations.len());
    let mut calls = Vec::with_capacity(block.operations.len() - first_cleanup);
    for operation in &block.operations[first_cleanup..] {
        if operation.source.is_some() {
            return Err(LegalizationError::instruction_metadata(
                "compiler cleanup call carries authored instruction metadata",
            ));
        }
        let TargetUnitOperation::CleanupCall {
            owner,
            callee,
            call_plan,
            arguments,
            requirement_obligations,
        } = &operation.kind
        else {
            return Err(LegalizationError::custody());
        };
        let edge = owner.edge().ok_or(LegalizationError::custody())?;
        calls.push((
            *owner,
            LegalizedScalarCall {
                descriptor_origins: Vec::new(),
                access_bindings: Vec::new(),
                source: target_operations::NativeCallOrigin::Authored,
                target: target_operations::NativeCallTarget::Machine(*callee),
                call_plan: call_plan.clone(),
                arguments: structural_arguments(machine, edge, arguments, facts)?,
                result_placement: None,
                structural_result: None,
                claim_transfers: Vec::new(),
                requirement_obligations: requirement_obligations.clone(),
                crash_continuations: Vec::new(),
            },
        ));
    }
    if calls.is_empty() {
        return Ok(result);
    }
    if let LegalizedScalarTerminator::Return(returned) = result {
        return Ok(LegalizedScalarTerminator::ReturnCleanup { returned, calls });
    }
    for (owner, call) in calls {
        let destination = match &mut result {
            LegalizedScalarTerminator::Jump { successor, .. }
                if owner.edge() == Some(successor.edge) =>
            {
                &mut successor.cleanup_calls
            }
            LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if owner.edge() == Some(when_true.edge) {
                    &mut when_true.cleanup_calls
                } else if owner.edge() == Some(when_false.edge) {
                    &mut when_false.cleanup_calls
                } else {
                    return Err(LegalizationError::custody());
                }
            }
            LegalizedScalarTerminator::StructuralCase { cases, .. } => {
                &mut cases
                    .iter_mut()
                    .find(|case| owner.edge() == Some(case.edge))
                    .ok_or(LegalizationError::custody())?
                    .cleanup_calls
            }
            _ => return Err(LegalizationError::custody()),
        };
        destination.push((owner, call));
    }
    Ok(result)
}

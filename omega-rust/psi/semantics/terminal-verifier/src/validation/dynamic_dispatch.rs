//! Dynamic dispatch: descriptor parameters and arguments, conformance
//! selections, and every direct, indirect, stored and parameter dispatch.
//!
//! `validate_dynamic_dispatches` validates the descriptor parameters, then
//! the conformance `selections`, the `direct_dispatches` (which consume
//! selections), the `rebound_descriptors`, the `indirect_dispatches`
//! (which consume descriptors), the stored and parameter dispatches and
//! descriptor arguments, and finally requires every site consumed
//! (`consumption`).

mod consumption;
mod direct_dispatches;
mod indirect_dispatches;
mod rebound_descriptors;
mod selections;

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::{
    ClosedConformanceCallableResult, OperationKind, OperationResult, StructuralAccess,
    TerminalDynamicDescriptorParameter, TerminalDynamicDescriptorSource, TerminalMachine,
    TerminalMachineResult, TerminalModule,
};

use super::ModuleError;
use super::foundation::resolve_structural_path;
use super::structural_operations::validate_structural_arguments;

pub(super) fn validate_dynamic_dispatches(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    validate_dynamic_descriptor_parameters(module, machines)?;
    selections::validate_conformance_selections(module, machines)?;
    let mut consumed_selections = direct_dispatches::validate_direct_dispatches(module, machines)?;
    rebound_descriptors::validate_rebound_descriptors(module, machines, &mut consumed_selections)?;
    let (indirect_coordinates, mut consumed_descriptors) =
        indirect_dispatches::validate_indirect_dispatches(module, machines)?;
    let selections = &module.dynamic_dispatch.selections;
    let descriptors = &module.dynamic_dispatch.rebound_descriptors;
    let (stored_dispatch_coordinates, consumed_stored_descriptors) =
        validate_stored_dynamic_dispatches(module, machines, selections, &mut consumed_selections)?;
    consumed_descriptors.extend(validate_dynamic_descriptor_arguments(
        module,
        machines,
        descriptors,
        selections,
    )?);
    consumed_selections.extend(
        module
            .dynamic_dispatch
            .arguments
            .iter()
            .filter_map(|argument| match argument.source {
                TerminalDynamicDescriptorSource::Selection { ordinal } => {
                    Some((argument.owner, ordinal))
                }
                TerminalDynamicDescriptorSource::ReboundDescriptor { .. }
                | TerminalDynamicDescriptorSource::Parameter { .. } => None,
            }),
    );
    validate_parameter_dynamic_dispatches(module, machines)?;
    consumption::require_every_dynamic_site_consumed(
        module,
        &consumed_selections,
        &indirect_coordinates,
        &stored_dispatch_coordinates,
        &consumed_descriptors,
        &consumed_stored_descriptors,
    )?;
    Ok(())
}

fn validate_stored_dynamic_dispatches(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    selections: &[terminal_psi::TerminalDynamicConformanceSelection],
    consumed_selections: &mut BTreeSet<(MachineId, u32)>,
) -> Result<
    (
        BTreeSet<(MachineId, OperationId)>,
        BTreeSet<(MachineId, u32)>,
    ),
    ModuleError,
> {
    let descriptors = &module.dynamic_dispatch.stored_descriptors;
    let mut descriptor_coordinates = BTreeSet::new();
    let mut establishment_coordinates = BTreeSet::new();
    let mut expected_ordinals = BTreeMap::<MachineId, u32>::new();
    if !descriptors
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].ordinal) < (pair[1].owner, pair[1].ordinal))
    {
        return Err(ModuleError::NonCanonicalStoredDynamicDescriptorOrder);
    }
    for descriptor in descriptors {
        if !descriptor_coordinates.insert((descriptor.owner, descriptor.ordinal))
            || !establishment_coordinates
                .insert((descriptor.owner, descriptor.establishment_operation))
            || module
                .dynamic_dispatch
                .rebound_descriptors
                .iter()
                .any(|rebound| {
                    rebound.owner == descriptor.owner && rebound.ordinal == descriptor.ordinal
                })
        {
            return Err(ModuleError::DuplicateStoredDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            });
        }
        let expected = expected_ordinals.entry(descriptor.owner).or_default();
        if descriptor.ordinal != *expected {
            return Err(ModuleError::NonDenseStoredDynamicDescriptor {
                owner: descriptor.owner,
                expected: *expected,
                actual: descriptor.ordinal,
            });
        }
        *expected = expected
            .checked_add(1)
            .ok_or(ModuleError::InvalidStoredDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            })?;
        let selection_count = selections
            .iter()
            .filter(|selection| {
                selection.owner == descriptor.owner
                    && selection.ordinal == descriptor.selection_ordinal
            })
            .count();
        let Some(caller) = machines.get(&descriptor.owner).copied() else {
            return Err(ModuleError::InvalidStoredDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            });
        };
        let establishments = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == descriptor.establishment_operation)
            .collect::<Vec<_>>();
        let [establishment] = establishments.as_slice() else {
            return Err(ModuleError::InvalidStoredDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            });
        };
        if selection_count != 1
            || descriptor.aggregate_type_identity.is_empty()
            || descriptor.field_identity.is_empty()
            || !matches!(
                (&establishment.kind, &establishment.result),
                (
                    OperationKind::StoreDynamicDescriptor { descriptor_ordinal },
                    OperationResult::Unit,
                ) if *descriptor_ordinal == descriptor.ordinal
            )
        {
            return Err(ModuleError::InvalidStoredDynamicDescriptor {
                owner: descriptor.owner,
                ordinal: descriptor.ordinal,
            });
        }
    }

    let dispatches = &module.dynamic_dispatch.stored_dispatches;
    let mut dispatch_coordinates = BTreeSet::new();
    if !dispatches
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].operation) < (pair[1].owner, pair[1].operation))
    {
        return Err(ModuleError::NonCanonicalStoredDynamicDispatchOrder);
    }
    let mut consumed_descriptors = BTreeSet::new();
    for dispatch in dispatches {
        if !dispatch_coordinates.insert((dispatch.owner, dispatch.operation)) {
            return Err(ModuleError::DuplicateStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        }
        let matching_descriptors = descriptors
            .iter()
            .filter(|descriptor| {
                descriptor.owner == dispatch.owner
                    && descriptor.ordinal == dispatch.descriptor_ordinal
            })
            .collect::<Vec<_>>();
        let [descriptor] = matching_descriptors.as_slice() else {
            return Err(ModuleError::InvalidStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        };
        let selection = selections
            .iter()
            .find(|selection| {
                selection.owner == descriptor.owner
                    && selection.ordinal == descriptor.selection_ordinal
            })
            .ok_or(ModuleError::InvalidStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            })?;
        let applications = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == selection.owner
                    && application.report_fingerprint
                        == selection.conformance_application_report_fingerprint
                    && application.commitment == selection.conformance_application_commitment
            })
            .collect::<Vec<_>>();
        let [application] = applications.as_slice() else {
            return Err(ModuleError::InvalidStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        };
        let rows = application
            .rows
            .iter()
            .filter(|row| {
                row.declaring_trait_identity == dispatch.declaring_trait_identity
                    && row.public_requirement_identity == dispatch.public_requirement_identity
                    && row.family_tuple == dispatch.family_tuple
                    && row.requirement_identity == dispatch.requirement_identity
                    && row.realization_identity == dispatch.realization_identity
                    && row.realization_callable_identity.as_deref()
                        == Some(dispatch.realization_callable_identity.as_str())
            })
            .count();
        let callables = application
            .realization_callables
            .iter()
            .filter(|callable| {
                callable.source_callable_identity == dispatch.realization_callable_identity
                    && callable.machine == dispatch.realization
            })
            .count();
        let Some(caller) = machines.get(&dispatch.owner).copied() else {
            return Err(ModuleError::InvalidStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        };
        let Some(realization) = machines.get(&dispatch.realization).copied() else {
            return Err(ModuleError::InvalidStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        };
        let operation = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == dispatch.operation)
            .collect::<Vec<_>>();
        let [operation] = operation.as_slice() else {
            return Err(ModuleError::InvalidStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        };
        let ordered_in_one_block = caller.blocks.iter().any(|block| {
            let establishment = block
                .operations
                .iter()
                .position(|operation| operation.id == descriptor.establishment_operation);
            let call = block
                .operations
                .iter()
                .position(|operation| operation.id == dispatch.operation);
            establishment
                .zip(call)
                .is_some_and(|(store, call)| store < call)
        });
        let exact_call = matches!(
            (&operation.kind, &operation.result, &realization.result),
            (
                OperationKind::CallDynamicScalar {
                    descriptor_ordinal,
                    requirement_obligations,
                    crash_continuations,
                },
                OperationResult::Scalar(operation_result),
                TerminalMachineResult::Scalar(callable_result),
            ) if *descriptor_ordinal == dispatch.descriptor_ordinal
                && operation_result.scalar_type == callable_result.scalar_type
                && operation_result.qualifications == callable_result.qualifications
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
        );
        let source_type = dynamic_source_type_identity(module, machines, selection);
        let realization_source_type = realization
            .structural_parameters
            .as_slice()
            .first()
            .and_then(|parameter| {
                (realization.structural_parameters.len() == 1).then_some(parameter.structural_type)
            })
            .and_then(|structural_type| {
                module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
            })
            .map(|declaration| declaration.identity.as_str());
        validate_structural_arguments(
            module,
            caller,
            std::slice::from_ref(&selection.source),
            &realization.structural_parameters,
            dispatch.operation,
            true,
            super::structural_operations::StructuralArgumentSourcePolicy::OnlyParameters,
        )?;
        if !ordered_in_one_block
            || !exact_call
            || rows != 1
            || callables != 1
            || source_type.as_deref() != application.subject_identity.as_deref()
            || source_type.as_deref() != realization_source_type
            || !realization.parameters.is_empty()
            || !realization.contract.requires.is_empty()
            || !realization.contract.ensures.is_empty()
            || !realization.contract.outcome_specific_ensures.is_empty()
            || !realization.contract.crash_routes.is_empty()
            || !realization.entry_claims.is_empty()
            || !realization.content_entry_claims.is_empty()
            || !realization.content_identity_reshuffles.is_empty()
            || !realization.content_partition_compositions.is_empty()
            || realization
                .published_service_ceiling
                .iter()
                .any(|service| !caller.published_service_ceiling.contains(service))
            || dispatch.declaring_trait_identity.is_empty()
            || dispatch.public_requirement_identity.is_empty()
            || dispatch.requirement_identity.is_empty()
            || dispatch.realization_identity.is_empty()
            || dispatch.realization_callable_identity.is_empty()
            || !consumed_descriptors.insert((dispatch.owner, dispatch.descriptor_ordinal))
        {
            return Err(ModuleError::InvalidStoredDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        }
        consumed_selections.insert((selection.owner, selection.ordinal));
    }
    Ok((dispatch_coordinates, consumed_descriptors))
}

fn validate_dynamic_descriptor_parameters(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let parameters = &module.dynamic_dispatch.parameters;
    if !parameters
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].ordinal) < (pair[1].owner, pair[1].ordinal))
    {
        return Err(ModuleError::NonCanonicalDynamicDescriptorParameterOrder);
    }
    let mut coordinates = BTreeSet::new();
    let mut source_positions = BTreeSet::new();
    let mut expected_ordinals = BTreeMap::<MachineId, u32>::new();
    for parameter in parameters {
        if !coordinates.insert((parameter.owner, parameter.ordinal)) {
            return Err(ModuleError::DuplicateDynamicDescriptorParameter {
                owner: parameter.owner,
                ordinal: parameter.ordinal,
            });
        }
        let expected = expected_ordinals.entry(parameter.owner).or_default();
        if parameter.ordinal != *expected {
            return Err(ModuleError::NonDenseDynamicDescriptorParameter {
                owner: parameter.owner,
                expected: *expected,
                actual: parameter.ordinal,
            });
        }
        *expected =
            expected
                .checked_add(1)
                .ok_or(ModuleError::InvalidDynamicDescriptorParameter {
                    owner: parameter.owner,
                    ordinal: parameter.ordinal,
                })?;
        let requirements_are_canonical =
            parameter
                .requirements
                .iter()
                .enumerate()
                .all(|(slot, requirement)| {
                    usize::try_from(requirement.slot) == Ok(slot)
                        && !requirement.declaring_trait_identity.is_empty()
                        && !requirement.public_requirement_identity.is_empty()
                });
        // One requirement slot names a `(declaring trait, requirement
        // overload, canonical family tuple)` coordinate: sibling finite-family
        // tuples share the two identities and remain distinct slots.
        let requirement_identities = parameter
            .requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.declaring_trait_identity.as_str(),
                    requirement.public_requirement_identity.as_str(),
                    requirement.family_tuple.as_slice(),
                )
            })
            .collect::<BTreeSet<_>>();
        if !machines.contains_key(&parameter.owner)
            || !source_positions.insert((parameter.owner, parameter.source_position))
            || parameter.trait_identity.is_empty()
            || !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
            || !requirements_are_canonical
            || requirement_identities.len() != parameter.requirements.len()
        {
            return Err(ModuleError::InvalidDynamicDescriptorParameter {
                owner: parameter.owner,
                ordinal: parameter.ordinal,
            });
        }
    }
    Ok(())
}

fn validate_dynamic_descriptor_arguments(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    descriptors: &[terminal_psi::TerminalReboundDynamicDescriptor],
    selections: &[terminal_psi::TerminalDynamicConformanceSelection],
) -> Result<BTreeSet<(MachineId, u32)>, ModuleError> {
    let arguments = &module.dynamic_dispatch.arguments;
    if !arguments.windows(2).all(|pair| {
        (pair[0].owner, pair[0].operation, pair[0].parameter_ordinal)
            < (pair[1].owner, pair[1].operation, pair[1].parameter_ordinal)
    }) {
        return Err(ModuleError::NonCanonicalDynamicDescriptorArgumentOrder);
    }
    let mut coordinates = BTreeSet::new();
    for argument in arguments {
        if !coordinates.insert((
            argument.owner,
            argument.operation,
            argument.parameter_ordinal,
        )) {
            return Err(ModuleError::DuplicateDynamicDescriptorArgument {
                owner: argument.owner,
                operation: argument.operation,
                parameter_ordinal: argument.parameter_ordinal,
            });
        }
    }

    let mut consumed_coordinates = BTreeSet::new();
    let mut consumed_descriptors = BTreeSet::new();
    for (owner, machine) in machines {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let (callee, admits_dynamic_arguments) = match operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. } => {
                    (callee, false)
                }
                OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. } => (callee, true),
                _ => continue,
            };
            let target_parameters = module
                .dynamic_dispatch
                .parameters
                .iter()
                .filter(|parameter| parameter.owner == callee)
                .collect::<Vec<_>>();
            let supplied = arguments
                .iter()
                .filter(|argument| argument.owner == *owner && argument.operation == operation.id)
                .collect::<Vec<_>>();
            if (!target_parameters.is_empty() && !admits_dynamic_arguments)
                || supplied.len() != target_parameters.len()
            {
                return Err(ModuleError::InvalidDynamicDescriptorArgument {
                    owner: *owner,
                    operation: operation.id,
                    parameter_ordinal: u32::try_from(supplied.len()).unwrap_or(u32::MAX),
                });
            }
            for (target, argument) in target_parameters.into_iter().zip(supplied) {
                consumed_coordinates.insert((
                    argument.owner,
                    argument.operation,
                    argument.parameter_ordinal,
                ));
                if let TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } =
                    argument.source
                {
                    consumed_descriptors.insert((argument.owner, ordinal));
                }
                if argument.parameter_ordinal != target.ordinal
                    || !dynamic_argument_matches_parameter(
                        module,
                        *owner,
                        argument.source,
                        target,
                        descriptors,
                        selections,
                    )
                {
                    return Err(ModuleError::InvalidDynamicDescriptorArgument {
                        owner: *owner,
                        operation: operation.id,
                        parameter_ordinal: argument.parameter_ordinal,
                    });
                }
            }
        }
    }
    if consumed_coordinates != coordinates {
        let (owner, operation, parameter_ordinal) = coordinates
            .difference(&consumed_coordinates)
            .next()
            .copied()
            .expect("consumed dynamic argument coordinates are a subset of declared coordinates");
        return Err(ModuleError::InvalidDynamicDescriptorArgument {
            owner,
            operation,
            parameter_ordinal,
        });
    }
    Ok(consumed_descriptors)
}

fn dynamic_argument_matches_parameter(
    module: &TerminalModule,
    owner: MachineId,
    source: TerminalDynamicDescriptorSource,
    target: &TerminalDynamicDescriptorParameter,
    descriptors: &[terminal_psi::TerminalReboundDynamicDescriptor],
    selections: &[terminal_psi::TerminalDynamicConformanceSelection],
) -> bool {
    match source {
        TerminalDynamicDescriptorSource::Selection { ordinal } => selections
            .iter()
            .find(|selection| selection.owner == owner && selection.ordinal == ordinal)
            .is_some_and(|selection| selection_matches_parameter(module, owner, selection, target)),
        TerminalDynamicDescriptorSource::Parameter { ordinal } => module
            .dynamic_dispatch
            .parameters
            .iter()
            .find(|parameter| parameter.owner == owner && parameter.ordinal == ordinal)
            .is_some_and(|source| dynamic_interfaces_match(source, target)),
        TerminalDynamicDescriptorSource::ReboundDescriptor { ordinal } => {
            let Some(descriptor) = descriptors
                .iter()
                .find(|descriptor| descriptor.owner == owner && descriptor.ordinal == ordinal)
            else {
                return false;
            };
            let Some(selection) = selections.iter().find(|selection| {
                selection.owner == owner
                    && selection.ordinal == descriptor.rebound_selection_ordinal
            }) else {
                return false;
            };
            selection_matches_parameter(module, owner, selection, target)
        }
    }
}

fn selection_matches_parameter(
    module: &TerminalModule,
    owner: MachineId,
    selection: &terminal_psi::TerminalDynamicConformanceSelection,
    target: &TerminalDynamicDescriptorParameter,
) -> bool {
    module
        .closed_conformance_applications
        .iter()
        .find(|application| {
            application.owner == owner
                && application.report_fingerprint
                    == selection.conformance_application_report_fingerprint
                && application.commitment == selection.conformance_application_commitment
        })
        .is_some_and(|application| {
            application.trait_identity == target.trait_identity
                && application.rows.len() == target.requirements.len()
                && application
                    .rows
                    .iter()
                    .zip(&target.requirements)
                    .all(|(row, requirement)| {
                        row.declaring_trait_identity == requirement.declaring_trait_identity
                            && row.public_requirement_identity
                                == requirement.public_requirement_identity
                            && row.family_tuple == requirement.family_tuple
                            && row
                                .realization_callable_identity
                                .as_ref()
                                .and_then(|identity| {
                                    application.realization_callables.iter().find(|callable| {
                                        callable.source_callable_identity == *identity
                                    })
                                })
                                .is_some_and(|callable| callable.result == requirement.result)
                    })
        })
}

fn dynamic_interfaces_match(
    source: &TerminalDynamicDescriptorParameter,
    target: &TerminalDynamicDescriptorParameter,
) -> bool {
    source.trait_identity == target.trait_identity
        && source.access == target.access
        && source.requirements == target.requirements
}

fn validate_parameter_dynamic_dispatches(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let dispatches = &module.dynamic_dispatch.parameter_dispatches;
    if !dispatches
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].operation) < (pair[1].owner, pair[1].operation))
    {
        return Err(ModuleError::NonCanonicalParameterDynamicDispatchOrder);
    }
    let mut coordinates = BTreeSet::new();
    for dispatch in dispatches {
        if !coordinates.insert((dispatch.owner, dispatch.operation)) {
            return Err(ModuleError::DuplicateParameterDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        }
        let parameter = module.dynamic_dispatch.parameters.iter().find(|parameter| {
            parameter.owner == dispatch.owner && parameter.ordinal == dispatch.parameter_ordinal
        });
        let requirement = parameter.and_then(|parameter| {
            parameter
                .requirements
                .iter()
                .find(|requirement| requirement.slot == dispatch.requirement_slot)
        });
        let operation = machines.get(&dispatch.owner).and_then(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == dispatch.operation)
        });
        let valid = match (requirement, operation) {
            (Some(requirement), Some(operation)) => {
                matches!(
                    (&operation.kind, &operation.result, requirement.result),
                    (
                        OperationKind::CallDynamicParameterScalar {
                            parameter_ordinal,
                            requirement_slot,
                            requirement_obligations,
                            crash_continuations,
                        },
                        OperationResult::Scalar(result),
                        ClosedConformanceCallableResult::I32,
                    ) if *parameter_ordinal == dispatch.parameter_ordinal
                        && *requirement_slot == dispatch.requirement_slot
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                        && result.scalar_type == semantic_vocabulary::ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                                .expect("i32 is a valid Terminal scalar type")
                        )
                ) || matches!(
                    (&operation.kind, &operation.result, requirement.result),
                    (
                        OperationKind::CallDynamicParameterScalar {
                            parameter_ordinal,
                            requirement_slot,
                            requirement_obligations,
                            crash_continuations,
                        },
                        OperationResult::Scalar(result),
                        ClosedConformanceCallableResult::Bool,
                    ) if *parameter_ordinal == dispatch.parameter_ordinal
                        && *requirement_slot == dispatch.requirement_slot
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                        && result.scalar_type == semantic_vocabulary::ScalarType::Boolean
                ) || matches!(
                    (&operation.kind, &operation.result, requirement.result),
                    (
                        OperationKind::CallDynamicParameterUnit {
                            parameter_ordinal,
                            requirement_slot,
                            requirement_obligations,
                            crash_continuations,
                        },
                        OperationResult::Unit,
                        ClosedConformanceCallableResult::Unit,
                    ) if *parameter_ordinal == dispatch.parameter_ordinal
                        && *requirement_slot == dispatch.requirement_slot
                        && requirement_obligations.is_empty()
                        && crash_continuations.is_empty()
                )
            }
            _ => false,
        };
        if !valid {
            return Err(ModuleError::InvalidParameterDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        }
    }
    for (owner, machine) in machines {
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            if matches!(
                operation.kind,
                OperationKind::CallDynamicParameterScalar { .. }
                    | OperationKind::CallDynamicParameterUnit { .. }
            ) && !coordinates.contains(&(*owner, operation.id))
            {
                return Err(ModuleError::InvalidParameterDynamicDispatch {
                    owner: *owner,
                    operation: operation.id,
                });
            }
        }
    }
    Ok(())
}

fn invalid_dispatch(owner: MachineId, operation: OperationId) -> ModuleError {
    ModuleError::InvalidDirectDynamicDispatch { owner, operation }
}

fn invalid_descriptor(owner: MachineId, ordinal: u32) -> ModuleError {
    ModuleError::InvalidReboundDynamicDescriptor { owner, ordinal }
}

fn invalid_indirect_dispatch(owner: MachineId, operation: OperationId) -> ModuleError {
    ModuleError::InvalidIndirectDynamicDispatch { owner, operation }
}

fn dynamic_source_type_identity(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    selection: &terminal_psi::TerminalDynamicConformanceSelection,
) -> Option<String> {
    let caller = machines.get(&selection.owner).copied()?;
    let source_type = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == selection.source.place)
        .and_then(|parameter| {
            resolve_structural_path(module, parameter.structural_type, &selection.source.path)
        })?;
    module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == source_type)
        .map(|declaration| declaration.identity.clone())
}

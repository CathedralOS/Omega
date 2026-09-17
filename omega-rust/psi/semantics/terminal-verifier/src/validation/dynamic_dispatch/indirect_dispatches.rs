//! The indirect dynamic dispatches and the descriptors they consume.

use super::super::ModuleError;
use super::super::structural_operations::validate_structural_arguments;
use super::{dynamic_source_type_identity, invalid_indirect_dispatch};
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    OperationKind, OperationResult, TerminalMachine, TerminalMachineResult, TerminalModule,
};

/// The indirect dynamic dispatches: coordinates are unique and ordered, and
/// each dispatch names a dynamic call operation whose descriptor it
/// consumes. Returns the dispatch coordinates and the descriptors consumed.
pub(super) fn validate_indirect_dispatches(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<
    (
        BTreeSet<(MachineId, OperationId)>,
        BTreeSet<(MachineId, u32)>,
    ),
    ModuleError,
> {
    let selections = &module.dynamic_dispatch.selections;
    let descriptors = &module.dynamic_dispatch.rebound_descriptors;
    let indirect_dispatches = &module.dynamic_dispatch.indirect_dispatches;
    let mut indirect_coordinates = BTreeSet::new();
    for dispatch in indirect_dispatches {
        if !indirect_coordinates.insert((dispatch.owner, dispatch.operation)) {
            return Err(ModuleError::DuplicateIndirectDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        }
    }
    if !indirect_dispatches
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].operation) < (pair[1].owner, pair[1].operation))
    {
        return Err(ModuleError::NonCanonicalIndirectDynamicDispatchOrder);
    }
    let mut consumed_descriptors = BTreeSet::new();
    for dispatch in indirect_dispatches {
        let descriptor = descriptors
            .iter()
            .filter(|descriptor| {
                descriptor.owner == dispatch.owner
                    && descriptor.ordinal == dispatch.descriptor_ordinal
            })
            .collect::<Vec<_>>();
        let [descriptor] = descriptor.as_slice() else {
            return Err(invalid_indirect_dispatch(
                dispatch.owner,
                dispatch.operation,
            ));
        };
        let latest = selections
            .iter()
            .find(|selection| {
                selection.owner == dispatch.owner
                    && selection.ordinal == descriptor.rebound_selection_ordinal
            })
            .ok_or_else(|| invalid_indirect_dispatch(dispatch.owner, dispatch.operation))?;
        let initial = selections
            .iter()
            .find(|selection| {
                selection.owner == dispatch.owner
                    && selection.ordinal == descriptor.initial_selection_ordinal
            })
            .ok_or_else(|| invalid_indirect_dispatch(dispatch.owner, dispatch.operation))?;
        let application = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == dispatch.owner
                    && application.report_fingerprint
                        == latest.conformance_application_report_fingerprint
                    && application.commitment == latest.conformance_application_commitment
            })
            .collect::<Vec<_>>();
        let [application] = application.as_slice() else {
            return Err(invalid_indirect_dispatch(
                dispatch.owner,
                dispatch.operation,
            ));
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
            return Err(invalid_indirect_dispatch(
                dispatch.owner,
                dispatch.operation,
            ));
        };
        let Some(realization) = machines.get(&dispatch.realization).copied() else {
            return Err(invalid_indirect_dispatch(
                dispatch.owner,
                dispatch.operation,
            ));
        };
        let operations = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == dispatch.operation)
            .collect::<Vec<_>>();
        let [operation] = operations.as_slice() else {
            return Err(invalid_indirect_dispatch(
                dispatch.owner,
                dispatch.operation,
            ));
        };
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
        ) || matches!(
            (&operation.kind, &operation.result, &realization.result),
            (
                OperationKind::CallDynamicUnit {
                    descriptor_ordinal,
                    requirement_obligations,
                    crash_continuations,
                },
                OperationResult::Unit,
                TerminalMachineResult::Unit,
            ) if *descriptor_ordinal == dispatch.descriptor_ordinal
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
        );
        let source_type = dynamic_source_type_identity(module, machines, latest);
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
            std::slice::from_ref(&initial.source),
            &realization.structural_parameters,
            dispatch.operation,
            true,
            super::super::structural_operations::StructuralArgumentSourcePolicy::OnlyParameters,
        )?;
        validate_structural_arguments(
            module,
            caller,
            std::slice::from_ref(&latest.source),
            &realization.structural_parameters,
            dispatch.operation,
            true,
            super::super::structural_operations::StructuralArgumentSourcePolicy::OnlyParameters,
        )?;
        if !exact_call
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
            return Err(invalid_indirect_dispatch(
                dispatch.owner,
                dispatch.operation,
            ));
        }
    }
    Ok((indirect_coordinates, consumed_descriptors))
}

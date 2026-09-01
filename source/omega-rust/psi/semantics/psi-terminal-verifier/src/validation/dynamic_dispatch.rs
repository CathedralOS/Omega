use std::collections::{BTreeMap, BTreeSet};

use psi_core::{MachineId, OperationId};
use psi_terminal::{
    OperationKind, OperationResult, TerminalMachine, TerminalMachineResult, TerminalModule,
};

use super::ModuleError;
use super::foundation::resolve_structural_path;

pub(super) fn validate_direct_dynamic_dispatches(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<(), ModuleError> {
    let selections = &module.dynamic_dispatch.selections;
    let mut selection_coordinates = BTreeSet::new();
    for selection in selections {
        if !selection_coordinates.insert((selection.owner, selection.ordinal)) {
            return Err(ModuleError::DuplicateDynamicConformanceSelection {
                owner: selection.owner,
                ordinal: selection.ordinal,
            });
        }
    }
    if !selections
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].ordinal) < (pair[1].owner, pair[1].ordinal))
    {
        return Err(ModuleError::NonCanonicalDynamicConformanceSelectionOrder);
    }
    let mut expected_ordinals = BTreeMap::<MachineId, u32>::new();
    for selection in selections {
        let expected = expected_ordinals.entry(selection.owner).or_default();
        if selection.ordinal != *expected {
            return Err(ModuleError::NonDenseDynamicConformanceSelection {
                owner: selection.owner,
                expected: *expected,
                actual: selection.ordinal,
            });
        }
        *expected =
            expected
                .checked_add(1)
                .ok_or(ModuleError::InvalidDynamicConformanceSelection {
                    owner: selection.owner,
                    ordinal: selection.ordinal,
                })?;
        let application_count = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == selection.owner
                    && application.report_fingerprint
                        == selection.conformance_application_report_fingerprint
                    && application.commitment == selection.conformance_application_commitment
            })
            .count();
        if !machines.contains_key(&selection.owner)
            || selection.conformance_application_report_fingerprint == 0
            || selection.conformance_application_commitment.is_zero()
            || application_count != 1
        {
            return Err(ModuleError::InvalidDynamicConformanceSelection {
                owner: selection.owner,
                ordinal: selection.ordinal,
            });
        }
    }

    let dispatches = &module.dynamic_dispatch.direct_dispatches;
    let mut dispatch_coordinates = BTreeSet::<(MachineId, OperationId)>::new();
    for dispatch in dispatches {
        if !dispatch_coordinates.insert((dispatch.owner, dispatch.operation)) {
            return Err(ModuleError::DuplicateDirectDynamicDispatch {
                owner: dispatch.owner,
                operation: dispatch.operation,
            });
        }
    }
    if !dispatches
        .windows(2)
        .all(|pair| (pair[0].owner, pair[0].operation) < (pair[1].owner, pair[1].operation))
    {
        return Err(ModuleError::NonCanonicalDirectDynamicDispatchOrder);
    }
    let mut consumed_selections = BTreeSet::new();
    for dispatch in dispatches {
        let matching_selections = selections
            .iter()
            .filter(|selection| {
                selection.owner == dispatch.owner && selection.ordinal == dispatch.selection_ordinal
            })
            .collect::<Vec<_>>();
        let [selection] = matching_selections.as_slice() else {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        };
        let matching_applications = module
            .closed_conformance_applications
            .iter()
            .filter(|application| {
                application.owner == dispatch.owner
                    && application.report_fingerprint
                        == selection.conformance_application_report_fingerprint
                    && application.commitment == selection.conformance_application_commitment
            })
            .collect::<Vec<_>>();
        let [application] = matching_applications.as_slice() else {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        };
        let matching_rows = application
            .rows
            .iter()
            .filter(|row| {
                row.declaring_trait_identity == dispatch.declaring_trait_identity
                    && row.public_requirement_identity == dispatch.public_requirement_identity
                    && row.requirement_identity == dispatch.requirement_identity
                    && row.realization_identity == dispatch.realization_identity
                    && row.realization_callable_identity.as_deref()
                        == Some(dispatch.realization_callable_identity.as_str())
            })
            .collect::<Vec<_>>();
        let [_row] = matching_rows.as_slice() else {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        };
        let matching_callables = application
            .realization_callables
            .iter()
            .filter(|callable| {
                callable.source_callable_identity == dispatch.realization_callable_identity
                    && callable.machine == dispatch.realization
            })
            .collect::<Vec<_>>();
        let [_callable] = matching_callables.as_slice() else {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        };
        let Some(caller) = machines.get(&dispatch.owner).copied() else {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        };
        let Some(realization) = machines.get(&dispatch.realization).copied() else {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        };
        let source_type = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == selection.source.place)
            .and_then(|parameter| {
                resolve_structural_path(module, parameter.structural_type, &selection.source.path)
            })
            .and_then(|source_type| {
                module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == source_type)
            });
        let operations = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| operation.id == dispatch.operation)
            .collect::<Vec<_>>();
        let [operation] = operations.as_slice() else {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        };
        let exact_call = matches!(
            (&operation.kind, &operation.result, &realization.result),
            (
                OperationKind::CallStructuralScalar {
                    callee,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                OperationResult::Scalar(operation_result),
                TerminalMachineResult::Scalar(callable_result),
            ) if *callee == dispatch.realization
                && structural_arguments.as_slice() == std::slice::from_ref(&selection.source)
                && operation_result.scalar_type == callable_result.scalar_type
                && claim_transfers.is_empty()
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
        );
        if !exact_call
            || source_type.map(|declaration| declaration.identity.as_str())
                != application.subject_identity.as_deref()
            || dispatch.declaring_trait_identity.is_empty()
            || dispatch.public_requirement_identity.is_empty()
            || dispatch.requirement_identity.is_empty()
            || dispatch.realization_identity.is_empty()
            || dispatch.realization_callable_identity.is_empty()
        {
            return Err(invalid_dispatch(dispatch.owner, dispatch.operation));
        }
        consumed_selections.insert((dispatch.owner, dispatch.selection_ordinal));
    }
    for selection in selections {
        if !consumed_selections.contains(&(selection.owner, selection.ordinal)) {
            return Err(ModuleError::OrphanDynamicConformanceSelection {
                owner: selection.owner,
                ordinal: selection.ordinal,
            });
        }
    }
    Ok(())
}

fn invalid_dispatch(owner: MachineId, operation: OperationId) -> ModuleError {
    ModuleError::InvalidDirectDynamicDispatch { owner, operation }
}

//! The direct dynamic dispatches and the selections they consume.

use super::super::ModuleError;
use super::super::foundation::resolve_structural_path;
use super::invalid_dispatch;
use semantic_vocabulary::{MachineId, OperationId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    OperationKind, OperationResult, TerminalMachine, TerminalMachineResult, TerminalModule,
};

/// The direct dynamic dispatches: coordinates are unique and ordered, and
/// each dispatch names a known operation whose selection it consumes.
/// Returns the selections consumed.
pub(super) fn validate_direct_dispatches(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
) -> Result<BTreeSet<(MachineId, u32)>, ModuleError> {
    let selections = &module.dynamic_dispatch.selections;
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
                    && row.family_tuple == dispatch.family_tuple
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
                    arguments,
                    erased_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                OperationResult::Scalar(operation_result),
                TerminalMachineResult::Scalar(callable_result),
            ) if *callee == dispatch.realization
                && arguments.is_empty()
                && erased_arguments.is_empty()
                && structural_arguments.as_slice() == std::slice::from_ref(&selection.source)
                && operation_result.scalar_type == callable_result.scalar_type
                && operation_result.qualifications == callable_result.qualifications
                && claim_transfers.is_empty()
                && requirement_obligations.is_empty()
                && crash_continuations.is_empty()
        ) || matches!(
            (&operation.kind, &operation.result, &realization.result),
            (
                OperationKind::CallUnit {
                    callee,
                    arguments,
                    erased_arguments,
                    structural_arguments,
                    claim_transfers,
                    requirement_obligations,
                    crash_continuations,
                },
                OperationResult::Unit,
                TerminalMachineResult::Unit,
            ) if *callee == dispatch.realization
                && arguments.is_empty()
                && erased_arguments.is_empty()
                && structural_arguments.as_slice() == std::slice::from_ref(&selection.source)
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
    Ok(consumed_selections)
}

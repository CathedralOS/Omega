//! Selection custody is separate from semantic and physical call correctness.
//!
//! The graph readers replay the original boundary, candidate contract, operands,
//! result and ABI. A second valid catalog candidate could satisfy those checks.
//! Before constructing the validated target owner, compare selected calls with
//! their separately supplied installation or admitted family catalog selection.
//! The immutable source owner retains that authority. This comparison grants no admission
//! when used with synthetic evidence in tests.

use std::collections::BTreeSet;

use installation_evidence::ProviderInstallationEvidence;
use target_operations::{NativeCallOrigin, TargetOperationPlan, TargetUnitOperation};
use terminal_psi::OperationResult;

use crate::LoweringError;

pub(crate) fn validate(
    target: &TargetOperationPlan,
    installation: Option<&dyn ProviderInstallationEvidence>,
    plan: &abstract_operations::AbstractOperationPlan,
    input: Option<&terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
) -> Result<(), LoweringError> {
    if installation.is_some_and(|installation| installation.psi() != target.psi) {
        return Err(LoweringError::ProviderInstallationIdentityMismatch);
    }
    let installed = installation.map_or_else(Vec::new, |installation| {
        installation.installed_provider_calls()
    });
    let selected = crate::lowering::selected_boundary_calls::index(
        plan,
        installed
            .iter()
            .cloned()
            .map(|call| ((call.caller, call.psi_operation, call.boundary), call))
            .collect(),
        input,
    )?;
    let mut observed = BTreeSet::new();
    for function in &target.functions {
        for operation in function.graph.operations() {
            let (psi_operation, call_target, origin, scalar_arguments, arguments) = match operation
            {
                TargetUnitOperation::Call {
                    psi_operation,
                    target,
                    origin,
                    scalar_arguments,
                    arguments,
                    ..
                } => (*psi_operation, *target, origin, scalar_arguments, arguments),
                _ => continue,
            };
            if let NativeCallOrigin::TargetFamily { boundary, .. } = origin {
                let key = (function.machine, psi_operation, *boundary);
                let invalid = || LoweringError::SelectedBoundaryCallEvidenceMismatch {
                    machine: key.0,
                    operation: key.1,
                    boundary: key.2,
                };
                let expected = selected.get(&key).ok_or_else(invalid)?;
                if !observed.insert(key)
                    || &expected.origin != origin
                    || call_target != target_operations::NativeCallTarget::Machine(expected.callee)
                {
                    return Err(invalid());
                }
                continue;
            }
            let NativeCallOrigin::InstalledProvider {
                boundary,
                provider,
                assumption,
                completion_claim_sources,
                completion_receipts,
            } = origin
            else {
                continue;
            };
            let key = (function.machine, psi_operation, *boundary);
            let invalid = || LoweringError::SelectedBoundaryCallEvidenceMismatch {
                machine: function.machine,
                operation: psi_operation,
                boundary: *boundary,
            };
            if !observed.insert(key) {
                return Err(LoweringError::DuplicateSelectedBoundaryCall {
                    machine: function.machine,
                    operation: psi_operation,
                    boundary: *boundary,
                });
            }
            let mut matching = installed
                .iter()
                .filter(|call| (call.caller, call.psi_operation, call.boundary) == key);
            let Some(call) = matching.next() else {
                return Err(invalid());
            };
            if matching.next().is_some() {
                return Err(invalid());
            }
            let result_matches = match (operation, &call.result) {
                (
                    TargetUnitOperation::Call {
                        result: target_operations::TargetCallResult::Unit,
                        ..
                    },
                    OperationResult::Unit,
                ) => true,
                (
                    TargetUnitOperation::Call {
                        result: target_operations::TargetCallResult::Structural { result, .. },
                        ..
                    },
                    OperationResult::Structural(expected),
                ) => result == expected,
                (
                    TargetUnitOperation::Call {
                        result: target_operations::TargetCallResult::Scalar(result),
                        ..
                    },
                    OperationResult::Scalar(expected),
                ) => {
                    result.source_value == expected.id && result.scalar_type == expected.scalar_type
                }
                _ => false,
            };
            if !result_matches
                || &call.provider != provider
                || &call.assumption != assumption
                || call_target
                    != target_operations::NativeCallTarget::Machine(call.provider.candidate)
                || scalar_arguments.len() != call.scalar_arguments.len()
                || !scalar_arguments
                    .iter()
                    .zip(&call.scalar_arguments)
                    .all(|(actual, expected)| actual.source_value() == *expected)
                || arguments.len() != call.structural_arguments.len()
                || !arguments
                    .iter()
                    .zip(&call.structural_arguments)
                    .all(|(actual, expected)| {
                        actual.place == expected.place
                            && actual.access == expected.access
                            && actual.path == expected.path
                    })
                || completion_receipts != &call.completion_receipts
                || completion_claim_sources.len() != call.completion_claim_sources.len()
                || !completion_claim_sources
                    .iter()
                    .zip(&call.completion_claim_sources)
                    .all(|(actual, expected)| {
                        actual.claim == expected.claim
                            && actual.entry == expected.entry
                            && actual.content == expected.content
                    })
            {
                return Err(invalid());
            }
        }
    }
    for call in &installed {
        if !observed.contains(&(call.caller, call.psi_operation, call.boundary)) {
            return Err(LoweringError::UnknownSelectedBoundaryCall {
                machine: call.caller,
                operation: call.psi_operation,
                boundary: call.boundary,
            });
        }
    }
    for key in selected.keys() {
        if !observed.contains(key) {
            return Err(LoweringError::UnknownSelectedBoundaryCall {
                machine: key.0,
                operation: key.1,
                boundary: key.2,
            });
        }
    }
    Ok(())
}

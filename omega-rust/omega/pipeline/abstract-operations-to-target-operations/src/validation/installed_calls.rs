//! Selection custody is separate from semantic and physical call correctness.
//!
//! The graph readers replay the original boundary, candidate contract, operands,
//! result and ABI. A second valid catalog candidate could satisfy those checks.
//! Before constructing the validated target owner, compare every installed call
//! with the separately supplied installation and retain that opaque installation
//! beside the immutable program. This diagnostic comparison grants no admission
//! when used with synthetic evidence in tests.

use std::collections::BTreeSet;

use installation_evidence::ProviderInstallationEvidence;
use target_operations::{NativeCallOrigin, TargetOperationPlan, TargetUnitOperation};
use terminal_psi::OperationResult;

use crate::LoweringError;

pub(crate) fn validate(
    target: &TargetOperationPlan,
    installation: Option<&dyn ProviderInstallationEvidence>,
) -> Result<(), LoweringError> {
    if installation.is_some_and(|installation| installation.psi() != target.psi) {
        return Err(LoweringError::ProviderInstallationIdentityMismatch);
    }
    let installed = installation.map_or_else(Vec::new, |installation| {
        installation.installed_provider_calls()
    });
    let mut observed = BTreeSet::new();
    for function in &target.functions {
        for operation in function
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
        {
            let (psi_operation, callee, origin, scalar_arguments, arguments) = match operation {
                TargetUnitOperation::Call {
                    psi_operation,
                    callee,
                    origin,
                    scalar_arguments,
                    arguments,
                    ..
                }
                | TargetUnitOperation::StructuralResultCall {
                    psi_operation,
                    callee,
                    origin,
                    scalar_arguments,
                    arguments,
                    ..
                } => (*psi_operation, *callee, origin, scalar_arguments, arguments),
                _ => continue,
            };
            let NativeCallOrigin::InstalledProvider {
                boundary,
                provider,
                completion_claim_sources,
                completion_receipts,
            } = origin
            else {
                continue;
            };
            let key = (function.machine, psi_operation, *boundary);
            let invalid = || LoweringError::InstalledProviderCallEvidenceMismatch {
                machine: function.machine,
                operation: psi_operation,
                boundary: *boundary,
            };
            if !observed.insert(key) {
                return Err(LoweringError::DuplicateInstalledProviderCall {
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
                        result_home: None, ..
                    },
                    OperationResult::Unit,
                ) => true,
                (
                    TargetUnitOperation::StructuralResultCall { result, .. },
                    OperationResult::Structural(expected),
                ) => result == expected,
                (
                    TargetUnitOperation::Call {
                        result_home: Some(result),
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
                || callee != call.provider.candidate
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
            return Err(LoweringError::UnknownInstalledProviderCall {
                machine: call.caller,
                operation: call.psi_operation,
                boundary: call.boundary,
            });
        }
    }
    Ok(())
}

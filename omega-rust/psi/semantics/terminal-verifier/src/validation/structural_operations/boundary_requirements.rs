//! Boundary requirements and completion receipts.

use crate::validation::structural_operations::claim_transfers::claim_input;
use crate::validation::structural_operations::structural_arguments::{
    linear_call_result, structural_occurrence_carries_qualification,
};
use crate::validation::{
    BTreeSet, BoundaryMachineDeclaration, CompletionReceipt, ModuleError, OperationId,
    StructuralArgument, TerminalMachine,
};

pub(crate) fn validate_boundary_requirements(
    caller: &TerminalMachine,
    boundary: &BoundaryMachineDeclaration,
    arguments: &[StructuralArgument],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for requirement in &boundary.requires {
        let argument = &arguments[requirement.argument_index as usize];
        // Literal and claim-free result operands cannot supply domain
        // evidence; a claimed linear call result carries it on its own
        // declared qualifications.
        let supplied = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .map(|parameter| {
                (
                    parameter.qualifications.as_slice(),
                    parameter.projected_qualifications.as_slice(),
                )
            })
            .or_else(|| {
                linear_call_result(caller, argument.place).map(|(_, result)| {
                    (
                        result.qualifications.as_slice(),
                        result.projected_qualifications.as_slice(),
                    )
                })
            });
        if supplied.is_none_or(|(qualifications, projected)| {
            !structural_occurrence_carries_qualification(
                qualifications,
                projected,
                &argument.path,
                requirement.domain,
            )
        }) {
            return Err(ModuleError::BoundaryArgumentMissingQualification {
                operation,
                argument_index: requirement.argument_index,
                domain: requirement.domain,
            });
        }
    }
    Ok(())
}

pub(crate) fn validate_boundary_completion_receipts(
    caller: &TerminalMachine,
    arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            caller
                .entry_claims
                .iter()
                .filter_map(move |claim| {
                    (claim.input == argument.place
                        && (argument.path.is_empty() || claim.path == argument.path))
                        .then_some((index as u32, claim.claim))
                })
                .chain(caller.content_entry_claims.iter().filter_map(move |claim| {
                    (claim.input.root == argument.place).then_some((index as u32, claim.claim))
                }))
                .chain(
                    linear_call_result(caller, argument.place)
                        .into_iter()
                        .flat_map(move |(_, result)| {
                            result.claims.iter().filter_map(move |binding| {
                                (argument.path.is_empty()
                                    || binding.path.as_slice() == argument.path.as_slice())
                                .then_some((index as u32, binding.claim))
                            })
                        }),
                )
        })
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for receipt in receipts {
        if !actual.insert((receipt.argument_index, receipt.claim)) || !claims.insert(receipt.claim)
        {
            return Err(ModuleError::DuplicateBoundaryCompletionReceipt(operation));
        }
        let Some(argument) = arguments.get(receipt.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: receipt.argument_index,
            });
        };
        // A receipted claim's home is its live holder: a moved result's
        // own claim bindings before the caller's entry roster.
        let result_home = linear_call_result(caller, argument.place).and_then(|(_, result)| {
            result
                .claims
                .iter()
                .find(|binding| binding.claim == receipt.claim)
                .map(|binding| (result.place, binding.path.as_slice()))
        });
        let Some((claim_input, claim_path)) =
            result_home.or_else(|| claim_input(caller, receipt.claim))
        else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: receipt.claim,
            });
        };
        if claim_input != argument.place
            || (!argument.path.is_empty() && claim_path != argument.path.as_slice())
        {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: receipt.claim,
                argument_index: receipt.argument_index,
            });
        }
    }
    if actual != expected {
        return Err(ModuleError::BoundaryCompletionReceiptMismatch(operation));
    }
    if receipts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalBoundaryCompletionReceipts(
            operation,
        ));
    }
    Ok(())
}

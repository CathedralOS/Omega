//! Boundary requirements and completion receipts.

use crate::validation::structural_operations::claim_transfers::claim_input;
use crate::validation::structural_operations::structural_arguments::structural_occurrence_carries_qualification;
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
        let actual = caller
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == argument.place);
        // Literal and claim-free result operands cannot supply domain evidence.
        if actual.is_none_or(|actual| {
            !structural_occurrence_carries_qualification(
                &actual.qualifications,
                &actual.projected_qualifications,
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
        let Some((claim_input, claim_path)) = claim_input(caller, receipt.claim) else {
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

//! Boundary requirements and completion receipts.

use crate::validation::structural::operations::structural_arguments::{
    linear_call_result, literal_qualifications, structural_occurrence_carries_qualification,
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
        // Claim-free result operands cannot supply domain evidence; a
        // claimed linear call result carries it on its own declared
        // qualifications, and a byte-sequence literal carries the
        // memberships its establishment replayed from the checked
        // byte-predicate discharge.
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
            })
            .or_else(|| {
                literal_qualifications(caller, argument.place)
                    .map(|qualifications| (qualifications, &[][..]))
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

/// The declaration-phase half of a boundary call's receipts: each row names an
/// argument that exists, no claim is receipted twice, and the roster is in
/// canonical order.
///
/// Which claims the roster must cover, and where each one currently lives, is
/// not a declaration-phase question. `ownership.md` makes the receipt set the
/// verifier-derived one for every claim *live* at each exact argument
/// occurrence, and a claim's home moves with every owned successor transfer, so
/// a state arrival holds obligations no static roster records. The ownership
/// frontier replays both at the call (`validation/frontier/operations.rs`),
/// exactly as it already does for an ordinary owned call's claim transfers,
/// whose declaration phase says the same thing about its own owned arguments.
pub(crate) fn validate_boundary_completion_receipts(
    arguments: &[StructuralArgument],
    receipts: &[CompletionReceipt],
    operation: OperationId,
) -> Result<(), ModuleError> {
    let mut rows = BTreeSet::new();
    let mut claims = BTreeSet::new();
    for receipt in receipts {
        if !rows.insert((receipt.argument_index, receipt.claim)) || !claims.insert(receipt.claim) {
            return Err(ModuleError::DuplicateBoundaryCompletionReceipt(operation));
        }
        if arguments.get(receipt.argument_index as usize).is_none() {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: receipt.argument_index,
            });
        }
    }
    if receipts.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalBoundaryCompletionReceipts(
            operation,
        ));
    }
    Ok(())
}

//! The rows every plain structural call shares: the produced result, the
//! call's own rosters and the callee's verified contract must agree that no
//! claim, requirement or evidence lane is carried, and the call's crash
//! continuations are exactly the contract's own routes. Per-shape
//! classification stays with each call contract; this entrance owns the rows
//! they repeat.
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::ValueId;

/// One plain structural call's bound rows, with the shared rows already
/// checked.
pub(crate) struct PlainStructuralCall<'a> {
    pub(crate) arguments: &'a [ValueId],
    pub(crate) structural_arguments: &'a [terminal_psi::StructuralArgument],
    pub(crate) result: &'a terminal_psi::StructuralOperationResult,
    pub(crate) selected_evidence: &'a [terminal_psi::OutcomeSpecificCallEvidence],
    pub(crate) signature: &'a terminal_psi::StructuralResultDeclaration,
    pub(crate) contract: &'a terminal_psi::MachineContract,
}

/// The shared plain-call contract of one `CallStructural` occurrence, or
/// `None` when it carries any claim, requirement or evidence row.
pub(crate) fn plain_structural_call<'a>(
    operation: &'a O,
    callee: &'a PsiOptimizationFunction,
) -> Option<PlainStructuralCall<'a>> {
    let O::CallStructural {
        result:
            AbstractStructuralCallResult::Structural {
                result,
                selected_evidence,
            },
        arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = operation
    else {
        return None;
    };
    let signature = callee.result.structural()?;
    let contract = callee.verified_contract.as_ref()?;
    (result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && *crash_continuations == contract.crash_routes
        && callee.entry_claim_declarations.is_empty()
        && callee.content_entry_claims.is_empty()
        && callee.evidence_contract_lanes.is_empty()
        && contract.requires.is_empty()
        && contract.ensures.is_empty())
    .then_some(PlainStructuralCall {
        arguments: arguments.as_slice(),
        structural_arguments: structural_arguments.as_slice(),
        result,
        selected_evidence: selected_evidence.as_slice(),
        signature,
        contract,
    })
}

//! Rejoin an operation's declared obligation with its exact accepted fact.
//!
//! Rewrite proposals retain that fact identity; this lookup does not admit a
//! proof or replace the independent optimization-unit validator.

use optimization_core::AcceptedObligationFactIdentity;
use optimization_unit::{OptimizationFact, PsiOptimizationUnit};
use semantic_vocabulary::{MachineId, OperationId};

use crate::RuleProposalError;

pub(super) fn accepted_obligation_fact(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
) -> Result<AcceptedObligationFactIdentity, RuleProposalError> {
    let obligation = unit.function(machine).and_then(|function| {
        function.facts.iter().find_map(|fact| match fact {
            OptimizationFact::OperationObligationReference {
                obligation,
                support,
            } if *support == operation => Some(*obligation),
            _ => None,
        })
    });
    let Some(obligation) = obligation else {
        return Err(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: None,
        });
    };
    unit.accepted_obligation_facts
        .iter()
        .find(|fact| {
            fact.machine == machine
                && fact.owner == optimization_unit::PsiProvenance::Operation(operation)
                && fact.obligation == obligation
        })
        .map(|fact| fact.identity)
        .ok_or(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: Some(obligation),
        })
}

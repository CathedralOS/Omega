//! Exact proof and scalar-fact lookup shared by candidate producers.

use optimization_core::{AcceptedObligationFactIdentity, ScalarConstantFactIdentity};
use optimization_unit::{OptimizationFact, PsiOptimizationUnit};
use semantic_vocabulary::{IntegerValue, MachineId, OperationId, ValueId};

use crate::{RuleProposalError, ScalarConstant, ScalarConstantAnalysis};

pub(in crate::rules::passes) fn accepted_obligation_fact(
    unit: &PsiOptimizationUnit,
    machine: MachineId,
    operation: OperationId,
) -> Result<AcceptedObligationFactIdentity, RuleProposalError> {
    let obligation = unit
        .functions
        .iter()
        .find(|function| function.machine == machine)
        .and_then(|function| {
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
            fact.machine == machine && fact.operation == operation && fact.obligation == obligation
        })
        .map(|fact| fact.identity)
        .ok_or(RuleProposalError::MissingAcceptedObligation {
            machine,
            operation,
            obligation: Some(obligation),
        })
}

pub(in crate::rules::passes) fn literal_integer_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(IntegerValue, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine
            && fact.value == value
            && fact.support.literal_operation().is_some())
        .then_some(fact)
        .and_then(|fact| match fact.constant {
            ScalarConstant::Integer(value) => fact.identity.map(|identity| (value, identity)),
            ScalarConstant::Boolean(_) => None,
        })
    })
}

pub(in crate::rules::passes) fn boolean_constant(
    constants: &ScalarConstantAnalysis,
    machine: MachineId,
    value: ValueId,
) -> Option<(bool, ScalarConstantFactIdentity)> {
    constants.facts.iter().find_map(|fact| {
        (fact.valid_in.machine == machine && fact.value == value)
            .then_some(fact)
            .and_then(|fact| match fact.constant {
                ScalarConstant::Boolean(value) => fact.identity.map(|identity| (value, identity)),
                ScalarConstant::Integer(_) => None,
            })
    })
}

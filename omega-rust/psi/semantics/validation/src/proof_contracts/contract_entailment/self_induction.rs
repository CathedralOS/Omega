//! Available self-induction hypotheses.

use crate::proof_contracts::contract_entailment::RESULT_BINDER;
use crate::proof_contracts::contract_entailment::citations::instantiated_fact_established;
use crate::proof_contracts::contract_entailment::refuted_requires::collect_instantiated_conjuncts;
use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralTerm,
};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;

/// Intake every structurally descending self-application's ensures whose
/// requires are proven in `judge` at the current statement boundary.  The
/// recursion validator separately licenses descent; this helper only governs
/// the conditional contract attached to that already-licensed application.
pub(crate) fn intake_available_self_induction_hypotheses(
    program: &TypedTrees,
    machine: &Machine,
    parameter_names: &[String],
    value: &StructuralTerm,
    judge: &mut StructuralJudge<'_>,
) {
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    let mut requires = Vec::new();
    let mut requires_out_of_language = false;
    let mut ensures = Vec::new();
    for contract in program.machine_contracts(machine) {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match (&contract.kind, fact) {
                (SignatureContractKind::Requires, ProofFact::Expression(expression)) => {
                    requires.push(*expression);
                }
                (SignatureContractKind::Requires, ProofFact::Membership(_)) => {
                    requires_out_of_language = true;
                }
                (SignatureContractKind::Requires, ProofFact::Proposition(_)) => {
                    requires_out_of_language = true;
                }
                (SignatureContractKind::Ensures, ProofFact::Expression(expression)) => {
                    ensures.push(*expression);
                }
                _ => {}
            }
        }
    }
    if requires_out_of_language {
        return;
    }

    let mut applications = Vec::new();
    StructuralJudge::self_applications(
        value,
        program
            .machine_states(machine)
            .first()
            .map_or(symbols::SymbolHandle::invalid(), |state| state.symbol),
        &mut applications,
    );
    for application in applications {
        let StructuralTerm::Application { arguments, .. } = &application else {
            continue;
        };
        let mut map: Vec<(String, StructuralTerm)> = parameter_names
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        let requirements_established = requires.iter().all(|fact| {
            instantiated_fact_established(
                program,
                judge,
                program.state_parameters(entry),
                *fact,
                &map,
            )
        });
        if !requirements_established {
            continue;
        }
        map.push((RESULT_BINDER.to_owned(), application.clone()));
        for fact in &ensures {
            let mut equations = Vec::new();
            collect_instantiated_conjuncts(program, *fact, &map, &mut equations);
            for (left, right) in equations {
                judge.intake_equation(left, right, 0);
            }
        }
    }
}

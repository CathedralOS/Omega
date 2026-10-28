//! Shared operator selection, crash and realization contract queries.
//!
//! These read authored declarations and value origins; they do not establish
//! a checked judgment. Both compile-time preparation and checking use them.

use checked_trees::{CheckedOperatorFacts, CheckedValueFacts};
use typed_trees::expression::ExpressionNode;

/// Derive selections over the typed graph without claiming contract or flow proof.
/// The caller must still check the selected executable program before execution.
pub fn derive_pre_flow_operator_selections(
    program: &dyn crate::CheckingInput,
) -> CheckedOperatorFacts {
    let values = crate::collect_executable_value_origins(
        program,
        &typed_trees::references::DeclarationSelection::all(),
    );
    build_operator_facts(program, &values)
}

pub fn build_operator_facts(
    program: &dyn crate::CheckingInput,
    values: &CheckedValueFacts,
) -> CheckedOperatorFacts {
    crate::build_operator_uses(program, values)
        .with_operator_crash_contracts(derive_checked_operator_crash_contracts(program))
        .with_operator_realization_contracts(derive_checked_operator_realization_contracts(program))
}

pub fn derive_checked_operator_realization_contracts(
    program: &dyn crate::CheckingInput,
) -> Vec<checked_trees::CheckedOperatorRealizationContract> {
    let mut rows = Vec::new();
    for machine in program.machines() {
        if program.machine_states(machine).first().is_none() {
            continue;
        }
        for conformance in program.machine_trait_conformances(machine) {
            if program
                .trait_definition_by_symbol(conformance.symbol)
                .is_some()
            {
                continue;
            }
            let Some(operator) = typed_trees::operator::declaration_by_symbol(
                program,
                conformance.requirement_symbol,
            ) else {
                continue;
            };
            rows.push(checked_trees::CheckedOperatorRealizationContract::new(
                machine.symbol,
                operator.symbol,
            ));
        }
    }
    rows.sort_by_key(|row| {
        (
            row.machine_symbol().arena_index(),
            row.machine_symbol().generation(),
            row.operator_symbol().arena_index(),
            row.operator_symbol().generation(),
        )
    });
    rows
}

pub fn derive_checked_operator_crash_contracts(
    program: &dyn crate::CheckingInput,
) -> Vec<checked_trees::CheckedOperatorCrashContract> {
    use std::collections::BTreeMap;
    use typed_trees::{domain::ProofFact, signature::SignatureContractKind};

    #[derive(Default)]
    struct Bucket {
        unconditional: bool,
        facts: Vec<arena::Handle<ProofFact>>,
    }

    let operators = program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    );
    let mut rows = operators
        .map(|operator| {
            let mut buckets = BTreeMap::<checked_trees::CrashCause, Bucket>::new();
            for contract in program.operator_contracts(operator) {
                let SignatureContractKind::Crashes { cause } = contract.kind else {
                    continue;
                };
                let cause = checked_trees::CrashCause::from(cause);
                let bucket = buckets.entry(cause).or_default();
                if contract.facts.is_empty() {
                    bucket.unconditional = true;
                    continue;
                }
                for fact in contract.facts.handles() {
                    let is_true = matches!(
                        program.proof_facts.get(fact),
                        ProofFact::Expression(expression)
                            if matches!(
                                program.expression_table.expression(*expression),
                                ExpressionNode::Boolean(true)
                            )
                    );
                    if is_true {
                        bucket.unconditional = true;
                    } else {
                        bucket.facts.push(fact);
                    }
                }
            }
            let buckets = buckets
                .into_iter()
                .map(|(cause, bucket)| {
                    checked_trees::CheckedOperatorCrashBucket::new(
                        cause,
                        bucket.unconditional,
                        if bucket.unconditional {
                            Vec::new()
                        } else {
                            bucket.facts
                        },
                    )
                })
                .collect();
            checked_trees::CheckedOperatorCrashContract::new(operator.symbol, buckets)
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| {
        (
            row.operator_symbol().arena_index(),
            row.operator_symbol().generation(),
        )
    });
    rows
}

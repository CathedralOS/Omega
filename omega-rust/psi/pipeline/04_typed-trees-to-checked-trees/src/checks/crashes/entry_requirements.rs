//! Invocation-entry hypotheses remain facts about the original operands even
//! after body storage changes. Named-state requirements describe an arrival,
//! not the invocation snapshot, and must never enter this collection.

use checked_trees::{CheckedOperatorFacts, CrashPredicateIdentity};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
};

mod consequences;

#[derive(Default)]
pub(super) struct EntryRequirements {
    pub(super) conjuncts: Vec<CrashPredicateIdentity>,
    pub(super) consequences: Vec<CrashPredicateIdentity>,
}

pub(super) fn collect(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operators: &CheckedOperatorFacts,
    parameter_names: &[String],
    content_conservation: &[validation::ContentConservationSourcePlan],
    integer_types: &super::IntegerTypeClassification,
) -> EntryRequirements {
    let mut result = EntryRequirements::default();
    let Some(entry) = program.machine_states(machine).first() else {
        return result;
    };
    let mut common = consequences::Consequences::new(
        program,
        parameter_names,
        content_conservation,
        integer_types,
    );
    for contract in program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if !has_exact_entry_meaning(program, machine, operators, parameter_names, *expression) {
                continue;
            }
            result
                .conjuncts
                .push(crate::facts::canonical_crash_path_predicate(
                    program,
                    *expression,
                    false,
                    parameter_names,
                    content_conservation,
                ));
            super::collect_structural_guard_consequences(
                program,
                *expression,
                false,
                parameter_names,
                content_conservation,
                integer_types,
                &mut result.consequences,
            );
            if let Some(consequences) = common.collect(*expression, false, 0) {
                result.consequences.extend(consequences);
            }
        }
    }
    result.consequences.sort_unstable();
    result.consequences.dedup();
    // Consequences already retain the exact polarity established at entry.
    // A published spelling such as `flag == false` is covered by `!flag`
    // only after independently checking that the published operator is also
    // builtin and its operands inhabit this exact entry namespace. Retain its
    // original identity as a consequence; never rewrite the public contract.
    for contract in program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .filter(|contract| {
            matches!(
                contract.kind,
                typed_trees::signature::SignatureContractKind::Crashes { .. }
            )
        })
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                continue;
            };
            if !has_exact_entry_meaning(program, machine, operators, parameter_names, *expression) {
                continue;
            }
            if common.establishes(*expression, false, &result.consequences, 0) == Some(true) {
                result
                    .consequences
                    .push(crate::facts::canonical_crash_path_predicate(
                        program,
                        *expression,
                        false,
                        parameter_names,
                        content_conservation,
                    ));
                result.consequences.sort_unstable();
                result.consequences.dedup();
            }
        }
    }
    result.conjuncts.sort();
    result.conjuncts.dedup();
    result.consequences.sort();
    result.consequences.dedup();
    result
}

fn has_exact_entry_meaning(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    operators: &CheckedOperatorFacts,
    parameter_names: &[String],
    expression: ExpressionHandle,
) -> bool {
    // This strict owner checks exact entry symbols, builtin operation meanings
    // and an acyclic live expression tree. Direct integer comparisons retain
    // their existing total numeric contract owner; partial arithmetic cannot
    // become a hypothesis merely because it appears in a Requires clause.
    if crate::values::lower_machine_entry_crash_contract_expression(
        program,
        operators,
        machine,
        expression,
        &[],
    )
    .is_none()
    {
        return false;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let parameters = program.state_parameters(entry);
    // The canonical identity encoder uses names for parameter ordinals.
    // The strict reader has already joined every observed root and field to
    // exact symbols. Require the encoder's entire authored namespace to agree,
    // including unread structural operands and the explicit receiver slot.
    parameter_names.len() == parameters.len()
        && parameter_names
            .iter()
            .zip(parameters)
            .all(|(name, parameter)| name == parameter.name.as_str())
}

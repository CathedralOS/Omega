//! Invocation-entry hypotheses remain facts about the original operands even
//! after body storage changes. Named-state requirements describe an arrival,
//! not the invocation snapshot, and must never enter this collection.

use checked_trees::{CheckedOperatorFacts, CrashPredicateIdentity};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator},
};

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
) -> EntryRequirements {
    let mut result = EntryRequirements::default();
    let Some(entry) = program.machine_states(machine).first() else {
        return result;
    };
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
                &mut result.consequences,
            );
        }
    }
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
            let Some((operand, negated)) = equivalent_boolean_polarity(program, *expression) else {
                continue;
            };
            let equivalent = crate::facts::canonical_crash_path_predicate(
                program,
                operand,
                negated,
                parameter_names,
                content_conservation,
            );
            if result.consequences.contains(&equivalent) {
                result
                    .consequences
                    .push(crate::facts::canonical_crash_path_predicate(
                        program,
                        *expression,
                        false,
                        parameter_names,
                        content_conservation,
                    ));
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
    // This strict owner checks Boolean-only operands, exact entry symbols,
    // builtin operation meanings and an acyclic live expression tree. Numeric
    // clauses retain their independent totality/proof owner.
    if crate::values::lower_machine_entry_scalar_contract_expression(
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
    // Require agreement with the already checked exact symbols before use.
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                let ordinal = members.first().and_then(|name| {
                    parameter_names
                        .iter()
                        .position(|candidate| candidate == name.as_str())
                });
                if ordinal
                    .and_then(|ordinal| parameters.get(ordinal))
                    .is_none_or(|parameter| parameter.symbol != path.symbol)
                {
                    return false;
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            _ => {}
        }
    }
    true
}

/// Peel equivalent builtin Boolean wrappers, not arbitrary consequences of
/// the requested route. In particular a conjunction's one conjunct cannot
/// stand in for the complete published conjunction.
fn equivalent_boolean_polarity(
    program: &TypedTrees,
    mut expression: ExpressionHandle,
) -> Option<(ExpressionHandle, bool)> {
    let mut negated = false;
    for _ in 0..64 {
        match program.expression_table.expression(expression) {
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                expression = unary.operand;
                negated = !negated;
            }
            ExpressionNode::Binary(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equal | BinaryOperator::NotEqual
                ) =>
            {
                let (operand, literal) = match (
                    program.expression_table.expression(binary.left),
                    program.expression_table.expression(binary.right),
                ) {
                    (ExpressionNode::Boolean(literal), _) => (binary.right, *literal),
                    (_, ExpressionNode::Boolean(literal)) => (binary.left, *literal),
                    _ => return Some((expression, negated)),
                };
                let equality_is_negated = if binary.operator == BinaryOperator::Equal {
                    negated
                } else {
                    !negated
                };
                negated = equality_is_negated == literal;
                expression = operand;
            }
            _ => return Some((expression, negated)),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    fn requirements(program: &TypedTrees, names: &[&str]) -> EntryRequirements {
        collect(
            program,
            &program.machines()[0],
            &CheckedOperatorFacts::default(),
            &names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>(),
            &[],
        )
    }

    #[test]
    fn only_boolean_entry_requirements_seed_crash_consequences() {
        let program = typed("machine value(mut flag: bool) -> bool\nrequires !flag\n{ flag }");
        assert!(!requirements(&program, &["flag"]).consequences.is_empty());
        let program = typed("machine value(input: u32) -> u32\nrequires input > 0\n{ input }");
        assert!(requirements(&program, &["input"]).consequences.is_empty());
        let program = typed(
            "machine value(flag: bool) -> bool { transition { _ -> finish(true) } state finish(flag: bool) -> bool\nrequires flag\n{ flag } }",
        );
        assert!(requirements(&program, &["flag"]).consequences.is_empty());
    }

    #[test]
    fn canonical_parameter_order_must_agree_with_exact_entry_symbols() {
        let program =
            typed("machine value(flag: bool, other: bool) -> bool\nrequires flag\n{ other }");
        assert!(
            !requirements(&program, &["flag", "other"])
                .consequences
                .is_empty()
        );
        assert!(
            requirements(&program, &["other", "flag"])
                .consequences
                .is_empty()
        );
        assert!(requirements(&program, &[]).consequences.is_empty());
    }

    #[test]
    fn authored_boolean_equality_is_not_builtin_requirement_evidence() {
        let program = typed(
            "boundary operator == bool::custom(left: bool, right: bool) -> bool;\nmachine value(flag: bool) -> bool\nrequires flag == true\n{ flag }",
        );
        assert!(requirements(&program, &["flag"]).consequences.is_empty());
    }

    #[test]
    fn published_equality_requires_its_own_exact_builtin_meaning() {
        for (carrier, covered) in [("bool", false), ("f64", true)] {
            let program = typed(&format!(
                "boundary operator == {carrier}::custom(left: {carrier}, right: {carrier}) -> bool;\n\
                 machine value(flag: bool) -> bool\nrequires !flag\ncrashes Trap flag == false\n{{ crash Trap; }}",
            ));
            let machine = &program.machines()[0];
            let route = program
                .machine_contracts(machine)
                .iter()
                .find(|contract| {
                    matches!(
                        contract.kind,
                        typed_trees::signature::SignatureContractKind::Crashes { .. }
                    )
                })
                .and_then(|contract| program.proof_facts.span_or_empty(contract.facts).first())
                .and_then(|fact| match fact {
                    typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                    _ => None,
                })
                .expect("exact published route");
            let identity = crate::facts::canonical_crash_path_predicate(
                &program,
                route,
                false,
                &["flag".to_owned()],
                &[],
            );
            assert_eq!(
                requirements(&program, &["flag"])
                    .consequences
                    .contains(&identity),
                covered,
                "{carrier} equality"
            );
        }
    }
}

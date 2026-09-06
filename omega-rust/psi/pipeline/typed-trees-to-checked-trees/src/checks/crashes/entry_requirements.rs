//! Invocation-entry hypotheses remain facts about the original operands even
//! after body storage changes. Named-state requirements describe an arrival,
//! not the invocation snapshot, and must never enter this collection.

use checked_trees::{CheckedOperatorFacts, CrashPredicateIdentity};
use typed_trees::{TypedTrees, expression::ExpressionNode};

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
    let parameters = program.state_parameters(entry);
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
            // This strict reader checks Boolean-only source operands, exact
            // entry formal symbols, builtin meanings and an acyclic live tree.
            // Numeric contracts retain their independent totality/proof owner.
            if crate::values::lower_machine_entry_scalar_contract_expression(
                program,
                operators,
                machine,
                *expression,
                &[],
            )
            .is_none()
            {
                continue;
            }
            // The existing canonical consequence encoder uses names to encode
            // parameter ordinals. Require agreement with the checked symbols;
            // a spelling mismatch cannot recover or substitute another formal.
            let mut pending = vec![*expression];
            let mut compatible = true;
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
                            compatible = false;
                            break;
                        }
                    }
                    ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
                    ExpressionNode::Unary(unary) => pending.push(unary.operand),
                    _ => {}
                }
            }
            if !compatible {
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
    result.conjuncts.sort();
    result.conjuncts.dedup();
    result.consequences.sort();
    result.consequences.dedup();
    result
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
}

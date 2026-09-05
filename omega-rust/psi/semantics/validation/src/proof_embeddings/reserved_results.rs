//! Rejoin reserved `result` occurrences to their exact authored contract owner.

use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn type_reference(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    reserved_result_owner(program, expression).map(|(_, type_reference)| type_reference)
}

/// Identify the exact machine owning a reserved result occurrence. Matching
/// carrier types do not establish ownership, and an authored parameter named
/// `result` takes precedence over the reserved contract form.
pub(crate) fn reserved_result_owner(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(symbols::SymbolHandle, TypeReferenceHandle)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    if !matches!(program.expression_table.name_path_members(path.members),
        [name] if name.as_str() == "result")
    {
        return None;
    }

    let mut owner = None;
    for machine in program.machines() {
        let Some(entry) = program.machine_states(machine).first() else {
            continue;
        };
        for contract in program.machine_contracts(machine) {
            let mut nodes = Vec::new();
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(root) => {
                        super::collect_expression_nodes(program, *root, &mut nodes);
                    }
                    ProofFact::Membership(membership) => {
                        super::collect_expression_nodes(program, membership.value, &mut nodes);
                    }
                    ProofFact::Proposition(application) => {
                        for argument in program
                            .expression_table
                            .expression_handles(application.arguments)
                        {
                            super::collect_expression_nodes(program, *argument, &mut nodes);
                        }
                    }
                }
            }
            if !nodes.contains(&expression) {
                continue;
            }
            // Spelling is only the reserved-form discriminator. The full
            // expression handle must belong to this owning ensures clause;
            // another machine with an equal result type is not an owner.
            if contract.kind != SignatureContractKind::Ensures
                || !entry.return_type.is_valid()
                || program
                    .state_parameters(entry)
                    .iter()
                    .any(|parameter| parameter.name.as_str() == "result")
            {
                return None;
            }
            if owner.is_some_and(|(symbol, _)| symbol != machine.symbol) {
                return None;
            }
            owner = Some((machine.symbol, entry.return_type));
        }
    }
    owner
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

    fn result_occurrences(program: &TypedTrees) -> Vec<ExpressionHandle> {
        program
            .expression_table
            .iter_expressions()
            .filter_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Name(path)
                if matches!(program.expression_table.name_path_members(path.members),
                    [name] if name.as_str() == "result"))
                .then_some(handle)
            })
            .collect()
    }

    #[test]
    fn equal_result_carriers_keep_distinct_exact_contract_owners() {
        let program = typed(
            "machine first(input: u16) -> u16 ensures result == input { input }
             machine second(input: u16) -> u16 ensures result == input { input }",
        );
        let occurrences = result_occurrences(&program);
        assert_eq!(occurrences.len(), 2);
        let owners = occurrences
            .iter()
            .map(|expression| {
                let owner = reserved_result_owner(&program, *expression).unwrap();
                assert_eq!(type_reference(&program, *expression), Some(owner.1));
                assert_eq!(
                    program.primitive_type_reference(owner.1),
                    Some(typed_trees::types::PrimitiveType::U16)
                );
                owner.0
            })
            .collect::<Vec<_>>();
        assert_ne!(owners[0], owners[1]);
        for machine in program.machines() {
            assert!(owners.contains(&machine.symbol));
        }
    }

    #[test]
    fn authored_result_parameter_shadows_the_reserved_form() {
        let program =
            typed("machine identity(result: u16) -> u16 ensures result == result { result }");
        let occurrences = result_occurrences(&program);
        assert!(!occurrences.is_empty());
        for expression in occurrences {
            assert_eq!(reserved_result_owner(&program, expression), None);
            assert_eq!(type_reference(&program, expression), None);
        }
    }

    #[test]
    fn result_spelling_outside_ensures_has_no_reserved_owner() {
        let program =
            typed("machine invalid(input: u16) -> u16 requires result == input { input }");
        let occurrences = result_occurrences(&program);
        assert_eq!(occurrences.len(), 1);
        assert_eq!(reserved_result_owner(&program, occurrences[0]), None);
    }
}

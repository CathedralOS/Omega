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
    owner.map(|(_, type_reference)| type_reference)
}

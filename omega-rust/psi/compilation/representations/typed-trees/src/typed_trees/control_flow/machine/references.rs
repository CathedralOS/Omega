//! Rewrite references within one machine, never unrelated arena residents.

use crate::TypedTrees;
use crate::domain::ProofFact;
use crate::machine::Machine;
use crate::signature::SignatureContract;
use arena::HandleSpan;
use symbols::SymbolHandle;

impl TypedTrees {
    /// Retarget a machine's body, contract and `via` references to replacement states.
    /// The declaration's own symbols and other machines remain unchanged.
    /// Callers supply exact state identities, not name-based substitutions.
    pub fn retarget_machine_calls(
        &mut self,
        machine: &Machine,
        replacements: &[(SymbolHandle, SymbolHandle)],
    ) {
        if replacements.is_empty() {
            return;
        }
        self.retarget_contract_calls(machine.contracts, replacements);
        // Walk declarations, not the selected conformance roster: ownership
        // of an expression does not depend on whether this body is realized.
        let via_roots = self
            .declared_machine_trait_conformances(machine)
            .iter()
            .map(|conformance| conformance.via_expression)
            .collect::<Vec<_>>();
        for root in via_roots {
            self.expression_table.remap_symbols_in(root, replacements);
        }
        for owned in self.machine_owned_data(machine).to_vec() {
            self.expression_table
                .remap_symbols_in(owned.initial_value, replacements);
        }
        for state in self.machine_states(machine).to_vec() {
            self.retarget_contract_calls(state.contracts, replacements);
            let tables = &mut self.tables;
            tables.statement_table.remap_symbols_in(
                state.statement_nodes,
                &mut tables.expression_table,
                &mut tables.type_reference_table,
                replacements,
            );
        }
    }

    fn retarget_contract_calls(
        &mut self,
        contracts: HandleSpan<SignatureContract>,
        replacements: &[(SymbolHandle, SymbolHandle)],
    ) {
        let roots = self
            .signature_contracts
            .span_or_empty(contracts)
            .iter()
            .flat_map(|contract| self.proof_facts.span_or_empty(contract.facts))
            .flat_map(|fact| match fact {
                ProofFact::Expression(expression) => std::slice::from_ref(expression),
                ProofFact::Membership(membership) => std::slice::from_ref(&membership.value),
                ProofFact::Proposition(application) => self
                    .expression_table
                    .expression_handles(application.arguments),
            })
            .copied()
            .collect::<Vec<_>>();
        for root in roots {
            self.expression_table.remap_symbols_in(root, replacements);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::{ExpressionNode, TableAtomicExpression, TableNamePath};
    use crate::state::State;
    use crate::statement::StatementNode;
    use language_core::atomic::{
        AtomicExpressionResultCustody, AtomicOrderingPlan, MemoryOrdering,
    };

    #[test]
    fn scoped_retargeting_visits_declared_via_roots_without_touching_other_roots() {
        for target in [
            None,
            Some(crate::name::Identifier::generated("demo_target")),
        ] {
            let mut program = TypedTrees::default();
            let original = SymbolHandle::from_arena_index(1);
            let replacement = SymbolHandle::from_arena_index(2);
            let reference = ExpressionNode::Name(TableNamePath {
                members: HandleSpan::empty(),
                member_symbols: HandleSpan::empty(),
                head_symbol: original,
                symbol: original,
            });
            let via = program.expression_table.insert(reference.clone());
            let unrelated = program.expression_table.insert(reference);
            let mut machine = Machine {
                target,
                ..Machine::default()
            };
            program.push_machine_trait_conformance(
                &mut machine,
                crate::machine::TraitConformance {
                    via_expression: via,
                    ..Default::default()
                },
            );
            program.retarget_machine_calls(&machine, &[(original, replacement)]);
            for (root, expected) in [(via, replacement), (unrelated, original)] {
                let ExpressionNode::Name(path) = program.expression_table.expression(root) else {
                    panic!("retargeting preserves the expression shape");
                };
                assert_eq!(path.symbol, expected);
            }
        }
    }

    #[test]
    fn scoped_retargeting_visits_atomic_results_without_touching_other_roots() {
        let mut program = TypedTrees::default();
        let original = SymbolHandle::from_arena_index(1);
        let replacement = SymbolHandle::from_arena_index(2);
        let reference = ExpressionNode::Name(TableNamePath {
            members: HandleSpan::empty(),
            member_symbols: HandleSpan::empty(),
            head_symbol: original,
            symbol: original,
        });
        let result = program.expression_table.insert(reference.clone());
        let unrelated = program.expression_table.insert(reference);
        let value = program
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let atomic =
            program
                .expression_table
                .insert(ExpressionNode::Atomic(TableAtomicExpression {
                    value,
                    result,
                    ordering: AtomicOrderingPlan::Load(MemoryOrdering::Receive),
                    result_custody: AtomicExpressionResultCustody::Scalar,
                }));
        let mut state = State::default();
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(atomic),
        );
        let mut machine = Machine::default();
        program.push_machine_state(&mut machine, state);
        program.retarget_machine_calls(&machine, &[(original, replacement)]);
        for (root, expected) in [(result, replacement), (unrelated, original)] {
            let ExpressionNode::Name(path) = program.expression_table.expression(root) else {
                panic!("retargeting preserves the expression shape");
            };
            assert_eq!(path.symbol, expected);
        }
    }
}

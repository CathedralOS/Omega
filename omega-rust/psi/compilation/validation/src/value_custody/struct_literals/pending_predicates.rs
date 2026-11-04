//! A record's bounds can depend on reads through a saved reference. Keep an
//! unresolved construction proof until the storage graph can describe those
//! reads. It remains an obligation: the constructor's result cannot prove it,
//! and every pending predicate must succeed before checked publication.

use crate::frozen_program::expression_table;
use crate::proof_contracts::contract_entailment::{
    construction_predicate_judgments, structural_judgment::StructuralJudgment,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingConstructionPredicate {
    pub(super) predicate: ExpressionHandle,
    pub(super) diagnostic: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingConstructionPredicates {
    pub(super) machine: SymbolHandle,
    pub(super) state: SymbolHandle,
    pub(super) statement_index: usize,
    pub(super) expression: ExpressionHandle,
    pub(super) parameters: Vec<StateParameter>,
    pub(super) predicates: Vec<PendingConstructionPredicate>,
}

pub(crate) fn discharge_pending_predicates(
    program: &dyn crate::CheckingInput,
    pending: &[PendingConstructionPredicates],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for construction in pending {
        let predicates = construction
            .predicates
            .iter()
            .map(|obligation| obligation.predicate)
            .collect::<Vec<_>>();
        let judgments = program
            .machine(construction.machine)
            .and_then(|machine| {
                let state = program.state_in_machine(machine, construction.state)?;
                let ExpressionNode::StructLiteral(literal) =
                    expression_table(program).expression(construction.expression)
                else {
                    return None;
                };
                // The existing operation visitor stops before the result. Its
                // captured children supply the premises, not the record's type.
                Some(construction_predicate_judgments(
                    program,
                    machine,
                    state,
                    construction.statement_index,
                    construction.expression,
                    literal,
                    &construction.parameters,
                    &predicates,
                ))
            })
            .unwrap_or_default();
        for (index, obligation) in construction.predicates.iter().enumerate() {
            if !matches!(judgments.get(index), Some(StructuralJudgment::Proven)) {
                diagnostics.push(
                    Diagnostic::error(obligation.diagnostic.clone()).with_source_span(
                        expression_table(program).source_span(construction.expression),
                    ),
                );
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

//! Predicates at one admitted, executed statement prefix.
//!
//! The existing structural prefix owns input domains, operand ordering, calls
//! and storage revisions. This observer supplies a question, never a premise.
//! Its point precedes the statement itself; later operands or exits cannot
//! contribute observations to that question.

use super::structural_judgment::StructuralJudgment;
use super::structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
use typed_trees::domain::ProofFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::PrimitiveType;
use typed_trees::{machine::Machine, state::State};

/// Ordinary Boolean questions over the current values of a checked prefix.
pub struct StatementPredicateObservations<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    acyclic: std::cell::Cell<Option<bool>>,
}

impl<'program> StatementPredicateObservations<'program> {
    pub fn new(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
    ) -> Self {
        Self {
            program,
            machine,
            state,
            acyclic: std::cell::Cell::new(None),
        }
    }

    /// Prove an actual Boolean root at its own pre-statement frontier.
    /// A question borrowed from a different statement or an outgoing-operand
    /// frontier needs that owner's observation and cannot use this one.
    pub fn proves(&self, statement_index: usize, expression: ExpressionHandle) -> bool {
        let Some(statement) = self
            .program
            .statement_table
            .statement_at(self.state, statement_index)
        else {
            return false;
        };
        let owns_goal = self
            .program
            .tables
            .expression_roots(statement)
            .into_iter()
            .any(|(_, root)| root == expression)
            || statement.as_assembly_fact().is_some_and(|fact| {
                matches!(
                    self.program.proof_facts.get(fact.fact),
                    ProofFact::Expression(root) if *root == expression
                )
            });
        if !owns_goal
            || crate::expression_result_type_reference(
                self.program,
                self.machine,
                self.state,
                expression,
            )
            .and_then(|reference| self.program.primitive_type_reference(reference))
                != Some(PrimitiveType::Bool)
        {
            return false;
        }
        let mut query = Predicate {
            site: StructuralPrefixSite {
                state: self.state.symbol,
                statement_index,
                in_transition_target: false,
            },
            expression,
            seen: false,
            proven: true,
        };
        let acyclic = self.acyclic.get().unwrap_or_else(|| {
            let value = self
                .program
                .machine_states(self.machine)
                .first()
                .is_some_and(|entry| {
                    crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
                        self.program,
                        self.machine,
                        entry,
                    )
                });
            self.acyclic.set(Some(value));
            value
        });
        let complete = if acyclic {
            super::structural_prefix::observe_acyclic_invocation_prefix(
                self.program,
                self.machine,
                &mut query,
            )
        } else {
            super::structural_prefix::observe_runtime_state_prefix(
                self.program,
                self.machine,
                self.state,
                &mut query,
            )
        };
        complete.is_some() && query.seen && query.proven
    }
}

struct Predicate {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for Predicate {
    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (_program, machine, state, judge, environment) = observation;
        let Some(predicate) = judge.predicate_term_in_scope(
            (machine, state, state.return_type),
            self.expression,
            environment,
        ) else {
            self.proven = false;
            return;
        };
        self.proven &= matches!(
            judge.judge_observed_predicate(predicate, 0),
            StructuralJudgment::Proven
        );
    }
}

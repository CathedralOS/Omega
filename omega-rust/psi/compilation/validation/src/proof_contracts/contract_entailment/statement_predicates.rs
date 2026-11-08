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

/// Formed declaration predicates at the invocation entry, without body facts
/// or private caller observations. A crash condition is a question here; it
/// never becomes one of the entry hypotheses used to answer itself.
pub struct EntryPredicateObservations<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    judge: std::cell::OnceCell<super::structural_judgment::StructuralJudge<'program>>,
}

impl<'program> EntryPredicateObservations<'program> {
    pub fn new(program: &'program dyn crate::CheckingInput, machine: &'program Machine) -> Self {
        Self {
            program,
            machine,
            judge: std::cell::OnceCell::new(),
        }
    }

    fn judge(&self) -> &super::structural_judgment::StructuralJudge<'program> {
        self.judge.get_or_init(|| {
            let entry = self.program.machine_states(self.machine).first();
            let requires = self
                .program
                .machine_contracts(self.machine)
                .iter()
                .chain(
                    entry
                        .into_iter()
                        .flat_map(|state| self.program.state_contracts(state)),
                )
                .filter(|contract| {
                    contract.kind == typed_trees::signature::SignatureContractKind::Requires
                })
                .flat_map(|contract| self.program.proof_facts.span_or_empty(contract.facts))
                .filter_map(|fact| fact.as_expression().copied())
                .collect::<Vec<_>>();
            super::structural_judgment::StructuralJudge::from_runtime_requires(
                self.program,
                self.machine,
                &requires,
            )
        })
    }

    pub fn proves_crash_condition(&self, expression: ExpressionHandle) -> bool {
        let Some(entry) = self.program.machine_states(self.machine).first() else {
            return false;
        };
        let owned = self
            .program
            .machine_contracts(self.machine)
            .iter()
            .filter(|contract| {
                matches!(
                    contract.kind,
                    typed_trees::signature::SignatureContractKind::Crashes { .. }
                )
            })
            .flat_map(|contract| self.program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| fact.as_expression() == Some(&expression));
        owned
            && matches!(
                self.judge().judge_predicate_in_scope(
                    (self.machine, entry, entry.return_type),
                    expression,
                    &[],
                    0
                ),
                StructuralJudgment::Proven
            )
    }
}

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
        self.proven &= matches!(
            judge.judge_predicate_in_scope(
                (machine, state, state.return_type),
                self.expression,
                environment,
                0,
            ),
            StructuralJudgment::Proven
        );
    }
}

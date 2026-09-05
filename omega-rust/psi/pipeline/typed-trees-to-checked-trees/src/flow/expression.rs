//! Chapter 5 execution order for statement and transition expression effects.
//!
//! Borrow call ordinals identify authored occurrences; their preorder is not
//! execution order. This traversal owns invocation scheduling, not arithmetic
//! interpretation or callee execution.

use super::*;
use typed_trees::expression::BinaryOperator;
use typed_trees::statement::{TableTransition, TransitionTargetHandle, TransitionTargetNode};

mod transitions;

#[cfg(test)]
mod tests;

struct Invocation<'a> {
    call: &'a BorrowCallFact,
    site: InvocationSite,
    visited: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InvocationSite {
    Expression(ExpressionHandle),
    Statement,
    Transition(TransitionTargetHandle),
}

pub(super) struct Execution<'a, 'b, 'plans> {
    program: &'a typed_trees::TypedTrees,
    borrow: &'a BorrowFacts,
    proof: &'a ProofFacts,
    domains: &'a DomainFacts,
    machine: &'a typed_trees::machine::Machine,
    state: &'a typed_trees::state::State,
    statement_index: usize,
    pub(super) semantic: &'b mut FactPlan,
    pub(super) context: &'b mut FlowBuildContext<'plans>,
    state_calls: &'b mut HandleSpan<FlowCallFact>,
    invocations: Vec<Invocation<'a>>,
    operand_writes: Vec<Option<Vec<CanonicalPlace>>>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_statement_calls(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    context: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    calls: &[BorrowCallFact],
    state_calls: &mut HandleSpan<FlowCallFact>,
    contexts: &mut HandleSpan<FlowSemanticContextRef>,
    constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let mut execution = Execution::new(
        program,
        borrow,
        proof,
        domains,
        machine,
        state,
        statement_index,
        semantic,
        context,
        state_calls,
        calls,
        contexts,
        constraints,
    );
    match statement {
        StatementNode::Assignment(assignment) => {
            execution.expression(assignment.value, contexts, constraints);
        }
        StatementNode::LocalData(local) => {
            execution.expression(local.initial_value, contexts, constraints);
        }
        StatementNode::Expression(expression) => {
            execution.expression(*expression, contexts, constraints);
        }
        StatementNode::Call(call) => {
            let mut operands = Vec::new();
            for argument in program.statement_table.expression_handles(call.arguments) {
                operands.push((*argument, execution.operand_writes.len()));
                execution.expression(*argument, contexts, constraints);
            }
            execution.invoke(InvocationSite::Statement, &operands, contexts, constraints);
        }
        StatementNode::AssemblyFact(_) | StatementNode::Transition(_) => {}
    }
}

impl<'a, 'b, 'plans> Execution<'a, 'b, 'plans> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        program: &'a typed_trees::TypedTrees,
        borrow: &'a BorrowFacts,
        proof: &'a ProofFacts,
        domains: &'a DomainFacts,
        machine: &'a typed_trees::machine::Machine,
        state: &'a typed_trees::state::State,
        statement_index: usize,
        semantic: &'b mut FactPlan,
        context: &'b mut FlowBuildContext<'plans>,
        state_calls: &'b mut HandleSpan<FlowCallFact>,
        calls: &'a [BorrowCallFact],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> Self {
        let mut malformed = false;
        let invocations = calls
            .iter()
            .filter_map(|call| {
                let site = match find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    call.call_ordinal,
                ) {
                    Some(CallSite::Expression {
                        expression,
                        call: source,
                    }) => {
                        let (receiver, path) =
                            crate::lookup::call_receiver_parts(program, source.receiver);
                        let target = crate::lookup::resolve_state_call_target(
                            program,
                            machine,
                            state,
                            receiver,
                            source.target_symbol,
                            path.as_deref(),
                            &source.target,
                        );
                        if call.receiver_symbol != receiver
                            || call.target_symbol
                                != if target.is_valid() {
                                    target
                                } else {
                                    source.target_symbol
                                }
                            || !program.expression_table.expression_is_valid(expression)
                        {
                            malformed = true;
                            return None;
                        }
                        InvocationSite::Expression(expression)
                    }
                    Some(CallSite::Statement(_)) => InvocationSite::Statement,
                    Some(CallSite::TransitionNamed { .. }) => {
                        let Some(target) = crate::semantic_calls::transition_call_target(
                            program,
                            machine,
                            state,
                            statement_index,
                            call.call_ordinal,
                        )
                        .filter(|target| target.is_valid()) else {
                            malformed = true;
                            return None;
                        };
                        InvocationSite::Transition(target)
                    }
                    _ => {
                        malformed = true;
                        return None;
                    }
                };
                Some(Invocation {
                    call,
                    site,
                    visited: false,
                })
            })
            .collect();
        if malformed {
            // Missing occurrence custody cannot retain pre-call storage promises.
            *contexts = HandleSpan::empty();
            *constraints = HandleSpan::empty();
        }
        Self {
            program,
            borrow,
            proof,
            domains,
            machine,
            state,
            statement_index,
            semantic,
            context,
            state_calls,
            invocations,
            operand_writes: Vec::new(),
        }
    }

    fn invoke(
        &mut self,
        site: InvocationSite,
        operands: &[(ExpressionHandle, usize)],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let Some(invocation) = self
            .invocations
            .iter_mut()
            .find(|invocation| !invocation.visited && invocation.site == site)
        else {
            return;
        };
        invocation.visited = true;
        let borrowed_call = invocation.call;
        let captured_sources = self.changed_operand_sources(operands);
        self.filter_captured_sources(&captured_sources, contexts, constraints);
        let writes = super::call_phases::call_storage_writes(
            self.program,
            self.borrow,
            self.context,
            self.machine,
            self.state,
            borrowed_call,
        );
        let mut call = build_call_flow_fact(
            self.program,
            self.borrow,
            self.proof,
            self.semantic,
            self.domains,
            self.context,
            self.machine,
            self.state,
            contexts,
            constraints,
            borrowed_call,
        );
        self.operand_writes.push(writes);
        // A by-value argument is not the storage value after a later operand
        // changed it. Do not republish that substitution through callee ensures.
        self.filter_captured_sources(&captured_sources, contexts, constraints);
        call.exit_semantic_contexts = *contexts;
        call.exit_constraints = *constraints;
        self.context
            .control
            .calls
            .append_to_span(self.state_calls, call);
    }

    pub(super) fn expression(
        &mut self,
        expression: ExpressionHandle,
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> Option<bool> {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return None;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => return Some(*value),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                if let ExpressionNode::Member(member) =
                    self.program.expression_table.expression(expression)
                {
                    self.expression(member.receiver, contexts, constraints);
                }
                return self.live_boolean(expression, *contexts);
            }
            ExpressionNode::Binary(binary) => {
                let left = self.expression(binary.left, contexts, constraints);
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let evaluate_when = binary.operator == BinaryOperator::And;
                    if left == Some(!evaluate_when) {
                        return left;
                    }
                    if left == Some(evaluate_when) {
                        return self.expression(binary.right, contexts, constraints);
                    }
                    let skipped_contexts = *contexts;
                    let skipped_constraints = *constraints;
                    super::exits::append_predicate_context(
                        self.program,
                        self.semantic,
                        self.context,
                        self.state.symbol,
                        self.statement_index,
                        binary.left,
                        evaluate_when,
                        ProgramPoint::Statement {
                            machine_symbol: self.machine.symbol,
                            state_symbol: self.state.symbol,
                            statement_index: self.statement_index,
                        },
                        contexts,
                        constraints,
                    );
                    let right = self.expression(binary.right, contexts, constraints);
                    self.meet(skipped_contexts, skipped_constraints, contexts, constraints);
                    return (right == Some(!evaluate_when)).then_some(!evaluate_when);
                }
                self.expression(binary.right, contexts, constraints);
            }
            ExpressionNode::Call(call) => {
                let mut operands = vec![(call.receiver, self.operand_writes.len())];
                self.expression(call.receiver, contexts, constraints);
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    operands.push((*argument, self.operand_writes.len()));
                    self.expression(*argument, contexts, constraints);
                }
                self.invoke(
                    InvocationSite::Expression(expression),
                    &operands,
                    contexts,
                    constraints,
                );
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.expression(*value, contexts, constraints);
                }
            }
            ExpressionNode::StructLiteral(value) => {
                for field in self.program.expression_table.struct_fields(value.fields) {
                    self.expression(field.value, contexts, constraints);
                }
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start, contexts, constraints);
                self.expression(range.end, contexts, constraints);
            }
            ExpressionNode::Indexed(indexed) => {
                self.expression(indexed.collection, contexts, constraints);
                self.expression(indexed.index, contexts, constraints);
            }
            ExpressionNode::Atomic(atomic) => {
                self.expression(atomic.value, contexts, constraints);
            }
            ExpressionNode::Cast(cast) => {
                self.expression(cast.value, contexts, constraints);
            }
            ExpressionNode::Borrow(borrow) => {
                self.expression(borrow.target, contexts, constraints);
            }
            ExpressionNode::Unary(unary) => {
                self.expression(unary.operand, contexts, constraints);
            }
            ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
        None
    }

    fn live_boolean(
        &mut self,
        expression: ExpressionHandle,
        contexts: HandleSpan<FlowSemanticContextRef>,
    ) -> Option<bool> {
        let place = canonical_place_from_expression_in_state(
            self.program,
            self.state.symbol,
            self.statement_index,
            expression,
        )?;
        match crate::values::scalar_value_at_place(
            self.program,
            self.semantic,
            self.context
                .contexts
                .semantic_context_refs
                .span_or_empty(contexts)
                .iter()
                .map(|reference| self.semantic.contexts.get(reference.context)),
            &place,
        )? {
            facts::ScalarValue::Boolean(value) => Some(value),
            _ => None,
        }
    }

    fn changed_operand_sources(
        &self,
        operands: &[(ExpressionHandle, usize)],
    ) -> Vec<CanonicalPlace> {
        let mut changed = Vec::new();
        for (expression, first_write) in operands {
            // A direct call result is an occurrence value, not the current
            // storage of its own arguments. Its guarantees have separate custody.
            if matches!(
                self.program.expression_table.expression(*expression),
                ExpressionNode::Call(_)
            ) {
                continue;
            }
            let mut occurrences = Vec::new();
            crate::contract_occurrences::append_expression_occurrences(
                self.program,
                *expression,
                &mut occurrences,
            );
            for occurrence in occurrences {
                let Some(place) = canonical_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    occurrence,
                ) else {
                    continue;
                };
                let overlaps = self.operand_writes[*first_write..].iter().any(|writes| {
                    writes.as_ref().is_none_or(|writes| {
                        writes.iter().any(|write| {
                            super::ownership::normalized_event_place_root(self.program, place.root)
                                == super::ownership::normalized_event_place_root(
                                    self.program,
                                    write.root,
                                )
                                && canonical_place_segments_may_overlap(
                                    self.program,
                                    &place.segments,
                                    &write.segments,
                                )
                        })
                    })
                });
                if overlaps && !changed.contains(&place) {
                    changed.push(place);
                }
            }
        }
        changed
    }

    fn filter_captured_sources(
        &mut self,
        changed: &[CanonicalPlace],
        contexts: &mut HandleSpan<FlowSemanticContextRef>,
        constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        *contexts = filter_contexts_after_place_mutations(
            self.program,
            self.semantic,
            self.domains,
            &mut self.context.contexts.semantic_context_refs,
            &mut self.context.invalidations.segments,
            &mut self.context.invalidations.events,
            *contexts,
            changed,
            FlowInvalidationSource::Statement {
                statement_index: self.statement_index,
            },
        );
        *constraints = project_constraint_refs_to_active_contexts(
            &mut self.context.contexts.constraint_refs,
            *constraints,
            *contexts,
            &self.context.contexts.semantic_context_refs,
        );
    }

    fn meet(
        &mut self,
        skipped: HandleSpan<FlowSemanticContextRef>,
        skipped_constraints: HandleSpan<FlowConstraintRef>,
        evaluated: &mut HandleSpan<FlowSemanticContextRef>,
        evaluated_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let retained: Vec<_> = self
            .context
            .contexts
            .semantic_context_refs
            .span_or_empty(skipped)
            .iter()
            .filter(|reference| {
                self.context
                    .contexts
                    .semantic_context_refs
                    .span_or_empty(*evaluated)
                    .contains(reference)
            })
            .copied()
            .collect();
        *evaluated = HandleSpan::empty();
        for reference in retained {
            self.context
                .contexts
                .semantic_context_refs
                .append_to_span(evaluated, reference);
        }
        let retained: Vec<_> = self
            .context
            .contexts
            .constraint_refs
            .span_or_empty(skipped_constraints)
            .iter()
            .filter(|reference| {
                self.context
                    .contexts
                    .constraint_refs
                    .span_or_empty(*evaluated_constraints)
                    .contains(reference)
            })
            .copied()
            .collect();
        *evaluated_constraints = HandleSpan::empty();
        for reference in retained {
            self.context
                .contexts
                .constraint_refs
                .append_to_span(evaluated_constraints, reference);
        }
        *evaluated_constraints = project_constraint_refs_to_active_contexts(
            &mut self.context.contexts.constraint_refs,
            *evaluated_constraints,
            *evaluated,
            &self.context.contexts.semantic_context_refs,
        );
    }
}

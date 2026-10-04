//! One source evaluation order for ownership windows and loan exclusions.
//! This inventory orders existing move candidates; it does not classify them.

use crate::flow::CanonicalPlace;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::statement::StatementNode;

/// Distinct arms of the same selection cannot both execute. An occurrence
/// after their join has no branch token and may follow any of their loans.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct EvaluationBranch {
    expression: ExpressionHandle,
    alternative: usize,
}

pub(super) struct StatementOccurrence {
    pub(super) step: StatementStep,
    pub(super) route: Vec<EvaluationBranch>,
}

pub(super) fn routes_compatible(left: &[EvaluationBranch], right: &[EvaluationBranch]) -> bool {
    left.iter().all(|left| {
        right.iter().all(|right| {
            left.expression != right.expression || left.alternative == right.alternative
        })
    })
}

/// Ownership and loan checks replay evaluated occurrences, not a set of mentioned places.
/// This is scratch ordering for the existing ownership events, not a second
/// ownership classifier. Each observation precedes its exact move; invocations
/// follow their operands, and the caller applies the repair after these steps.
pub(super) enum StatementStep {
    /// A read of one selected place by `expression`; the handle locates the
    /// read on its match arm's edge for the arm-local checks.
    Observe {
        place: CanonicalPlace,
        expression: ExpressionHandle,
    },
    /// `ordered` is false for a move no evaluated occurrence claimed — a
    /// production destination or an operand of an unreachable arm — which is
    /// appended after every ordered step.
    Move {
        event: usize,
        conditional: bool,
        ordered: bool,
    },
    Complete(ExpressionHandle),
}

pub(super) fn statement_steps(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    moves: &[crate::flow::DiscoveredMoveEvent],
    operators: &checked_trees::CheckedOperatorFacts,
) -> Vec<StatementOccurrence> {
    let mut order = StatementOrder {
        program,
        state,
        statement_index,
        operators,
        remaining: moves
            .iter()
            .enumerate()
            .filter(|(_, event)| match event.source {
                crate::flow::FlowOwnershipEventSource::Statement {
                    statement_index: source,
                }
                | crate::flow::FlowOwnershipEventSource::Call {
                    statement_index: source,
                    ..
                } => source == statement_index,
            })
            .collect(),
        steps: Vec::new(),
        route: Vec::new(),
    };
    match statement {
        StatementNode::LocalData(local) => order.expression(local.initial_value, false),
        StatementNode::Expression(expression) => order.expression(*expression, false),
        StatementNode::Assignment(assignment) => {
            order.expression(assignment.value, false);
            order.selectors(assignment.target, false);
        }
        StatementNode::Call(call) => {
            if let Some(receiver) = crate::flow::canonical_receiver_place_for_call_site(
                program,
                machine.symbol,
                state.symbol,
                &crate::semantic::calls::CallSite::Statement(call),
                statement_index,
            ) {
                order.push(StatementStep::Observe {
                    place: receiver,
                    expression: ExpressionHandle::invalid(),
                });
            }
            // A statement receiver has no expression handle. Its extraction
            // still precedes the first positional argument.
            order.complete_moves(ExpressionHandle::invalid(), false);
            for argument in program.statement_table.expression_handles(call.arguments) {
                order.expression(*argument, false);
            }
            order.push(StatementStep::Complete(ExpressionHandle::invalid()));
        }
        StatementNode::RootBinding(binding) => {
            order.expression(binding.receiver, false);
            order.expression(binding.implementation_operand, false);
        }
        StatementNode::AssemblyFact(fact) => order.expression(fact.expression, false),
        StatementNode::Transition(transition) => {
            if let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                order.expression(guard, false);
            }
            for (alternative, target) in [transition.target, transition.continuation]
                .into_iter()
                .enumerate()
            {
                if !target.is_valid() {
                    continue;
                }
                order.route.push(EvaluationBranch {
                    expression: ExpressionHandle::invalid(),
                    alternative,
                });
                match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Value(value) => {
                        order.expression(*value, true)
                    }
                    // Transition arguments live in the statement table's
                    // handle arena, as statement-call arguments do; the
                    // expression table's span at the same offset names
                    // unrelated expressions.
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            order.expression(*argument, true);
                        }
                        order.push(StatementStep::Complete(ExpressionHandle::invalid()));
                    }
                    _ => {}
                }
                order.route.pop();
            }
        }
    }
    // Production destinations have no evaluated source. Their existing caller
    // classification skips them. Any other unmatched move lacks ordering
    // custody and must not gain a borrowed-storage window.
    for (event, _) in std::mem::take(&mut order.remaining) {
        order.push(StatementStep::Move {
            event,
            conditional: true,
            ordered: false,
        });
    }
    order.steps
}

struct StatementOrder<'a> {
    program: &'a typed_trees::TypedTrees,
    state: &'a typed_trees::state::State,
    statement_index: usize,
    operators: &'a checked_trees::CheckedOperatorFacts,
    remaining: Vec<(usize, &'a crate::flow::DiscoveredMoveEvent)>,
    steps: Vec<StatementOccurrence>,
    route: Vec<EvaluationBranch>,
}

impl StatementOrder<'_> {
    fn push(&mut self, step: StatementStep) {
        self.steps.push(StatementOccurrence {
            step,
            route: self.route.clone(),
        });
    }

    fn complete_moves(&mut self, expression: ExpressionHandle, conditional: bool) {
        let mut position = 0;
        while position < self.remaining.len() {
            if self.remaining[position].1.expression == expression {
                let (event, _) = self.remaining.remove(position);
                self.push(StatementStep::Move {
                    event,
                    conditional,
                    ordered: true,
                });
            } else {
                position += 1;
            }
        }
    }

    fn expression(&mut self, expression: ExpressionHandle, conditional: bool) {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return;
        }
        // Later guards and payload receivers name the dispatch's saved
        // result. They neither invoke the producer nor observe its consumed
        // operands again; its canonical occurrence keeps the ordinary walk.
        if validation::saved_dispatch_subject(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )
        .is_some_and(|(statement, subject)| {
            statement != self.statement_index || subject != expression
        }) {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Indexed(indexed) if self.selected_index(expression) => {
                self.expression(indexed.collection, conditional);
                self.expression(indexed.index, conditional);
            }
            ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Borrow(_) => {
                // Reading a projection observes its maximal selected place,
                // not every enclosing owner. Computed receivers and selectors
                // still execute before that observation.
                self.selectors(expression, conditional);
                if let Some(place) = crate::flow::canonical_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                ) {
                    self.push(StatementStep::Observe { place, expression });
                }
            }
            ExpressionNode::Call(call) => {
                self.expression(call.receiver, conditional);
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.expression(*argument, conditional);
                }
            }
            ExpressionNode::Match(dispatch) => {
                self.expression(dispatch.subject, conditional);
                for (alternative, (_, arm)) in
                    super::multiplicity::reachable_arms(self.program, dispatch.arms)
                        .into_iter()
                        .enumerate()
                {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        self.expression(pattern, true);
                    }
                    self.route.push(EvaluationBranch {
                        expression,
                        alternative,
                    });
                    self.expression(arm.value, true);
                    self.route.pop();
                }
            }
            ExpressionNode::Binary(binary) => {
                self.expression(binary.left, conditional);
                if matches!(
                    binary.operator,
                    typed_trees::expression::BinaryOperator::And
                        | typed_trees::expression::BinaryOperator::Or
                ) {
                    let evaluate_when =
                        binary.operator == typed_trees::expression::BinaryOperator::And;
                    // Only closed selected values can skip a subtree here.
                    // Runtime bindings may have changed during earlier operands.
                    let left = crate::values::evaluate_closed_boolean_expression(
                        self.program,
                        self.operators,
                        binary.left,
                        &[],
                    );
                    if left != Some(!evaluate_when) {
                        self.expression(binary.right, conditional || left.is_none());
                    }
                } else {
                    self.expression(binary.right, conditional);
                }
            }
            ExpressionNode::Unary(unary) => self.expression(unary.operand, conditional),
            ExpressionNode::Cast(cast) => self.expression(cast.value, conditional),
            ExpressionNode::Atomic(atomic) => self.expression(atomic.value, conditional),
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.expression(*value, conditional);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.expression(field.value, conditional);
                }
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start, conditional);
                self.expression(range.end, conditional);
            }
            _ => {}
        }
        // Ordinary calls and selected operators use their retained exact
        // invocation rows; syntax alone cannot invent an executed boundary.
        self.push(StatementStep::Complete(expression));
        self.complete_moves(expression, conditional);
    }

    fn selectors(&mut self, expression: ExpressionHandle, conditional: bool) {
        if !self
            .program
            .expression_table
            .expression_is_valid(expression)
        {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Indexed(_) if self.selected_index(expression) => {
                self.expression(expression, conditional);
            }
            ExpressionNode::Name(_) => {}
            ExpressionNode::Member(member) => self.selectors(member.receiver, conditional),
            ExpressionNode::Indexed(indexed) => {
                self.selectors(indexed.collection, conditional);
                self.expression(indexed.index, conditional);
            }
            ExpressionNode::Borrow(borrow) => self.selectors(borrow.target, conditional),
            _ => self.expression(expression, conditional),
        }
    }

    /// A nominal index operator observes or consumes its collection operand;
    /// only intrinsic place indexing can bypass an enclosing-owner read.
    fn selected_index(&self, expression: ExpressionHandle) -> bool {
        self.operators.resolved_uses().any(|usage| {
            usage.expression == expression
                && usage.occurrence == checked_trees::CheckedOperatorOccurrence::Expression
                && matches!(usage.origin, checked_trees::CheckedValueOrigin::StateStatement {
                    state_symbol, statement_index, ..
                } if state_symbol == self.state.symbol && statement_index == self.statement_index)
                && typed_trees::operator::declaration_by_symbol(
                    self.program,
                    usage.selected_operator_symbol,
                )
                .is_some()
        })
    }
}

//! One source evaluation order for ownership windows and loan exclusions.
//! This inventory orders existing move candidates; it does not classify them.

use validation::CanonicalPlace;

use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::statement::StatementNode;

/// One binary decision in an expression. Match patterns carry the failed
/// prefix, not their eventual arm: a later pattern cannot execute after an
/// earlier arm succeeds. An occurrence after the join has no branch token.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct EvaluationBranch {
    expression: ExpressionHandle,
    decision: usize,
    selected: bool,
}

impl EvaluationBranch {
    pub(super) fn same_decision(self, other: Self) -> bool {
        self.expression == other.expression && self.decision == other.decision
    }
}

pub(super) struct StatementOccurrence {
    pub(super) step: StatementStep,
    pub(super) route: Vec<EvaluationBranch>,
    /// The last step through which this occurrence's access is held: its own
    /// position, or the completion of the outermost invocation that stages it
    /// as a receiver or operand, since the callee observes it until then.
    pub(super) held_until: usize,
}

pub(super) fn routes_compatible(left: &[EvaluationBranch], right: &[EvaluationBranch]) -> bool {
    left.iter().all(|left| {
        right
            .iter()
            .all(|right| !left.same_decision(*right) || left.selected == right.selected)
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    moves: &[crate::flow::DiscoveredMoveEvent],
    operators: &checked_trees::CheckedOperatorFacts,
) -> Vec<StatementOccurrence> {
    statement_steps_at_evaluation(
        program,
        machine,
        state,
        statement_index,
        statement,
        moves,
        operators,
        None,
    )
}

/// The permission schedule has already selected a transition target. Only
/// selections inside that evaluation remain conditional for cleanup custody.
pub(super) fn statement_steps_at_evaluation(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    moves: &[crate::flow::DiscoveredMoveEvent],
    operators: &checked_trees::CheckedOperatorFacts,
    evaluation: Option<checked_trees::FlowPermissionEvaluation>,
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
        StatementNode::Assembly(_) => {}
        StatementNode::LocalData(local) => order.expression(local.initial_value, false),
        StatementNode::Expression(expression) => order.expression(*expression, false),
        StatementNode::Assignment(assignment) => {
            order.selectors(assignment.target, false);
            order.expression(assignment.value, false);
        }
        StatementNode::Call(call) => {
            if let Some(receiver) = crate::flow::canonical_receiver_place_for_call_site(
                program,
                machine.symbol,
                state.symbol,
                &validation::CallSite::Statement(call),
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
            order.hold_through_completion(0);
        }
        StatementNode::RootBinding(binding) => {
            order.expression(binding.receiver, false);
            order.expression(binding.implementation_operand, false);
        }
        StatementNode::AssemblyFact(fact) => {
            for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                order.expression(expression, false);
            }
        }
        StatementNode::Transition(transition) => {
            if !matches!(
                evaluation,
                Some(checked_trees::FlowPermissionEvaluation::TransitionTarget(_))
            ) && let typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard
            {
                order.expression(guard, false);
            }
            for (alternative, target) in [transition.target, transition.continuation]
                .into_iter()
                .enumerate()
            {
                if !target.is_valid() {
                    continue;
                }
                if evaluation.is_some_and(|evaluation| {
                    evaluation != checked_trees::FlowPermissionEvaluation::TransitionTarget(target)
                }) {
                    continue;
                }
                let conditional = evaluation
                    != Some(checked_trees::FlowPermissionEvaluation::TransitionTarget(
                        target,
                    ));
                if conditional {
                    order.route.push(EvaluationBranch {
                        expression: ExpressionHandle::invalid(),
                        decision: 0,
                        selected: alternative == 0,
                    });
                }
                match program.statement_table.transition_target(target) {
                    typed_trees::statement::TransitionTargetNode::Value(value) => {
                        order.expression(*value, conditional)
                    }
                    // Transition arguments live in the statement table's
                    // handle arena, as statement-call arguments do; the
                    // expression table's span at the same offset names
                    // unrelated expressions.
                    typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
                        let first = order.steps.len();
                        for argument in program.statement_table.expression_handles(*arguments) {
                            order.expression(*argument, conditional);
                        }
                        order.push(StatementStep::Complete(ExpressionHandle::invalid()));
                        order.hold_through_completion(first);
                    }
                    _ => {}
                }
                if conditional {
                    order.route.pop();
                }
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

/// An observation borrows its temporary receiver; it cannot discharge the
/// receiver's custody. Keep the actual completed expression root so ordinary
/// permission production and scope-exit checking own its residual partition,
/// including affine storage with no linear claims.
pub(super) fn observed_owned_temporaries(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    operators: &checked_trees::CheckedOperatorFacts,
) -> Vec<(ExpressionHandle, typed_trees::types::TypeReferenceHandle)> {
    let mut owners = Vec::new();
    let occurrences = statement_steps(
        program,
        machine,
        state,
        statement_index,
        statement,
        &[],
        operators,
    );
    for occurrence in &occurrences {
        let StatementStep::Observe { place, .. } = &occurrence.step else {
            continue;
        };
        let Some(expression) = place.root.as_expression() else {
            continue;
        };
        // Saved subjects retain their producer identity without evaluating it
        // again. Only a completion on this occurrence creates a temporary.
        if !occurrences.iter().any(|occurrence| matches!(occurrence.step, StatementStep::Complete(completed) if completed == expression))
            || owners.iter().any(|(owner, _)| *owner == expression) {
            continue;
        }
        let root = CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        };
        let Some(reference) = validation::canonical_place_type_reference(
            program,
            state.symbol,
            statement_index,
            &root,
        ) else {
            continue;
        };
        if program.type_multiplicity(reference) != language_semantics::Multiplicity::Unrestricted {
            owners.push((expression, reference));
        }
    }
    // Observation order differs from establishment order, for example when an
    // indexed observation evaluates another temporary for its selector. Reuse
    // this exact occurrence inventory rather than rebuilding an AST schedule.
    let mut completed_owners = Vec::new();
    for occurrence in &occurrences {
        if let StatementStep::Complete(expression) = occurrence.step
            && let Some(owner) = owners.iter().find(|(owner, _)| *owner == expression)
            && !completed_owners.contains(owner)
        {
            completed_owners.push(*owner);
        }
    }
    completed_owners
}

struct StatementOrder<'a> {
    program: &'a dyn ::validation::CheckingInput,
    state: &'a typed_trees::state::State,
    statement_index: usize,
    operators: &'a checked_trees::CheckedOperatorFacts,
    remaining: Vec<(usize, &'a crate::flow::DiscoveredMoveEvent)>,
    steps: Vec<StatementOccurrence>,
    route: Vec<EvaluationBranch>,
}

impl StatementOrder<'_> {
    fn push(&mut self, step: StatementStep) {
        let position = self.steps.len();
        self.steps.push(StatementOccurrence {
            step,
            route: self.route.clone(),
            held_until: position,
        });
    }

    /// An invocation holds every access it staged from `first` on until the
    /// completion step just pushed.
    fn hold_through_completion(&mut self, first: usize) {
        let completion = self.steps.len().saturating_sub(1);
        for occurrence in self.steps.get_mut(first..).into_iter().flatten() {
            occurrence.held_until = occurrence.held_until.max(completion);
        }
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
        let saved_subject = validation::saved_dispatch_subject(
            self.program,
            self.state,
            self.statement_index,
            expression,
        );
        if saved_subject.is_some_and(|(statement, subject)| {
            statement != self.statement_index || subject != expression
        }) {
            return;
        }
        let mut invocation = None;
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
                if let Some(place) = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state.symbol,
                    self.statement_index,
                    expression,
                ) {
                    self.push(StatementStep::Observe { place, expression });
                }
            }
            ExpressionNode::Call(call) => {
                invocation = Some(self.steps.len());
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
                let prefix = self.route.len();
                let arms = super::multiplicity::reachable_arms(self.program, dispatch.arms);
                for (decision, (_, arm)) in arms.iter().enumerate() {
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        self.expression(pattern, conditional || decision != 0);
                    }
                    // Exhaustiveness is checked before ownership. The last
                    // reachable arm is the remaining route, not a new branch
                    // with a fabricated unmatched successor.
                    if decision + 1 < arms.len() {
                        self.route.push(EvaluationBranch {
                            expression,
                            decision,
                            selected: true,
                        });
                        self.expression(arm.value, true);
                        self.route.pop();
                        self.route.push(EvaluationBranch {
                            expression,
                            decision,
                            selected: false,
                        });
                    } else {
                        self.expression(arm.value, conditional || decision != 0);
                    }
                }
                self.route.truncate(prefix);
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
                        if left.is_none() {
                            self.route.push(EvaluationBranch {
                                expression,
                                decision: 0,
                                selected: evaluate_when,
                            });
                        }
                        self.expression(binary.right, conditional || left.is_none());
                        if left.is_none() {
                            self.route.pop();
                        }
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
        if let Some(first) = invocation {
            self.hold_through_completion(first);
        }
        // Capturing a computed dispatch subject also observes its completed
        // value for discrimination. Publish that ordinary observation in the
        // shared inventory so its anonymous owner is established once and its
        // selected exit retains the remaining disposal, just like a projected
        // observation of a completed value. Named subjects already observe
        // their existing storage above.
        if saved_subject.is_some()
            && let Some(place) = validation::canonical_place_from_expression_in_state(
                self.program,
                self.state.symbol,
                self.statement_index,
                expression,
            )
            && place.root == facts::PlaceRoot::Expression(expression)
            && place.segments.is_empty()
        {
            self.push(StatementStep::Observe { place, expression });
        }
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

//! A scalar write retires the equality facts its target participated in, so a
//! post-write `self.count == before + 1` cannot appeal to the live-at-entry
//! `self.count == before` directly. The value the place holds at the exit is
//! nevertheless the source expression of its last write — the flow plan
//! already knows which contexts were entering that statement, and each
//! equality still live at the write's incoming boundary holds for the source
//! expression's own readings. Substituting those equalities into the retained
//! source expression transports the goal across the write: the transported
//! form must equal the goal's other side structurally, and every surviving
//! leaf must read storage no statement between the write and the exit could
//! have changed. Statements whose write frame is opaque to this scan — calls,
//! expression statements, and root bindings — stop the transport entirely
//! rather than guess at what they may have written.

use facts::{FactPayload, PlaceRoot};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use crate::flow::{
    CanonicalPlace, canonical_place_from_expression_in_state, normalize_attached_place_root,
    statement_mutated_place,
};

use super::ExitScalars;

/// One equality edge live at the write's incoming boundary: occurrences of
/// `place` inside the stored source expression evaluate to `substitution`.
struct WriteSubstitution {
    place: CanonicalPlace,
    substitution: ExpressionHandle,
}

impl ExitScalars<'_, '_> {
    /// Proves an `==` atom when one side names a place whose last write stored
    /// an expression that transported equality makes identical to the other
    /// side. Failed search never supplies false; the ordinary value paths keep
    /// their own verdicts.
    pub(super) fn write_transported_equality(&self, expression: ExpressionHandle) -> bool {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return false;
        };
        binary.operator == BinaryOperator::Equal
            && (self.write_transport_proves(binary.left, binary.right)
                || self.write_transport_proves(binary.right, binary.left))
    }

    /// `place_side` is the goal operand naming the written place; `goal` is
    /// the operand it must equal at the exit.
    fn write_transport_proves(&self, place_side: ExpressionHandle, goal: ExpressionHandle) -> bool {
        let Some(state) = crate::semantic::calls::find_state_in_machine(
            self.program,
            self.exit.machine_symbol,
            self.exit.state_symbol,
        ) else {
            return false;
        };
        let Some(mut place) = canonical_place_from_expression_in_state(
            self.program,
            self.exit.state_symbol,
            self.exit.statement_index,
            place_side,
        ) else {
            return false;
        };
        normalize_attached_place_root(
            self.program,
            self.machine.symbol,
            self.exit.state_symbol,
            &mut place,
        );
        if !matches!(place.root, PlaceRoot::Symbol(_)) || !super::stable_segments(&place.segments) {
            return false;
        }
        if !transport_expression_is_pure(self.program, place_side)
            || !transport_expression_is_pure(self.program, goal)
        {
            return false;
        }
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        // Walk backward: the first statement reaching the goal place is the
        // last write. Everything between it and the exit must be accounted
        // for before either side's leaves can be compared across contexts.
        let mut later_writes = Vec::new();
        let mut write = None;
        for index in (0..self.exit.statement_index.min(statements.len())).rev() {
            let statement = &statements[index];
            match statement {
                typed_trees::statement::StatementNode::Assignment(assignment) => {
                    let Some(mut written) = statement_mutated_place(
                        self.program,
                        self.machine.symbol,
                        state.symbol,
                        index,
                        statement,
                    ) else {
                        continue;
                    };
                    normalize_attached_place_root(
                        self.program,
                        self.machine.symbol,
                        state.symbol,
                        &mut written,
                    );
                    if places_overlap(&written, &place) {
                        write = Some((index, assignment.value));
                        later_writes.push(place.clone());
                        break;
                    }
                    later_writes.push(written);
                }
                typed_trees::statement::StatementNode::LocalData(local) => {
                    // A declaration writes only its own fresh root; it is the
                    // last write exactly when the goal place is that local.
                    if matches!(place.root, PlaceRoot::Symbol(root) if root == local.symbol)
                        && place.segments.is_empty()
                    {
                        if local.initial_value.is_valid() {
                            write = Some((index, local.initial_value));
                        }
                        later_writes.push(place.clone());
                        break;
                    }
                }
                typed_trees::statement::StatementNode::AssemblyFact(_)
                | typed_trees::statement::StatementNode::Transition(_) => {}
                // Calls, expression statements, and root bindings write through
                // frames this scan does not own; stop rather than undercount.
                _ => return false,
            }
        }
        let Some((write_index, source)) = write else {
            return false;
        };
        if !source.is_valid() || !transport_expression_is_pure(self.program, source) {
            return false;
        }
        let substitutions = self.write_substitutions(state, write_index, &later_writes);
        self.transported_equal(
            source,
            goal,
            &substitutions,
            &later_writes,
            state,
            write_index,
        )
    }

    /// Equality edges holding at the write's incoming boundary. Each is
    /// retained only while its substitution keeps every leaf readable through
    /// the writes between it and the exit.
    fn write_substitutions(
        &self,
        state: &typed_trees::state::State,
        write_index: usize,
        writes: &[CanonicalPlace],
    ) -> Vec<WriteSubstitution> {
        let Some((_, state_flow)) =
            self.facts
                .flow
                .control
                .states
                .iter()
                .find(|(_, state_flow)| {
                    state_flow.machine_symbol == self.exit.machine_symbol
                        && state_flow.state_symbol == self.exit.state_symbol
                })
        else {
            return Vec::new();
        };
        let contexts = self
            .facts
            .flow
            .state_statement(state_flow, write_index)
            .map_or(state_flow.entry_semantic_contexts, |statement| {
                statement.entry_semantic_contexts
            });
        let mut substitutions: Vec<WriteSubstitution> = Vec::new();
        for reference in self
            .facts
            .flow
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts)
        {
            for fact in self
                .facts
                .semantic
                .context_view(self.facts.semantic.contexts.get(reference.context))
                .facts()
            {
                if matches!(
                    fact.origin,
                    facts::FactOrigin::CallRequires | facts::FactOrigin::CallEnsures
                ) {
                    continue;
                }
                let expressions = match fact.payload {
                    FactPayload::ContractBooleanExpression {
                        kind: facts::ContractFactKind::Requires,
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => equality_conjuncts(self.program, expression),
                    FactPayload::BooleanExpression(expression)
                    | FactPayload::BooleanValue {
                        expression,
                        value: true,
                    } => equality_conjuncts(self.program, expression),
                    _ => continue,
                };
                for (left, right) in expressions {
                    for (subject, substitution) in [(left, right), (right, left)] {
                        let Some(mut place) = canonical_place_from_expression_in_state(
                            self.program,
                            state.symbol,
                            write_index,
                            subject,
                        ) else {
                            continue;
                        };
                        normalize_attached_place_root(
                            self.program,
                            self.machine.symbol,
                            state.symbol,
                            &mut place,
                        );
                        if !matches!(place.root, PlaceRoot::Symbol(_))
                            || !super::stable_segments(&place.segments)
                            || !transport_expression_is_pure(self.program, substitution)
                        {
                            continue;
                        }
                        if self.substitution_survives(
                            &place,
                            substitution,
                            writes,
                            state,
                            write_index,
                        ) {
                            substitutions.push(WriteSubstitution {
                                place,
                                substitution,
                            });
                        }
                    }
                }
            }
        }
        // Two live equalities substituting different expressions into one
        // place are ambivalent; the transport drops both rather than pick.
        let mut index = 0;
        while index < substitutions.len() {
            let duplicate = substitutions.iter().enumerate().any(|(other, entry)| {
                other != index
                    && entry.place == substitutions[index].place
                    && !self
                        .program
                        .expression_table
                        .expressions_structurally_equal(
                            entry.substitution,
                            substitutions[index].substitution,
                        )
            });
            if duplicate {
                substitutions.remove(index);
            } else {
                index += 1;
            }
        }
        substitutions
    }

    /// A substitution holds at the write only while every leaf it re-reads was
    /// already settled there — an overlap with any write reaching the exit
    /// makes the entry and exit readings different values.
    fn substitution_survives(
        &self,
        _place: &CanonicalPlace,
        substitution: ExpressionHandle,
        writes: &[CanonicalPlace],
        state: &typed_trees::state::State,
        write_index: usize,
    ) -> bool {
        let mut nodes = Vec::new();
        crate::monomorphization::collect_expression_tree(self.program, substitution, &mut nodes);
        nodes.iter().all(|node| {
            let Some(mut leaf) = canonical_place_from_expression_in_state(
                self.program,
                state.symbol,
                write_index,
                *node,
            ) else {
                return true;
            };
            normalize_attached_place_root(
                self.program,
                self.machine.symbol,
                state.symbol,
                &mut leaf,
            );
            if !matches!(leaf.root, PlaceRoot::Symbol(_)) {
                return true;
            }
            !writes.iter().any(|write| places_overlap(write, &leaf))
        })
    }

    /// Structural comparison where the stored expression's leaves may first be
    /// rewritten by a live equality. An unsubstituted leaf matches only when
    /// it reads the same storage value at the write's incoming boundary as at
    /// the exit — no intervening write may reach its place.
    fn transported_equal(
        &self,
        expression: ExpressionHandle,
        goal: ExpressionHandle,
        substitutions: &[WriteSubstitution],
        writes: &[CanonicalPlace],
        state: &typed_trees::state::State,
        write_index: usize,
    ) -> bool {
        if let Some(mut leaf) = canonical_place_from_expression_in_state(
            self.program,
            state.symbol,
            write_index,
            expression,
        ) {
            normalize_attached_place_root(
                self.program,
                self.machine.symbol,
                state.symbol,
                &mut leaf,
            );
            if matches!(leaf.root, PlaceRoot::Symbol(_)) {
                if !super::stable_segments(&leaf.segments) {
                    return false;
                }
                if let Some(substitution) = substitutions
                    .iter()
                    .find(|substitution| substitution.place == leaf)
                {
                    return self
                        .program
                        .expression_table
                        .expressions_structurally_equal(substitution.substitution, goal);
                }
                return !writes.iter().any(|write| places_overlap(write, &leaf))
                    && self
                        .program
                        .expression_table
                        .expressions_structurally_equal(expression, goal);
            }
        }
        match (
            self.program.expression_table.expression(expression),
            self.program.expression_table.expression(goal),
        ) {
            (ExpressionNode::Binary(expression_binary), ExpressionNode::Binary(goal_binary)) => {
                expression_binary.operator == goal_binary.operator
                    && self.transported_equal(
                        expression_binary.left,
                        goal_binary.left,
                        substitutions,
                        writes,
                        state,
                        write_index,
                    )
                    && self.transported_equal(
                        expression_binary.right,
                        goal_binary.right,
                        substitutions,
                        writes,
                        state,
                        write_index,
                    )
            }
            (ExpressionNode::Unary(expression_unary), ExpressionNode::Unary(goal_unary)) => {
                expression_unary.operator == goal_unary.operator
                    && self.transported_equal(
                        expression_unary.operand,
                        goal_unary.operand,
                        substitutions,
                        writes,
                        state,
                        write_index,
                    )
            }
            _ => self
                .program
                .expression_table
                .expressions_structurally_equal(expression, goal),
        }
    }
}

/// Writes overlap when one place reaches through the other — a write to a
/// prefix replaces the whole subtree, and a write beneath the goal place
/// replaces one of its cells.
fn places_overlap(a: &CanonicalPlace, b: &CanonicalPlace) -> bool {
    a.root == b.root
        && (a.segments.len() <= b.segments.len()
            && a.segments
                .iter()
                .zip(b.segments.iter())
                .all(|(a, b)| a == b)
            || b.segments.len() < a.segments.len()
                && b.segments
                    .iter()
                    .zip(a.segments.iter())
                    .all(|(a, b)| a == b))
}

/// The transport argues by expression identity; nodes with effects, deferred
/// values, or aggregate structure that `expressions_structurally_equal` does
/// not decompose stay out of its language entirely.
fn transport_expression_is_pure(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    let mut nodes = Vec::new();
    crate::monomorphization::collect_expression_tree(program, expression, &mut nodes);
    program.expression_table.expression_is_valid(expression)
        && nodes.iter().all(|node| {
            !matches!(
                program.expression_table.expression(*node),
                ExpressionNode::Match(_)
                    | ExpressionNode::ArrayLiteral(_)
                    | ExpressionNode::Atomic(_)
                    | ExpressionNode::Call(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Range(_)
                    | ExpressionNode::StructLiteral(_)
                    | ExpressionNode::ZeroValue(_)
            )
        })
}

/// `A && B` asserts both conjuncts; each `==` conjunct supplies an edge.
fn equality_conjuncts(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Vec<(ExpressionHandle, ExpressionHandle)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            let mut conjuncts = equality_conjuncts(program, binary.left);
            conjuncts.extend(equality_conjuncts(program, binary.right));
            conjuncts
        }
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            vec![(binary.left, binary.right)]
        }
        _ => Vec::new(),
    }
}

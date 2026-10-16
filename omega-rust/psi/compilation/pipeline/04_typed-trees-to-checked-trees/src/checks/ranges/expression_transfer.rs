//! One evaluation schedule for range checking and expression-call fact transport.
//!
//! Selected arms and short-circuit operands receive branch-local facts; calls
//! invalidate written storage before later operands consume those facts. The
//! two consumers differ only at an indexed read or a completed call's entry.
//! Keeping separate walkers previously lost match guards in both consumers
//! and short-circuit guards in call-argument collection.
//!
//! A boolean pattern seeds the subject's truth. An integer value pattern
//! seeds the same stable subject as that integer for the selected arm only.
//! `match which { 0 -> items[0..which + 2] }` is then the window `items[0..2]`.
//! Later arms keep the facts from patterns that already failed; they do not
//! inherit the selected value.

use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableCallExpression,
    TableIndexedExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use validation::CallFrameResolver;

use super::expressions::{expression_integer_value, expression_name};
use super::facts::RangeFacts;
use super::guards::{seed_guard_facts, seed_negated_guard_facts, seed_value_vs_value_endpoints};

pub(super) trait ExpressionTransferSink {
    fn indexed_access(
        &mut self,
        _facts: &RangeFacts<'_>,
        _expression: ExpressionHandle,
        _indexed: &TableIndexedExpression,
    ) {
    }

    fn call_entry(&mut self, _facts: &RangeFacts<'_>, _call: &TableCallExpression) {}
}

pub(super) struct ExpressionTransfer<'program, 'frames> {
    pub program: &'program dyn ::validation::CheckingInput,
    pub machine: &'program Machine,
    pub state: &'frames State,
    pub call_frames: Option<&'frames CallFrameResolver<'program>>,
}

impl ExpressionTransfer<'_, '_> {
    fn is_read_only(&self, expression: ExpressionHandle) -> bool {
        self.call_frames.is_some_and(|frames| {
            frames
                .expression_write_frame(self.machine, expression)
                .into_complete_paths()
                .is_some_and(|paths| paths.is_empty())
        })
    }

    /// Record `subject == pattern` when the pattern folds to an integer.
    /// Names and fields are the carriers range proofs already fold; a call
    /// subject has no binding to update, and its own `ensures` supplies the
    /// value when that contract is exact.
    fn seed_matched_integer(
        &self,
        facts: &mut RangeFacts<'_>,
        subject: ExpressionHandle,
        pattern: ExpressionHandle,
    ) {
        let Some(value) = expression_integer_value(self.program, facts, pattern) else {
            return;
        };
        match self.program.expression_table.expression(subject) {
            ExpressionNode::Name(_) => {
                let Some((symbol, name)) = expression_name(self.program, subject) else {
                    return;
                };
                facts.define_local(symbol, name.unwrap_or_default(), None, Some(value));
            }
            ExpressionNode::Member(member) => {
                facts.define_field_integer(member.member_symbol, member.member.as_str(), value);
            }
            _ => {}
        }
    }

    fn seed_outcome(&self, facts: &mut RangeFacts<'_>, subject: ExpressionHandle, truth: bool) {
        if truth {
            seed_guard_facts(self.program, self.machine, self.state, facts, subject);
            seed_value_vs_value_endpoints(self.program, self.machine, self.state, facts, subject);
        } else {
            seed_negated_guard_facts(self.program, self.machine, self.state, facts, subject);
        }
    }

    fn invalidate_writes(&self, facts: &mut RangeFacts<'_>, expression: ExpressionHandle) {
        let writes = self.call_frames.and_then(|frames| {
            frames
                .expression_write_frame(self.machine, expression)
                .into_complete_paths()
        });
        facts.invalidate_call_writes(
            self.program,
            self.machine,
            self.state,
            writes.as_deref(),
            None,
        );
    }

    pub fn transfer(
        &self,
        facts: &mut RangeFacts<'_>,
        expression: ExpressionHandle,
        sink: &mut impl ExpressionTransferSink,
    ) {
        if !expression.is_valid() {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                self.transfer(facts, dispatch.subject, sink);
                let mut remaining = facts.clone();
                // Selected equality need not mean builtin equality. Rejoin
                // the same checked operator roster used by semantic flow;
                // missing authority cannot establish comparison evidence.
                let builtin_comparison = facts.checked_operators.is_some_and(|operators| {
                    !operators.uses.iter().any(|(_, selected)| {
                        selected.expression == expression
                            && selected.occurrence
                                != checked_trees::CheckedOperatorOccurrence::Expression
                    })
                });
                let mut stable_subject = builtin_comparison && self.is_read_only(dispatch.subject);
                let mut boolean_coverage = [false; 2];
                for arm in self.program.expression_table.match_arms(dispatch.arms) {
                    let mut outcome = None;
                    if let MatchPattern::Value(pattern) = arm.pattern {
                        self.transfer(&mut remaining, pattern, sink);
                        // The comparison uses the captured subject. A pattern
                        // write must not turn its old truth into a fresh fact
                        // about storage the arm will read.
                        stable_subject &= self.is_read_only(pattern);
                        if let Some(value) = self.program.expression_table.boolean_node(pattern) {
                            outcome = Some(*value);
                            if builtin_comparison {
                                boolean_coverage[usize::from(*value)] = true;
                            }
                        }
                    }
                    let mut selected = remaining.clone();
                    if stable_subject && let Some(truth) = outcome {
                        self.seed_outcome(&mut selected, dispatch.subject, truth);
                        self.seed_outcome(&mut remaining, dispatch.subject, !truth);
                    }
                    if stable_subject
                        && builtin_comparison
                        && let MatchPattern::Value(pattern) = arm.pattern
                    {
                        // Equality is arm-local. `remaining` is the path that
                        // failed this pattern, so it must not take the value.
                        self.seed_matched_integer(&mut selected, dispatch.subject, pattern);
                    }
                    self.transfer(&mut selected, arm.value, sink);
                    if matches!(arm.pattern, MatchPattern::Wildcard)
                        || boolean_coverage.iter().all(|covered| *covered)
                    {
                        break;
                    }
                }
                // No branch-only promise escapes. Conservatively retire the
                // union of possible writes, without applying an arm to the
                // failed-pattern path of a later arm.
                self.invalidate_writes(facts, expression);
            }
            ExpressionNode::Binary(binary) => {
                self.transfer(facts, binary.left, sink);
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    let mut selected = facts.clone();
                    if self.is_read_only(binary.left) {
                        self.seed_outcome(
                            &mut selected,
                            binary.left,
                            binary.operator == BinaryOperator::And,
                        );
                    }
                    self.transfer(&mut selected, binary.right, sink);
                    self.invalidate_writes(facts, binary.right);
                } else {
                    self.transfer(facts, binary.right, sink);
                }
            }
            ExpressionNode::Call(call) => {
                facts.begin_result_measurement(expression);
                self.transfer(facts, call.receiver, sink);
                if call.receiver.is_valid() {
                    facts.capture_result_measurement(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                        call.receiver,
                    );
                }
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.transfer(facts, *argument, sink);
                    facts.capture_result_measurement(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                        *argument,
                    );
                }
                sink.call_entry(facts, call);
                let writes = self.call_frames.and_then(|frames| {
                    frames
                        .expression_write_frame(self.machine, expression)
                        .into_complete_paths()
                });
                facts.invalidate_call_writes(
                    self.program,
                    self.machine,
                    self.state,
                    writes.as_deref(),
                    Some(&validation::CallSite::Expression { expression, call }),
                );
            }
            ExpressionNode::Cast(cast) => {
                let source = super::types::same_carrier_integer_cast_source(
                    self.program,
                    self.machine,
                    self.state,
                    expression,
                );
                // A recast's direct indexed operand selects an address; the
                // recast checker owns its footprint. Nested operand reads
                // retain their ordinary element bounds obligations.
                if cast.form.is_recast()
                    && let ExpressionNode::Indexed(indexed) =
                        self.program.expression_table.expression(cast.value)
                {
                    self.transfer(facts, indexed.collection, sink);
                    self.transfer(facts, indexed.index, sink);
                } else {
                    self.transfer(facts, cast.value, sink);
                    if let Some(source) = source {
                        // Retain the operand's reads so later mutations retire
                        // the cast's position facts, not just the source facts.
                        facts.record_expression_dependencies(
                            self.program,
                            self.machine,
                            self.state,
                            expression,
                        );
                        facts.alias_index(
                            &self.program.expression_table.display_name(source),
                            &self.program.expression_table.display_name(expression),
                        );
                    }
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.transfer(facts, indexed.collection, sink);
                self.transfer(facts, indexed.index, sink);
                sink.indexed_access(facts, expression, indexed);
            }
            ExpressionNode::Atomic(atomic) => self.transfer(facts, atomic.value, sink),
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.program.expression_table.expression_handles(*values) {
                    self.transfer(facts, *value, sink);
                }
            }
            ExpressionNode::Member(member) => self.transfer(facts, member.receiver, sink),
            ExpressionNode::Borrow(borrow) => self.transfer(facts, borrow.target, sink),
            ExpressionNode::Unary(unary) => self.transfer(facts, unary.operand, sink),
            ExpressionNode::Range(range) => {
                self.transfer(facts, range.start, sink);
                self.transfer(facts, range.end, sink);
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in self.program.expression_table.struct_fields(literal.fields) {
                    self.transfer(facts, field.value, sink);
                }
            }
            typed_trees::literal_expression_pattern!() => {}
        }
    }
}

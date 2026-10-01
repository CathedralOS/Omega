//! One evaluation schedule for range checking and expression-call fact transport.
//!
//! Selected arms and short-circuit operands receive branch-local facts; calls
//! invalidate written storage before later operands consume those facts. The
//! two consumers differ only at an indexed read or a completed call's entry.
//! Keeping separate walkers previously lost match guards in both consumers
//! and short-circuit guards in call-argument collection.

use typed_trees::TypedTrees;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern, TableCallExpression,
    TableIndexedExpression,
};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use validation::CallFrameResolver;

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
    pub program: &'program TypedTrees,
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
                        if let ExpressionNode::Boolean(value) =
                            self.program.expression_table.expression(pattern)
                        {
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
                self.transfer(facts, call.receiver, sink);
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.transfer(facts, *argument, sink);
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
                    Some(&crate::semantic::calls::CallSite::Expression { expression, call }),
                );
            }
            ExpressionNode::Cast(cast) => {
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
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Name(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::front_end::checked_program_result;

    #[test]
    fn selected_match_arms_receive_only_their_guard_facts() {
        for (body, accepted) in [
            (
                "match 0 < values.len { true -> values[0], false -> 0 }",
                true,
            ),
            (
                "match values.len == 0 { true -> 0, false -> values[0] }",
                true,
            ),
            ("match values.len == 0 { true -> 0, _ -> values[0] }", true),
            (
                "match 0 < values.len { true -> 0, false -> values[0] }",
                false,
            ),
            (
                "match values.len == 0 { true -> values[0], false -> 0 }",
                false,
            ),
            (
                "let selected: u8 = match 0 < values.len { true -> values[0], false -> 0 }; values[0]",
                false,
            ),
        ] {
            let source = format!("machine read(values: &[u8]) -> u8 {{ {body} }}");
            let outcome = checked_program_result(&source);
            if accepted {
                assert!(outcome.is_ok(), "{body}: {outcome:?}");
            } else {
                let diagnostics = outcome.expect_err(body);
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
                    "{body}: {diagnostics:?}"
                );
            }
        }
    }

    #[test]
    fn pattern_and_selected_arm_writes_retire_index_facts() {
        for body in [
            "match bag.index < 2 { overwrite(bag) -> bag.values[bag.index], _ -> 0 }",
            "match bag.index < 2 { overwrite(bag) -> 0, true -> bag.values[bag.index], _ -> 0 }",
            "match bag.index < 2 { true -> after(overwrite(bag), bag.values[bag.index]), false -> 0 }",
        ] {
            let source = format!(
                "data Bag {{ values: [u8; 2]; index: u64; }}
                 machine overwrite(bag: &mut Bag) -> bool {{ bag.index = 2; true }}
                 machine after(changed: bool, value: u8) -> u8 {{ value }}
                 machine read(bag: &mut Bag) -> u8 {{ {body} }}"
            );
            let diagnostics = checked_program_result(&source).expect_err(body);
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove index")),
                "{body}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn selected_arm_writes_do_not_run_on_the_other_arm() {
        let source = "data Bag { values: [u8; 2]; index: u64; }
            machine overwrite(bag: &mut Bag) -> u8 { bag.index = 2; 0 }
            machine read(bag: &mut Bag, choose: bool) -> u8
            requires bag.index < 2; {
                match choose { true -> overwrite(bag), false -> bag.values[bag.index] }
            }";
        let outcome = checked_program_result(source);
        assert!(outcome.is_ok(), "{outcome:?}");
    }
}

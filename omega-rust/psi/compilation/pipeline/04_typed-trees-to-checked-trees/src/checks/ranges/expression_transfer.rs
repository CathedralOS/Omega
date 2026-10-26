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
    fn checks_indexed(&self) -> bool {
        true
    }

    fn indexed_access(
        &mut self,
        _facts: &RangeFacts<'_>,
        _expression: ExpressionHandle,
        _indexed: &TableIndexedExpression,
    ) {
    }

    fn call_entry(&mut self, _facts: &RangeFacts<'_>, _call: &TableCallExpression) {}
}

pub(in crate::checks::ranges) fn expression_has_transfer_effect(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    checks_indexed: bool,
) -> bool {
    expression_has_effect_at_depth(program, expression, checks_indexed, true, 0)
}

pub(in crate::checks::ranges) fn expression_has_sink_effect(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    checks_indexed: bool,
) -> bool {
    expression_has_effect_at_depth(program, expression, checks_indexed, false, 0)
}

fn expression_has_effect_at_depth(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    checks_indexed: bool,
    include_fact_mutations: bool,
    depth: usize,
) -> bool {
    let Some(node) = program
        .expression_table
        .expression_at_depth(expression, depth)
    else {
        return false;
    };
    let next = depth + 1;
    match node {
        ExpressionNode::SavedSubject(_) => false,
        ExpressionNode::Call(_) => true,
        ExpressionNode::Indexed(indexed) => {
            checks_indexed
                || expression_has_effect_at_depth(
                    program,
                    indexed.collection,
                    checks_indexed,
                    include_fact_mutations,
                    next,
                )
                || expression_has_effect_at_depth(
                    program,
                    indexed.index,
                    checks_indexed,
                    include_fact_mutations,
                    next,
                )
        }
        ExpressionNode::Cast(cast) => {
            include_fact_mutations
                || expression_has_effect_at_depth(
                    program,
                    cast.value,
                    checks_indexed,
                    include_fact_mutations,
                    next,
                )
        }
        ExpressionNode::Atomic(atomic) => {
            include_fact_mutations
                || if let Some(carrier) = validation::atomic_once_carrier(program, expression) {
                    expression_has_effect_at_depth(
                        program,
                        carrier.place,
                        checks_indexed,
                        include_fact_mutations,
                        next,
                    ) || carrier.operands.into_iter().any(|operand| {
                        expression_has_effect_at_depth(
                            program,
                            operand,
                            checks_indexed,
                            include_fact_mutations,
                            next,
                        )
                    })
                } else {
                    expression_has_effect_at_depth(
                        program,
                        atomic.value,
                        checks_indexed,
                        include_fact_mutations,
                        next,
                    )
                }
        }
        ExpressionNode::Match(dispatch) => {
            expression_has_effect_at_depth(
                program,
                dispatch.subject,
                checks_indexed,
                include_fact_mutations,
                next,
            ) || program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .any(|arm| {
                    program
                        .expression_table
                        .match_pattern_operands(arm.pattern)
                        .any(|operand| {
                            expression_has_effect_at_depth(
                                program,
                                operand,
                                checks_indexed,
                                include_fact_mutations,
                                next,
                            )
                        })
                        || expression_has_effect_at_depth(
                            program,
                            arm.value,
                            checks_indexed,
                            include_fact_mutations,
                            next,
                        )
                })
        }
        ExpressionNode::Binary(binary) => {
            expression_has_effect_at_depth(
                program,
                binary.left,
                checks_indexed,
                include_fact_mutations,
                next,
            ) || expression_has_effect_at_depth(
                program,
                binary.right,
                checks_indexed,
                include_fact_mutations,
                next,
            )
        }
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .any(|value| {
                expression_has_effect_at_depth(
                    program,
                    *value,
                    checks_indexed,
                    include_fact_mutations,
                    next,
                )
            }),
        ExpressionNode::Member(member) => expression_has_effect_at_depth(
            program,
            member.receiver,
            checks_indexed,
            include_fact_mutations,
            next,
        ),
        ExpressionNode::Borrow(borrow) => expression_has_effect_at_depth(
            program,
            borrow.target,
            checks_indexed,
            include_fact_mutations,
            next,
        ),
        ExpressionNode::Unary(unary) => expression_has_effect_at_depth(
            program,
            unary.operand,
            checks_indexed,
            include_fact_mutations,
            next,
        ),
        ExpressionNode::Range(range) => {
            expression_has_effect_at_depth(
                program,
                range.start,
                checks_indexed,
                include_fact_mutations,
                next,
            ) || expression_has_effect_at_depth(
                program,
                range.end,
                checks_indexed,
                include_fact_mutations,
                next,
            )
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| {
                expression_has_effect_at_depth(
                    program,
                    field.value,
                    checks_indexed,
                    include_fact_mutations,
                    next,
                )
            }),
        typed_trees::literal_expression_pattern!() => false,
    }
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
        let checks_indexed = sink.checks_indexed();
        if !expression.is_valid()
            || !expression_has_transfer_effect(self.program, expression, checks_indexed)
        {
            return;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Match(dispatch) => {
                self.transfer(facts, dispatch.subject, sink);
                let arms = self.program.expression_table.match_arms(dispatch.arms);
                let any_arm_needs_visit = arms.iter().any(|arm| {
                    self.program
                        .expression_table
                        .match_pattern_operands(arm.pattern)
                        .any(|pattern| {
                            expression_has_transfer_effect(self.program, pattern, checks_indexed)
                        })
                        || expression_has_sink_effect(self.program, arm.value, checks_indexed)
                });
                if any_arm_needs_visit {
                    let mut remaining = facts.clone();
                    // Selected equality need not mean builtin equality. Rejoin
                    // the same checked operator roster used by semantic flow;
                    // missing authority cannot establish comparison evidence.
                    let builtin_comparison = facts.checked_operators.is_some_and(|operators| {
                        let mut builtin = true;
                        operators.for_each_expression_use(expression, |_, selected| {
                            if selected.occurrence
                                != checked_trees::CheckedOperatorOccurrence::Expression
                            {
                                builtin = false;
                            }
                        });
                        builtin
                    });
                    let mut stable_subject =
                        builtin_comparison && self.is_read_only(dispatch.subject);
                    let mut boolean_coverage = [false; 2];
                    for arm in arms {
                        let mut outcome = None;
                        for pattern in self
                            .program
                            .expression_table
                            .match_pattern_operands(arm.pattern)
                        {
                            self.transfer(&mut remaining, pattern, sink);
                            stable_subject &= self.is_read_only(pattern);
                        }
                        if let MatchPattern::Value(pattern) = arm.pattern {
                            // The comparison uses the captured subject. A pattern
                            // write must not turn its old truth into a fresh fact
                            // about storage the arm will read.
                            if let Some(value) = self.program.expression_table.boolean_node(pattern)
                            {
                                outcome = Some(*value);
                                if builtin_comparison {
                                    boolean_coverage[usize::from(*value)] = true;
                                }
                            }
                        }
                        if expression_has_sink_effect(self.program, arm.value, checks_indexed) {
                            let mut selected = remaining.clone();
                            if stable_subject && let Some(truth) = outcome {
                                self.seed_outcome(&mut selected, dispatch.subject, truth);
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
                        }
                        if stable_subject && let Some(truth) = outcome {
                            self.seed_outcome(&mut remaining, dispatch.subject, !truth);
                        }
                        if matches!(arm.pattern, MatchPattern::Wildcard)
                            || boolean_coverage.iter().all(|covered| *covered)
                        {
                            break;
                        }
                    }
                }
                // No branch-only promise escapes. Conservatively retire the
                // union of possible writes, without applying an arm to the
                // failed-pattern path of a later arm.
                if expression_has_transfer_effect(self.program, expression, false) {
                    self.invalidate_writes(facts, expression);
                }
            }
            ExpressionNode::Binary(binary) => {
                self.transfer(facts, binary.left, sink);
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                    if expression_has_sink_effect(self.program, binary.right, checks_indexed) {
                        let mut selected = facts.clone();
                        if self.is_read_only(binary.left) {
                            self.seed_outcome(
                                &mut selected,
                                binary.left,
                                binary.operator == BinaryOperator::And,
                            );
                        }
                        self.transfer(&mut selected, binary.right, sink);
                    }
                    if expression_has_transfer_effect(self.program, binary.right, false) {
                        self.invalidate_writes(facts, binary.right);
                    }
                } else {
                    self.transfer(facts, binary.right, sink);
                }
            }
            ExpressionNode::Call(call) => {
                facts.begin_result_measurement(expression);
                let measured_collection =
                    validation::normal_return_length_collection(self.program, call);
                self.transfer(facts, call.receiver, sink);
                if call.receiver.is_valid()
                    && let Some(collection) = measured_collection
                {
                    facts.capture_result_measurement_for_collection(
                        self.program,
                        self.machine,
                        self.state,
                        expression,
                        call.receiver,
                        collection,
                    );
                }
                for argument in self
                    .program
                    .expression_table
                    .expression_handles(call.arguments)
                {
                    self.transfer(facts, *argument, sink);
                    if let Some(collection) = measured_collection {
                        facts.capture_result_measurement_for_collection(
                            self.program,
                            self.machine,
                            self.state,
                            expression,
                            *argument,
                            collection,
                        );
                    }
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
            ExpressionNode::Atomic(atomic) => {
                if let Some(carrier) = validation::atomic_once_carrier(self.program, expression) {
                    self.transfer(facts, carrier.place, sink);
                    for operand in carrier.operands {
                        self.transfer(facts, operand, sink);
                    }
                    self.invalidate_writes(facts, expression);
                } else {
                    self.transfer(facts, atomic.value, sink);
                }
            }
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

/*
The transition family of the reader: an arm's guard and value pattern, its
target (a value, a terminal call or Unit) and the crash exit; and dispatch
over a sum, which stage 01 writes as one guarded transition per case arm.
*/

use crate::body::Qualification;
use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{
    StatementNode, TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

use super::authority::Access;
use super::expressions::{case_condition, case_test_of, same_computation};
use super::{Program, Slot, crash_cause};
use crate::body::{Expression, ExpressionKind, Statement, Target, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl Program<'_> {
    /// One transition arm: its guard, read once, and its target. A guard of
    /// `None` is an unconditional arm.
    pub(super) fn transition(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        transition: &TableTransition,
        result: Option<ValueType>,
    ) -> Checked<(Option<Expression>, Target)> {
        if transition.continuation.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Transition("continuation"),
            ));
        }
        if !transition.proof_selectors.is_empty() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Transition("proof selectors"),
            ));
        }
        let patterns = self
            .typed
            .statement_table
            .transition_patterns(transition.patterns);
        let condition = match (transition.guard, patterns) {
            (TransitionGuardNode::Always, []) => None,
            (TransitionGuardNode::When(guard), []) => Some(guard),
            (TransitionGuardNode::Always, [pattern]) => Some(self.value_pattern(id, pattern)?),
            // Stage 03 keeps the arm's value test twice: as the
            // transition guard and, as a separate copy, as the
            // pattern's condition. The guard is evaluated once;
            // the copy must read to the same computation.
            (TransitionGuardNode::When(guard), [pattern]) => {
                let condition = self.value_pattern(id, pattern)?;
                let read_guard = self.expression(
                    id,
                    slots,
                    guard,
                    Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                )?;
                let read_condition = self.expression(
                    id,
                    slots,
                    condition,
                    Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                )?;
                if !same_computation(&read_guard, &read_condition) {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Transition("guard and pattern condition differ"),
                    ));
                }
                Some(guard)
            }
            (_, [_, _, ..]) => {
                return Err(refuse(
                    id,
                    Construct::Machine,
                    Unserved::Transition("tuple pattern"),
                ));
            }
        };
        let guard = match condition {
            None => None,
            Some(guard) => {
                let guard = self.expression(
                    id,
                    slots,
                    guard,
                    Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                )?;
                if guard.result != Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE))
                {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Typing("guard is not bool"),
                    ));
                }
                Some(guard)
            }
        };
        // A `crash Cause;` statement, or an arm `-> crash Cause`,
        // is a transition whose exit is the crash and whose target
        // is the terminal node stage 01 gives it.
        let target = match transition.exit {
            TransitionExit::Ordinary => self.target(id, slots, transition.target, result)?,
            TransitionExit::Crash(cause) => {
                self.crash_target(id, transition.target)?;
                Target::Crash(crash_cause(cause))
            }
        };
        Ok((guard, target))
    }

    /// Whether a transition is a case arm: its one pattern has a head.
    /// Stage 01 gives a value arm no head and an unguarded arm no pattern.
    pub(super) fn case_arm(&self, transition: &TableTransition) -> bool {
        matches!(
            self.typed
                .statement_table
                .transition_patterns(transition.patterns),
            [pattern] if pattern.pattern.head.is_valid()
        )
    }

    /// `transition subject { S::A { .. } -> x  S::B -> y  _ -> z }` over a
    /// sum. Stage 01 writes it as a local holding the subject followed by
    /// one transition per arm, each guarded by `subject in case`, and
    /// replaces every use of a payload binding by the payload member of the
    /// subject it names. `statements` starts at the first case arm; the
    /// arms read are the run of case arms over that same subject and, when
    /// one follows, the unguarded arm that is the dispatch's `_`. Returns
    /// the dispatch and how many statements it took.
    ///
    /// Every arm completes the machine, so the dispatch does. An arm with a
    /// guard of its own, a pattern that tests a field, a second arm for one
    /// case and a `crash` arm are each refused by name. Coverage is decided
    /// where lowering builds the terminator from the declaration's cases.
    pub(super) fn dispatch(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        statements: &[StatementNode],
        result: Option<ValueType>,
    ) -> Checked<(Statement, usize)> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Transition(what));
        let mut subject: Option<Expression> = None;
        let mut arms: Vec<(usize, Target, SourceOrigin)> = Vec::new();
        let mut default = None;
        let mut taken = 0;
        let mut source = SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent);
        for statement in statements {
            let StatementNode::Transition(transition) = statement else {
                break;
            };
            let patterns = self
                .typed
                .statement_table
                .transition_patterns(transition.patterns);
            let case = match (transition.guard, patterns, &subject) {
                (TransitionGuardNode::When(guard), [pattern], _)
                    if pattern.pattern.head.is_valid() =>
                {
                    let read = self.expression(id, slots, pattern.subject, None)?;
                    let (ExpressionKind::Slot(_), Some(record)) =
                        (&read.kind, self.opened(id, read.result, Access::Read)?)
                    else {
                        return Err(unserved("case arm whose subject is not a data local"));
                    };
                    match &subject {
                        Some(known) if !same_computation(known, &read) => break,
                        Some(_) => {}
                        None => {
                            source = SourceOrigin::of(transition.source_span);
                            subject = Some(read.clone());
                        }
                    }
                    let case = self.case_pattern(id, record, &pattern.pattern)?;
                    // The arm's test is stored twice, as the transition
                    // guard and as the pattern's condition `test && true`.
                    // Both must read to this arm's own case test.
                    let guard = self.expression(
                        id,
                        slots,
                        guard,
                        Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                    )?;
                    let condition = self.expression(
                        id,
                        slots,
                        pattern.pattern.condition,
                        Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                    )?;
                    if !same_computation(&guard, &case_test_of(&read, case))
                        || !same_computation(&condition, &case_condition(&read, case))
                    {
                        return Err(unserved("case arm with a guard"));
                    }
                    if arms.iter().any(|(earlier, _, _)| *earlier == case) {
                        return Err(unserved("case arm for a case an earlier arm selects"));
                    }
                    Some(case)
                }
                (TransitionGuardNode::Always, [], Some(_)) => None,
                (TransitionGuardNode::Always | TransitionGuardNode::When(_), _, _) => break,
            };
            if transition.continuation.is_valid() {
                return Err(unserved("continuation"));
            }
            if !transition.proof_selectors.is_empty() {
                return Err(unserved("proof selectors"));
            }
            // A crash site's guard is the exact conjunction of the branch
            // decisions before it, over values. Which case a dispatch
            // selected is not a value, so the guard could not state it.
            let target = match transition.exit {
                TransitionExit::Ordinary => self.target(id, slots, transition.target, result)?,
                TransitionExit::Crash(_) => return Err(unserved("crash in a case arm")),
            };
            taken += 1;
            let arm_source = SourceOrigin::of(transition.source_span);
            match case {
                Some(case) => arms.push((case, target, arm_source)),
                None => {
                    default = Some((target, arm_source));
                    break;
                }
            }
        }
        let subject = subject.ok_or_else(|| unserved("dispatch without a case arm"))?;
        Ok((
            Statement::Dispatch {
                subject,
                arms,
                default,
                source,
            },
            taken,
        ))
    }

    /// A value pattern on a scalar subject is its condition. Stage 01 gives a
    /// value arm such as `true -> ...` one non-structural pattern with no head
    /// and no fields whose condition is `subject == value`
    /// (`01_tokens-to-syntax-trees/src/bodies_and_transitions.rs`,
    /// `retain_value_pattern`); the condition already names the captured
    /// subject, so it is the arm's whole guard. Any other pattern destructures
    /// and is refused.
    fn value_pattern(
        &self,
        id: MachineId,
        pattern: &typed_trees::statement::TransitionPattern,
    ) -> Checked<ExpressionHandle> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Transition(what)));
        let destructure = &pattern.pattern;
        if pattern.is_structural
            || destructure.head.is_valid()
            || !destructure.fields.is_empty()
            || destructure.application_type.is_valid()
            || !destructure.proof_selectors.is_empty()
        {
            return unserved("destructuring pattern");
        }
        // Stage 03 stores the value test as both the pattern's guard and its
        // condition, one handle.
        match (
            destructure.guard.is_valid(),
            destructure.condition.is_valid(),
        ) {
            (true, false) => Ok(destructure.guard),
            (false, true) => Ok(destructure.condition),
            (true, true) if destructure.guard == destructure.condition => Ok(destructure.condition),
            (true, true) => unserved("pattern with a separate guard and condition"),
            (false, false) => unserved("pattern without a condition"),
        }
    }

    fn target(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        handle: typed_trees::statement::TransitionTargetHandle,
        result: Option<ValueType>,
    ) -> Checked<Target> {
        let typed = self.typed;
        if !handle.is_valid() {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Transition("missing target"),
            ));
        }
        let target = match typed.statement_table.transition_target(handle) {
            TransitionTargetNode::Terminal => {
                if result.is_some() {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Typing("Unit arm in a value machine"),
                    ));
                }
                return Ok(Target::Unit);
            }
            TransitionTargetNode::Value(expression) => {
                self.expression(id, slots, *expression, result)?
            }
            TransitionTargetNode::Named {
                operational_acknowledgement,
                path,
                static_machine_parameter,
                arguments,
                evidence_arguments,
                source_span,
                authored_call_selection,
            } => {
                if operational_acknowledgement.acknowledges_suspend
                    || operational_acknowledgement.acknowledges_block
                {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("suspend or block acknowledgement"),
                    ));
                }
                if static_machine_parameter.is_valid() || !evidence_arguments.is_empty() {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("static or evidence call"),
                    ));
                }
                self.direct_selection(id, path.symbol, *authored_call_selection, None)?;
                // `Counter::step(count)`: two names, the first the data type
                // the callee is declared under, which only completes its
                // name (`names_carrier`).
                let members = typed.statement_table.name_path_members(path.members).len();
                let qualified = members == 2
                    && path.head_symbol == path.receiver_symbol
                    && self.names_carrier(path.symbol, path.receiver_symbol);
                if !qualified && (members != 1 || path.receiver_symbol.is_valid()) {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Call("qualified or receiver call"),
                    ));
                }
                let arguments = typed.statement_table.expression_handles(*arguments);
                self.call(
                    id,
                    slots,
                    path.symbol,
                    arguments,
                    SourceOrigin::of(*source_span),
                )?
            }
        };
        if target.result != result {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("arm value type"),
            ));
        }
        Ok(Target::Value(target))
    }

    /// A crash exit carries no target of its own: stage 01 gives it the
    /// terminal node. Any other target is refused, not ignored.
    fn crash_target(
        &self,
        id: MachineId,
        handle: typed_trees::statement::TransitionTargetHandle,
    ) -> Checked<()> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Transition(what)));
        if !handle.is_valid() {
            return unserved("crash without a target");
        }
        match self.typed.statement_table.transition_target(handle) {
            TransitionTargetNode::Terminal => Ok(()),
            TransitionTargetNode::Value(_) | TransitionTargetNode::Named { .. } => {
                unserved("crash with a value or call target")
            }
        }
    }
}

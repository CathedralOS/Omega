/*
The transition family of the reader: an arm's guard and value pattern, its
target (a value, a terminal call or Unit) and the crash exit.
*/

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::{Checked, Construct};
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::{
    TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

use super::expressions::same_computation;
use super::{Program, Slot, crash_cause};
use crate::body::{Expression, Target};
use crate::{SourceOrigin, Unserved, refuse};

impl Program<'_> {
    /// One transition arm: its guard, read once, and its target. A guard of
    /// `None` is an unconditional arm.
    pub(super) fn transition(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        transition: &TableTransition,
        result: Option<ScalarType>,
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
                let read_guard = self.expression(id, slots, guard, Some(ScalarType::Boolean))?;
                let read_condition =
                    self.expression(id, slots, condition, Some(ScalarType::Boolean))?;
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
                let guard = self.expression(id, slots, guard, Some(ScalarType::Boolean))?;
                if guard.result != Some(ScalarType::Boolean) {
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
        result: Option<ScalarType>,
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
                if typed.statement_table.name_path_members(path.members).len() != 1
                    || path.receiver_symbol.is_valid()
                {
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

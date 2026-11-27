use crate::read::refuse_at;
/*
The transition family of the reader: an arm's guard and value pattern, its
target (a value, a terminal call or Unit) and the crash exit; and dispatch
over a sum, which stage 01 writes as one guarded transition per case arm.
*/

use crate::body::Qualification;
use crate::source_state::expression::{ExpressionHandle, TableDestructurePattern};
use crate::source_state::statement::{
    StatementHandle, StatementNode, TableTransition, TransitionExit, TransitionGuardNode,
    TransitionPattern, TransitionTargetNode,
};
use semantic_vocabulary::{MachineId, ScalarType};
use terminal_check_tables::Checked;

use super::authority::Access;
use super::expressions::{case_test_of, same_computation};
use super::{Program, Slot, crash_cause};
use crate::body::{CaseSelector, Expression, ExpressionKind, Statement, Target, ValueType};
use crate::{SourceOrigin, Unserved};

impl Program<'_> {
    /// One transition arm: its guard, read once, and its target. A guard of
    /// `None` is an unconditional arm.
    pub(super) fn transition(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        statement: StatementHandle,
        result: Option<ValueType>,
    ) -> Checked<(Option<Expression>, Target)> {
        let StatementNode::Transition(transition) = self.typed.statement_table.statement(statement)
        else {
            return Err(refuse_at(
                id,
                Unserved::Transition("occurrence does not name a transition statement"),
            ));
        };
        if transition.continuation.is_valid() {
            return Err(refuse_at(id, Unserved::Transition("continuation")));
        }
        if !transition.proof_selectors.is_empty() {
            return Err(refuse_at(id, Unserved::Transition("proof selectors")));
        }
        let patterns = self
            .typed
            .statement_table
            .transition_patterns(transition.patterns);
        let guard = self.transition_predicate(id, slots, transition)?;
        // A `crash Cause;` statement, or an arm `-> crash Cause`,
        // is a transition whose exit is the crash and whose target
        // is the terminal node stage 01 gives it.
        let target = match transition.exit {
            TransitionExit::Ordinary => self.target(id, slots, statement, result)?,
            TransitionExit::Crash(cause) => {
                self.crash_target(id, transition.target)?;
                Target::Crash(crash_cause(cause))
            }
        };
        // How a warning about this arm names it: the value it tests, which
        // stage 03 states as `subject == value`, or `_` for the arm that
        // takes what the arms before it did not.
        let label = match (&guard, patterns) {
            (None, []) => Some(String::from("_")),
            (Some(guard), [_]) => match &guard.kind {
                ExpressionKind::Strict {
                    operation: crate::body::Strict::Equal,
                    right,
                    ..
                } => match right.kind {
                    ExpressionKind::Boolean(value) => Some(value.to_string()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
        if let (Some(label), Some(names)) = (label, self.names.get_mut(id)) {
            names.arms.push((
                transition.source_span,
                format!("the `{label}` arm of a transition"),
            ));
        }
        Ok((guard, target))
    }

    /// Read only the predicate, including its independently copied pattern
    /// condition. Static branch pruning still owes both exact source receipts;
    /// it must not read or execute the branch's target merely to obtain them.
    pub(super) fn transition_predicate(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        transition: &TableTransition,
    ) -> Checked<Option<Expression>> {
        let patterns = self
            .typed
            .statement_table
            .transition_patterns(transition.patterns);
        let condition = match (transition.guard, patterns) {
            (TransitionGuardNode::Always, []) => None,
            (TransitionGuardNode::When(guard), []) => Some(self.expression(
                id,
                slots,
                guard,
                Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
            )?),
            // A pattern that decomposes a record cannot fail, so its arm is
            // unconditional. Stage 01 has replaced each binding by a read of
            // its field from the local that holds the subject.
            (TransitionGuardNode::Always, [pattern])
                if pattern.is_structural && !self.names_case(pattern) =>
            {
                let subject = self.expression(id, slots, pattern.subject, None)?;
                let record = match self.opened(id, subject.result, Access::Read)? {
                    Some(record) if self.data_type(id, record.record())?.cases().is_empty() => {
                        record
                    }
                    _ => {
                        return Err(refuse_at(id, Unserved::Transition("destructuring pattern")));
                    }
                };
                self.record_arm(
                    id,
                    record,
                    &pattern.pattern,
                    SourceOrigin::of(transition.source_span),
                )?;
                None
            }
            (TransitionGuardNode::Always, [pattern]) => {
                let condition = self.value_pattern(id, pattern)?;
                Some(self.expression(
                    id,
                    slots,
                    condition,
                    Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                )?)
            }
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
                    return Err(refuse_at(
                        id,
                        Unserved::Transition("guard and pattern condition differ"),
                    ));
                }
                Some(read_guard)
            }
            (_, [_, _, ..]) => {
                return Err(refuse_at(id, Unserved::Transition("tuple pattern")));
            }
        };
        if condition.as_ref().is_some_and(|guard| {
            guard.result != Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE))
        }) {
            return Err(refuse_at(id, Unserved::Typing("guard is not bool")));
        }
        Ok(condition)
    }

    /// Whether a transition starts a sum dispatch. A structural transition
    /// with only a subject asks the checker to rule out every declared case.
    pub(super) fn case_arm(&self, transition: &TableTransition) -> bool {
        let patterns = self
            .typed
            .statement_table
            .transition_patterns(transition.patterns);
        matches!(patterns, [pattern] if pattern.pattern.head.is_valid()
            && (self.names_case(pattern) || !pattern.is_structural))
            || self.empty_dispatch_subject(transition, patterns).is_some()
    }

    /// Whether a pattern's head names a case, which makes its arm one of a
    /// sum dispatch. A head that names a record, and a positional pattern
    /// with no head, decompose the one shape their subject has.
    fn names_case(&self, pattern: &TransitionPattern) -> bool {
        let head = pattern.pattern.head;
        if !head.is_valid() {
            return false;
        }
        let named = match self.typed.expression_table.expression(head) {
            crate::source_state::expression::ExpressionNode::Name(path) => path.symbol,
            _ => return true,
        };
        !(named.is_valid()
            && self.typed.symbols.get(named).kind == symbols::SymbolKind::Data
            && self
                .typed
                .unique_data_definition_by_symbol(named)
                .is_some_and(|data| {
                    self.typed.data_members(data).iter().all(|member| {
                        matches!(member, crate::source_state::data::DataMember::Field(_))
                    })
                }))
    }

    fn empty_dispatch_subject(
        &self,
        transition: &TableTransition,
        patterns: &[TransitionPattern],
    ) -> Option<ExpressionHandle> {
        let table = &self.typed.statement_table;
        let [pattern] = patterns else {
            return None;
        };
        (pattern.is_structural
            && pattern.subject.is_valid()
            && pattern.pattern == TableDestructurePattern::default()
            && transition.guard == TransitionGuardNode::Always
            && transition.exit == TransitionExit::Ordinary
            && transition.proof_selectors.is_empty()
            && !transition.continuation.is_valid()
            && table.transition_target_is_valid(transition.target)
            && matches!(
                table.transition_target(transition.target),
                TransitionTargetNode::Terminal
            ))
        .then_some(pattern.subject)
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
    /// Every arm completes the machine. Field-value tests and repeated cases
    /// retain their order for the common decision lowerer; separate guards and
    /// crash arms remain unavailable. Lowering checks both tag coverage and
    /// the fallback required when a field test fails.
    pub(super) fn dispatch(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        statements: impl Iterator<Item = StatementHandle>,
        result: Option<ValueType>,
    ) -> Checked<(Statement, usize)> {
        let unserved = |what| refuse_at(id, Unserved::Transition(what));
        let mut subject: Option<Expression> = None;
        let mut arms: Vec<(CaseSelector, Target, SourceOrigin)> = Vec::new();
        let mut default = None;
        let mut taken = 0;
        let mut source = SourceOrigin::Unpositioned(crate::Unpositioned::SpanAbsent);
        let mut wildcard = None;
        let mut full_cases = Vec::new();
        let is_reflected = self
            .symbols
            .get(id)
            .and_then(|&symbol| self.typed.machine_by_symbol(symbol))
            .is_some_and(|machine| {
                machine.purpose
                    == crate::source_state::machine::MachinePurpose::ActiveCaseDispatcher
            });
        let mut reflected_coverage = None;
        for statement in statements {
            let StatementNode::Transition(transition) =
                self.typed.statement_table.statement(statement)
            else {
                break;
            };
            let patterns = self
                .typed
                .statement_table
                .transition_patterns(transition.patterns);
            if let Some(subject) = self.empty_dispatch_subject(transition, patterns) {
                if taken != 0 {
                    break;
                }
                let subject = self.expression(id, slots, subject, None)?;
                let source = SourceOrigin::of(transition.source_span);
                return Ok((
                    Statement::Dispatch {
                        subject,
                        arms: Vec::new(),
                        default: None,
                        source,
                    },
                    1,
                ));
            }
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
                    if full_cases.is_empty() {
                        full_cases.resize(self.data_type(id, record.record())?.cases().len(), None);
                    }
                    let case = self.case_selector(
                        id,
                        slots,
                        record,
                        &pattern.pattern,
                        &read,
                        SourceOrigin::of(transition.source_span),
                    )?;
                    // The arm's test is stored twice, as the transition
                    // guard and as the pattern's condition `test && true`.
                    // Both must read to this arm's own case test.
                    let guard = self.expression(
                        id,
                        slots,
                        guard,
                        Some(ValueType::Scalar(ScalarType::Boolean, Qualification::NONE)),
                    )?;
                    if !same_computation(&guard, &case_test_of(&read, case.case))
                        && !case
                            .condition
                            .as_deref()
                            .is_some_and(|condition| same_computation(&guard, condition))
                    {
                        return Err(unserved("case arm with a guard"));
                    }
                    // Generated visitation candidates name callbacks for precisely
                    // the cases that can occur. Use the same record and contracts
                    // as the dispatch lowerer before discovering the target.
                    if is_reflected {
                        let coverage = reflected_coverage.get_or_insert_with(|| {
                            terminal_semantics::inhabitation::Inhabitation::prepare(
                                &crate::body::inhabitation::SourceContracts(&self.records),
                            )
                        });
                        if coverage.as_ref().is_ok_and(|coverage| {
                            coverage.case_status(record.record(), case.case)
                                == terminal_semantics::inhabitation::Status::ProvedEmpty
                        }) {
                            taken += 1;
                            continue;
                        }
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
                TransitionExit::Ordinary => self.target(id, slots, statement, result)?,
                TransitionExit::Crash(_) => return Err(unserved("crash in a case arm")),
            };
            taken += 1;
            let arm_source = SourceOrigin::of(transition.source_span);
            if let Some(earlier) =
                wildcard.or_else(|| case.as_ref().and_then(|case| full_cases[case.case]))
            {
                if let Some(names) = self.names.get_mut(id) {
                    names
                        .shadowed_patterns
                        .push((transition.source_span, earlier));
                }
                continue;
            }
            match case {
                Some(case) => {
                    if case.condition.is_none() {
                        full_cases[case.case] = Some(transition.source_span);
                    }
                    arms.push((case, target, arm_source));
                }
                None => {
                    wildcard = Some(transition.source_span);
                    default = Some((target, arm_source));
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
    /// (`01_tokens-to-syntax-trees/src/transitions/guards.rs`,
    /// `retain_value_pattern`); the condition already names the captured
    /// subject, so it is the arm's whole guard. Any other pattern destructures
    /// and is refused.
    fn value_pattern(
        &self,
        id: MachineId,
        pattern: &crate::source_state::statement::TransitionPattern,
    ) -> Checked<ExpressionHandle> {
        let unserved = |what| Err(refuse_at(id, Unserved::Transition(what)));
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
        statement: StatementHandle,
        result: Option<ValueType>,
    ) -> Checked<Target> {
        let typed = self.typed;
        let StatementNode::Transition(transition) = typed.statement_table.statement(statement)
        else {
            return Err(refuse_at(
                id,
                Unserved::Transition("target has no exact transition statement"),
            ));
        };
        let handle = transition.target;
        if !typed.statement_table.transition_target_is_valid(handle) {
            return Err(refuse_at(id, Unserved::Transition("missing target")));
        }
        let mut target = match typed.statement_table.transition_target(handle) {
            TransitionTargetNode::Terminal => {
                if result.is_some() {
                    return Err(refuse_at(
                        id,
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
                if !evidence_arguments.is_empty() {
                    return Err(refuse_at(id, Unserved::Call("static or evidence call")));
                }
                // A boundary trait qualifier completes a requirement name;
                // it is never a value supplying an implicit argument.
                let members = typed.statement_table.name_path_members(path.members).len();
                let qualified = members == 2
                    && path.head_symbol == path.receiver_symbol
                    && (self.names_requirement(path.symbol, path.receiver_symbol)
                        || self
                            .record_requirement_signature(id, path.receiver_symbol, path.symbol)?
                            .is_some());
                if !qualified && (members != 1 || path.receiver_symbol.is_valid()) {
                    return Err(refuse_at(id, Unserved::Call("qualified or receiver call")));
                }
                let arguments = typed.statement_table.expression_handles(*arguments);
                let source = self.authored_node(
                    id,
                    crate::source_state::SourceNode::Statement(statement),
                    *source_span,
                )?;
                let mut call = if let Some(rigid) = self.rigid_named_call(
                    id,
                    slots,
                    path.symbol,
                    path.receiver_symbol
                        .is_valid()
                        .then_some(path.receiver_symbol),
                    arguments,
                    *authored_call_selection,
                    source,
                )? {
                    rigid
                } else {
                    let (authored, static_reach_binding) =
                        self.static_machine_target(id, path.symbol, *static_machine_parameter)?;
                    self.direct_selection(id, authored, *authored_call_selection, None)?;
                    let call = self.call_selected(
                        id,
                        slots,
                        path.symbol,
                        arguments,
                        source,
                        None,
                        static_reach_binding,
                    )?;
                    self.retain_static_reach(id, call, static_reach_binding)?
                };
                crate::lower::operational::retain(&mut call, *operational_acknowledgement, true);
                call
            }
        };
        crate::lower::operational::boundary(&mut target);
        if !target.diverges() && !self.agrees(target.result, result) {
            return Err(self.mistyped(
                id,
                target.source,
                super::rejections::Destination::Result,
                result,
                target.result,
                "arm value type",
            ));
        }
        Ok(Target::Value(target))
    }

    /// A crash exit carries no target of its own: stage 01 gives it the
    /// terminal node. Any other target is refused, not ignored.
    fn crash_target(
        &self,
        id: MachineId,
        handle: crate::source_state::statement::TransitionTargetHandle,
    ) -> Checked<()> {
        let unserved = |what| Err(refuse_at(id, Unserved::Transition(what)));
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

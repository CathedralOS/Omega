//! Normal-exit slot effects follow evaluation, not the result representation.
//! Each completed call captures and installs its slot relation before the next
//! operand. Alternatives join complete relations to their common entry, so a
//! skipped call preserves incoming contents and a later overwrite replaces an
//! earlier one. Unknown relations remain unknown.

use super::{
    CollectedStatementPrefix, FrameInference, Machine, SlotArgumentCapture, SlotEffect,
    SlotRelationTransfer, TopLevelSymbols,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::statement::{
    StatementNode, TransitionGuardNode, TransitionTargetHandle, TransitionTargetNode,
};

pub(super) struct CompletedTarget {
    pub target: TransitionTargetHandle,
    pub arguments: Vec<SlotArgumentCapture>,
    pub context: CollectedStatementPrefix,
}

/// Construction scratch for one requested value. A Call records its entry
/// inventory after every actual and before its own normal-exit transfer; all
/// other values use the ordinary completed-expression inventory. The ambient
/// evaluator still installs every effect before evaluating the next operand.
#[derive(Default)]
struct ObservedValue {
    operands: Vec<SlotArgumentCapture>,
    call_entry: Option<(CollectedStatementPrefix, FrameInference)>,
}

struct StatementEvaluation {
    transfers: StatementSlotTransfers,
    targets: Vec<CompletedTarget>,
}

/// Outgoing operands belong to their selected edge. Only the guard's effects
/// reach a transition's lexical successor; ordinary statements have one route.
pub(in super::super) enum StatementSlotTransfers {
    SingleRoute(SlotRelationTransfer),
    Transition {
        all_routes: SlotRelationTransfer,
        fallthrough: SlotRelationTransfer,
    },
}

impl StatementSlotTransfers {
    pub(in super::super) fn all_routes(self) -> SlotRelationTransfer {
        match self {
            Self::SingleRoute(transfer) => transfer,
            Self::Transition { all_routes, .. } => all_routes,
        }
    }

    pub(in super::super) fn fallthrough(self) -> SlotRelationTransfer {
        match self {
            Self::SingleRoute(transfer) => transfer,
            Self::Transition { fallthrough, .. } => fallthrough,
        }
    }
}

pub(super) fn completed_targets(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement: &StatementNode,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<CompletedTarget>> {
    let evaluated = evaluate_statement(
        program,
        machine,
        state,
        statement,
        context,
        symbols,
        inference,
        &mut Capture::default(),
    )?;
    evaluated
        .transfers
        .all_routes()
        .is_complete()
        .then_some(evaluated.targets)
}

pub(in super::super) fn statement_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement: &StatementNode,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<StatementSlotTransfers> {
    evaluate_statement(
        program,
        machine,
        state,
        statement,
        context,
        symbols,
        inference,
        &mut Capture::default(),
    )
    .map(|evaluated| evaluated.transfers)
}

pub(in super::super) fn effects_at_expression(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement: &StatementNode,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    expression: ExpressionHandle,
) -> Option<SlotRelationTransfer> {
    let mut capture = Capture {
        expression,
        ..Capture::default()
    };
    let _ = evaluate_statement(
        program,
        machine,
        state,
        statement,
        context,
        symbols,
        inference,
        &mut capture,
    );
    if capture.count == 1 {
        capture.transfer
    } else {
        None
    }
}

pub(in super::super) fn call_contexts(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement: &StatementNode,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<super::StoredReferenceCallContext>> {
    let mut capture = Capture {
        collect_calls: true,
        calls: vec![super::StoredReferenceCallContext::Entry],
        ..Capture::default()
    };
    evaluate_statement(
        program,
        machine,
        state,
        statement,
        context,
        symbols,
        inference,
        &mut capture,
    )?;
    Some(capture.calls)
}

#[derive(Default)]
struct Capture {
    expression: ExpressionHandle,
    count: usize,
    transfer: Option<SlotRelationTransfer>,
    collect_calls: bool,
    calls: Vec<super::StoredReferenceCallContext>,
    current_call_context: usize,
}

impl Capture {
    fn completed_call(&mut self, expression: ExpressionHandle, transfer: &SlotRelationTransfer) {
        if self.collect_calls {
            self.calls
                .push(super::StoredReferenceCallContext::Invocation {
                    entry: self.current_call_context,
                    expression,
                    effects: transfer
                        .is_complete()
                        .then(|| super::public_effects(transfer.effects.clone()))
                        .flatten(),
                });
            self.current_call_context = self.calls.len() - 1;
        }
    }

    fn join_call_contexts(&mut self, alternatives: Vec<usize>) {
        if !self.collect_calls {
            return;
        }
        let mut distinct = Vec::new();
        for alternative in alternatives {
            if !distinct.contains(&alternative) {
                distinct.push(alternative);
            }
        }
        if let [only] = distinct.as_slice() {
            self.current_call_context = *only;
        } else {
            self.calls.push(super::StoredReferenceCallContext::Join {
                alternatives: distinct,
            });
            self.current_call_context = self.calls.len() - 1;
        }
    }
}

fn evaluate_statement(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement: &StatementNode,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    capture: &mut Capture,
) -> Option<StatementEvaluation> {
    let query = Evaluation {
        program,
        machine,
        state,
        symbols,
    };
    let mut context = context.clone();
    let mut effects = SlotRelationTransfer::default();
    let mut fallthrough = None;
    let mut targets = Vec::new();
    let empty_prefix = SlotRelationTransfer::default();
    if let StatementNode::Transition(transition) = statement {
        if let TransitionGuardNode::When(guard) = transition.guard {
            query.expression(
                guard,
                &mut context,
                &mut effects,
                inference,
                &empty_prefix,
                capture,
                None,
            )?;
        }
        fallthrough = Some(effects.clone());
        let prefix = capture_prefix(&empty_prefix, &effects, capture);
        let mut alternatives = Vec::new();
        let entry_calls = capture.current_call_context;
        let mut alternative_calls = Vec::new();
        for target in [transition.target, transition.continuation] {
            capture.current_call_context = entry_calls;
            let mut branch = SlotRelationTransfer::default();
            let mut branch_context = context.clone();
            let mut branch_inference = inference.clone();
            let mut completed_arguments = Vec::new();
            if program.statement_table.transition_target_is_valid(target) {
                let arguments = match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        program.statement_table.expression_handles(*arguments)
                    }
                    TransitionTargetNode::Value(value) => std::slice::from_ref(value),
                    TransitionTargetNode::Terminal => &[],
                };
                for argument in arguments {
                    completed_arguments.push(query.observe(
                        *argument,
                        &mut branch_context,
                        &mut branch,
                        &mut branch_inference,
                        &prefix,
                        capture,
                    )?);
                }
                targets.push(CompletedTarget {
                    target,
                    arguments: completed_arguments,
                    context: branch_context,
                });
            }
            alternatives.push(branch);
            if capture.collect_calls {
                alternative_calls.push(capture.current_call_context);
            }
        }
        capture.join_call_contexts(alternative_calls);
        install(
            program,
            &mut context,
            &mut effects,
            join(alternatives),
            inference,
        );
    } else if !matches!(statement, StatementNode::AssemblyFact(_)) {
        let mut completed_arguments = Vec::new();
        for expression in super::super::demand::statement_value_expression_roots(program, statement)
        {
            if statement.as_call().is_some() {
                completed_arguments.push(query.observe(
                    expression,
                    &mut context,
                    &mut effects,
                    inference,
                    &empty_prefix,
                    capture,
                )?);
            } else {
                query.expression(
                    expression,
                    &mut context,
                    &mut effects,
                    inference,
                    &empty_prefix,
                    capture,
                    None,
                )?;
            }
        }
        if let Some(call) = statement.as_call() {
            let called = super::call_effects(
                program,
                machine,
                state,
                call,
                &completed_arguments,
                &context,
                symbols,
                inference,
            )
            .unwrap_or_else(|| {
                SlotRelationTransfer::unknown(
                    &context,
                    call.target_symbol,
                    false,
                    super::provider_output_relation_required(
                        program,
                        machine.symbol,
                        call.target_symbol,
                        ExpressionHandle::invalid(),
                    ),
                )
            });
            capture.completed_call(ExpressionHandle::invalid(), &called);
            install(program, &mut context, &mut effects, called, inference);
        }
    }
    let transfers = match fallthrough {
        Some(fallthrough) => StatementSlotTransfers::Transition {
            all_routes: effects,
            fallthrough,
        },
        None => StatementSlotTransfers::SingleRoute(effects),
    };
    Some(StatementEvaluation { transfers, targets })
}

struct Evaluation<'request, 'symbols> {
    program: &'request dyn crate::CheckingInput,
    machine: &'request Machine,
    state: &'request typed_trees::state::State,
    symbols: &'request TopLevelSymbols<'symbols>,
}

impl Evaluation<'_, '_> {
    /// Retain a value's actual evaluated inputs only when its caller needs the
    /// completed value. The callee selector and unused guard values still run
    /// through the same transfer without allocating an unused snapshot.
    fn observe(
        &self,
        expression: ExpressionHandle,
        context: &mut CollectedStatementPrefix,
        effects: &mut SlotRelationTransfer,
        inference: &mut FrameInference,
        prefix: &SlotRelationTransfer,
        capture: &mut Capture,
    ) -> Option<SlotArgumentCapture> {
        let mut observed = ObservedValue::default();
        self.expression(
            expression,
            context,
            effects,
            inference,
            prefix,
            capture,
            Some(&mut observed),
        )?;
        let (context, inference) = observed
            .call_entry
            .as_ref()
            .map(|(context, inference)| (context, inference))
            .unwrap_or((context, inference));
        Some(SlotArgumentCapture::at(
            expression,
            context,
            inference,
            observed.operands,
        ))
    }

    fn expression(
        &self,
        expression: ExpressionHandle,
        context: &mut CollectedStatementPrefix,
        effects: &mut SlotRelationTransfer,
        inference: &mut FrameInference,
        prefix: &SlotRelationTransfer,
        capture: &mut Capture,
        mut observed: Option<&mut ObservedValue>,
    ) -> Option<()> {
        if !expression.is_valid() {
            return Some(());
        }
        let node = self.program.expression_table.expression(expression);
        match node {
            ExpressionNode::Match(dispatch) => {
                self.expression(
                    dispatch.subject,
                    context,
                    effects,
                    inference,
                    prefix,
                    capture,
                    None,
                )?;
                let branch_prefix = capture_prefix(prefix, effects, capture);
                let mut prefix_context = context.clone();
                let mut prefix_effects = SlotRelationTransfer::default();
                let mut prefix_inference = inference.clone();
                let mut alternatives = Vec::new();
                let mut alternative_calls = Vec::new();
                for arm in self.program.expression_table.match_arms(dispatch.arms) {
                    for pattern in self
                        .program
                        .expression_table
                        .match_pattern_operands(arm.pattern)
                    {
                        self.expression(
                            pattern,
                            &mut prefix_context,
                            &mut prefix_effects,
                            &mut prefix_inference,
                            &branch_prefix,
                            capture,
                            None,
                        )?;
                    }
                    let prefix_calls = capture.current_call_context;
                    let mut branch_context = prefix_context.clone();
                    let mut branch = prefix_effects.clone();
                    let mut branch_inference = prefix_inference.clone();
                    if let Some(output) = observed.as_mut() {
                        output.operands.push(self.observe(
                            arm.value,
                            &mut branch_context,
                            &mut branch,
                            &mut branch_inference,
                            &branch_prefix,
                            capture,
                        )?);
                    } else {
                        self.expression(
                            arm.value,
                            &mut branch_context,
                            &mut branch,
                            &mut branch_inference,
                            &branch_prefix,
                            capture,
                            None,
                        )?;
                    }
                    alternatives.push(branch);
                    if capture.collect_calls {
                        alternative_calls.push(capture.current_call_context);
                    }
                    capture.current_call_context = prefix_calls;
                    if matches!(arm.pattern, MatchPattern::Wildcard) {
                        break;
                    }
                }
                capture.join_call_contexts(alternative_calls);
                install(
                    self.program,
                    context,
                    effects,
                    join(alternatives),
                    inference,
                );
                capture_completed(expression, prefix, effects, capture);
                return Some(());
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                    && crate::has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        Some(self.state),
                        expression,
                    ) =>
            {
                if let Some(output) = observed.as_mut() {
                    output.operands.push(self.observe(
                        binary.left,
                        context,
                        effects,
                        inference,
                        prefix,
                        capture,
                    )?);
                } else {
                    self.expression(
                        binary.left,
                        context,
                        effects,
                        inference,
                        prefix,
                        capture,
                        None,
                    )?;
                }
                let branch_prefix = capture_prefix(prefix, effects, capture);
                let skipped_calls = capture.current_call_context;
                let mut branch_context = context.clone();
                let mut branch = SlotRelationTransfer::default();
                let mut branch_inference = inference.clone();
                if let Some(output) = observed.as_mut() {
                    output.operands.push(self.observe(
                        binary.right,
                        &mut branch_context,
                        &mut branch,
                        &mut branch_inference,
                        &branch_prefix,
                        capture,
                    )?);
                } else {
                    self.expression(
                        binary.right,
                        &mut branch_context,
                        &mut branch,
                        &mut branch_inference,
                        &branch_prefix,
                        capture,
                        None,
                    )?;
                }
                if capture.collect_calls {
                    capture.join_call_contexts(vec![skipped_calls, capture.current_call_context]);
                }
                install(
                    self.program,
                    context,
                    effects,
                    join(vec![SlotRelationTransfer::default(), branch]),
                    inference,
                );
                capture_completed(expression, prefix, effects, capture);
                return Some(());
            }
            _ => {}
        }
        let mut children = Vec::new();
        crate::expression_traversal::executable_children(self.program, node, |child| {
            children.push(child)
        });
        let mut completed_inputs = Vec::new();
        for (position, child) in children.into_iter().enumerate() {
            let is_call = matches!(node, ExpressionNode::Call(_));
            if (is_call && position > 0) || (!is_call && observed.is_some()) {
                completed_inputs
                    .push(self.observe(child, context, effects, inference, prefix, capture)?);
            } else {
                self.expression(child, context, effects, inference, prefix, capture, None)?;
            }
        }
        if let ExpressionNode::Call(call) = node {
            // Return-origin substitution uses the already observed inputs; the
            // caller-visible stores below cannot retarget an earlier operand.
            capture_completed(expression, prefix, effects, capture);
            if let Some(output) = observed.as_mut() {
                output.call_entry = Some((context.clone(), inference.clone()));
            }
            // Primitive queries observe operands without installing reference
            // slots. Their operands have already executed above, including any
            // calls that do install slots. The declaration classifier keeps a
            // same-spelled ordinary callable on its own inferred effect route.
            if crate::compiler_operations::expression_call_only_observes_operands(
                self.program,
                call,
            ) {
                if let Some(output) = observed {
                    output.operands = completed_inputs;
                }
                return Some(());
            }
            let called = super::invocation_effects(
                self.program,
                self.machine,
                self.state,
                call.target_symbol,
                expression,
                super::super::caller_aliases::CallerWriteSite::Expression(expression),
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
                &completed_inputs,
                call.machine_arguments.is_empty(),
                context,
                self.symbols,
                inference,
            )
            .unwrap_or_else(|| {
                SlotRelationTransfer::unknown(
                    context,
                    call.target_symbol,
                    false,
                    super::provider_output_relation_required(
                        self.program,
                        self.machine.symbol,
                        call.target_symbol,
                        expression,
                    ),
                )
            });
            capture.completed_call(expression, &called);
            install(self.program, context, effects, called, inference);
            if let Some(output) = observed {
                output.operands = completed_inputs;
            }
            return Some(());
        }
        capture_completed(expression, prefix, effects, capture);
        if let Some(output) = observed {
            output.operands = completed_inputs;
        }
        Some(())
    }
}

fn capture_completed(
    expression: ExpressionHandle,
    prefix: &SlotRelationTransfer,
    current: &SlotRelationTransfer,
    capture: &mut Capture,
) {
    if expression == capture.expression {
        capture.count += 1;
        capture.transfer = Some(capture_prefix(prefix, current, capture));
    }
}

fn same_slot(left: &SlotEffect, right: &SlotEffect) -> bool {
    left.root == right.root
        && left.kind == right.kind
        && left.segments == right.segments
        && left.may_select == right.may_select
}

/// Compose a completed invocation and publish its current inventory to both
/// consumers. Unknown contents absorb incoming alternatives; a definite later
/// overwrite may replace them, never an earlier invocation's obligation.
fn install(
    program: &dyn crate::CheckingInput,
    context: &mut CollectedStatementPrefix,
    accumulated: &mut SlotRelationTransfer,
    next: SlotRelationTransfer,
    inference: &mut FrameInference,
) {
    super::apply_transfer(program, &mut context.stored, &next);
    for record in &context.stored {
        inference.record_local(record);
    }
    accumulate(accumulated, next);
}

/// Capture demand changes snapshot materialization, never the evaluation order
/// or the normal-exit relation consumed by existing statement queries.
fn capture_prefix(
    prefix: &SlotRelationTransfer,
    current: &SlotRelationTransfer,
    capture: &Capture,
) -> SlotRelationTransfer {
    if !capture.expression.is_valid() {
        return SlotRelationTransfer::default();
    }
    let mut combined = prefix.clone();
    accumulate(&mut combined, current.clone());
    combined
}

fn accumulate(accumulated: &mut SlotRelationTransfer, next: SlotRelationTransfer) {
    accumulated.template_dependent |= next.template_dependent;
    accumulated.incomplete_calls.extend(next.incomplete_calls);
    for effect in next.case_effects {
        if !accumulated.case_effects.contains(&effect) {
            accumulated.case_effects.push(effect);
        }
    }
    for mut effect in next.effects {
        let replaces_pointer = effect.sources.is_some()
            && !effect.may_select
            && (!effect.incoming
                || effect
                    .sources
                    .as_ref()
                    .is_none_or(|sources| !sources.is_empty()));
        if let Some(previous) = accumulated
            .effects
            .iter()
            .find(|previous| same_slot(previous, &effect))
            && effect.incoming
        {
            effect.incoming = previous.incoming;
            match (&mut effect.sources, &previous.sources) {
                (Some(sources), Some(previous)) => {
                    for source in previous {
                        if !sources.contains(source) {
                            sources.push(source.clone());
                        }
                    }
                }
                _ => effect.sources = None,
            }
        }
        accumulated.effects.retain(|previous| {
            !same_slot(previous, &effect)
                && !(replaces_pointer
                    && previous.root == effect.root
                    && previous.segments.starts_with(&effect.segments))
        });
        accumulated.effects.push(effect);
    }
}

/// Missing rows are identity. An explicit unknown row dominates known sources
/// on every surviving alternative, including a branch which does no write.
fn join(alternatives: Vec<SlotRelationTransfer>) -> SlotRelationTransfer {
    let mut joined = SlotRelationTransfer::default();
    for alternative in &alternatives {
        joined.template_dependent |= alternative.template_dependent;
    }
    for effect in alternatives
        .iter()
        .flat_map(|alternative| &alternative.effects)
    {
        if joined
            .effects
            .iter()
            .any(|candidate| same_slot(candidate, effect))
        {
            continue;
        }
        let mut combined = effect.clone();
        for alternative in &alternatives {
            if let Some(candidate) = alternative
                .effects
                .iter()
                .find(|candidate| same_slot(candidate, effect))
            {
                combined.incoming |= candidate.incoming;
                match (&mut combined.sources, &candidate.sources) {
                    (Some(sources), Some(candidate)) => {
                        for source in candidate {
                            if !sources.contains(source) {
                                sources.push(source.clone());
                            }
                        }
                    }
                    _ => combined.sources = None,
                }
            } else {
                combined.incoming = true;
            }
        }
        joined.effects.push(combined);
    }
    for alternative in alternatives {
        joined.incomplete_calls.extend(alternative.incomplete_calls);
        // A skipped branch preserves its earlier epoch, while another branch
        // may replace it. Their join therefore widens the affected case family;
        // it never keeps one branch's selected tag as an unconditional fact.
        for effect in alternative.case_effects {
            if !joined.case_effects.contains(&effect) {
                joined.case_effects.push(effect);
            }
        }
    }
    joined
}

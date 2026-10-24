//! Normal-exit slot effects follow evaluation, not the result representation.
//! Each completed call captures and installs its slot relation before the next
//! operand. Alternatives join complete relations to their common entry, so a
//! skipped call preserves incoming contents and a later overwrite replaces an
//! earlier one. Unknown relations remain unknown.

use super::{
    CollectedStatementPrefix, FrameInference, Machine, SlotEffect, SlotRelationTransfer,
    TopLevelSymbols,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(in super::super) fn statement_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement: &StatementNode,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    let query = Evaluation {
        program,
        machine,
        state,
        symbols,
    };
    let mut context = context.clone();
    let mut effects = SlotRelationTransfer::default();
    if let StatementNode::Transition(transition) = statement {
        if let TransitionGuardNode::When(guard) = transition.guard {
            query.expression(guard, &mut context, &mut effects, inference)?;
        }
        let mut alternatives = Vec::new();
        for target in [transition.target, transition.continuation] {
            let mut branch = SlotRelationTransfer::default();
            let mut branch_context = context.clone();
            let mut branch_inference = inference.clone();
            if program.statement_table.transition_target_is_valid(target) {
                let arguments = match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        program.statement_table.expression_handles(*arguments)
                    }
                    TransitionTargetNode::Value(value) => std::slice::from_ref(value),
                    TransitionTargetNode::Terminal => &[],
                };
                for argument in arguments {
                    query.expression(
                        *argument,
                        &mut branch_context,
                        &mut branch,
                        &mut branch_inference,
                    )?;
                }
            }
            alternatives.push(branch);
        }
        install(
            program,
            &mut context,
            &mut effects,
            join(alternatives),
            inference,
        );
    } else if !matches!(statement, StatementNode::AssemblyFact(_)) {
        for expression in super::super::demand::statement_value_expression_roots(program, statement)
        {
            query.expression(expression, &mut context, &mut effects, inference)?;
        }
        if let Some(call) = statement.as_call() {
            let called =
                super::call_effects(program, machine, state, call, &context, symbols, inference)
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
            install(program, &mut context, &mut effects, called, inference);
        }
    }
    Some(effects)
}

struct Evaluation<'request, 'symbols> {
    program: &'request dyn crate::CheckingInput,
    machine: &'request Machine,
    state: &'request typed_trees::state::State,
    symbols: &'request TopLevelSymbols<'symbols>,
}

impl Evaluation<'_, '_> {
    fn expression(
        &self,
        expression: ExpressionHandle,
        context: &mut CollectedStatementPrefix,
        effects: &mut SlotRelationTransfer,
        inference: &mut FrameInference,
    ) -> Option<()> {
        if !expression.is_valid() {
            return Some(());
        }
        let node = self.program.expression_table.expression(expression);
        match node {
            ExpressionNode::Match(dispatch) => {
                self.expression(dispatch.subject, context, effects, inference)?;
                let mut prefix_context = context.clone();
                let mut prefix_effects = SlotRelationTransfer::default();
                let mut prefix_inference = inference.clone();
                let mut alternatives = Vec::new();
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
                        )?;
                    }
                    let mut branch_context = prefix_context.clone();
                    let mut branch = prefix_effects.clone();
                    let mut branch_inference = prefix_inference.clone();
                    self.expression(
                        arm.value,
                        &mut branch_context,
                        &mut branch,
                        &mut branch_inference,
                    )?;
                    alternatives.push(branch);
                    if matches!(arm.pattern, MatchPattern::Wildcard) {
                        break;
                    }
                }
                install(
                    self.program,
                    context,
                    effects,
                    join(alternatives),
                    inference,
                );
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
                self.expression(binary.left, context, effects, inference)?;
                let mut branch_context = context.clone();
                let mut branch = SlotRelationTransfer::default();
                self.expression(
                    binary.right,
                    &mut branch_context,
                    &mut branch,
                    &mut inference.clone(),
                )?;
                install(
                    self.program,
                    context,
                    effects,
                    join(vec![SlotRelationTransfer::default(), branch]),
                    inference,
                );
                return Some(());
            }
            _ => {}
        }
        let mut children = Vec::new();
        crate::expression_traversal::executable_children(self.program, node, |child| {
            children.push(child)
        });
        for child in children {
            self.expression(child, context, effects, inference)?;
        }
        if let ExpressionNode::Call(call) = node {
            // Primitive queries observe operands without installing reference
            // slots. Their operands have already executed above, including any
            // calls that do install slots. The declaration classifier keeps a
            // same-spelled ordinary callable on its own inferred effect route.
            if crate::compiler_operations::expression_call_only_observes_operands(
                self.program,
                call,
            ) {
                return Some(());
            }
            let called = super::invocation_effects(
                self.program,
                self.machine,
                self.state,
                call.target_symbol,
                expression,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
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
            install(self.program, context, effects, called, inference);
        }
        Some(())
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
    super::apply(program, &mut context.stored, &next.effects);
    for record in &context.stored {
        inference.record_local(record);
    }
    accumulated.template_dependent |= next.template_dependent;
    accumulated.incomplete_calls.extend(next.incomplete_calls);
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
    }
    joined
}

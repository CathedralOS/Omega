//! Normal-exit slot effects follow evaluation, not the result representation.
//! Each completed call captures and installs its slot relation before the next
//! operand. Alternatives join complete relations to their common entry, so a
//! skipped call preserves incoming contents and a later overwrite replaces an
//! earlier one. Unknown relations remain unknown.

use super::{CollectedStatementPrefix, FrameInference, Machine, SlotEffect, TopLevelSymbols};
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
) -> Option<Vec<SlotEffect>> {
    let query = Evaluation {
        program,
        machine,
        state,
        symbols,
    };
    let mut context = context.clone();
    let mut effects = Vec::new();
    if let StatementNode::Transition(transition) = statement {
        if let TransitionGuardNode::When(guard) = transition.guard {
            query.expression(guard, &mut context, &mut effects, inference)?;
        }
        let mut alternatives = Vec::new();
        for target in [transition.target, transition.continuation] {
            let mut branch = Vec::new();
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
        install(program, &mut context, &mut effects, join(alternatives));
    } else if !matches!(statement, StatementNode::AssemblyFact(_)) {
        for expression in super::super::demand::statement_value_expression_roots(program, statement)
        {
            query.expression(expression, &mut context, &mut effects, inference)?;
        }
        if let Some(call) = statement.as_call() {
            let called = super::call_effects(program, machine, call, &context, symbols, inference)?;
            install(program, &mut context, &mut effects, called);
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
        effects: &mut Vec<SlotEffect>,
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
                let mut prefix_effects = Vec::new();
                let mut prefix_inference = inference.clone();
                let mut alternatives = Vec::new();
                for arm in self.program.expression_table.match_arms(dispatch.arms) {
                    if let MatchPattern::Value(pattern) = arm.pattern {
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
                install(self.program, context, effects, join(alternatives));
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
                let mut branch = Vec::new();
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
                    join(vec![Vec::new(), branch]),
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
            // Primitive predicates observe bytes without installing reference
            // slots. Their operands have already executed above, including any
            // calls that do install slots. The declaration classifier keeps a
            // same-spelled ordinary callable on its own inferred effect route.
            if matches!(
                crate::compiler_operations::classify_expression_call_intrinsic(self.program, call),
                Some(language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic::ByteSequencePredicate(_))
            ) {
                return Some(());
            }
            let called = super::invocation_effects(
                self.program,
                self.machine,
                call.target_symbol,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
                call.machine_arguments.is_empty(),
                context,
                self.symbols,
                inference,
            )?;
            install(self.program, context, effects, called);
        }
        Some(())
    }
}

fn same_slot(left: &SlotEffect, right: &SlotEffect) -> bool {
    left.root == right.root
        && left.segments == right.segments
        && left.may_select == right.may_select
}

/// Compose a completed operation. Its incoming identity names the preceding
/// inventory, not necessarily the statement's original incoming value.
fn install(
    program: &dyn crate::CheckingInput,
    context: &mut CollectedStatementPrefix,
    accumulated: &mut Vec<SlotEffect>,
    next: Vec<SlotEffect>,
) {
    super::apply(program, &mut context.stored, &next);
    for mut effect in next {
        let replaces_pointer =
            !effect.may_select && (!effect.incoming || !effect.sources.is_empty());
        if let Some(previous) = accumulated
            .iter()
            .find(|previous| same_slot(previous, &effect))
            && effect.incoming
        {
            effect.incoming = previous.incoming;
            for source in &previous.sources {
                if !effect.sources.contains(source) {
                    effect.sources.push(source.clone());
                }
            }
        }
        accumulated.retain(|previous| {
            !same_slot(previous, &effect)
                && !(replaces_pointer
                    && previous.root == effect.root
                    && previous.segments.starts_with(&effect.segments))
        });
        accumulated.push(effect);
    }
}

/// An absent row is the identity relation on that branch, never an empty
/// replacement. Keep every possible descriptor/backing pair at the join.
fn join(alternatives: Vec<Vec<SlotEffect>>) -> Vec<SlotEffect> {
    let mut joined = Vec::<SlotEffect>::new();
    for effect in alternatives.iter().flatten() {
        if joined.iter().any(|candidate| same_slot(candidate, effect)) {
            continue;
        }
        let mut combined = effect.clone();
        for alternative in &alternatives {
            if let Some(candidate) = alternative
                .iter()
                .find(|candidate| same_slot(candidate, effect))
            {
                combined.incoming |= candidate.incoming;
                for source in &candidate.sources {
                    if !combined.sources.contains(source) {
                        combined.sources.push(source.clone());
                    }
                }
            } else {
                combined.incoming = true;
            }
        }
        joined.push(combined);
    }
    joined
}

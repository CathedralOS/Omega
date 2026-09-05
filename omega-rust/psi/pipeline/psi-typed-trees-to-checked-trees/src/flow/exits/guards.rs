use super::super::*;
use psi_typed_trees::statement::TransitionGuardNode;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_guard_context(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    transition_target: psi_typed_trees::statement::TransitionTargetHandle,
    guard: TransitionGuardNode,
    value: bool,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let TransitionGuardNode::When(expression) = guard else {
        return;
    };
    append_predicate_context(
        program,
        semantic,
        ctx,
        state_symbol,
        statement_index,
        expression,
        value,
        ProgramPoint::TransitionArm {
            machine_symbol,
            state_symbol,
            statement_index,
            transition_target,
        },
        active_contexts,
        active_constraints,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn append_predicate_context(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    value: bool,
    point: ProgramPoint,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    // An effectful invocation's result is not a promise about evaluating that
    // invocation again. A saved local boolean is an ordinary stable place.
    if !expression_is_stable_predicate(program, expression) {
        return;
    }
    let mut occurrences = Vec::new();
    crate::contract_occurrences::append_expression_occurrences(
        program,
        expression,
        &mut occurrences,
    );
    let mut places = Vec::new();
    for occurrence in occurrences {
        let Some(place) = crate::semantic_places::canonical_place_to_fact_place_in_state(
            program,
            semantic,
            state_symbol,
            statement_index,
            occurrence,
        ) else {
            return;
        };
        if !places.contains(&place) {
            places.push(place);
        }
    }
    let mut refs = HandleSpan::empty();
    for place in places
        .iter()
        .copied()
        .map(FactPlace::Place)
        .chain(places.is_empty().then_some(FactPlace::Unknown))
    {
        let fact = semantic.append_fact(Fact {
            place,
            point,
            origin: FactOrigin::TransitionGuard,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::BooleanValue { expression, value },
        });
        semantic.append_ref(&mut refs, fact);
    }
    let context = semantic.append_context(point, refs);
    *active_contexts =
        clone_flow_contexts(&mut ctx.contexts.semantic_context_refs, *active_contexts);
    *active_constraints =
        clone_constraint_refs(&mut ctx.contexts.constraint_refs, *active_constraints);
    ctx.contexts
        .semantic_context_refs
        .append_to_span(active_contexts, FlowSemanticContextRef { context });
    append_constraint_ref(
        &mut ctx.contexts.constraint_refs,
        active_constraints,
        FlowConstraintKind::SemanticContext { context },
    );
}

fn expression_is_stable_predicate(
    program: &psi_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_) => true,
        ExpressionNode::Member(member) => expression_is_stable_predicate(program, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            expression_is_stable_predicate(program, indexed.collection)
                && expression_is_stable_predicate(program, indexed.index)
        }
        ExpressionNode::Unary(unary) => expression_is_stable_predicate(program, unary.operand),
        ExpressionNode::Binary(binary) => {
            expression_is_stable_predicate(program, binary.left)
                && expression_is_stable_predicate(program, binary.right)
        }
        ExpressionNode::Borrow(borrow) => expression_is_stable_predicate(program, borrow.target),
        _ => false,
    }
}

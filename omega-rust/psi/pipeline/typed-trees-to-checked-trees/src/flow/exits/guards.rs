use super::super::*;
use typed_trees::statement::TransitionGuardNode;

#[allow(clippy::too_many_arguments)]
pub(super) fn append_guard_context(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    transition_target: typed_trees::statement::TransitionTargetHandle,
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
    program: &typed_trees::TypedTrees,
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
    append_observation_context(
        program,
        semantic,
        ctx,
        state_symbol,
        statement_index,
        occurrences,
        FactPayload::BooleanValue { expression, value },
        point,
        active_contexts,
        active_constraints,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn append_match_pattern_context(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    payload: FactPayload,
    point: ProgramPoint,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let Some((subject, pattern, _)) = payload.match_pattern_comparison(program) else {
        return;
    };
    if !expression_is_stable_predicate(program, subject)
        || !expression_is_stable_predicate(program, pattern)
        || !match_input_has_builtin_meaning(program, ctx.operators, subject)
        || !match_input_has_builtin_meaning(program, ctx.operators, pattern)
    {
        return;
    }
    let mut occurrences = Vec::new();
    for expression in [subject, pattern] {
        crate::contract_occurrences::append_expression_occurrences(
            program,
            expression,
            &mut occurrences,
        );
    }
    append_observation_context(
        program,
        semantic,
        ctx,
        state_symbol,
        statement_index,
        occurrences,
        payload,
        point,
        active_contexts,
        active_constraints,
    );
}

fn match_input_has_builtin_meaning(
    program: &typed_trees::TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> bool {
    let recurse = |child| match_input_has_builtin_meaning(program, operators, child);
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            crate::values::operator_is_builtin(operators, expression)
                && crate::authored_selections::typed_operator_has_no_authored_selection(
                    program, expression,
                )
                && recurse(binary.left)
                && recurse(binary.right)
        }
        ExpressionNode::Unary(unary) => {
            crate::values::operator_is_builtin(operators, expression)
                && crate::authored_selections::typed_operator_has_no_authored_selection(
                    program, expression,
                )
                && recurse(unary.operand)
        }
        ExpressionNode::Indexed(indexed) => {
            crate::values::operator_is_builtin(operators, expression)
                && recurse(indexed.collection)
                && recurse(indexed.index)
        }
        ExpressionNode::Member(member) => recurse(member.receiver),
        ExpressionNode::Borrow(borrow) => recurse(borrow.target),
        ExpressionNode::Name(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn append_observation_context(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    occurrences: Vec<ExpressionHandle>,
    payload: FactPayload,
    point: ProgramPoint,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
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
            payload,
        });
        semantic.append_ref(&mut refs, fact);
    }
    let context = semantic.append_context(point, refs);
    *active_contexts =
        retained_flow_contexts(&ctx.contexts.semantic_context_refs, *active_contexts);
    *active_constraints =
        retained_constraint_refs(&ctx.contexts.constraint_refs, *active_constraints);
    common::append_flow_reference(
        &mut ctx.contexts.semantic_context_refs,
        active_contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut ctx.contexts.constraint_refs,
        active_constraints,
        FlowConstraintKind::SemanticContext { context },
    );
}

fn expression_is_stable_predicate(
    program: &typed_trees::TypedTrees,
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

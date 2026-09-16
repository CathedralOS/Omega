use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::common;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::{Handle, HandleSpan};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;
use typed_trees::proposition::ProofSubstitutions;
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
    let point = ProgramPoint::TransitionArm {
        machine_symbol,
        state_symbol,
        statement_index,
        transition_target,
    };
    append_predicate_context(
        program,
        semantic,
        ctx,
        state_symbol,
        statement_index,
        expression,
        value,
        point,
        active_contexts,
        active_constraints,
    );
    if value {
        append_case_constraint_context(
            program,
            semantic,
            ctx,
            state_symbol,
            statement_index,
            expression,
            point,
            active_contexts,
            active_constraints,
        );
    }
}

/// CASE-CONSTRAINTS (ch12): on the matched branch a case-membership guard
/// (`subject in Type::Case`, lowered to `subject == Type::Case`) contributes
/// the selected case's `where` propositions, substituted onto the exact
/// subject -- matching an established constrained case is how its
/// constraints become arm-local facts (wiki/spec/language/patterns.md). The
/// clause's payload and common-field names rebase to `subject.<field>`
/// through the same canonical-term renderer call contracts use, so a
/// `requires` proven against the destructured payload sees the contributed
/// identity. Facts ride on the subject's canonical place, so ordinary write
/// invalidation retires them exactly like the guard's own evidence. Generic
/// data is refused upstream, so head/case symbols alone identify the
/// variant; membership and proposition-application clauses still wait for
/// their carriers.
#[allow(clippy::too_many_arguments)]
fn append_case_constraint_context(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    point: ProgramPoint,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let mut conjuncts = Vec::new();
    flatten_and_conjuncts(program, expression, &mut conjuncts);
    let mut refs = HandleSpan::empty();
    for conjunct in conjuncts {
        let Some((subject, variant, definition)) = case_membership_claim(program, conjunct) else {
            continue;
        };
        if variant.where_facts.is_empty() {
            continue;
        }
        let Some(place) = crate::semantic_places::canonical_place_to_fact_place_in_state(
            program,
            semantic,
            state_symbol,
            statement_index,
            subject,
        ) else {
            continue;
        };
        let subject_label = program.render_proof_expression(subject, ProofSubstitutions::None);
        let mut substitutions = Vec::new();
        for field in program.data_payload_fields(variant) {
            substitutions.push((
                field.symbol,
                field.name.as_str().to_owned(),
                format!("{subject_label}.{}", field.name),
            ));
        }
        for member in program.data_members(definition) {
            if let typed_trees::data::DataMember::Field(field) = member {
                substitutions.push((
                    field.symbol,
                    field.name.as_str().to_owned(),
                    format!("{subject_label}.{}", field.name),
                ));
            }
        }
        for offset in 0..variant.where_facts.count() {
            let fact_handle = Handle::from_parts(
                variant
                    .where_facts
                    .start()
                    .arena_index()
                    .saturating_add(offset),
                variant.where_facts.start().generation(),
            );
            let typed_trees::domain::ProofFact::Expression(fact_expression) =
                program.proof_facts.get(fact_handle)
            else {
                continue;
            };
            let instantiated =
                semantic.append_instantiated_expression(program.render_proof_expression(
                    *fact_expression,
                    ProofSubstitutions::ByParameter(&substitutions),
                ));
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::TransitionGuard,
                evidence: QualificationEvidence::default(),
                payload: FactPayload::ContractBooleanExpression {
                    kind: facts::ContractFactKind::Requires,
                    fact: fact_handle,
                    expression: *fact_expression,
                    instantiated,
                },
            });
            semantic.append_ref(&mut refs, fact);
        }
    }
    if refs.is_empty() {
        return;
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

fn flatten_and_conjuncts(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    conjuncts: &mut Vec<ExpressionHandle>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::And =>
        {
            flatten_and_conjuncts(program, binary.left, conjuncts);
            flatten_and_conjuncts(program, binary.right, conjuncts);
        }
        _ => conjuncts.push(expression),
    }
}

/// Recognize `subject == Type::Case` (or its commute), the tag compare a
/// case-membership `in` test lowers to, and return the subject with the
/// resolved variant and its data definition.
fn case_membership_claim(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(
    ExpressionHandle,
    &typed_trees::data::DataVariant,
    &typed_trees::data::DataDefinition,
)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator != typed_trees::expression::BinaryOperator::Equal {
        return None;
    }
    for (case_reference, subject) in [(binary.right, binary.left), (binary.left, binary.right)] {
        let ExpressionNode::Name(path) = program.expression_table.expression(case_reference) else {
            continue;
        };
        let Some(definition) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == path.head_symbol)
        else {
            continue;
        };
        let Some(variant) =
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.symbol == path.symbol =>
                    {
                        Some(variant)
                    }
                    _ => None,
                })
        else {
            continue;
        };
        return Some((subject, variant, definition));
    }
    None
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
    crate::facts::contract_occurrences::append_expression_occurrences(
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
    let FactPayload::MatchPattern { expression, .. } = payload else {
        return;
    };
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return;
    };
    // Selected float equality need not mean mathematical equality. Missing
    // selection is rejected by checking, never promoted to builtin evidence.
    if matches!(
        validation::match_subject_primitive_type(program, dispatch),
        Some(typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64)
    ) || ctx.operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && operator_use.occurrence != checked_trees::CheckedOperatorOccurrence::Expression
    }) {
        return;
    }
    if !expression_is_stable_predicate(program, subject)
        || !expression_is_stable_predicate(program, pattern)
        || !match_input_has_builtin_meaning(program, ctx.operators, subject)
        || !match_input_has_builtin_meaning(program, ctx.operators, pattern)
    {
        return;
    }
    let mut occurrences = Vec::new();
    for expression in [subject, pattern] {
        crate::facts::contract_occurrences::append_expression_occurrences(
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

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
        append_guard_bounds_context(
            program,
            semantic,
            ctx,
            machine_symbol,
            state_symbol,
            statement_index,
            expression,
            point,
            active_contexts,
            active_constraints,
        );
    }
}

/// The satisfied arm of a comparison guard bounds each compared stable place
/// by the literal endpoint: `self.i >= 1 && self.i <= 9` carries
/// `self.i in [1, 9]` onto that edge. Conjuncts of one guard INTERSECT into a
/// single bounds fact per place, so `>=`/`<=` pairs keep their meet rather
/// than hulling to the carrier. Only literal endpoints bound here --
/// place-versus-place endpoints belong to the range checker's declared-range
/// seeders. Ordinary write invalidation retires the observation on
/// reassignment, and the destination's incoming-field meet drops it on edges
/// that do not all prove it.
#[allow(clippy::too_many_arguments)]
fn append_guard_bounds_context(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    point: ProgramPoint,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let Some(machine) = crate::lookup::machine_by_symbol(program, machine_symbol) else {
        return;
    };
    let Some(state) =
        crate::semantic_calls::find_state_in_machine(program, machine_symbol, state_symbol)
    else {
        return;
    };
    let mut conjuncts = Vec::new();
    // Arm tests arrive as `<guard> == true`; peel that wrapper off before
    // reading conjuncts, then split `&&` as the arm's own test would.
    append_true_arm_conjuncts(program, expression, &mut conjuncts);
    struct GuardBound {
        place: facts::PlaceHandle,
        carrier: facts::IntegerRange,
        lower: Option<numerics::bignum::BigInt>,
        upper: Option<numerics::bignum::BigInt>,
    }
    let mut bounds: Vec<GuardBound> = Vec::new();
    let literal = |operand: ExpressionHandle| match program.expression_table.expression(operand) {
        ExpressionNode::Integer(value) => value.value_bignum(),
        _ => None,
    };
    let mirrored = |operator: typed_trees::expression::BinaryOperator| {
        use typed_trees::expression::BinaryOperator as Operator;
        match operator {
            Operator::Less => Operator::Greater,
            Operator::LessOrEqual => Operator::GreaterOrEqual,
            Operator::Greater => Operator::Less,
            Operator::GreaterOrEqual => Operator::LessOrEqual,
            other => other,
        }
    };
    for conjunct in conjuncts {
        if !validation::has_builtin_decomposed_guard_meaning(
            program,
            machine,
            Some(state),
            conjunct,
        ) {
            continue;
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(conjunct) else {
            continue;
        };
        let (operand, endpoint, operator) = match (literal(binary.left), literal(binary.right)) {
            (None, Some(endpoint)) => (binary.left, endpoint, binary.operator),
            (Some(endpoint), None) => (binary.right, endpoint, mirrored(binary.operator)),
            _ => continue,
        };
        let one = numerics::bignum::BigInt::from_u64(1);
        let (lower, upper) = match operator {
            typed_trees::expression::BinaryOperator::Less => (None, Some(endpoint.sub(&one))),
            typed_trees::expression::BinaryOperator::LessOrEqual => (None, Some(endpoint)),
            typed_trees::expression::BinaryOperator::Greater => (Some(endpoint.add(&one)), None),
            typed_trees::expression::BinaryOperator::GreaterOrEqual => (Some(endpoint), None),
            typed_trees::expression::BinaryOperator::Equal => {
                (Some(endpoint.clone()), Some(endpoint))
            }
            _ => continue,
        };
        let Some(place) = crate::semantic_places::canonical_place_to_fact_place_in_state(
            program,
            semantic,
            state_symbol,
            statement_index,
            operand,
        ) else {
            continue;
        };
        let Some(carrier) = crate::flow::expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            operand,
        )
        .and_then(|reference| program.primitive_type_reference(reference))
        .and_then(crate::values::bounds::primitive_range) else {
            continue;
        };
        // `append_place` allocates a fresh handle per call, so place identity
        // is structural here: same root and same segment chain, the same place
        // the readers compare by contents.
        let row = match bounds.iter_mut().position(|row| {
            let (stored, candidate) = (semantic.places.get(row.place), semantic.places.get(place));
            stored.root == candidate.root
                && semantic.place_segments.span_or_empty(stored.segments)
                    == semantic.place_segments.span_or_empty(candidate.segments)
        }) {
            Some(index) => &mut bounds[index],
            None => {
                bounds.push(GuardBound {
                    place,
                    carrier,
                    lower: None,
                    upper: None,
                });
                bounds.last_mut().expect("pushed")
            }
        };
        if let Some(lower) = lower {
            row.lower = Some(match row.lower.take() {
                Some(previous) => previous.max(lower),
                None => lower,
            });
        }
        if let Some(upper) = upper {
            row.upper = Some(match row.upper.take() {
                Some(previous) => previous.min(upper),
                None => upper,
            });
        }
    }
    let mut refs = HandleSpan::empty();
    for row in bounds {
        if row.lower.is_none() && row.upper.is_none() {
            continue;
        }
        let mut range = facts::IntegerRange {
            minimum: row.lower.unwrap_or_else(|| row.carrier.minimum.clone()),
            maximum: row.upper.unwrap_or_else(|| row.carrier.maximum.clone()),
        };
        // The guard fact must refine the value's live interval, not widen it:
        // readers union every live bounds payload at the place, so a row that
        // reaches past the already-established extent would only union the
        // dead half back in. Intersect instead; an empty meet means this arm
        // is unreachable and mints nothing.
        let live = crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(row.place),
        )
        .and_then(|subject| {
            crate::values::integer_bounds_at_place(
                program,
                semantic,
                ctx.contexts
                    .semantic_context_refs
                    .span_or_empty(*active_contexts)
                    .iter()
                    .map(|reference| semantic.contexts.get(reference.context)),
                &subject,
            )
        });
        if let Some(live) = live {
            range.minimum = range.minimum.max(live.minimum);
            range.maximum = range.maximum.min(live.maximum);
        }
        if range.minimum > range.maximum {
            continue;
        }
        let bounds = semantic.integer_ranges.append(range);
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(row.place),
            point,
            origin: FactOrigin::TransitionGuard,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::AssignedIntegerBounds { bounds },
        });
        semantic.append_ref(&mut refs, fact);
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

/// Conjuncts asserted by taking the `true` arm: split `&&` recursively, and
/// peel the arm-test `<expr> == true` wrappers. `expr == false` under a true
/// arm is contradictory and contributes nothing.
fn append_true_arm_conjuncts(
    program: &typed_trees::TypedTrees,
    expression: ExpressionHandle,
    conjuncts: &mut Vec<ExpressionHandle>,
) {
    let boolean = |operand: ExpressionHandle| match program.expression_table.expression(operand) {
        ExpressionNode::Boolean(value) => Some(*value),
        _ => None,
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::And =>
        {
            append_true_arm_conjuncts(program, binary.left, conjuncts);
            append_true_arm_conjuncts(program, binary.right, conjuncts);
        }
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Equal =>
        {
            match (boolean(binary.left), boolean(binary.right)) {
                (None, Some(true)) => append_true_arm_conjuncts(program, binary.left, conjuncts),
                (Some(true), None) => append_true_arm_conjuncts(program, binary.right, conjuncts),
                (None, Some(false)) | (Some(false), None) => {}
                _ => conjuncts.push(expression),
            }
        }
        _ => conjuncts.push(expression),
    }
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

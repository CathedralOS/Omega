//! Selected predicates describe live storage, including comparisons against
//! constructed values. Construction is repeatable only when every explicit
//! child is repeatable; a literal case is not an opaque invocation. Retaining
//! the whole predicate lets a later short-circuit operand recover enclosing
//! tag evidence after an inner branch meet. Effectful children remain barriers,
//! and ordinary storage invalidation retires every retained dependency.

/// Wire `context` into the retained context/constraint rosters and record
/// the flow semantic-context reference and its constraint edge.
fn retain_semantic_context(
    build: &mut FlowBuildContext,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
    context: facts::FactContextHandle,
) {
    *active_contexts =
        retained_flow_contexts(&build.contexts.semantic_context_refs, *active_contexts);
    *active_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, *active_constraints);
    reference_spans::append_flow_reference(
        &mut build.contexts.semantic_context_refs,
        active_contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut build.contexts.constraint_refs,
        active_constraints,
        FlowConstraintKind::SemanticContext { context },
    );
}

use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::reference_spans;
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
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    transition_target: typed_trees::statement::TransitionTargetHandle,
    guard: TransitionGuardNode,
    value: bool,
    entry_writes: HandleSpan<checked_trees::FlowOperandReferent>,
    entry_unknown_write: bool,
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
        build,
        state_symbol,
        statement_index,
        expression,
        value,
        point,
        entry_writes,
        entry_unknown_write,
        active_contexts,
        active_constraints,
    );
    if value {
        append_case_constraint_context(
            program,
            semantic,
            build,
            state_symbol,
            statement_index,
            expression,
            point,
            active_contexts,
            active_constraints,
        );
        append_comparison_bounds_context(
            program,
            semantic,
            build,
            machine_symbol,
            state_symbol,
            statement_index,
            expression,
            point,
            FactOrigin::TransitionGuard,
            active_contexts,
            active_constraints,
        );
    }
}

/// A satisfied entry requirement or guard bounds each compared stable place
/// by the constant endpoint: `self.i >= 1 && self.i <= 9` carries
/// `self.i in [1, 9]`. Conjuncts of one expression INTERSECT into a
/// single bounds fact per place, so `>=`/`<=` pairs keep their meet rather
/// than hulling to the carrier. Only literal or type-owned fixed-length
/// endpoints bound here; place-versus-place endpoints belong to the range
/// checker's declared-range seeders. Ordinary write invalidation retires the observation on
/// reassignment, and the destination's incoming-field meet drops it on edges
/// that do not all prove it.
#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn append_comparison_bounds_context(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    point: ProgramPoint,
    origin: FactOrigin,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let Some(machine) = build
        .machine_index(program, machine_symbol)
        .map(|index| &program.machines()[index])
    else {
        return;
    };
    let Some(state) = build
        .state_index_in_machine(program, machine_symbol, state_symbol)
        .map(|index| &program.machine_states(machine)[index])
    else {
        return;
    };
    let mut float_predicates = Vec::new();
    crate::values::float_predicates::collect_true(
        program,
        machine,
        state,
        statement_index,
        expression,
        &mut float_predicates,
    );
    for (subject, predicate) in float_predicates {
        append_observation_context(
            program,
            semantic,
            build,
            state_symbol,
            statement_index,
            &[subject],
            FactPayload::FloatPredicate { predicate },
            point,
            origin,
            active_contexts,
            active_constraints,
        );
    }
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
        ExpressionNode::Member(_) => {
            let checked_trees::CheckedScalarExpression::IntegerLiteral { literal } =
                crate::values::lower_unit_scalar_argument(
                    program,
                    build.operators,
                    state,
                    statement_index,
                    operand,
                    typed_trees::types::PrimitiveType::U64,
                )?
            else {
                return None;
            };
            literal.value_bignum()
        }
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
        let Some(binary) = program.expression_table.binary_node(conjunct) else {
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
        let Some(place) = crate::semantic::places::canonical_place_to_fact_place_in_state(
            program,
            semantic,
            state_symbol,
            statement_index,
            operand,
        ) else {
            continue;
        };
        let Some(carrier) = validation::expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            operand,
        )
        .and_then(|reference| program.primitive_type_reference(reference))
        .and_then(validation::carrier_integer_range) else {
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
                build
                    .contexts
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
            origin,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::AssignedIntegerBounds { bounds },
        });
        semantic.append_ref(&mut refs, fact);
    }
    if refs.is_empty() {
        return;
    }
    let context = semantic.append_context(point, refs);
    retain_semantic_context(build, active_contexts, active_constraints, context);
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
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
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
        let Some(place) = crate::semantic::places::canonical_place_to_fact_place_in_state(
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
            if let Some(field) = member.as_field() {
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
            let Some(fact_expression) = program.proof_facts.get(fact_handle).as_expression() else {
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
    retain_semantic_context(build, active_contexts, active_constraints, context);
}

/// Conjuncts asserted by taking the `true` arm: split `&&` recursively, and
/// peel the arm-test `<expr> == true` wrappers. `expr == false` under a true
/// arm is contradictory and contributes nothing.
fn append_true_arm_conjuncts(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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

/// Signed observations guaranteed by the selected Boolean outcome, in
/// evaluation order. A true conjunction or false disjunction observes both
/// children. Other short-circuit outcomes establish no individual operand.
fn guard_observations(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    value: bool,
    observations: &mut Vec<(ExpressionHandle, bool)>,
) {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};
    match program.expression_table.expression(expression) {
        ExpressionNode::Unary(unary)
            if unary.operator == UnaryOperator::LogicalNot
                && validation::has_builtin_decomposed_guard_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                ) =>
        {
            guard_observations(program, machine, state, unary.operand, !value, observations);
            return;
        }
        ExpressionNode::Binary(binary)
            if (binary.operator == BinaryOperator::And && value)
                || (binary.operator == BinaryOperator::Or && !value) =>
        {
            guard_observations(program, machine, state, binary.left, value, observations);
            guard_observations(program, machine, state, binary.right, value, observations);
            return;
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) && validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) =>
        {
            let observed = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::Boolean(expected)) => Some((binary.left, *expected)),
                (ExpressionNode::Boolean(expected), _) => Some((binary.right, *expected)),
                _ => None,
            };
            if let Some((operand, expected)) = observed {
                let equal = value == (binary.operator == BinaryOperator::Equal);
                guard_observations(
                    program,
                    machine,
                    state,
                    operand,
                    expected == equal,
                    observations,
                );
                return;
            }
        }
        _ => {}
    }
    observations.push((expression, value));
}

/// Recognize `subject == Type::Case` (or its commute), the tag compare a
/// case-membership `in` test lowers to, and return the subject with the
/// resolved variant and its data definition.
fn case_membership_claim(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(
    ExpressionHandle,
    &typed_trees::data::DataVariant,
    &typed_trees::data::DataDefinition,
)> {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return None;
    };
    if binary.operator != typed_trees::expression::BinaryOperator::Equal {
        return None;
    }
    for (case_reference, subject) in [(binary.right, binary.left), (binary.left, binary.right)] {
        let Some(path) = program.expression_table.name_path_node(case_reference) else {
            continue;
        };
        let Some(definition) = program.data_definition_by_symbol(path.head_symbol) else {
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
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    value: bool,
    point: ProgramPoint,
    entry_writes: HandleSpan<checked_trees::FlowOperandReferent>,
    entry_unknown_write: bool,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    append_entry_path_context(
        program,
        semantic,
        build,
        state_symbol,
        statement_index,
        expression,
        value,
        point,
        entry_writes,
        entry_unknown_write,
        active_contexts,
        active_constraints,
    );
    // A tag observation describes the evaluated storage, not the expression
    // that produced it. In particular a saved dispatch result keeps its
    // canonical occurrence without executing its call again. Retain the
    // tag on that exact place before the generic predicate's repeatability
    // check; a call-containing predicate itself remains non-repeatable.
    if let Some((machine_position, state_position)) = build.state_location(program, state_symbol) {
        let machine = &program.machines()[machine_position];
        let state = &program.machine_states(machine)[state_position];
        // Each signed observation held when it was evaluated, and the
        // last one still holds at guard completion. An earlier one was
        // followed by its successors' effects, and the short-circuit meet
        // with the skipped path has already dropped its tag. It survives
        // exactly when no write of this guard can reach its subject; a
        // later conjunct that replaces the sum, or an unknown write,
        // retires it.
        let mut observations = Vec::new();
        guard_observations(
            program,
            machine,
            state,
            expression,
            value,
            &mut observations,
        );
        let last = observations.len().saturating_sub(1);
        for (position, (observation, selected)) in observations.into_iter().enumerate() {
            let Some(binary) = program.expression_table.binary_node(observation) else {
                continue;
            };
            if !validation::has_exact_case_membership_meaning(
                program,
                machine,
                Some(state),
                observation,
                binary,
            ) {
                continue;
            }
            let ExpressionNode::Name(case) = program.expression_table.expression(binary.right)
            else {
                continue;
            };
            if position != last
                && !validation::canonical_place_from_expression_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    binary.left,
                )
                .is_some_and(|subject| {
                    crate::flow::entry_origin_is_stable(
                        program,
                        &build.control,
                        entry_writes,
                        entry_unknown_write,
                        &subject,
                    )
                })
            {
                continue;
            }
            append_observation_context(
                program,
                semantic,
                build,
                state_symbol,
                statement_index,
                &[binary.left],
                if selected {
                    FactPayload::AssignedCase {
                        variant: case.symbol,
                    }
                } else {
                    FactPayload::BooleanValue {
                        expression: observation,
                        value: false,
                    }
                },
                point,
                FactOrigin::TransitionGuard,
                active_contexts,
                active_constraints,
            );
        }
    }
    // A `pred(place)` scrutinee taken true is whole-carrier evidence: every
    // byte currently at the operand's place sits in the named per-byte class.
    // The fact rides the operand's place, not the call's spelling, so any
    // downstream `requires pred(..)` or predicate-domain write discharges
    // wherever the operand travels. The refused arm mints nothing — a byte
    // carrier failing the class is not a positive class.
    if value {
        append_byte_predicate_context(
            program,
            semantic,
            build,
            state_symbol,
            statement_index,
            expression,
            point,
            FactOrigin::TransitionGuard,
            active_contexts,
            active_constraints,
        );
    }
    // An effectful invocation's result is not a promise about evaluating that
    // invocation again. A saved local boolean is an ordinary stable place.
    if !expression_is_stable_predicate(program, expression) {
        return;
    }
    if let Some(predicate) =
        build.guard_predicate_at(program, state_symbol, statement_index, expression, value)
    {
        let context = crate::facts::boolean_predicates::seed(
            semantic,
            predicate,
            point,
            FactOrigin::TransitionGuard,
        );
        retain_semantic_context(build, active_contexts, active_constraints, context);
    }
    let occurrences = build.expression_occurrences_at(program, expression);
    append_observation_context(
        program,
        semantic,
        build,
        state_symbol,
        statement_index,
        occurrences.as_slice(),
        FactPayload::BooleanValue { expression, value },
        point,
        FactOrigin::TransitionGuard,
        active_contexts,
        active_constraints,
    );
}

#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn append_match_pattern_context(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    payload: FactPayload,
    point: ProgramPoint,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    if let FactPayload::MatchPattern {
        expression,
        matched,
        ..
    } = payload
    {
        append_entry_path_context(
            program,
            semantic,
            build,
            state_symbol,
            statement_index,
            expression,
            matched,
            point,
            HandleSpan::empty(),
            true,
            active_contexts,
            active_constraints,
        );
    }
    let Some((subject, pattern, _)) = payload.match_pattern_comparison(program) else {
        return;
    };
    let FactPayload::MatchPattern { expression, .. } = payload else {
        return;
    };
    let Some(dispatch) = program.expression_table.match_node(expression) else {
        return;
    };
    // Selected float equality need not mean mathematical equality. Missing
    // selection is rejected by checking, never promoted to builtin evidence.
    if matches!(
        validation::match_subject_primitive_type(program, dispatch),
        Some(typed_trees::types::PrimitiveType::F32 | typed_trees::types::PrimitiveType::F64)
    ) || build.operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && operator_use.occurrence != checked_trees::CheckedOperatorOccurrence::Expression
    }) {
        return;
    }
    if !expression_is_stable_predicate(program, subject)
        || !expression_is_stable_predicate(program, pattern)
        || !match_input_has_builtin_meaning(program, build.operators, subject)
        || !match_input_has_builtin_meaning(program, build.operators, pattern)
    {
        return;
    }
    let mut occurrences = Vec::new();
    for expression in [subject, pattern] {
        occurrences.extend(
            build
                .expression_occurrences_at(program, expression)
                .iter()
                .copied(),
        );
    }
    append_observation_context(
        program,
        semantic,
        build,
        state_symbol,
        statement_index,
        &occurrences,
        payload,
        point,
        FactOrigin::TransitionGuard,
        active_contexts,
        active_constraints,
    );
}

fn match_input_has_builtin_meaning(
    program: &dyn ::validation::CheckingInput,
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

/// Mint a `pred(place)` byte-predicate fact at the operand's places: the
/// operand of a unary byte-predicate probe is a byte carrier, and a positive
/// probe verdict is whole-carrier evidence (`FactPayload::BytePredicate`'s own
/// semantics). The one operation every mint site shares — transition guards
/// carrying the taken arm, and state/machine `requires` clauses materializing
/// their entry assumptions — so the predicate is consumed identically
/// wherever it is proved. Arm tests arrive as `pred(..) == true` (peeled by
/// the same conjunct walk the bounds minter uses); `pred(..) == false`
/// contributes nothing, a refused probe is not a positive class.
#[allow(clippy::too_many_arguments)]
pub(in crate::flow) fn append_byte_predicate_context(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    point: ProgramPoint,
    origin: FactOrigin,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let mut conjuncts = Vec::new();
    append_true_arm_conjuncts(program, expression, &mut conjuncts);
    for conjunct in conjuncts {
        let Some((predicate, operand)) =
            typed_trees::byte_predicates::byte_predicate_call(program, conjunct)
        else {
            continue;
        };
        let occurrences = build.expression_occurrences_at(program, operand);
        append_observation_context(
            program,
            semantic,
            build,
            state_symbol,
            statement_index,
            occurrences.as_slice(),
            FactPayload::BytePredicate { predicate },
            point,
            origin,
            active_contexts,
            active_constraints,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_observation_context(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    occurrences: &[ExpressionHandle],
    payload: FactPayload,
    point: ProgramPoint,
    origin: FactOrigin,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let mut places = Vec::new();
    for occurrence in occurrences.iter().copied() {
        let Some(place) = crate::semantic::places::canonical_place_to_fact_place_in_state(
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
            origin,
            evidence: QualificationEvidence::default(),
            payload,
        });
        semantic.append_ref(&mut refs, fact);
    }
    let context = semantic.append_context(point, refs);
    retain_semantic_context(build, active_contexts, active_constraints, context);
}

fn expression_is_stable_predicate(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => true,
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| expression_is_stable_predicate(program, field.value)),
        ExpressionNode::ArrayLiteral(values) => program
            .expression_table
            .expression_handles(*values)
            .iter()
            .all(|value| expression_is_stable_predicate(program, *value)),
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
        // A selected primitive observation is repeatable while its operand
        // storage is preserved. Observation contexts retain those operand
        // places, so ordinary alias-aware invalidation owns both polarities.
        ExpressionNode::Call(_) => {
            typed_trees::byte_predicates::byte_predicate_call(program, expression)
                .is_some_and(|(_, operand)| expression_is_stable_predicate(program, operand))
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn append_entry_path_context(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    value: bool,
    point: ProgramPoint,
    entry_writes: HandleSpan<checked_trees::FlowOperandReferent>,
    entry_unknown_write: bool,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let Some(context) = crate::facts::boolean_predicates::capture_entry_guard(
        program,
        build.operators,
        semantic,
        state,
        expression,
        value,
        point,
        &mut |leaf| {
            build.entry_subject_at(
                program,
                state,
                statement_index,
                leaf,
                entry_writes,
                entry_unknown_write,
            )
        },
    ) else {
        return;
    };
    retain_semantic_context(build, active_contexts, active_constraints, context);
}

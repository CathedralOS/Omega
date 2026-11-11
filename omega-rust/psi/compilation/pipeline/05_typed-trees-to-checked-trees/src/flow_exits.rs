/*
    Transition and exit facts. `append_transition_flow_facts` evaluates a guard,
    then for the target and continuation arms adds the guard outcome (`guards`),
    evaluates the arm, and records its exit facts. `append_state_exit_facts`
    records one `FlowExitFact` for each matching contract exit.
*/

use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::append_flow_contexts_for_points;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::HandleSpan;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowCallFact, FlowConstraintKind, FlowConstraintRef,
    FlowExitFact, FlowSemanticContextRef, ProofFacts,
};
use facts::{FactPlan, ProgramPoint};
use symbols::SymbolHandle;

mod guards {

    /*
        Selected predicates describe live storage, including comparisons against
        constructed values. Construction is repeatable only when every explicit
        child is repeatable; a literal case is not an opaque invocation. Retaining
        the whole predicate lets a later short-circuit operand recover enclosing
        tag evidence after an inner branch meet. Effectful children remain barriers,
        and ordinary storage invalidation retires every retained dependency.
    */
    /*
        Wire `context` into the retained context/constraint rosters and record
        the flow semantic-context reference and its constraint edge.
    */
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

    use super::super::case_tag_segments_may_overlap;
    use super::super::observations::case_observation;
    use super::super::observations::predicate_case_dependencies;
    use super::super::operand_contexts::ContentObservationPoint;
    use super::super::operand_contexts::SourceWriteRelation;
    use super::super::operand_contexts::contents_observation_survives;
    use super::super::operand_contexts::contents_overlap;
    use super::super::reference_spans::appended_span_since;
    use super::super::stable_case_subject;
    use crate::flow::append_constraint_ref;
    use crate::flow::expressions;
    use crate::flow::guard_predicates::{expression_is_stable_predicate, guard_observations};
    use crate::flow::reference_spans;
    use crate::flow::retained_constraint_refs;
    use crate::flow::retained_flow_contexts;
    use crate::flow::{FlowBuildContext, GuardObservation};
    use crate::published::facts::boolean_predicates;
    use crate::published::semantic::places::append_place;
    use crate::shared::authored_selections::typed_operator_has_no_authored_selection;
    use crate::values;
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
        append_boolean_observations(
            program,
            semantic,
            build,
            machine_symbol,
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
    }

    /*
        Every selective Boolean consumer publishes one evaluated decision. Expression
        matches and transitions share capture recovery, polarity, storage epochs and
        bound grouping; neither executes the saved predicate again.
    */
    #[allow(clippy::too_many_arguments)]
    fn append_boolean_observations(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        machine_symbol: SymbolHandle,
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
        let captured = build.state_location(program, state_symbol).map_or_else(
            Vec::new,
            |(machine_position, state_position)| {
                let machine = &program.machines()[machine_position];
                crate::flow::saved_guard_observations(
                    program,
                    machine,
                    &program.machine_states(machine)[state_position],
                    statement_index,
                    expression,
                    value,
                    &[],
                    build.call_frames,
                )
            },
        );
        let mut bound_expressions: Vec<ExpressionHandle> = Vec::new();
        let current = GuardObservation::evaluated(expression, value, statement_index);
        for observation in std::iter::once(current).chain(captured) {
            let GuardObservation {
                expression, value, ..
            } = observation;
            append_predicate_context(
                program,
                semantic,
                build,
                state_symbol,
                statement_index,
                observation,
                point,
                entry_writes,
                entry_unknown_write,
                active_contexts,
                active_constraints,
            );
            if value && observation.current_expression.is_valid() {
                append_case_constraint_context(
                    program,
                    semantic,
                    build,
                    state_symbol,
                    observation.statement_index,
                    expression,
                    point,
                    active_contexts,
                    active_constraints,
                );
                bound_expressions.push(expression);
            }
        }
        if !bound_expressions.is_empty() {
            append_comparison_bounds_context(
                program,
                semantic,
                build,
                machine_symbol,
                state_symbol,
                statement_index,
                &bound_expressions,
                point,
                FactOrigin::TransitionGuard,
                active_contexts,
                active_constraints,
            );
        }
    }

    /*
        A satisfied entry requirement or guard bounds each compared stable place
        by the constant endpoint: `self.i >= 1 && self.i <= 9` carries
        `self.i in [1, 9]`. Conjuncts of one expression INTERSECT into a
        single bounds fact per place, so `>=`/`<=` pairs keep their meet rather
        than hulling to the carrier. Only literal or type-owned fixed-length
        endpoints bound here; place-versus-place endpoints belong to the range
        checker's declared-range seeders. Ordinary write invalidation retires the observation on
        reassignment, and the destination's incoming-field meet drops it on edges
        that do not all prove it.
    */
    #[allow(clippy::too_many_arguments)]
    pub(in crate::flow) fn append_comparison_bounds_context(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expressions: &[ExpressionHandle],
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
        for &expression in expressions {
            let mut float_predicates = Vec::new();
            values::float_predicates::collect_true(
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
                    &[],
                    FactPayload::FloatPredicate { predicate },
                    point,
                    origin,
                    active_contexts,
                    active_constraints,
                );
            }
        }
        let mut conjuncts = Vec::new();
        for &expression in expressions {
            if !validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) {
                continue;
            }
            append_true_arm_conjuncts(program, expression, &mut conjuncts);
        }
        if conjuncts.is_empty() {
            return;
        }
        struct GuardBound {
            place: facts::PlaceHandle,
            carrier: facts::IntegerRange,
            interval: validation::BigIntInterval,
        }
        let mut bounds: Vec<GuardBound> = Vec::new();
        let literal = |operand: ExpressionHandle| match program.expression_table.expression(operand)
        {
            ExpressionNode::Integer(value) => value.value_bignum(),
            ExpressionNode::Member(_) => {
                let checked_trees::CheckedScalarExpression::IntegerLiteral { literal } =
                    values::lower_unit_scalar_argument(
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
            let (operand, endpoint, subject_on_left) =
                match (literal(binary.left), literal(binary.right)) {
                    (None, Some(endpoint)) => (binary.left, endpoint, true),
                    (Some(endpoint), None) => (binary.right, endpoint, false),
                    _ => continue,
                };
            let Some(interval) = validation::BigIntInterval::from_comparison(
                binary.operator,
                subject_on_left,
                endpoint,
            ) else {
                continue;
            };
            let Some(place) =
                build.completed_read_place_at(program, state_symbol, statement_index, operand)
            else {
                continue;
            };
            let place = append_place(semantic, place.root, &place.segments);
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
            let row = match bounds.iter_mut().position(|row| {
                let (stored, candidate) =
                    (semantic.places.get(row.place), semantic.places.get(place));
                stored.root == candidate.root
                    && semantic.place_segments.span_or_empty(stored.segments)
                        == semantic.place_segments.span_or_empty(candidate.segments)
            }) {
                Some(index) => &mut bounds[index],
                None => {
                    bounds.push(GuardBound {
                        place,
                        carrier,
                        interval: validation::BigIntInterval::unbounded(),
                    });
                    bounds.last_mut().expect("pushed")
                }
            };
            row.interval = row.interval.intersect(&interval);
        }
        let mut refs = HandleSpan::empty();
        for row in bounds {
            if row.interval == validation::BigIntInterval::unbounded() {
                continue;
            }
            let mut range = facts::IntegerRange {
                minimum: row.interval.low.unwrap_or(row.carrier.minimum),
                maximum: row.interval.high.unwrap_or(row.carrier.maximum),
            };
            let live = crate::flow::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(row.place),
            )
            .and_then(|subject| {
                values::integer_bounds_at_place(
                    program,
                    semantic,
                    build.semantic_context_views(semantic, *active_contexts),
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

    /*
        CASE-CONSTRAINTS (ch12): on the matched branch a case-membership guard
        (`subject in Type::Case`, lowered to `subject == Type::Case`) contributes
        the selected case's `requires` propositions, substituted onto the exact
        subject -- matching an established constrained case is how its
        constraints become arm-local facts (wiki/spec/language/patterns.md). The
        clause's payload and common-field names rebase to `subject.<field>`
        through the same canonical-term renderer call contracts use, so a
        `requires` proven against the destructured payload sees the contributed
        identity. Facts ride on the subject's canonical place, so ordinary write
        invalidation retires them exactly like the guard's own evidence. Generic
        data is refused upstream, so head/case symbols alone identify the
        variant; membership and proposition-application clauses still wait for
        their carriers.
    */
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
        let Some((machine_position, state_position)) = build.state_location(program, state_symbol)
        else {
            return;
        };
        let machine = &program.machines()[machine_position];
        let state = &program.machine_states(machine)[state_position];
        let mut conjuncts = Vec::new();
        flatten_and_conjuncts(program, expression, &mut conjuncts);
        let mut refs = HandleSpan::empty();
        for conjunct in conjuncts {
            let Some(binary) = program.expression_table.binary_node(conjunct) else {
                continue;
            };
            let Some(selected) = validation::applied_case_membership_variant(
                program,
                machine,
                Some(state),
                conjunct,
                binary,
            ) else {
                continue;
            };
            let Some((owner_position, _)) = program.data_variant_position_for_symbol(selected)
            else {
                continue;
            };
            let definition = &program.data_definitions()[owner_position as usize];
            let Some(variant) = program.data_variant(definition, selected) else {
                continue;
            };
            let subject = binary.left;
            if variant.requires_facts.is_empty() {
                continue;
            }
            let Some(place) =
                crate::published::semantic::places::canonical_place_to_fact_place_in_state(
                    program,
                    semantic,
                    state_symbol,
                    statement_index,
                    subject,
                )
            else {
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
            for offset in 0..variant.requires_facts.count() {
                let fact_handle = Handle::from_parts(
                    variant
                        .requires_facts
                        .start()
                        .arena_index()
                        .saturating_add(offset),
                    variant.requires_facts.start().generation(),
                );
                let Some(fact_expression) = program.proof_facts.get(fact_handle).as_expression()
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
        retain_semantic_context(build, active_contexts, active_constraints, context);
    }

    /*
        Conjuncts asserted by taking the `true` arm: split `&&` recursively, and
        peel the arm-test `<expr> == true` wrappers. `expr == false` under a true
        arm is contradictory and contributes nothing.
    */
    fn append_true_arm_conjuncts(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        conjuncts: &mut Vec<ExpressionHandle>,
    ) {
        let boolean = |operand: ExpressionHandle| match program.expression_table.expression(operand)
        {
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
                    (None, Some(true)) => {
                        append_true_arm_conjuncts(program, binary.left, conjuncts)
                    }
                    (Some(true), None) => {
                        append_true_arm_conjuncts(program, binary.right, conjuncts)
                    }
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

    #[allow(clippy::too_many_arguments)]
    pub(in crate::flow) fn append_predicate_context(
        program: &dyn ::validation::CheckingInput,
        semantic: &mut FactPlan,
        build: &mut FlowBuildContext,
        state_symbol: SymbolHandle,
        statement_index: usize,
        observation: GuardObservation,
        point: ProgramPoint,
        entry_writes: HandleSpan<checked_trees::FlowOperandReferent>,
        entry_unknown_write: bool,
        active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        let GuardObservation {
            expression,
            value,
            statement_index: observation_statement_index,
            current_expression,
        } = observation;

        if current_expression.is_valid() {
            append_entry_path_context(
                program,
                semantic,
                build,
                state_symbol,
                observation_statement_index,
                expression,
                value,
                point,
                entry_writes,
                entry_unknown_write,
                active_contexts,
                active_constraints,
            );
        }
        let predicate_dependencies = predicate_case_dependencies(
            program,
            build,
            state_symbol,
            observation_statement_index,
            expression,
        );
        let repeatable = expression_is_stable_predicate(program, expression);
        if let Some((machine_position, state_position)) =
            build.state_location(program, state_symbol)
        {
            let machine = &program.machines()[machine_position];
            let state = &program.machine_states(machine)[state_position];
            let mut observations = Vec::new();
            guard_observations(
                program,
                machine,
                state,
                expression,
                value,
                &mut observations,
            );
            if !repeatable && current_expression.is_valid() {
                for (observation, selected) in observations.iter().copied() {
                    if !expression_is_stable_predicate(program, observation) {
                        continue;
                    }
                    let Some(dependencies) = predicate_case_dependencies(
                        program,
                        build,
                        state_symbol,
                        observation_statement_index,
                        observation,
                    ) else {
                        continue;
                    };
                    let Some(predicate) = build.completed_guard_predicate_at(
                        program,
                        state_symbol,
                        observation_statement_index,
                        observation,
                        selected,
                        entry_writes,
                        entry_unknown_write,
                    ) else {
                        continue;
                    };
                    if predicate
                        .bindings
                        .iter()
                        .map(|binding| &binding.place)
                        .chain(dependencies.places.iter())
                        .any(|place| {
                            !stable_case_subject(program, machine, state, statement_index, place)
                                || !super::super::entry_origin_is_stable(
                                    program,
                                    &build.control,
                                    entry_writes,
                                    entry_unknown_write,
                                    place,
                                )
                        })
                    {
                        continue;
                    }
                    let context = boolean_predicates::seed_with_dependencies(
                        semantic,
                        &predicate,
                        point,
                        FactOrigin::TransitionGuard,
                        &dependencies.places,
                    );
                    retain_descriptor_observations(build, context, &dependencies.descriptors);
                    retain_semantic_context(build, active_contexts, active_constraints, context);
                }
            }
            for (observation, selected) in observations {
                let Some(binary) = program.expression_table.binary_node(observation) else {
                    continue;
                };
                let Some(case) = validation::applied_case_membership_variant(
                    program,
                    machine,
                    Some(state),
                    observation,
                    binary,
                ) else {
                    continue;
                };
                let Some(observed) = case_observation(
                    program,
                    build,
                    state_symbol,
                    observation_statement_index,
                    observation,
                    binary.left,
                ) else {
                    continue;
                };
                let statements =
                    appended_span_since(&build.control.statements, build.state_statements_start);
                let first = ContentObservationPoint {
                    state: state_symbol,
                    statement: observation_statement_index,
                    writes: observed.origin_writes,
                    unknown_write: observed.origin_unknown_write,
                };
                let second = ContentObservationPoint {
                    state: state_symbol,
                    statement: statement_index,
                    writes: entry_writes,
                    unknown_write: entry_unknown_write,
                };
                let write_changes: SourceWriteRelation = if observed.descriptor.is_valid() {
                    contents_overlap
                } else {
                    |program, place, written| {
                        case_tag_segments_may_overlap(program, &place.segments, written)
                    }
                };
                if std::iter::once(&observed.subject)
                    .chain(&observed.dependencies)
                    .any(|subject| {
                        !contents_observation_survives(
                            program,
                            &build.control,
                            statements,
                            first,
                            second,
                            subject,
                            write_changes,
                        )
                    })
                {
                    continue;
                }
                let place =
                    append_place(semantic, observed.subject.root, &observed.subject.segments);
                let dependencies = observed
                    .dependencies
                    .iter()
                    .map(|dependency| {
                        FactPlace::Place(append_place(
                            semantic,
                            dependency.root,
                            &dependency.segments,
                        ))
                    })
                    .collect::<Vec<_>>();
                let context = crate::published::semantic::facts::append_observation_context(
                    semantic,
                    Fact {
                        place: FactPlace::Place(place),
                        point,
                        origin: FactOrigin::TransitionGuard,
                        evidence: QualificationEvidence::default(),
                        payload: FactPayload::CaseValue {
                            variant: case,
                            value: selected,
                        },
                    },
                    &dependencies,
                );
                if observed.descriptor.is_valid() {
                    build.control.descriptor_context_observations.append(
                        checked_trees::FlowDescriptorContextObservation {
                            context,
                            observation: observed.descriptor,
                        },
                    );
                }
                retain_semantic_context(build, active_contexts, active_constraints, context);
            }
        }
        if value && current_expression.is_valid() {
            append_byte_predicate_context(
                program,
                semantic,
                build,
                state_symbol,
                observation_statement_index,
                expression,
                point,
                FactOrigin::TransitionGuard,
                active_contexts,
                active_constraints,
            );
        }
        if !repeatable || !current_expression.is_valid() {
            return;
        }
        let Some(dependencies) = predicate_dependencies else {
            return;
        };
        if dependencies.places.iter().any(|dependency| {
            !crate::flow::entry_origin_is_stable(
                program,
                &build.control,
                entry_writes,
                entry_unknown_write,
                dependency,
            )
        }) {
            return;
        }
        if let Some(predicate) = build.completed_guard_predicate_at(
            program,
            state_symbol,
            observation_statement_index,
            expression,
            value,
            entry_writes,
            entry_unknown_write,
        ) {
            let context = boolean_predicates::seed_with_dependencies(
                semantic,
                &predicate,
                point,
                FactOrigin::TransitionGuard,
                &dependencies.places,
            );
            retain_descriptor_observations(build, context, &dependencies.descriptors);
            retain_semantic_context(build, active_contexts, active_constraints, context);
        }
        let occurrences = build.expression_occurrences_at(program, expression);
        let context = append_observation_context(
            program,
            semantic,
            build,
            state_symbol,
            observation_statement_index,
            occurrences.as_slice(),
            &dependencies.places,
            FactPayload::BooleanValue { expression, value },
            point,
            FactOrigin::TransitionGuard,
            active_contexts,
            active_constraints,
        );
        retain_descriptor_observations(build, context, &dependencies.descriptors);
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
        entry_writes: HandleSpan<checked_trees::FlowOperandReferent>,
        entry_unknown_write: bool,
        active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) {
        if let FactPayload::MatchPattern {
            expression,
            arm,
            matched,
        } = payload
            && let Some(dispatch) = program.expression_table.match_node(expression)
            && let Some((machine_position, state_position)) =
                build.state_location(program, state_symbol)
            && let Some(offset) = arm
                .arena_index()
                .checked_sub(dispatch.arms.start().arena_index())
            && dispatch.arms.handles().nth(offset as usize) == Some(arm)
            && let Some(source_arm) = expressions(program)
                .match_arms(dispatch.arms)
                .get(offset as usize)
        {
            let machine = &program.machines()[machine_position];
            let state = &program.machine_states(machine)[state_position];
            if let Some(case) = validation::structural_match_pattern_case(
            program,
            machine,
            state,
            dispatch,
            source_arm.pattern,
        ) && case.is_valid()
            // Success selects the case even for a guarded pattern. Failure
            // excludes it only when the whole pattern tests that case alone.
            && (matched || validation::structural_match_pattern_is_total(program, source_arm.pattern))
            {
                let alias = if dispatch.capture.is_valid() {
                    expressions(program)
                        .value_capture(dispatch.capture)
                        .value_anchor
                } else {
                    ExpressionHandle::invalid()
                };
                let subject = if alias.is_valid() {
                    alias
                } else {
                    dispatch.subject
                };
                append_observation_context(
                    program,
                    semantic,
                    build,
                    state_symbol,
                    statement_index,
                    &[subject],
                    &[],
                    FactPayload::CaseValue {
                        variant: case,
                        value: matched,
                    },
                    point,
                    FactOrigin::TransitionGuard,
                    active_contexts,
                    active_constraints,
                );
                let source = build.canonical_place_at(
                    program,
                    state_symbol,
                    statement_index,
                    dispatch.subject,
                );
                let observed = build.completed_read_place_at(
                    program,
                    state_symbol,
                    statement_index,
                    dispatch.subject,
                );
                if alias.is_valid()
                    && dispatch.subject.is_valid()
                    && alias != dispatch.subject
                    && source.is_some()
                    && observed == source
                {
                    append_observation_context(
                        program,
                        semantic,
                        build,
                        state_symbol,
                        statement_index,
                        &[dispatch.subject],
                        &[],
                        FactPayload::CaseValue {
                            variant: case,
                            value: matched,
                        },
                        point,
                        FactOrigin::TransitionGuard,
                        active_contexts,
                        active_constraints,
                    );
                }
            }
        }
        let Some((subject, pattern, matched)) = payload.match_pattern_comparison(program) else {
            return;
        };
        let FactPayload::MatchPattern { expression, .. } = payload else {
            return;
        };
        let Some(dispatch) = program.expression_table.match_node(expression) else {
            return;
        };
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
        if let ExpressionNode::Boolean(expected) = program.expression_table.expression(pattern)
            && validation::match_subject_primitive_type(program, dispatch)
                == Some(typed_trees::types::PrimitiveType::Bool)
            && let Some((machine_position, _)) = build.state_location(program, state_symbol)
        {
            let machine_symbol = program.machines()[machine_position].symbol;
            append_boolean_observations(
                program,
                semantic,
                build,
                machine_symbol,
                state_symbol,
                statement_index,
                subject,
                matched == *expected,
                point,
                entry_writes,
                entry_unknown_write,
                active_contexts,
                active_constraints,
            );
        }
        if let Some(predicate) = boolean_predicates::bind_current_match(
            program,
            build.operators,
            state_symbol,
            statement_index,
            subject,
            pattern,
            matched,
        )
        .and_then(|predicate| {
            build.complete_predicate_bindings(program, state_symbol, statement_index, predicate)
        }) {
            if let Some(expression) = build.entry_predicate_at(
                program,
                state_symbol,
                statement_index,
                &predicate,
                entry_writes,
                entry_unknown_write,
            ) && let Some(context) = boolean_predicates::capture_entry_expression(
                program,
                semantic,
                state_symbol,
                Some(expression),
                point,
            ) {
                retain_semantic_context(build, active_contexts, active_constraints, context);
            }
            if !build.active_completed_copies.is_empty() {
                let context = boolean_predicates::seed(
                    semantic,
                    &predicate,
                    point,
                    FactOrigin::TransitionGuard,
                );
                retain_semantic_context(build, active_contexts, active_constraints, context);
            }
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
            &[],
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
                values::operator_is_builtin(operators, expression)
                    && typed_operator_has_no_authored_selection(program, expression)
                    && recurse(binary.left)
                    && recurse(binary.right)
            }
            ExpressionNode::Unary(unary) => {
                values::operator_is_builtin(operators, expression)
                    && typed_operator_has_no_authored_selection(program, expression)
                    && recurse(unary.operand)
            }
            ExpressionNode::Indexed(indexed) => {
                values::operator_is_builtin(operators, expression)
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

    /*
        Mint a `pred(place)` byte-predicate fact at the operand's places: the
        operand of a unary byte-predicate probe is a byte carrier, and a positive
        probe verdict is whole-carrier evidence (`FactPayload::BytePredicate`'s own
        semantics). The one operation every mint site shares — transition guards
        carrying the taken arm, and state/machine `requires` clauses materializing
        their entry assumptions — so the predicate is consumed identically
        wherever it is proved. Arm tests arrive as `pred(..) == true` (peeled by
        the same conjunct walk the bounds minter uses); `pred(..) == false`
        contributes nothing, a refused probe is not a positive class.
    */
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
                &[],
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
        dependencies: &[validation::CanonicalPlace],
        payload: FactPayload,
        point: ProgramPoint,
        origin: FactOrigin,
        active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
        active_constraints: &mut HandleSpan<FlowConstraintRef>,
    ) -> facts::FactContextHandle {
        let mut places = Vec::new();
        for occurrence in occurrences.iter().copied() {
            let Some(source) =
                build.canonical_place_at(program, state_symbol, statement_index, occurrence)
            else {
                return Handle::invalid();
            };
            let Some(observed) = build.completed_read_place(
                program,
                state_symbol,
                statement_index,
                occurrence,
                &source,
            ) else {
                return Handle::invalid();
            };
            if matches!(
                payload,
                FactPayload::BooleanValue { .. } | FactPayload::MatchPattern { .. }
            ) && observed != source
            {
                return Handle::invalid();
            }
            let place = append_place(semantic, observed.root, &observed.segments);
            crate::shared::collections::push_unique(&mut places, place);
        }
        let mut refs = HandleSpan::empty();
        let mut dependent = Handle::invalid();
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
            if !dependent.is_valid() {
                dependent = fact;
            }
            semantic.append_ref(&mut refs, fact);
        }
        for dependency in dependencies {
            let place = append_place(semantic, dependency.root, &dependency.segments);
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin,
                evidence: QualificationEvidence::default(),
                payload: FactPayload::StorageDependency { dependent },
            });
            semantic.append_ref(&mut refs, fact);
        }
        let context = semantic.append_context(point, refs);
        retain_semantic_context(build, active_contexts, active_constraints, context);
        context
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
        let Some(context) = boolean_predicates::capture_entry_guard(
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

    fn retain_descriptor_observations(
        build: &mut FlowBuildContext,
        context: facts::FactContextHandle,
        observations: &[Handle<checked_trees::FlowValueObservationFact>],
    ) {
        if !context.is_valid() {
            return;
        }
        for observation in observations {
            build.control.descriptor_context_observations.append(
                checked_trees::FlowDescriptorContextObservation {
                    context,
                    observation: *observation,
                },
            );
        }
    }
}
pub(super) use guards::{
    append_byte_predicate_context, append_comparison_bounds_context, append_match_pattern_context,
    append_predicate_context,
};

/*
    The parameter origins one exit retained, in roster order. Every contract
    consumer reads the same two-level arena — the exit's own span, then each
    origin's row — so the read lives here once.
*/
pub(crate) fn exit_parameter_origins<'a>(
    facts: checked_trees::CheckFactsRef<'a>,
    exit: &FlowExitFact,
) -> &'a [checked_trees::FlowExitParameterOrigin] {
    facts
        .flow
        .control
        .exit_parameter_origins
        .span_or_empty(exit.parameter_origins)
}

/* The parameter origins every exit of one state retained, in exit order. */
pub(crate) fn state_exit_parameter_origins<'a>(
    facts: checked_trees::CheckFactsRef<'a>,
    flow: &checked_trees::FlowStateFact,
) -> Vec<&'a checked_trees::FlowExitParameterOrigin> {
    facts
        .flow
        .control
        .exits
        .span_or_empty(flow.exits)
        .iter()
        .flat_map(|exit| exit_parameter_origins(facts, exit))
        .collect()
}

/* Preserve the outgoing source frontier independently from return contracts. */
pub(super) fn append_outgoing_state_fact(
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    transition_target: typed_trees::statement::TransitionTargetHandle,
    operand_entry_constraints: HandleSpan<FlowConstraintRef>,
    constraints: HandleSpan<FlowConstraintRef>,
    returned_value: arena::Handle<checked_trees::FlowOperandFact>,
    completed_storage: arena::Handle<checked_trees::FlowStorageSnapshot>,
    cannot_return_normally: bool,
    copied_payloads: HandleSpan<checked_trees::FlowOperandFact>,
) {
    let constraints = retained_constraint_refs(&build.contexts.constraint_refs, constraints);
    build
        .control
        .outgoing
        .append(checked_trees::FlowStateOutgoingFact {
            machine_symbol,
            state_symbol,
            statement_index,
            transition_target,
            returned_value,
            completed_storage,
            cannot_return_normally,
            operand_entry_constraints,
            copied_payloads,
            constraints,
        });
}

pub(super) fn append_state_exit_facts(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    transition_target: typed_trees::statement::TransitionTargetHandle,
    is_continuation: bool,
    active_contexts: arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: arena::HandleSpan<FlowConstraintRef>,
) -> arena::HandleSpan<FlowExitFact> {
    let mut state_exits = arena::HandleSpan::empty();

    let exit_handles = proof.contract_exits_at(machine_symbol, state_symbol, transition_target);
    for exit_handle in exit_handles {
        let contract_exit = proof.contract_exits.get(exit_handle);
        let outgoing = build
            .control
            .outgoing
            .span_or_empty(
                build
                    .control
                    .outgoing
                    .span_since(build.state_outgoing_start),
            )
            .iter()
            .filter_map(|outgoing| {
                (outgoing.machine_symbol == machine_symbol
                    && outgoing.state_symbol == state_symbol
                    && outgoing.statement_index == contract_exit.statement_index
                    && outgoing.transition_target == transition_target)
                    .then_some((outgoing.returned_value, outgoing.completed_storage))
            })
            .collect::<Vec<_>>();
        let (returned_value, mut completed_storage) = match outgoing.as_slice() {
            [capture] => *capture,
            _ => (arena::Handle::invalid(), arena::Handle::invalid()),
        };
        if !transition_target.is_valid() && outgoing.is_empty() {
            if program.state_by_symbol(state_symbol).is_some_and(|state| {
                contract_exit.statement_index
                    == program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len()
            }) {
                completed_storage = build
                    .control
                    .statements
                    .span_or_empty(
                        build
                            .control
                            .statements
                            .span_since(build.state_statements_start),
                    )
                    .last()
                    .map(|statement| statement.storage_snapshot)
                    .unwrap_or_else(|| {
                        super::stored_values::append_state_entry(&mut build.control)
                    });
            }
            append_outgoing_state_fact(
                build,
                machine_symbol,
                state_symbol,
                contract_exit.statement_index,
                transition_target,
                active_constraints,
                active_constraints,
                arena::Handle::invalid(),
                completed_storage,
                false,
                HandleSpan::empty(),
            );
        }
        let entry_exit_contexts =
            retained_flow_contexts(&build.contexts.semantic_context_refs, active_contexts);
        let entry_constraints =
            retained_constraint_refs(&build.contexts.constraint_refs, active_constraints);
        let mut ensures_contexts = arena::HandleSpan::empty();
        let mut ensures_constraints = arena::HandleSpan::empty();
        append_flow_contexts_for_points(
            semantic,
            &mut build.contexts.semantic_context_refs,
            &mut ensures_contexts,
            &mut build.contexts.constraint_refs,
            &mut ensures_constraints,
            &[ProgramPoint::Exit {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                transition_target,
            }],
        );

        let machine = build
            .machine_index(program, machine_symbol)
            .map(|index| &program.machines()[index])
            .expect("exit machine");
        let state = build
            .state_index_in_machine(program, machine_symbol, state_symbol)
            .and_then(|index| program.machine_states(machine).get(index))
            .expect("exit state");
        let (rebased_contexts, parameter_origins) = super::entry_origins::rebase_contexts(
            program,
            semantic,
            build,
            machine,
            state,
            ensures_contexts,
            false,
        );
        ensures_contexts = rebased_contexts;
        ensures_constraints = HandleSpan::empty();
        for context in build
            .contexts
            .semantic_context_refs
            .span_or_empty(ensures_contexts)
            .to_vec()
        {
            append_constraint_ref(
                &mut build.contexts.constraint_refs,
                &mut ensures_constraints,
                FlowConstraintKind::SemanticContext {
                    context: context.context,
                },
            );
        }
        build.control.exits.append_to_span(
            &mut state_exits,
            FlowExitFact {
                machine_symbol,
                state_symbol,
                statement_index: contract_exit.statement_index,
                transition_target,
                is_continuation,
                returned_value,
                completed_storage,
                parameter_origins,
                entry_semantic_contexts: entry_exit_contexts,
                entry_constraints,
                ensures_contexts,
                ensures_constraints,
                ensures: contract_exit.ensures,
            },
        );
    }

    state_exits
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_transition_flow_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_state: &checked_trees::StateBorrowFact,
    execution_order: &typed_trees::statement::StatementExecutionOrder,
    statement_index: usize,
    statement_fact: arena::Handle<checked_trees::FlowStatementFact>,
    transition: &typed_trees::statement::TableTransition,
    calls: &[BorrowCallFact],
    state_calls: &mut HandleSpan<FlowCallFact>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) -> super::expression::EvaluatedWritePrefix {
    let mut execution = super::expression::Execution::new(
        program,
        borrow,
        proof,
        domains,
        machine,
        state,
        statement_index,
        statement_fact,
        semantic,
        build,
        state_calls,
        calls,
        active_contexts,
        active_constraints,
    );
    execution.evaluation = checked_trees::FlowPermissionEvaluation::TransitionGuard;
    if let typed_trees::statement::TransitionGuardNode::When(expression) = transition.guard {
        execution.expression(expression, active_contexts, active_constraints);
    }
    let guard_writes = execution.write_prefix();
    let guard_contexts = *active_contexts;
    let guard_constraints = *active_constraints;
    for (is_continuation, target) in [(false, transition.target), (true, transition.continuation)] {
        if !target.is_valid() {
            continue;
        }
        execution.restore_write_prefix(guard_writes);
        let mut branch_contexts = retained_flow_contexts(
            &execution.context.contexts.semantic_context_refs,
            guard_contexts,
        );
        let mut branch_constraints = retained_constraint_refs(
            &execution.context.contexts.constraint_refs,
            guard_constraints,
        );
        guards::append_guard_context(
            program,
            execution.semantic,
            execution.context,
            machine.symbol,
            state.symbol,
            statement_index,
            target,
            transition.guard,
            !is_continuation,
            guard_writes.writes,
            guard_writes.unknown,
            &mut branch_contexts,
            &mut branch_constraints,
        );
        let target_activation_start = execution.context.borrow_lifetimes.activations.frontier();
        execution.transition_target(
            transition,
            target,
            &mut branch_contexts,
            &mut branch_constraints,
        );
        let target_activations = super::reference_spans::appended_span_since(
            &execution.context.borrow_lifetimes.activations,
            target_activation_start,
        );
        append_state_exit_facts(
            program,
            proof,
            execution.semantic,
            execution.context,
            machine.symbol,
            state.symbol,
            target,
            is_continuation,
            branch_contexts,
            branch_constraints,
        );
        for activation in execution
            .context
            .borrow_lifetimes
            .activations
            .span_or_empty(target_activations)
        {
            if borrow.state_owns_statement_resource(borrow_state, activation.loan) {
                super::borrow_lifetimes::weaken_expired_loan(
                    &mut execution.context.borrow_lifetimes.weakenings,
                    borrow,
                    activation.loan,
                    state.statement_nodes.len(),
                    execution_order,
                    checked_trees::FlowBorrowWeakeningReason::StateExit,
                );
            }
        }
    }
    execution.restore_write_prefix(guard_writes);
    *active_contexts = guard_contexts;
    *active_constraints = guard_constraints;
    guards::append_guard_context(
        program,
        execution.semantic,
        execution.context,
        machine.symbol,
        state.symbol,
        statement_index,
        Default::default(),
        transition.guard,
        false,
        guard_writes.writes,
        guard_writes.unknown,
        active_contexts,
        active_constraints,
    );
    guard_writes
}

//! Ordinary call requirements share exact arithmetic coordinates, not rank facts.
use super::{
    BTreeMap, BigInt, BinaryOperator, Engine, ExpressionHandle, ExpressionNode, Machine,
    Polynomial, ProofFact, RankingRangeMeasure, RankingRangeState, SignatureContractKind, State,
    StrictArithmeticBindingValue, TypedTrees, calls, collect_guard, comparison_proven,
    field_coordinates, fields, inductive_judgment, integer_bindings, lengths, meanings,
};
use field_coordinates::FieldCoordinates;

/// The EntryInvariant graph tier re-establishes every readable scalar/length
/// comparison. Its ignored Boolean or nominal facts confer no such guarantee.
pub fn arithmetic_entry_requirement_is_covered(
    program: &TypedTrees,
    machine: &Machine,
    goal: ExpressionHandle,
) -> bool {
    let Some(root) = program.machine_states(machine).first() else {
        return false;
    };
    if !program.machine_contracts(machine).iter().any(|contract| {
        contract.kind == SignatureContractKind::Requires
            && program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .any(
                    |fact| matches!(fact, ProofFact::Expression(expression) if *expression == goal),
                )
    }) || meanings::builtin(program, machine, root, goal, 0).is_none()
    {
        return false;
    }
    let Some(bindings) = integer_bindings(program, root) else {
        return false;
    };
    let mut engine = Engine::strict_with_symbol_bindings(program, machine, &bindings);
    let lengths = lengths::bindings(program, machine, root, None);
    lengths::install(program, machine, root, root, &lengths, &mut engine, &[goal]).is_some()
        && engine.strict_symbol_bindings_are_valid()
        && engine.collect_comparisons(&[goal], &mut Vec::new())
}

/// Check the actual callee requirement against the caller's surviving facts.
/// Caller and goal engines remain separate even on a self-call: binding a
/// formal must not rewrite the hypotheses or recursively substitute an actual.
/// This proves arithmetic implication, not operand execution or type formation.
pub fn prove_arithmetic_call_requirement(
    program: &TypedTrees,
    caller: &Machine,
    caller_state: &State,
    callee: &Machine,
    hypotheses: &[(ExpressionHandle, bool)],
    goal: ExpressionHandle,
    arguments: &[ExpressionHandle],
) -> bool {
    prove(
        program,
        caller,
        caller_state,
        callee,
        hypotheses,
        goal,
        arguments,
    )
    .unwrap_or(false)
}

fn prove(
    program: &TypedTrees,
    caller: &Machine,
    caller_state: &State,
    callee: &Machine,
    hypotheses: &[(ExpressionHandle, bool)],
    goal: ExpressionHandle,
    arguments: &[ExpressionHandle],
) -> Option<bool> {
    let target = program.machine_states(callee).first()?;
    let parameters = program.state_parameters(target);
    if parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count()
        != arguments.len()
        || !program
            .machine_states(caller)
            .iter()
            .any(|state| state.symbol == caller_state.symbol)
    {
        return None;
    }
    meanings::builtin(program, callee, target, goal, 0)?;
    for argument in arguments {
        // No call, borrow, or unknown operand can change an earlier snapshot.
        meanings::builtin(program, caller, caller_state, *argument, 0)?;
    }
    let hypotheses = hypotheses
        .iter()
        .copied()
        .filter(|(expression, _)| {
            meanings::builtin(program, caller, caller_state, *expression, 0).is_some()
        })
        .collect::<Vec<_>>();
    let expressions = arguments
        .iter()
        .copied()
        .chain(hypotheses.iter().map(|(expression, _)| *expression))
        .collect::<Vec<_>>();

    let caller_bindings = integer_bindings(program, caller_state)?;
    let goal_bindings = integer_bindings(program, target)?;
    // A ranged caller member's own authored range is proven site evidence at
    // an internal call site: the member's state-edge judgment re-establishes
    // the produced rank's membership and endpoint pinning on every internal
    // arrival, and the carried-entry correspondence that judgment resolved
    // binds the site formals the carrier names denote. Requires facts stay
    // entry-site evidence and are never installed here.
    let site_invariant =
        crate::machine_calls::call_cycles::runtime_ranking::ranged_member_site_invariant(
            program,
            caller,
            caller_state,
        );
    let mut carrier_equalities = Vec::new();
    let mut engine_bindings = caller_bindings.clone();
    if let Some((_, _, entry_parameters)) = &site_invariant
        && let Some((bindings, equalities)) =
            calls::telescoped_bindings(program, caller, caller_state, entry_parameters)
    {
        carrier_equalities = equalities;
        engine_bindings = bindings;
    }
    let mut source_engine = Engine::strict_with_symbol_bindings(program, caller, &engine_bindings);
    let mut goal_engine = Engine::strict_with_symbol_bindings(program, callee, &goal_bindings);
    if !source_engine.strict_symbol_bindings_are_valid()
        || !goal_engine.strict_symbol_bindings_are_valid()
    {
        return None;
    }
    let mut source_fields = FieldCoordinates::empty();
    let mut goal_fields = FieldCoordinates::empty();
    source_fields.install(
        program,
        caller_state,
        caller_state,
        None,
        &mut source_engine,
        &expressions,
    )?;
    goal_fields.install(program, target, target, None, &mut goal_engine, &[goal])?;
    let source_lengths = lengths::bindings(program, caller, caller_state, None);
    let goal_lengths = lengths::bindings(program, callee, target, None);
    lengths::install(
        program,
        caller,
        caller_state,
        caller_state,
        &source_lengths,
        &mut source_engine,
        &expressions,
    )?;
    lengths::install(
        program,
        callee,
        target,
        target,
        &goal_lengths,
        &mut goal_engine,
        &[goal],
    )?;
    let mut obligations = Vec::new();
    if !goal_engine.collect_comparisons(&[goal], &mut obligations) {
        return None;
    }
    let mut substitutions = BTreeMap::new();
    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
    {
        for field in goal_fields
            .coordinates()
            .filter(|field| field.parameter.symbol == parameter.symbol)
        {
            // An exact carrier actual -- a formal forward, `&x`, or a
            // member-target `p.f`/`&p.f` whose projection lands on the goal
            // coordinate's root -- produces the caller-side coordinate itself,
            // so its declared field bounds join the hypotheses; only a literal
            // or computed actual falls to `actual`'s declaration walk.
            let actual = if let Some(actual) =
                field.actual_coordinate(program, caller_state, *argument, field.borrowed)
            {
                let value = actual.value();
                source_fields.include(actual);
                value
            } else {
                field.actual(
                    program,
                    caller_state,
                    &mut source_engine,
                    *argument,
                    field.borrowed,
                )?
            };
            substitutions.insert(field.identity.clone(), actual);
        }
        if let Some(binding) = goal_bindings
            .iter()
            .find(|binding| binding.symbol == parameter.symbol)
        {
            let StrictArithmeticBindingValue::Atom { identity, .. } = &binding.value else {
                return None;
            };
            substitutions.insert(identity.clone(), source_engine.normalize(*argument)?);
        }
    }
    let mut comparisons = source_fields.comparisons(program);
    comparisons.extend(source_lengths.iter().map(|(_, identity)| {
        (
            BinaryOperator::GreaterOrEqual,
            Polynomial::atom(identity.clone()),
            Polynomial::default(),
        )
    }));
    for binding in &caller_bindings {
        let parameter = program
            .state_parameters(caller_state)
            .iter()
            .find(|parameter| parameter.symbol == binding.symbol)?;
        if let Some((minimum, maximum)) =
            crate::enforced_integer_type_bounds(program, parameter.type_reference)
        {
            let StrictArithmeticBindingValue::Atom { identity, .. } = &binding.value else {
                return None;
            };
            comparisons.extend([
                (
                    BinaryOperator::GreaterOrEqual,
                    Polynomial::atom(identity.clone()),
                    Polynomial::constant(BigInt::from_i64(minimum)),
                ),
                (
                    BinaryOperator::LessOrEqual,
                    Polynomial::atom(identity.clone()),
                    Polynomial::constant(BigInt::from_i64(maximum)),
                ),
            ]);
        }
    }
    for (expression, holds) in hypotheses {
        collect_guard(&mut source_engine, expression, holds, &mut comparisons, 0)?;
    }
    // The duplicated-carrier equalities and the member's proven rank-range
    // membership are established site hypotheses: the member's own
    // state-edge judgment proves them on every internal arrival, and an
    // arrival that judgment cannot prove fails there rather than being
    // assumed here.
    comparisons.extend(carrier_equalities);
    if let Some((measure, range_handle, entry_parameters)) = &site_invariant
        && let ExpressionNode::Range(range) = program.expression_table.expression(*range_handle)
    {
        let rank = if let RankingRangeMeasure::Field { subject, measure } = measure {
            // A field-view member's rank is the record's exact projection
            // coordinate, telescoped from its entry formal onto the unique
            // site carrier holding that role -- the same atom the member's
            // own edge judgment re-establishes membership on at every
            // internal arrival. Entry-spelled member chains inside the
            // authored endpoints (`countdown.limit`) resolve at the entry
            // scope and arrive on the same carrier, so their site atoms are
            // what `normalize` reads below. A role with no unique carrier
            // resolves nothing and the site abstains, as before.
            program.machine_states(caller).first().and_then(|entry| {
                let coordinate =
                    fields::FieldCoordinate::resolve(program, entry, *subject, *measure)?;
                let coordinate = coordinate.at_arrival(
                    program,
                    RankingRangeState {
                        state: caller_state,
                        entry_parameters,
                    },
                    coordinate.parameter.symbol,
                )?;
                let mut coordinates = FieldCoordinates::new(coordinate);
                coordinates.install(
                    program,
                    caller_state,
                    entry,
                    Some(entry_parameters),
                    &mut source_engine,
                    &[range.start, range.end, *subject],
                )?;
                comparisons.extend(coordinates.comparisons(program));
                coordinates.value()
            })
        } else {
            // Every other measure still spells its endpoints -- and a
            // member-chain subject such as `pair.left` -- in the entry scope:
            // `remaining in 0..=limits.cap` reads `limits.cap`, which names no
            // site atom until the projection telescopes onto the unique site
            // carrier holding `limits`' role (`bounds.cap`). Install those
            // member chains the same way the field arm does; a role with no
            // unique carrier resolves nothing and the site abstains, as
            // before. Bare-formal endpoints and subjects are already aliased
            // by the telescoped bindings and leave nothing to install.
            program.machine_states(caller).first().and_then(|entry| {
                let mut spellings = vec![range.start, range.end];
                match measure {
                    RankingRangeMeasure::Single(subject)
                    | RankingRangeMeasure::SliceLength(subject)
                    | RankingRangeMeasure::Computed { subject, .. } => spellings.push(*subject),
                    RankingRangeMeasure::Distance { lower, upper } => {
                        spellings.extend([*lower, *upper])
                    }
                    RankingRangeMeasure::IncreasingTo { subject, limit } => {
                        spellings.extend([*subject, *limit])
                    }
                    RankingRangeMeasure::Field { .. } => {}
                }
                let mut endpoints = FieldCoordinates::empty();
                endpoints.install(
                    program,
                    caller_state,
                    entry,
                    Some(entry_parameters),
                    &mut source_engine,
                    &spellings,
                )?;
                comparisons.extend(endpoints.comparisons(program));
                if let RankingRangeMeasure::SliceLength(subject) = measure {
                    // A carried collection's produced length is the rank; the
                    // telescope's length bindings name the slice formal, or
                    // the record-carried slice leaf, that holds its role at
                    // this site.
                    let (length_roles, length_equalities) = calls::telescoped_length_bindings(
                        program,
                        caller,
                        caller_state,
                        entry_parameters,
                    );
                    comparisons.extend(length_equalities);
                    lengths::parameter(program, entry, *subject).and_then(|parameter| {
                        length_roles
                            .iter()
                            .find(|(symbol, _)| *symbol == parameter.symbol)
                            .map(|(_, identity)| Polynomial::atom(identity.clone()))
                    })
                } else {
                    calls::rank_coordinate(program, caller, &mut source_engine, *measure)
                }
            })
        };
        if let (Some(rank), Some(floor), Some(ceiling)) = (
            rank,
            source_engine.normalize(range.start),
            source_engine.normalize(range.end),
        ) {
            comparisons.extend(calls::arrival_invariant(
                *measure,
                &rank,
                &floor,
                &ceiling,
                range.end_inclusive,
            ));
        }
    }
    if !source_engine.install_hypotheses(comparisons) {
        return None;
    }
    // Subslice geometry consumes established caller facts; the requirement's
    // own conclusion is never installed to form the next slice.
    for (parameter, argument) in parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(arguments)
    {
        if let Some((_, identity)) = goal_lengths
            .iter()
            .find(|(symbol, _)| *symbol == parameter.symbol)
        {
            substitutions.insert(
                identity.clone(),
                lengths::actual(
                    program,
                    caller,
                    caller_state,
                    *argument,
                    &source_lengths,
                    &mut source_engine,
                )?,
            );
        }
    }
    for (operator, left, right) in obligations {
        let left = inductive_judgment::apply_argument_map(&left, &substitutions)?;
        let right = inductive_judgment::apply_argument_map(&right, &substitutions)?;
        if !source_engine.requires_unsatisfiable
            && !comparison_proven(&source_engine, operator, &left, &right)
        {
            return Some(false);
        }
    }
    Some(true)
}

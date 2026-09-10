//! Ordinary call requirements share exact arithmetic coordinates, not rank facts.

use super::*;
use field_coordinates::FieldCoordinates;
use fields::FieldCoordinate;

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
    let lengths = lengths::bindings(program, root, None);
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
    let mut source_engine = Engine::strict_with_symbol_bindings(program, caller, &caller_bindings);
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
    let source_lengths = lengths::bindings(program, caller_state, None);
    let goal_lengths = lengths::bindings(program, target, None);
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
            let actual = if let Some(actual) =
                FieldCoordinate::resolve(program, caller_state, *argument, field.field.symbol)
            {
                let value = actual.value();
                source_fields.include(actual);
                value
            } else {
                field.actual(program, caller_state, &mut source_engine, *argument)?
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

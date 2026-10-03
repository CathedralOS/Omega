//! Bound generic cleanup inputs are ordinary call obligations. Their origin is
//! the retained generic death row, not an authored `requires` clause; only the
//! actual's evaluated operand facts can discharge the instantiated predicate.
use arena::Arena;
use checked_trees::{
    CheckFacts, CheckedBooleanExpression, CheckedScalarExpression,
    CheckedStructuralPredicatePathSegment, ContextualCleanupEntryRequirement, FlowCallFact,
    FlowStateFact,
};
use diagnostics::Diagnostic;
use facts::FactContextHandle;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionHandle, signature::StateParameter};

use crate::flow::CanonicalPlace;

#[cfg(test)]
mod tests;

pub(crate) fn target_machine(program: &TypedTrees, call: &FlowCallFact) -> Option<SymbolHandle> {
    crate::lookup::machine_by_symbol(program, call.target_symbol)
        .map(|machine| machine.symbol)
        .or_else(|| {
            crate::semantic::calls::find_machine_by_entry_state(program, call.target_symbol)
                .map(|(machine, _)| machine.symbol)
        })
}

struct Actual<'program> {
    parameter: &'program StateParameter,
    argument: Option<ExpressionHandle>,
    place: CanonicalPlace,
    path: Vec<facts::PlaceSegment>,
    operand_position: usize,
}

fn actual<'program>(
    program: &'program TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
    paths: &Arena<CheckedStructuralPredicatePathSegment>,
) -> Option<Actual<'program>> {
    if target_machine(program, call)? != requirement.machine {
        return None;
    }
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
    let (_, path, endpoint, _) = crate::values::resolve_structural_parameter_path(
        program,
        parameters,
        requirement.parameter_position,
        paths.span(requirement.path)?,
    )?;
    if program.primitive_type_reference(endpoint) != Some(typed_trees::types::PrimitiveType::Bool) {
        return None;
    }
    actual_at(
        program,
        state,
        call,
        requirement.parameter_position as usize,
        path,
    )
}

fn actual_at<'program>(
    program: &'program TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    position: usize,
    path: Vec<facts::PlaceSegment>,
) -> Option<Actual<'program>> {
    let site = crate::semantic::calls::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
    let parameter = parameters.get(position)?;
    let receiver_offset = usize::from(
        matches!(&site, crate::semantic::calls::CallSite::Expression { call, .. } if call.receiver.is_valid()),
    );
    let ordinal = parameters[..position]
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let operand_position = if parameter.is_self {
        0
    } else {
        ordinal + receiver_offset
    };
    let argument = if parameter.is_self {
        None
    } else {
        let ordinal = parameters[..position]
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
        Some(*crate::semantic::calls::call_site_argument_expressions(program, &site).get(ordinal)?)
    };
    let place = if let Some(argument) = argument {
        crate::flow::canonical_place_from_expression_in_state(
            program,
            state.state_symbol,
            call.statement_index,
            argument,
        )?
    } else {
        crate::flow::canonical_receiver_place_for_call_site(
            program,
            state.machine_symbol,
            state.state_symbol,
            &site,
            call.statement_index,
        )?
    };
    Some(Actual {
        parameter,
        argument,
        place,
        path,
        operand_position,
    })
}

pub(crate) fn actual_place(
    program: &TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
    paths: &Arena<CheckedStructuralPredicatePathSegment>,
) -> Option<CanonicalPlace> {
    let mut actual = actual(program, state, call, requirement, paths)?;
    actual.place.segments.extend(actual.path);
    Some(actual.place)
}

/// Bind a callee observation to its actual operand, without assuming that the
/// formal's declaration position is a dense scalar position.
fn observation_actual<'program>(
    program: &'program TypedTrees,
    state: &FlowStateFact,
    call: &FlowCallFact,
    observation: &crate::facts::boolean_predicates::BoundObservation,
) -> Option<Actual<'program>> {
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
    let position = parameters.iter().position(|parameter| {
        observation.place.root == facts::PlaceRoot::Symbol(parameter.symbol)
    })?;
    actual_at(
        program,
        state,
        call,
        position,
        observation.place.segments.clone(),
    )
}

fn projected_expression(program: &TypedTrees, actual: &Actual<'_>) -> Option<ExpressionHandle> {
    let expression = actual.argument?;
    if actual.path.is_empty() {
        return Some(expression);
    }
    let projections = crate::flow::literal_value_projections(
        program,
        expression,
        actual.parameter.type_reference,
        &actual.path,
        false,
    )?;
    let [projection] = projections.as_slice() else {
        return None;
    };
    projection
        .remaining
        .is_empty()
        .then_some(projection.expression)
}

fn captured_actual<'facts>(
    program: &TypedTrees,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Option<&'facts checked_trees::FlowOperandFact> {
    let site = crate::semantic::calls::find_call_site(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let receiver = match &site {
        crate::semantic::calls::CallSite::Expression { call, .. } if call.receiver.is_valid() => {
            Some(call.receiver)
        }
        _ => None,
    };
    let expected = receiver.into_iter().chain(
        crate::semantic::calls::call_site_argument_expressions(program, &site)
            .iter()
            .copied(),
    );
    let operands = facts.flow.control.operands.span(call.operands)?;
    if !operands
        .iter()
        .map(|operand| operand.expression)
        .eq(expected)
    {
        return None;
    }
    operands.get(actual.operand_position)
}

pub(crate) fn entry_subject(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
) -> Option<(u32, Vec<CheckedStructuralPredicatePathSegment>)> {
    let actual = actual(
        program,
        state,
        call,
        requirement,
        &facts.contextual_cleanup.entry_requirements.paths,
    )?;
    let operand = captured_actual(program, facts, state, call, &actual)?;
    let mut place = actual.place;
    place.segments.extend(actual.path);
    crate::flow::operand_entry_origin_is_stable(program, &facts.flow, operand, &place)
        .then_some(())?;
    crate::facts::entry_parameter_place_before_statement(
        program,
        state.machine_symbol,
        state.state_symbol,
        call.statement_index,
        &place,
    )
}

/// Transport only entry-custodied observations. A callee selector remains a
/// formula; it never becomes an unconditional caller cleanup demand.
pub(crate) fn entry_antecedent(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    requirement: &ContextualCleanupEntryRequirement,
) -> Option<CheckedBooleanExpression> {
    if target_machine(program, call)? != requirement.machine {
        return None;
    }
    let predicate = crate::facts::boolean_predicates::bind_entry(
        program,
        requirement.machine,
        requirement.antecedent.clone(),
    )?;
    let mut expression = predicate.expression.clone();
    crate::facts::boolean_predicates::map_observations(&mut expression, &mut |operand| {
        let observation = predicate
            .bindings
            .iter()
            .find(|binding| binding.operand == *operand)?;
        let actual = observation_actual(program, state, call, observation)?;
        let primitive = operand.primitive_type()?;
        let capture = captured_actual(program, facts, state, call, &actual)?;
        let (machine, body) = crate::lookup::symbols::machine_state_by_symbol(
            program,
            state.machine_symbol,
            state.state_symbol,
        )?;
        if let Some(expression) = projected_expression(program, &actual)
            && let Some(value) = crate::values::lower_entry_scalar_with_subjects(
                program,
                &facts.operators,
                machine,
                body,
                expression,
                primitive,
                &|leaf| {
                    let place = crate::flow::canonical_place_from_expression_in_state(
                        program,
                        state.state_symbol,
                        call.statement_index,
                        leaf,
                    )?;
                    crate::flow::operand_entry_origin_is_stable(
                        program,
                        &facts.flow,
                        capture,
                        &place,
                    )
                    .then_some(())?;
                    crate::facts::entry_parameter_place_before_statement(
                        program,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        &place,
                    )
                },
            )
        {
            *operand = value;
            return Some(());
        }
        let mut place = actual.place;
        place.segments.extend(actual.path);
        crate::flow::operand_entry_origin_is_stable(program, &facts.flow, capture, &place)
            .then_some(())?;
        let (position, path) = crate::facts::entry_parameter_place_before_statement(
            program,
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            &place,
        )?;
        *operand = if path.is_empty() {
            let machine = program.machine_by_symbol(state.machine_symbol)?;
            let index = crate::checks::termination::named_transition_target_state_index(
                program,
                machine,
                machine.symbol,
            )?;
            let entry = program.machine_states(machine).get(index)?;
            let parameters = program.state_parameters(entry);
            if program.primitive_type_reference(parameters.get(position as usize)?.type_reference)
                != Some(primitive)
            {
                return None;
            }
            CheckedScalarExpression::Parameter {
                position: parameters[..position as usize]
                    .iter()
                    .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
                    .count(),
                primitive_type: primitive,
            }
        } else {
            CheckedScalarExpression::StructuralParameterField {
                parameter_position: position,
                path,
                primitive_type: primitive,
            }
        };
        Some(())
    })?;
    Some(expression)
}

/// Rebind a selected actual expression to exact current storage. Local slots
/// denote stored values; their initializer expressions are never replayed.
fn current_actual(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
    primitive: typed_trees::types::PrimitiveType,
    contexts: &[FactContextHandle],
    bindings: &mut Vec<crate::facts::boolean_predicates::BoundObservation>,
) -> Option<CheckedScalarExpression> {
    let (machine, body) = crate::lookup::symbols::machine_state_by_symbol(
        program,
        state.machine_symbol,
        state.state_symbol,
    )?;
    if let Some(expression) = projected_expression(program, actual) {
        if !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(body),
            expression,
        ) {
            return None;
        }
        if let Some(value) = crate::values::lower_unit_scalar_argument(
            program,
            &facts.operators,
            body,
            call.statement_index,
            expression,
            primitive,
        ) {
            let symbols =
                crate::values::unit_scalar_binding_symbols(program, body, call.statement_index)?;
            let mut value = value;
            crate::facts::boolean_predicates::map_scalar(&mut value, &mut |operand| {
                let parameters = program.state_parameters(body);
                let place = match operand {
                    CheckedScalarExpression::Parameter { position, .. }
                    | CheckedScalarExpression::Local { position, .. } => {
                        crate::flow::canonical_place_from_symbol(*symbols.get(*position)?)?
                    }
                    CheckedScalarExpression::StorageRead { symbol, .. } => {
                        crate::flow::canonical_place_from_symbol(*symbol)?
                    }
                    CheckedScalarExpression::StructuralParameterField {
                        parameter_position,
                        path,
                        ..
                    } => {
                        let (root, segments, _, _) =
                            crate::values::resolve_structural_parameter_path(
                                program,
                                parameters,
                                *parameter_position,
                                path,
                            )?;
                        CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(root),
                            segments,
                        }
                    }
                    _ => return None,
                };
                *operand = bind_current(
                    program,
                    facts,
                    state,
                    call,
                    place,
                    operand.primitive_type()?,
                    contexts,
                    bindings,
                )?;
                Some(())
            })?;
            return Some(value);
        }
    }
    let mut place = actual.place.clone();
    place.segments.extend_from_slice(&actual.path);
    bind_current(
        program, facts, state, call, place, primitive, contexts, bindings,
    )
}

fn bind_current(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
    primitive: typed_trees::types::PrimitiveType,
    contexts: &[FactContextHandle],
    bindings: &mut Vec<crate::facts::boolean_predicates::BoundObservation>,
) -> Option<CheckedScalarExpression> {
    let type_reference = crate::flow::canonical_place_type_reference(
        program,
        state.state_symbol,
        call.statement_index,
        &place,
    )?;
    if program.primitive_type_reference(type_reference) != Some(primitive) {
        return None;
    }
    if let Some(value) = scalar_at(program, facts, contexts, &place) {
        if let Some(literal) = observed_literal(value, primitive) {
            return Some(literal);
        }
    }
    let operand = CheckedScalarExpression::Parameter {
        position: bindings.len(),
        primitive_type: primitive,
    };
    bindings.push(crate::facts::boolean_predicates::BoundObservation {
        operand: operand.clone(),
        place,
        type_reference,
    });
    Some(operand)
}

fn scalar_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    place: &CanonicalPlace,
) -> Option<facts::ScalarValue> {
    super::prover::boolean_value_at_place(program, &facts.semantic, contexts, place)
        .map(facts::ScalarValue::Boolean)
        .or_else(|| {
            crate::values::scalar_value_at_place(
                program,
                &facts.semantic,
                contexts
                    .iter()
                    .map(|handle| facts.semantic.contexts.get(*handle)),
                place,
            )
        })
}

fn observed_literal(
    value: facts::ScalarValue,
    primitive: typed_trees::types::PrimitiveType,
) -> Option<CheckedScalarExpression> {
    match value {
        facts::ScalarValue::Boolean(value)
            if primitive == typed_trees::types::PrimitiveType::Bool =>
        {
            Some(CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::Constant(value),
            )))
        }
        facts::ScalarValue::Integer(value) => {
            let text = value.to_string();
            let digits = text.strip_prefix('-').unwrap_or(&text);
            let literal = numerics::literals::IntegerLiteral::from_parts(
                text.starts_with('-'),
                numerics::literals::IntegerRadix::Decimal,
                digits,
            )
            .ok()?
            .with_landing(numerics::literals::IntegerLanding {
                landed_type: crate::values::landed_for_primitive(primitive)?,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
            Some(CheckedScalarExpression::IntegerLiteral { literal })
        }
        _ => None,
    }
}

pub(crate) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    requirement: &ContextualCleanupEntryRequirement,
) -> bool {
    let Some(predicate) = crate::facts::boolean_predicates::bind_entry(
        program,
        requirement.machine,
        requirement.predicate(&facts.contextual_cleanup.entry_requirements.paths),
    ) else {
        return false;
    };
    if target_machine(program, call) != Some(requirement.machine) {
        return false;
    }
    let mut selected_contexts: Option<Vec<FactContextHandle>> = None;
    let mut current = predicate.clone();
    current.bindings.clear();
    if crate::facts::boolean_predicates::map_observations(&mut current.expression, &mut |operand| {
        let observation = predicate
            .bindings
            .iter()
            .find(|binding| binding.operand == *operand)?;
        let actual = observation_actual(program, state, call, observation)?;
        let capture = captured_actual(program, facts, state, call, &actual)?;
        let operand_contexts = crate::flow::operand_contexts(
            program,
            &facts.flow,
            &facts.semantic,
            capture,
            actual.parameter.type_reference,
            call.entry_constraints,
        );
        if let Some(selected) = &mut selected_contexts {
            selected.retain(|context| operand_contexts.contains(context));
        } else {
            selected_contexts = Some(operand_contexts.clone());
        }
        *operand = current_actual(
            program,
            facts,
            state,
            call,
            &actual,
            operand.primitive_type()?,
            &operand_contexts,
            &mut current.bindings,
        )?;
        Some(())
    })
    .is_none()
    {
        return false;
    }
    let selected_contexts = selected_contexts.unwrap_or_else(|| contexts.to_vec());
    let retained =
        crate::facts::boolean_predicates::retained(program, &facts.semantic, &selected_contexts);
    crate::facts::boolean_predicates::proves(&retained, &current, |place| {
        scalar_at(program, facts, &selected_contexts, place)
    })
}

pub(super) fn check(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[FactContextHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target) = target_machine(program, call) else {
        return;
    };
    let requirements = &facts.contextual_cleanup.entry_requirements;
    for (_, requirement) in requirements
        .rows
        .iter()
        .filter(|(_, row)| row.machine == target)
    {
        if proves(program, facts, state, call, contexts, requirement) {
            continue;
        }
        let subject = actual_place(program, state, call, requirement, &requirements.paths)
            .map(|place| place.label(program))
            .unwrap_or_else(|| format!("parameter {}", requirement.parameter_position));
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove contextual cleanup prerequisite for call {} from {}: {} == {} required by {} clause `{}`",
            crate::labels::call_target_label(program, call.target_symbol),
            crate::labels::machine_name(program, state.machine_symbol),
            subject, requirement.expected,
            crate::labels::machine_name(program, requirement.hook_machine),
            program.expression_table.display_name(requirement.hook_clause),
        )));
    }
}

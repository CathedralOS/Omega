//! Direct structural operands execute before their enclosing call, retaining
//! captured preorder coordinates rather than renumbering by execution order.

use super::*;

pub(in crate::flow::terminal_unit) enum Operand<'facts> {
    Call(&'facts checked_trees::FlowCallFact),
    Array(crate::values::CallArrayConstruction),
}

/// Retain ordinary call plans at their expression nodes. Their order here is
/// catalog order only: the structural value evaluator invokes each call when
/// that operand is reached, after all earlier authored field evaluations.
#[allow(clippy::too_many_arguments)]
pub(in crate::flow::terminal_unit) fn value_calls(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    trivial_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    count: &mut usize,
    root: checked_trees::CheckedStructuralValueHandle,
) -> Option<Vec<checked_trees::CheckedStructuralValueCall>> {
    let plans = &facts.values.structural_values;
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut output = Vec::new();
    while let Some(value) = pending.pop() {
        if !plans.nodes.is_valid(value) || visited.contains(&value) {
            return None;
        }
        visited.push(value);
        match &plans.nodes.get(value).kind {
            checked_trees::CheckedStructuralValueKind::Call { source_call } => {
                if !facts.flow.control.calls.is_valid(*source_call) {
                    return None;
                }
                let call = facts.flow.control.calls.get(*source_call);
                if call.authored_expression != plans.nodes.get(value).expression {
                    return None;
                }
                let reference = crate::flow::call_target_return_type(program, call.target_symbol)?;
                let mut result = checked_structural_result_type(
                    program,
                    shapes,
                    reference,
                    &machine_binders(program, machine),
                )?;
                result.statement_index = u32::try_from(call.statement_index).ok()?;
                result.binding_ordinal = u32::try_from(*count).ok()?;
                let operation = build_call_operation(
                    program,
                    facts,
                    machine,
                    state,
                    parameters,
                    trivial_locals,
                    entry_claims,
                    call,
                    false,
                    Some(ExpectedCallValueResult::Structural(&result)),
                    results,
                )?;
                let mut operation = bind_structural_call_result(operation, result)?;
                let CheckedUnitEffectOperationPlan::StructuralCall {
                    discard_result_on_return,
                    ..
                } = &mut operation
                else {
                    return None;
                };
                *discard_result_on_return = false;
                output.push(checked_trees::CheckedStructuralValueCall::new(
                    value, operation,
                )?);
                *count = count.checked_add(1)?;
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. } => {
                for field in plans.record_fields.span(*fields)?.iter().rev() {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) =
                        field.value
                    {
                        pending.push(value);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(
                    plans
                        .dispatch_arms
                        .span(*arms)?
                        .iter()
                        .rev()
                        .map(|arm| arm.value),
                );
            }
            checked_trees::CheckedStructuralValueKind::Case(_)
            | checked_trees::CheckedStructuralValueKind::Place(_) => {}
        }
    }
    Some(output)
}

pub(in crate::flow::terminal_unit) fn for_call<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> Option<Vec<&'a checked_trees::FlowCallFact>> {
    Some(
        operations_for_call(program, facts, machine, state, call)?
            .into_iter()
            .filter_map(|operand| match operand {
                Operand::Call(call) => Some(call),
                Operand::Array(_) => None,
            })
            .collect(),
    )
}

pub(in crate::flow::terminal_unit) fn operations_for_call<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> Option<Vec<Operand<'a>>> {
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    let arrays = crate::values::call_array_constructions(
        program,
        &facts.flow,
        machine,
        state,
        call.statement_index,
    );
    if arrays.is_empty()
        && !calls.iter().any(|nested| {
            nested.statement_index == call.statement_index
                && nested.call_ordinal != 0
                && result(
                    program,
                    facts,
                    machine.symbol,
                    nested.authored_expression,
                    &mut ShapeCollector::new(program),
                )
                .is_some()
        })
    {
        return Some(Vec::new());
    }
    let mut output = Vec::new();
    collect(
        program,
        facts,
        machine,
        state,
        calls,
        &arrays,
        call,
        &mut Vec::new(),
        &mut output,
    )?;
    Some(output)
}

fn collect<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
    arrays: &[crate::values::CallArrayConstruction],
    call: &checked_trees::FlowCallFact,
    active: &mut Vec<typed_trees::expression::ExpressionHandle>,
    output: &mut Vec<Operand<'a>>,
) -> Option<()> {
    let site = crate::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let arguments = crate::call_site_argument_expressions(program, &site);
    let parameters = crate::call_target_parameters(program, call.target_symbol)?;
    let explicit_self = arguments.len()
        > parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
    let parameters = parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| !parameter.is_self || explicit_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }
    for (argument, (position, parameter)) in arguments.iter().zip(&parameters) {
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            continue;
        }
        let source = checked_trees::CheckedArrayConstructionSource::CallArgument {
            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
            parameter_position: u32::try_from(*position).ok()?,
        };
        if let Some(array) = arrays.iter().find(|array| array.source == source) {
            if array.expression != *argument
                || array.type_reference != parameter.type_reference
                || output.iter().any(
                    |operand| matches!(operand, Operand::Array(prior) if prior.source == source),
                )
            {
                return None;
            }
            output.push(Operand::Array(*array));
            continue;
        }
        let place = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            call.statement_index,
            *argument,
        );
        let expression = match place {
            Some(place)
                if place.segments.iter().all(|segment| {
                    matches!(
                        segment,
                        facts::PlaceSegment::Field { .. } | facts::PlaceSegment::FixedIndex { .. }
                    )
                }) =>
            {
                match place.root {
                    facts::PlaceRoot::Expression(expression) => expression,
                    _ => *argument,
                }
            }
            _ => *argument,
        };
        let ExpressionNode::Call(authored) = program.expression_table.expression(expression) else {
            continue;
        };
        result(
            program,
            facts,
            machine.symbol,
            expression,
            &mut ShapeCollector::new(program),
        )?;
        if active.contains(&expression)
            || output
                .iter()
                .any(|prior| matches!(prior, Operand::Call(prior) if prior.authored_expression == expression))
            || !authored.machine_arguments.is_empty()
            || !authored.evidence_arguments.is_empty()
            || authored.static_requirement_dispatch.is_some()
            || authored.quotient_operation.is_some()
            || authored.private_layout_operation.is_some()
        {
            return None;
        }
        let mut matching = calls.iter().filter(|nested| {
            nested.statement_index == call.statement_index
                && nested.call_ordinal > call.call_ordinal
                && nested.authored_expression == expression
                && nested.target_symbol == authored.target_symbol
        });
        let nested = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        active.push(expression);
        collect(
            program, facts, machine, state, calls, arrays, nested, active, output,
        )?;
        active.pop();
        output.push(Operand::Call(nested));
    }
    Some(())
}

/// Anonymous results use the same stored-owned shape as named boundary results.
/// This selects only the source signature; build_call_operation still checks
/// the complete target contract, arguments, and ownership events.
pub(in crate::flow::terminal_unit) fn result(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    shapes: &mut ShapeCollector<'_>,
) -> Option<CheckedStructuralResultPlan> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    let return_type = crate::values::nested_structural_call_return_type(program, caller, call)?;
    let mut owners = program.machines().iter().filter_map(|owner| {
        let [state] = program.machine_states(owner) else {
            return None;
        };
        (state.symbol == call.target_symbol).then_some((owner, state))
    });
    let binders = if let Some((owner, state)) = owners.next() {
        if owners.next().is_some() {
            return None;
        }
        if !owner.supply_mode.is_boundary_declaration() {
            if owner.supply_mode == MachineSupplyMode::CheckedBody
                && validation::is_closed_primitive_array_type(program, return_type)
                && machine_binders(program, owner).is_empty()
            {
                return Some(CheckedStructuralResultPlan {
                    type_identity: shapes.add_type(return_type, &[], &[])?,
                    multiplicity: Multiplicity::Unrestricted,
                    qualifications: Vec::new(),
                });
            }
            let mut targets = facts
                .flow
                .terminal_structural_returns
                .claim_free_affine_machines
                .iter()
                .filter(|target| target.machine == owner.symbol && target.state == state.symbol);
            let target = targets.next()?;
            return (targets.next().is_none()
                && target.result.multiplicity == Multiplicity::Affine
                && target.result.qualifications.is_empty())
            .then(|| target.result.clone());
        }
        machine_binders(program, owner)
    } else {
        Vec::new()
    };
    let CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity: Multiplicity::Affine,
        qualifications,
    } = boundary_result_plan(program, shapes, return_type, &binders)?
    else {
        return None;
    };
    qualifications
        .is_empty()
        .then_some(CheckedStructuralResultPlan {
            type_identity,
            multiplicity: Multiplicity::Affine,
            qualifications,
        })
}

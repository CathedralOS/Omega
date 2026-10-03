//! Direct structural operands execute before their enclosing call, retaining
//! captured preorder coordinates rather than renumbering by execution order.
//! Recursive collection retains each value's consuming call and formal slot;
//! flattening evaluation order must not relabel a nested operand as an outer one.
use super::super::{
    CheckedStructuralResultPlan, CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEntryClaimPlan,
};
use super::{
    CheckFacts, CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, ExpressionNode,
    MachineSupplyMode, Multiplicity, SymbolHandle, TypedTrees,
};
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::types::{
    ShapeCollector, machine_binders, parameter_qualifications, state_flow,
};

use crate::execution::terminal_unit::ScalarCalleePlans;

use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::boundary_machine::boundary_result_plan;
use crate::execution::terminal_unit::control::call_results::bind_structural_call_result;
use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;

pub(in crate::execution::terminal_unit) enum Operand<'facts> {
    Call(&'facts checked_trees::FlowCallFact),
    Array(crate::values::CallArrayConstruction),
    /// A construction or copied place retained by the checker as a
    /// structural value rooted at the authored argument expression. The statement
    /// sequence establishes it as a state-local operand before the
    /// consuming call, giving the argument the same binding-ordinal source
    /// an anonymous call result carries.
    Value {
        root: &'facts checked_trees::CheckedStructuralValueRoot,
        source: checked_trees::CheckedArrayConstructionSource,
    },
}

/// An argument position carries an established value operand only
/// when the checker retained a construction or copy rooted exactly at that
/// expression for this statement and machine, typed to the matched formal.
fn evaluated_value_root<'a>(
    program: &TypedTrees,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    argument: typed_trees::expression::ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<&'a checked_trees::CheckedStructuralValueRoot> {
    if program
        .primitive_type_reference(parameter.type_reference)
        .is_some()
    {
        return None;
    }
    let root = facts.values.structural_values.root_for_expression(
        state,
        u32::try_from(statement_index).ok()?,
        argument,
    )?;
    if root.machine != machine
        || program.normalized_type_identity(root.type_reference)
            != program.normalized_type_identity(parameter.type_reference)
        || !matches!(
            facts.values.structural_values.nodes.get(root.root).kind,
            checked_trees::CheckedStructuralValueKind::Case(_)
                | checked_trees::CheckedStructuralValueKind::StructuralCase { .. }
                | checked_trees::CheckedStructuralValueKind::Record { .. }
                | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { .. }
        )
    {
        return None;
    }
    Some(root)
}

/// Retain ordinary call plans at their expression nodes. Their order here is
/// catalog order only: the structural value evaluator invokes each call when
/// that operand is reached, after all earlier authored field evaluations.
///
/// Each call retains its own argument preparation. The enclosing value
/// evaluator runs that sequence when it reaches the call, after earlier fields
/// and only inside the selected arm. A whole call publishes the same sequence
/// into its surrounding operation stream through `establish_value`. Earlier
/// result bindings provide source custody without hoisting child evaluation.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn value_calls(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    trivial_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    preamble: &[CheckedUnitEffectOperationPlan],
    count: &mut usize,
    root: checked_trees::CheckedStructuralValueHandle,
    trace: &LocalConstructionTrace,
) -> Option<Vec<checked_trees::CheckedStructuralValueCall>> {
    let plans = &facts.values.structural_values;
    let mut scoped_results = results.to_vec();
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
                let mut preparation = preamble.to_vec();
                let predecessor_count = preparation.len();
                plan_call_operands(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    parameters,
                    trivial_locals,
                    entry_claims,
                    &mut scoped_results,
                    &mut preparation,
                    count,
                    call,
                    trace,
                )?;
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
                    Some(scalar_callees),
                    machine,
                    state,
                    parameters,
                    trivial_locals,
                    entry_claims,
                    call,
                    false,
                    Some(ExpectedCallValueResult::Structural(&result)),
                    &scoped_results,
                    trace,
                )?;
                let mut operation = bind_structural_call_result(operation, result.clone())?;
                // Only an ordinary structural call produces a value operand.
                let (_, discard_result_on_return) = operation
                    .call_mut()
                    .filter(|call| !call.is_boundary())?
                    .structural_result_mut()?;
                *discard_result_on_return = false;
                let mut preparation = preparation.split_off(predecessor_count);
                super::statement_sequence::consume_retained_results(&mut preparation, &operation)?;
                output.push(checked_trees::CheckedStructuralValueCall::with_operands(
                    value,
                    checked_trees::CheckedOperandSequence::new(preparation),
                    operation,
                )?);
                scoped_results.push((
                    result,
                    facts::PlaceRoot::Expression(call.authored_expression),
                ));
                // A whole call is the value's own producer. Its caller
                // publishes that same result binding; aggregate children need
                // separate slots because the enclosing value has its own home.
                if value != root {
                    *count = count.checked_add(1)?;
                }
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span(*fields)?.iter().rev() {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(value) =
                        field.value
                    {
                        if let checked_trees::CheckedStructuralValueKind::Place(argument) = &plans.nodes.get(value).kind
                            && matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { .. })
                        {
                            // Byte fields embed their geometry in the record;
                            // a fresh child additionally needs its own carrier
                            // and qualification theory before record assembly.
                            let binders = machine_binders(program, machine);
                            if shapes.add_type(field.type_reference, &binders, &[])? != argument.type_identity {
                                return None;
                            }
                            parameter_qualifications(program, shapes, field.type_reference, &binders)?;
                        }
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
            checked_trees::CheckedStructuralValueKind::Projection { source, .. } => {
                pending.push(*source);
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                pending.extend(elements.iter().rev().copied());
            }
            // A borrowed leaf read projects a type no signature owns; its
            // identity must exist in the unit's structural catalog for the
            // leaf observation to resolve a structural type id downstream.
            // When the leaf is itself a borrowed carrier the argument plan
            // names the `ref(...)` identity, so the shell must be registered
            // — `add_type` alone peels the borrow and leaves it missing.
            checked_trees::CheckedStructuralValueKind::ScalarCasePlace { leaf, .. }
            | checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { leaf, .. } => {
                let binders = machine_binders(program, machine);
                if crate::execution::terminal_unit::types::borrowed_named_view(program, *leaf) {
                    let _ = shapes.add_view_type(
                        *leaf,
                        &binders,
                        language_semantics::ReferenceAccess::Shared,
                    )?;
                } else if crate::execution::terminal_unit::types::borrowed_slice_view(
                    program, *leaf,
                ) {
                    let _ = shapes.add_slice_view_type(*leaf, &binders)?;
                } else {
                    let _ = shapes.add_type(*leaf, &binders, &[])?;
                }
            }
            checked_trees::CheckedStructuralValueKind::Reference { .. }
            | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::Case(_)
            | checked_trees::CheckedStructuralValueKind::ViewElementCopy { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
            | checked_trees::CheckedStructuralValueKind::Place(_) => {}
            // A literal view member exists only to be decomposed into member
            // stores; no establishment carries a shared view operand.
            checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. } => return None,
        }
    }
    Some(output)
}

/// Publish the semantic producer of a value. A call already establishes its
/// result, so wrapping it in a second construction hides its argument schedule
/// and invents a second storage identity. Actual aggregates keep their evaluator
/// and its authored member/selection schedule. Every context uses this decision.
pub(in crate::execution::terminal_unit) fn establish_value(
    facts: &CheckFacts,
    result: CheckedUnitStructuralResultBindingPlan,
    value: checked_trees::CheckedStructuralValueHandle,
    calls: Vec<checked_trees::CheckedStructuralValueCall>,
    operand_source: Option<checked_trees::CheckedArrayConstructionSource>,
    discard_result_on_return: bool,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<CheckedUnitEffectOperationPlan> {
    if matches!(
        facts.values.structural_values.nodes.get(value).kind,
        checked_trees::CheckedStructuralValueKind::Call { .. }
    ) {
        let [call] = calls.as_slice() else {
            return None;
        };
        if call.value != value || operand_source.is_some() {
            return None;
        }
        let mut operation = call.operation().clone();
        let (binding, discard) = operation.call_mut()?.structural_result_mut()?;
        if *binding != result {
            return None;
        }
        *discard = discard_result_on_return;
        operations.extend(call.operands().iter().cloned());
        return Some(operation);
    }
    Some(CheckedUnitEffectOperationPlan::EstablishStructuralValue {
        result,
        value,
        calls,
        operand_source,
        discard_result_on_return,
    })
}

pub(in crate::execution::terminal_unit) fn for_call<'a>(
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
                Operand::Array(_) | Operand::Value { .. } => None,
            })
            .collect(),
    )
}

pub(in crate::execution::terminal_unit) fn operations_for_call<'a>(
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
    let has_value_operands = crate::semantic::calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )
    .map(|site| {
        let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
        let Some(parameters) =
            crate::semantic::calls::call_target_parameters(program, call.target_symbol)
        else {
            return false;
        };
        let explicit_self = arguments.len()
            > parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
        let parameters = parameters
            .iter()
            .filter(|parameter| !parameter.is_self || explicit_self)
            .collect::<Vec<_>>();
        parameters.len() == arguments.len()
            && arguments
                .iter()
                .zip(parameters.iter())
                .any(|(argument, parameter)| {
                    evaluated_value_root(
                        program,
                        facts,
                        machine.symbol,
                        state.symbol,
                        call.statement_index,
                        *argument,
                        parameter,
                    )
                    .is_some()
                })
    })
    .unwrap_or(false);
    if arrays.is_empty()
        && !has_value_operands
        && !calls.iter().any(|nested| {
            nested.statement_index == call.statement_index
                && nested.call_ordinal != 0
                && !facts.flow.control.is_retired(state.symbol, nested)
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
    let site = crate::semantic::calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
    let parameters = crate::semantic::calls::call_target_parameters(program, call.target_symbol)?;
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
        if let Some(root) = evaluated_value_root(
            program,
            facts,
            machine.symbol,
            state.symbol,
            call.statement_index,
            *argument,
            parameter,
        ) {
            if output.iter().any(|operand| {
                matches!(operand, Operand::Value { root: prior, .. } if prior.expression == *argument)
            }) {
                return None;
            }
            output.push(Operand::Value { root, source });
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
            || !authored.carries_only_positional_arguments()
            || !authored.selects_only_nominal_route()
        {
            return None;
        }
        let mut matching = calls.iter().filter(|nested| {
            nested.statement_index == call.statement_index
                && nested.call_ordinal > call.call_ordinal
                && nested.authored_expression == expression
                && nested.target_symbol == crate::flow::call_target_symbol(program, authored)
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
pub(in crate::execution::terminal_unit) fn result(
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
                && program.type_multiplicity(return_type) == Multiplicity::Unrestricted
                && (validation::has_plain_owned_contents(program, return_type)
                    || (validation::bounded_byte_buffer_capacity(program, return_type).is_some()
                        && validation::has_plain_owned_contents_with_represented_constraints(
                            program,
                            return_type,
                        )))
                && machine_binders(program, owner).is_empty()
            {
                return Some(CheckedStructuralResultPlan {
                    type_identity: shapes.add_type(return_type, &[], &[])?,
                    multiplicity: Multiplicity::Unrestricted,
                    // An anonymous result carries the same declared domains
                    // as a named result; scheduling it establishes none of
                    // them. Callee completion and receiving replay own that proof.
                    qualifications: parameter_qualifications(program, shapes, return_type, &[])?,
                    projected_qualifications:
                        validation::structural_result_projected_qualifications(program, return_type)
                            .ok()?,
                });
            }
            // An owned record carrying borrowed referents is affine through
            // its captured leaf loans, not through claim-free contents. The
            // operand's own call operation replays that leaf custody; the
            // anonymous result only needs its declared identity here.
            if owner.supply_mode == MachineSupplyMode::CheckedBody
                && validation::reference_result_custody::is_reference_record(program, return_type)
                && machine_binders(program, owner).is_empty()
            {
                return Some(CheckedStructuralResultPlan {
                    type_identity: shapes.add_type(return_type, &[], &[])?,
                    multiplicity: Multiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                });
            }
            // A borrowed-view result (`&[u8]`, `&'a V`) lends caller-frame
            // storage through a shared loan rather than owned contents: the
            // operand's own call operation replays that loan custody, and
            // the anonymous result only needs its declared identity — the
            // same story as the reference-record arm above.
            if owner.supply_mode == MachineSupplyMode::CheckedBody
                && (crate::execution::terminal_unit::types::borrowed_slice_view(
                    program,
                    return_type,
                ) || crate::execution::terminal_unit::types::borrowed_named_view(
                    program,
                    return_type,
                ))
                && machine_binders(program, owner).is_empty()
            {
                return Some(CheckedStructuralResultPlan {
                    type_identity: shapes.add_type(return_type, &[], &[])?,
                    multiplicity: Multiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                });
            }
            let mut targets = facts
                .flow
                .terminal_structural_returns
                .claim_free_affine_machines
                .iter()
                .filter(|target| target.machine == owner.symbol && target.state == state.symbol);
            if let Some(target) = targets.next() {
                return (targets.next().is_none()
                    && target.result.multiplicity == Multiplicity::Affine
                    && target.result.qualifications.is_empty())
                .then(|| target.result.clone());
            }
            // A checked-body producer whose result is affine plain owned
            // storage carries no claims inside the value for the operand's own
            // call operation to replay; the anonymous result only needs the
            // declared identity, exactly as the closed-array and
            // reference-record arms above.
            if owner.supply_mode == MachineSupplyMode::CheckedBody
                && program.type_multiplicity(return_type) == Multiplicity::Affine
                && validation::has_plain_owned_contents(program, return_type)
                && machine_binders(program, owner).is_empty()
            {
                return Some(CheckedStructuralResultPlan {
                    type_identity: shapes.add_type(return_type, &[], &[])?,
                    multiplicity: Multiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                });
            }
            return None;
        }
        machine_binders(program, owner)
    } else {
        Vec::new()
    };
    let CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity: Multiplicity::Affine,
        qualifications,
    } = boundary_result_plan(program, shapes, return_type, &binders, &[])?
    else {
        return None;
    };
    qualifications
        .is_empty()
        .then_some(CheckedStructuralResultPlan {
            type_identity,
            multiplicity: Multiplicity::Affine,
            qualifications,
            projected_qualifications: Vec::new(),
        })
}

/// Prepare structural operands through the same construction and result
/// binding path for statement calls and calls inside selected values. The
/// caller owns the operation sequence, so an edge retains these operations
/// until selection instead of executing them in its predecessor.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn plan_call_operands(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    trivial_affine_locals: &[(CheckedTrivialAffineStructuralLocalPlan, SymbolHandle)],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    structural_results: &mut Vec<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)>,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
    structural_count: &mut usize,
    call: &checked_trees::FlowCallFact,
    trace: &LocalConstructionTrace,
) -> Option<()> {
    let statement_index = u32::try_from(call.statement_index).ok()?;
    let binders = machine_binders(program, machine);
    for operand in operations_for_call(program, facts, machine, state, call)? {
        let nested = match operand {
            Operand::Array(array) => {
                if structural_results
                    .iter()
                    .any(|(_, root)| *root == facts::PlaceRoot::Expression(array.expression))
                {
                    continue;
                }
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(*structural_count).ok()?,
                    type_identity: shapes.add_type(array.type_reference, &[], &[])?,
                    multiplicity: Multiplicity::Unrestricted,
                };
                let elements = super::scalar_arrays::elements(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    array.source,
                    array.expression,
                    array.type_reference,
                )?;
                *structural_count = structural_count.checked_add(1)?;
                structural_results.push((
                    result.clone(),
                    facts::PlaceRoot::Expression(array.expression),
                ));
                operations.push(CheckedUnitEffectOperationPlan::EstablishScalarArray {
                    source: array.source,
                    result,
                    elements,
                });
                continue;
            }
            Operand::Value { root, source } => {
                if structural_results
                    .iter()
                    .any(|(_, source)| *source == facts::PlaceRoot::Expression(root.expression))
                {
                    continue;
                }
                // The checker retained this inline case construction as a
                // structural value; establish it as a state-local operand
                // before the consuming call, mirroring the local-binding
                // lane without a source local.
                let calls = value_calls(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    trivial_affine_locals,
                    entry_claims,
                    structural_results,
                    &*operations,
                    &mut *structural_count,
                    root.root,
                    trace,
                )?;
                super::statement_sequence::consume_value_places(
                    facts,
                    root.root,
                    structural_results,
                    &mut *operations,
                )?;
                super::statement_sequence::consume_value_call_sources(
                    facts,
                    structural_results,
                    &mut *operations,
                    &calls,
                )?;
                let multiplicity = program.type_multiplicity(root.type_reference);
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(*structural_count).ok()?,
                    type_identity: shapes.add_type(root.type_reference, &binders, &[])?,
                    multiplicity,
                };
                *structural_count = structural_count.checked_add(1)?;
                structural_results.push((
                    result.clone(),
                    facts::PlaceRoot::Expression(root.expression),
                ));
                let establishment = establish_value(
                    facts,
                    result,
                    root.root,
                    calls,
                    Some(source),
                    multiplicity == Multiplicity::Affine,
                    operations,
                )?;
                operations.push(establishment);
                continue;
            }
            Operand::Call(nested) => nested,
        };
        if structural_results.iter().any(|(_, root)| {
            matches!(root, facts::PlaceRoot::Expression(expression)
                if *expression == nested.authored_expression)
        }) {
            continue;
        }
        let target = result(
            program,
            facts,
            machine.symbol,
            nested.authored_expression,
            shapes,
        )?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: target.type_identity,
            multiplicity: target.multiplicity,
        };
        let operation = build_call_operation(
            program,
            facts,
            Some(scalar_callees),
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            entry_claims,
            nested,
            false,
            Some(ExpectedCallValueResult::Structural(&result)),
            structural_results,
            trace,
        )?;
        let operation = bind_structural_call_result(operation, result.clone())?;
        super::statement_sequence::consume_results(&mut *operations, &operation)?;
        operations.push(operation);
        structural_results.push((
            result,
            facts::PlaceRoot::Expression(nested.authored_expression),
        ));
        *structural_count = structural_count.checked_add(1)?;
    }
    Some(())
}

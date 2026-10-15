//! Direct structural operands execute before their enclosing call, retaining
//! captured preorder coordinates rather than renumbering by execution order.
//! Recursive collection retains each value's consuming call and formal slot;
//! flattening evaluation order must not relabel a nested operand as an outer one.
//! A borrowed owned actual uses the same extraction ledger as a named move.
//! Its checked expression retains detached custody while the original place
//! retains repair debt. Replacement planning runs after this preparation, so
//! it never invents a second displaced value. Terminal's shared argument
//! scheduler interleaves these producers with scalar operands in authored order.
use super::super::{
    CheckedStructuralResultPlan, CheckedTrivialAffineStructuralLocalPlan, CheckedUnitEntryClaimPlan,
};
use super::{
    CheckFacts, CheckedBoundaryMachineResultPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralResultBindingPlan, MachineSupplyMode,
    Multiplicity, SymbolHandle,
};
use crate::execution::terminal_unit::calls::{ExpectedCallValueResult, build_call_operation};
use crate::execution::terminal_unit::types::{
    ShapeCollector, machine_binders, parameter_qualifications, state_flow,
};

use crate::execution::terminal_unit::ScalarCalleeClosure;

use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::control::boundary_machine::boundary_result_plan;
use crate::execution::terminal_unit::control::call_results::bind_structural_call_result;
use crate::execution::terminal_unit::control::call_results::checked_structural_result_type;

pub(in crate::execution::terminal_unit) enum Operand<'facts> {
    Call(&'facts checked_trees::FlowCallFact),
    /// Existing storage transferred to an owned formal. Borrowed referents
    /// first materialize their value and retain exact restoration debt.
    Storage {
        expression: typed_trees::expression::ExpressionHandle,
        type_reference: typed_trees::types::TypeReferenceHandle,
    },
    /// A construction or copied place retained by the checker as a
    /// structural value rooted at the authored argument expression. The statement
    /// sequence establishes it as a state-local operand before the
    /// consuming call, giving the argument the same binding-ordinal source
    /// an anonymous call result carries.
    Value {
        root: &'facts checked_trees::CheckedStructuralValueRoot,
        source: checked_trees::CheckedArrayConstructionSource,
        access: checked_trees::CheckedStructuralAccess,
    },
}

/// Rejoin the checked value at one exact argument occurrence. Shared formals
/// lend the completed referent home; slice formals retain their descriptor.
/// This classification establishes neither a new loan nor temporary cleanup:
/// the consuming call or state edge still owns those independent obligations.
pub(in crate::execution::terminal_unit) fn evaluated_argument_root<'a>(
    program: &dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    argument: typed_trees::expression::ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<&'a checked_trees::CheckedStructuralValueRoot> {
    if parameter.is_const
        || parameter.relevance.is_erased()
        || program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
    {
        return None;
    }
    let (argument, expected) = match program
        .type_reference_table
        .type_reference(parameter.type_reference)
    {
        typed_trees::types::TypeReferenceNode::Reference { .. }
            if validation::reference_result_custody::stored_loan_parts(
                program,
                parameter.type_reference,
            )
            .is_some()
                && validation::reference_formation_source(program, argument)
                    .is_some_and(|formation| formation.view_type.is_valid()) =>
        {
            let machine = program.machine_by_symbol(machine)?;
            let state = program.state_by_symbol(state)?;
            (
                argument,
                crate::values::reference_sources::formed_view_type(
                    program, machine, state, argument,
                )?,
            )
        }
        // The descriptor is the view value; peeling its shell would look for
        // a nonexistent owned slice instead of the already checked loan.
        typed_trees::types::TypeReferenceNode::Reference { .. }
            if validation::slice_view_shell(program, parameter.type_reference, &[]).is_some() =>
        {
            (argument, parameter.type_reference)
        }
        // Registration and receiving planning retain the same source-selected
        // descriptor. The parameter's annotation does not create that value.
        typed_trees::types::TypeReferenceNode::Reference { .. }
            if let Some(actual) = crate::values::reference_sources::transported_value_type(
                program,
                program.machine_by_symbol(machine)?,
                program.state_by_symbol(state)?,
                statement_index,
                argument,
            ) =>
        {
            validation::checked_argument_matches_type_reference(
                program,
                argument,
                parameter.type_reference,
            )
            .then_some(())?;
            (argument, actual)
        }
        typed_trees::types::TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            ..
        } => {
            let referent = crate::semantic::calls::shared_argument_referent(
                program,
                argument,
                parameter.type_reference,
            )?;
            // A loan of existing storage must keep that storage's identity.
            // Only a completed value with no canonical stored home needs an
            // argument establishment; a copied Name would be a new referent.
            if validation::canonical_place_from_expression_in_state(
                program,
                state,
                statement_index,
                referent.0,
            )
            .is_some_and(|place| place.root.as_symbol().is_some())
            {
                return None;
            }
            referent
        }
        typed_trees::types::TypeReferenceNode::Reference { .. } => return None,
        _ => (argument, parameter.type_reference),
    };
    let root = facts.values.structural_values.root_for_expression(
        state,
        u32::try_from(statement_index).ok()?,
        argument,
    )?;
    if root.machine != machine
        || program.normalized_type_identity(root.type_reference)
            != program.normalized_type_identity(expected)
        || !facts.values.structural_values.nodes.is_valid(root.root)
        || facts
            .values
            .structural_values
            .nodes
            .get(root.root)
            .expression
            != argument
    {
        return None;
    }
    Some(root)
}

/// Ordinary calls collect a call through its existing producer route, rather
/// than establishing a second whole operand. State-edge
/// establishment can consume the same authenticated root with its own ordered
/// producer/cleanup roster.
fn evaluated_value_root<'a>(
    program: &dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    argument: typed_trees::expression::ExpressionHandle,
    parameter: &typed_trees::signature::StateParameter,
) -> Option<&'a checked_trees::CheckedStructuralValueRoot> {
    let mut root = evaluated_argument_root(
        program,
        facts,
        machine,
        state,
        statement_index,
        argument,
        parameter,
    )?;
    // Every projection selects from its original producer's home. Prepare
    // that producer once for borrowed and owned operands alike; the argument
    // plan retains the path and the continuation owns the residual cleanup.
    // A whole-value projection selects nothing: it is the value it views.
    while let checked_trees::CheckedStructuralValueKind::Projection {
        source, ref path, ..
    } = facts.values.structural_values.nodes.get(root.root).kind
        && !path.is_empty()
    {
        if validation::canonical_place_from_expression_in_state(
            program,
            state,
            statement_index,
            root.expression,
        )
        .is_some_and(|place| place.root.as_symbol().is_some())
        {
            return None;
        }
        root = facts.values.structural_values.root_for_expression(
            state,
            u32::try_from(statement_index).ok()?,
            facts.values.structural_values.nodes.get(source).expression,
        )?;
    }
    // A call already owns a producer operation and result binding. Retaining
    // its value for computed consumers does not turn that operation into an
    // aggregate establishment; collect it through the ordinary call route.
    // The loop above has already moved a selecting projection to its owner;
    // a whole-value projection that remains is established like the value it
    // views.
    if matches!(
        facts.values.structural_values.nodes.get(root.root).kind,
        checked_trees::CheckedStructuralValueKind::Call { .. }
    ) {
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
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
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
    windows: &mut super::super::borrowed_windows::OpenWindows,
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
            checked_trees::CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                pending.extend(arguments.iter().rev().copied());
            }
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
                    windows,
                )?;
                let reference = validation::call_target_return_type(program, call.target_symbol)?;
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
                    call,
                    Some(ExpectedCallValueResult::Structural(&result)),
                    &scoped_results,
                    &preparation,
                    trace,
                )?;
                let mut operation = bind_structural_call_result(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    operation,
                    result.clone(),
                )?;
                // Every completed structural call produces a value operand.
                let (_, discard_result_on_return) =
                    operation.call_mut()?.structural_result_mut()?;
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
            checked_trees::CheckedStructuralValueKind::Projection { source, .. }
            | checked_trees::CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
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
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { leaf, .. } => {
                let binders = machine_binders(program, machine);
                shapes.add_stored_type(*leaf, &binders, &[])?;
            }
            checked_trees::CheckedStructuralValueKind::Reference {
                source,
                source_value,
                backing_type,
                ..
            } => {
                shapes.add_reference_backing(
                    *backing_type,
                    source.type_identity,
                    &machine_binders(program, machine),
                )?;
                if source_value.is_valid() {
                    pending.push(*source_value);
                }
            }
            checked_trees::CheckedStructuralValueKind::BorrowedSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { .. }
            | checked_trees::CheckedStructuralValueKind::Case(_)
            | checked_trees::CheckedStructuralValueKind::ZeroedScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
            | checked_trees::CheckedStructuralValueKind::Scalar { .. }
            | checked_trees::CheckedStructuralValueKind::ZeroedStructural { .. }
            | checked_trees::CheckedStructuralValueKind::ByteLiteralView { .. }
            | checked_trees::CheckedStructuralValueKind::Place(_) => {}
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
        // Value planning binds the call before knowing which local it
        // initializes; the caller's binding names that local.
        let anonymous = CheckedUnitStructuralResultBindingPlan {
            local: Default::default(),
            ..result.clone()
        };
        if *binding != anonymous {
            return None;
        }
        *binding = result;
        *discard = discard_result_on_return;
        operations.extend(call.operands().iter().cloned());
        return Some(operation);
    }
    Some(CheckedUnitEffectOperationPlan::EstablishLocal(
        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
            result,
            value,
            calls,
            operand_source,
            discard_result_on_return,
        },
    ))
}

/// An asm intrinsic owns no authored parameter roster: its instruction
/// contract fixes every operand as a register value. Its call therefore owns
/// no structural operand and no argument call; call planning still decides
/// whether the instruction itself lowers.
pub(in crate::execution::terminal_unit) fn has_register_operands_only(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> bool {
    program
        .symbols
        .builtin_function_for_symbol(target)
        .is_some_and(|function| function.is_asm_intrinsic())
}

pub(in crate::execution::terminal_unit) fn for_call<'a>(
    program: &dyn ::validation::CheckingInput,
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
                Operand::Value { .. } | Operand::Storage { .. } => None,
            })
            .collect(),
    )
}

pub(in crate::execution::terminal_unit) fn operations_for_call<'a>(
    program: &dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> Option<Vec<Operand<'a>>> {
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    // Enumerate each actual operand. A constructor/nested-call eligibility
    // probe would omit the ordinary transfer out of a borrowed place.
    let mut output = Vec::new();
    collect(
        program,
        facts,
        machine,
        state,
        calls,
        call,
        &mut Vec::new(),
        &mut output,
    )?;
    Some(output)
}

fn collect<'a>(
    program: &dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    calls: &'a [checked_trees::FlowCallFact],
    call: &checked_trees::FlowCallFact,
    active: &mut Vec<typed_trees::expression::ExpressionHandle>,
    output: &mut Vec<Operand<'a>>,
) -> Option<()> {
    let site = validation::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    if has_register_operands_only(program, call.target_symbol) {
        return Some(());
    }
    let arguments = validation::call_site_argument_expressions(program, &site);
    let parameters = validation::call_target_parameters(program, call.target_symbol)?;
    let parameters = parameters.iter().enumerate().collect::<Vec<_>>();
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
                matches!(operand, Operand::Value { root: prior, .. } if prior.expression == root.expression)
            }) {
                return None;
            }
            output.push(Operand::Value {
                root,
                source,
                access: super::super::types::structural_access_for_type_reference(
                    program,
                    parameter.type_reference,
                )?,
            });
            continue;
        }
        let place = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            call.statement_index,
            *argument,
        );
        if place
            .as_ref()
            .is_some_and(|place| place.root.as_symbol().is_some())
            && !parameter.is_const
            && !parameter.relevance.is_erased()
            && !crate::checks::type_reference_is_reference(program, parameter.type_reference)
            && program.type_multiplicity(parameter.type_reference) != Multiplicity::Unrestricted
        {
            output.push(Operand::Storage {
                expression: *argument,
                type_reference: parameter.type_reference,
            });
            continue;
        }
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
        let Some(authored) = program.expression_table.call_node(expression) else {
            continue;
        };
        result(
            program,
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
                && nested.target_symbol == validation::call_target_symbol(program, authored)
        });
        let nested = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        active.push(expression);
        collect(
            program, facts, machine, state, calls, nested, active, output,
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
    program: &dyn ::validation::CheckingInput,
    caller: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    shapes: &mut ShapeCollector<'_>,
) -> Option<CheckedStructuralResultPlan> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let call = program.expression_table.call_node(expression)?;
    let return_type = crate::values::nested_structural_call_return_type(program, caller, call)?;
    let mut owners = program.machines().iter().filter_map(|owner| {
        let [state] = program.machine_states(owner) else {
            return None;
        };
        (state.symbol == call.target_symbol).then_some((owner, state))
    });
    let binders =
        if let Some((owner, _)) = owners.next() {
            if owners.next().is_some() {
                return None;
            }
            if !owner.supply_mode.is_boundary_declaration() {
                if owner.supply_mode != MachineSupplyMode::CheckedBody
                    || !machine_binders(program, owner).is_empty()
                {
                    return None;
                }
                // Binding and anonymous operand production classify the same
                // declared storage. Neither classifier decides whether this call's
                // body/effects/custody are executable; build_call_operation owns that.
                let binding = checked_structural_result_type(program, shapes, return_type, &[])?;
                return Some(CheckedStructuralResultPlan {
                    type_identity: binding.type_identity,
                    multiplicity: binding.multiplicity,
                    qualifications: parameter_qualifications(program, shapes, return_type, &[])?,
                    projected_qualifications:
                        validation::structural_result_projected_qualifications(program, return_type)
                            .ok()?,
                });
            }
            machine_binders(program, owner)
        } else {
            Vec::new()
        };
    let CheckedBoundaryMachineResultPlan::Structural {
        type_identity,
        multiplicity,
        qualifications,
        projected_qualifications,
    } = boundary_result_plan(program, shapes, return_type, &binders, &[])?
    else {
        return None;
    };
    Some(CheckedStructuralResultPlan {
        type_identity,
        multiplicity,
        qualifications,
        projected_qualifications,
    })
}

/// Prepare structural operands through the same construction and result
/// binding path for statement calls and calls inside selected values. The
/// caller owns the operation sequence, so an edge retains these operations
/// until selection instead of executing them in its predecessor.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn plan_call_operands(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleeClosure,
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
    windows: &mut super::super::borrowed_windows::OpenWindows,
) -> Option<()> {
    let statement_index = u32::try_from(call.statement_index).ok()?;
    let binders = machine_binders(program, machine);
    for operand in operations_for_call(program, facts, machine, state, call)? {
        let storage = match &operand {
            Operand::Storage {
                expression,
                type_reference,
            } => Some((*expression, *type_reference)),
            Operand::Value {
                root,
                access: checked_trees::CheckedStructuralAccess::Owned,
                ..
            } if program.type_multiplicity(root.type_reference) != Multiplicity::Unrestricted => {
                Some((root.expression, root.type_reference))
            }
            _ => None,
        };
        if let Some((expression, type_reference)) = storage
            && let Some((operation, result)) = windows.move_call_operand(
                program,
                facts,
                shapes,
                machine,
                state,
                structural_parameters,
                statement_index,
                expression,
                type_reference,
                u32::try_from(*structural_count).ok()?,
            )
        {
            *structural_count = structural_count.checked_add(1)?;
            structural_results.push((result, facts::PlaceRoot::Expression(expression)));
            operations.push(operation);
            continue;
        }
        let nested = match operand {
            Operand::Storage { .. } => continue,
            Operand::Value { root, source, .. } => {
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
                    windows,
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
                    root.root,
                )?;
                let multiplicity = validation::reference_result_custody::result_multiplicity(
                    program,
                    root.type_reference,
                );
                let result = CheckedUnitStructuralResultBindingPlan {
                    statement_index,
                    binding_ordinal: u32::try_from(*structural_count).ok()?,
                    type_identity: shapes.add_stored_type(root.type_reference, &binders, &[])?,
                    multiplicity,
                    local: Default::default(),
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
        let target = result(program, machine.symbol, nested.authored_expression, shapes)?;
        let result = CheckedUnitStructuralResultBindingPlan {
            statement_index,
            binding_ordinal: u32::try_from(*structural_count).ok()?,
            type_identity: target.type_identity,
            multiplicity: target.multiplicity,
            local: Default::default(),
        };
        let operation = build_call_operation(
            program,
            facts,
            Some(scalar_callees),
            machine,
            state,
            structural_parameters,
            trivial_affine_locals,
            nested,
            Some(ExpectedCallValueResult::Structural(&result)),
            structural_results,
            operations,
            trace,
        )?;
        let operation = bind_structural_call_result(
            program,
            facts,
            machine.symbol,
            state.symbol,
            operation,
            result.clone(),
        )?;
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

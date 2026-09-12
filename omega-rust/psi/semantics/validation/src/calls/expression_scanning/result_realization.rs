//! Fail-closed fences for value calls whose result cannot yet be realized.
//!
//! These checks are separate from target resolution and argument validation:
//! they reject call shapes that otherwise silently bind zero or read an
//! unmaterialized nested result at runtime.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableAssignment, TransitionTargetNode};

mod boundary_return;

#[cfg(test)]
mod tests;

/// Result operations own the outer call while their operands use the shared
/// scalar evaluator. Each caller family retains its existing source topology.
pub fn result_initializer_call_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    // Completion owns its call directly, while a local initializer may instead
    // belong to the scalar graph's whole-call computation. Do not reclassify
    // those existing local computations merely because final calls are supported.
    if let [state] = program.machine_states(machine)
        && program
            .primitive_type_reference(state.return_type)
            .is_some()
        && matches!(program.statement_table.statements(state.statement_nodes).last(),
            Some(StatementNode::Expression(expression)) if *expression == value)
        && program.expression_table.expression_is_valid(value)
    {
        return initializer_target_is_supported(
            program,
            machine,
            value,
            state.return_type,
            true,
            false,
        );
    }
    unit_result_initializer_call_is_supported(program, machine, value)
        || boundary_return::is_supported(program, machine, value)
}

/// Unimplemented local receiver paths historically read ZII through ambient
/// attachment resolution. The ordinary shared scalar-record route now carries
/// the local's actual structural place; keep the fence for receiver storage and
/// type categories that do not yet retain that source-to-consumer relationship.
pub(crate) fn report_local_receiver_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    if !call.receiver.is_valid() {
        return;
    }
    // BUILTIN view/operand methods (`view.bytes()`, `arr.as_slice()`, min/max
    // shapes) compose on locals through the operand machinery -- the same
    // exemption list as the nested-argument fence above.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    // Only a BARE single-member NAME receiver can be a local; `self.x` and
    // deeper member paths route through the supported field machinery.
    let ExpressionNode::Name(path) = program.expression_table.expression(call.receiver) else {
        return;
    };
    let members = program.expression_table.name_path_members(path.members);
    let [receiver_name] = members else {
        return;
    };
    let receiver = receiver_name.as_str();
    if receiver == "self" {
        return;
    }
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == state_name)
    else {
        return;
    };
    // Only this state's exact parameter is a supported parameter receiver.
    let is_parameter = program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == path.symbol && parameter.name.as_str() == receiver);
    if is_parameter {
        return;
    }
    // A machine FIELD read as a bare name cannot happen (fields spell
    // `self.x`), but keep the check total: owned-data names pass through.
    let is_field = program
        .machine_owned_data(machine)
        .iter()
        .any(|owned| owned.name.as_str() == receiver);
    if is_field {
        return;
    }
    // Local receiver realization concerns this state's declaration only.
    let local = {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                (local.symbol == path.symbol && local.name.as_str() == receiver).then_some(local)
            })
    };
    let Some(local) = local else {
        return;
    };
    // Plain shared record receivers now retain an ordinary structural operand
    // in scalar computation calls. The checked producer and source replay must
    // still establish this exact local before use; this is signature admission,
    // not permission to substitute attachment storage or a constructor value.
    if shared_record_receiver_signature(program, call, local) {
        return;
    }
    // A checked local dynamic coercion is not an ordinary local receiver:
    // closed-row lowering devirtualizes the call onto the coercion's retained
    // source place. Invalid, missing, or ambiguous conformance selection is
    // rejected independently by `collect_dynamic_conformance_selections`.
    if type_reference_contains_dynamic_trait(program, local.type_reference)
        && crate::traits::normalized_dynamic_coercion(program, local.initial_value).is_some()
    {
        return;
    }
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{state_name}`: value call `{}.{}(..)` uses a LET-bound          local as its receiver, which reads ZII (zeroes) natively -- receiver          resolution reaches machine fields and state parameters only. Store the          value in a data field (`self.{} = {}; self.{}.{}(..)`) or pass it as a          state parameter.",
        machine.name,
        receiver,
        call.target.as_str(),
        receiver,
        receiver,
        receiver,
        call.target.as_str(),
    )));
}

fn shared_record_receiver_signature(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    use typed_trees::types::TypeReferenceNode;
    if local.is_mutable
        || !matches!(
            program.type_multiplicity(local.type_reference),
            language_semantics::Multiplicity::Affine
                | language_semantics::Multiplicity::Unrestricted
        )
        || !crate::has_plain_owned_contents_with_numeric_constraints(program, local.type_reference)
    {
        return false;
    }
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return false;
    };
    let Some(data) = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
    else {
        return false;
    };
    if !program.data_type_parameters(data).is_empty()
        || !program.data_members(data).iter().all(|member| matches!(member,
            typed_trees::data::DataMember::Field(field)
                if !field.relevance.is_erased()
                    && matches!(program.type_reference_table.type_reference(field.type_reference), TypeReferenceNode::Named { .. })
                    && program.primitive_type_reference(field.type_reference).is_some()))
    {
        return false;
    }
    let mut targets = program.machines().iter().filter_map(|owner| {
        let entry = program.machine_states(owner).first()?;
        (entry.symbol == call.target_symbol).then_some((owner, entry))
    });
    let Some((owner, entry)) = targets.next() else {
        return false;
    };
    if targets.next().is_some()
        || owner.attached_data_symbol != *symbol
        || owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !matches!(
            program
                .type_reference_table
                .type_reference(entry.return_type),
            TypeReferenceNode::Named { .. }
        )
        || program
            .primitive_type_reference(entry.return_type)
            .is_none()
    {
        return false;
    }
    let parameters = program.state_parameters(entry);
    parameters.first().is_some_and(|parameter| {
        parameter.is_self
            && matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Shared,
                    ..
                }
            )
    }) && parameters
        .iter()
        .filter(|parameter| parameter.is_self)
        .count()
        == 1
}

fn type_reference_contains_dynamic_trait(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_contains_dynamic_trait(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_contains_dynamic_trait(program, *base_type)
        }
        typed_trees::types::TypeReferenceNode::DynamicTrait { .. } => true,
        _ => false,
    }
}

/// Free scalar graph initializers and immutable scalar result calls in a Unit body
/// retain nested operands through checked computation lowering. Other value
/// destinations keep the realization fence until they use that evaluation path.
pub(crate) fn report_nested_call_in_local_initializer(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if free_scalar_computation_call(program, machine, value)
        || result_initializer_call_is_supported(program, machine, value)
    {
        // This exempts a destination, not its semantics. Ordinary call checks
        // still validate every argument; unsupported computation nodes or call
        // custody fail lowering before any Terminal artifact can be published.
        return;
    }
    report_nested_call_in_bound_value_call(program, machine, state_name, value, diagnostics);
}

/// Result destinations handled by the ordinary checked statement sequence.
/// Typing and ordinary call validation still own result, argument and contract
/// compatibility; this predicate does not supply those semantic judgments.
/// Scalar and plain structural results use the authored statement sequence.
/// Structural ownership and cleanup are checked by that sequence's producer.
pub fn unit_result_initializer_call_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    let [state] = program.machine_states(machine) else {
        return false;
    };
    if !(unit_type(program, state.return_type)
        || crate::is_closed_primitive_array_type(program, state.return_type))
        || !program.expression_table.expression_is_valid(value)
    {
        return false;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    // A final array call has the state's result destination, not a synthetic
    // local. Its operands still use the same checked computation evaluator.
    if matches!(statements.last(), Some(StatementNode::Expression(expression)) if *expression == value)
        && crate::is_closed_primitive_array_type(program, state.return_type)
    {
        return initializer_target_is_supported(
            program,
            machine,
            value,
            state.return_type,
            true,
            false,
        );
    }
    let mut initializers =
        statements
            .iter()
            .enumerate()
            .filter_map(|(statement_index, statement)| match statement {
                StatementNode::LocalData(local) if local.initial_value == value => {
                    Some((statement_index, local))
                }
                _ => None,
            });
    let Some((statement_index, local)) = initializers.next() else {
        return false;
    };
    if initializers.next().is_some()
        || local.is_mutable
        || (statement_index != 0
            && program
                .primitive_type_reference(local.type_reference)
                .is_none()
            && !ordinary_structural_initializer(program, local.initial_value, local.type_reference)
            && !boundary_structural_initializer(program, machine, local))
    {
        return false;
    }
    initializer_target_is_supported(
        program,
        machine,
        local.initial_value,
        local.type_reference,
        true,
        false,
    )
}

fn boundary_structural_initializer(
    program: &TypedTrees,
    machine: &Machine,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    // The existing exact target resolver also covers boundary-trait requirements
    // and this caller's nominal machine parameters. This is not a new receiver,
    // qualification, reference, or linear-result realization route.
    program.type_multiplicity(local.type_reference) != language_semantics::Multiplicity::Linear
        && crate::has_plain_owned_contents(program, local.type_reference)
        && initializer_target_is_supported(
            program,
            machine,
            local.initial_value,
            local.type_reference,
            false,
            false,
        )
}

fn ordinary_structural_initializer(
    program: &TypedTrees,
    value: ExpressionHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !((program.type_multiplicity(result_type) == language_semantics::Multiplicity::Affine
        && crate::has_plain_owned_contents(program, result_type))
        || crate::is_closed_primitive_array_type(program, result_type))
    {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    program.machines().iter().any(|owner| {
        owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && program.machine_states(owner).first().is_some_and(|target| {
                target.symbol == call.target_symbol
                    && !unit_type(program, target.return_type)
                    && program
                        .primitive_type_reference(target.return_type)
                        .is_none()
                    && (crate::has_plain_owned_contents(program, target.return_type)
                        || crate::is_closed_primitive_array_type(program, target.return_type))
            })
    })
}

fn initializer_target_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
    allow_ordinary: bool,
    allow_parameter_receiver: bool,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    if !call.target_symbol.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return false;
    }
    let mut targets = program.machines().iter().filter_map(|owner| {
        let entry = program.machine_states(owner).first()?;
        (entry.symbol == call.target_symbol).then_some((owner, entry))
    });
    if let Some((owner, target)) = targets.next() {
        return targets.next().is_none()
            && (program.call_has_no_runtime_receiver(call, owner, target)
                || (allow_parameter_receiver
                    && owner.supply_mode.is_boundary_declaration()
                    && boundary_return::has_parameter_receiver(
                        program, machine, call, owner, target,
                    )))
            && !unit_type(program, target.return_type)
            && (allow_ordinary
                || program.primitive_type_reference(target.return_type)
                    == program.primitive_type_reference(result_type))
            && (owner.supply_mode.is_boundary_declaration()
                || (allow_ordinary
                    && ((program.primitive_type_reference(result_type).is_some()
                        && program
                            .primitive_type_reference(target.return_type)
                            .is_some())
                        || (program.primitive_type_reference(result_type).is_none()
                            && ordinary_structural_initializer(program, value, result_type)))));
    }

    let selected_parameter = program.machine_parameter_signature(call.target_symbol);
    let requirement = match selected_parameter {
        Some((owner, signature)) if owner.symbol == machine.symbol => signature.symbol,
        Some(_) => return false,
        None => call.target_symbol,
    };
    let mut requirements = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .filter(move |signature| signature.symbol == requirement)
                .map(move |signature| (definition, signature))
        });
    let Some((definition, signature)) = requirements.next() else {
        return false;
    };
    if requirements.next().is_some()
        || unit_type(program, signature.return_type)
        || (!allow_ordinary
            && program.primitive_type_reference(signature.return_type)
                != program.primitive_type_reference(result_type))
        || program
            .state_signature_parameters(signature)
            .iter()
            .any(|parameter| parameter.is_self)
    {
        return false;
    }
    if selected_parameter.is_some() {
        return !call.receiver.is_valid();
    }
    program.expression_table.expression_is_valid(call.receiver)
        && matches!(program.expression_table.expression(call.receiver),
            ExpressionNode::Name(path) if path.symbol == definition.symbol)
}

fn unit_type(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::Unit => return true,
            _ => return false,
        }
    }
}

/// Only an exact mutable scalar local is an assignment computation destination.
/// Projected storage, state parameters, and attached/structural callers keep the
/// realization fence until their writes have the same checked evaluation path.
pub(crate) fn report_nested_call_in_local_assignment(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    state_name: &str,
    assignment: &TableAssignment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(state) = state
        && let ExpressionNode::Name(path) = program.expression_table.expression(assignment.target)
        && path.symbol.is_valid()
        && program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1
        && program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                matches!(statement, StatementNode::LocalData(local)
                    if local.symbol == path.symbol
                        && local.is_mutable
                        && program.primitive_type_reference(local.type_reference).is_some())
            })
        && free_scalar_computation_call(program, machine, assignment.value)
    {
        return;
    }
    report_nested_call_in_bound_value_call(
        program,
        machine,
        state_name,
        assignment.value,
        diagnostics,
    );
}

fn free_scalar_computation_call(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    if !value.is_valid() {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    free_scalar_machine(program, machine)
        && !call.receiver.is_valid()
        && call.machine_arguments.is_empty()
        && call.evidence_arguments.is_empty()
        && call.static_requirement_dispatch.is_none()
        && program.machines().iter().any(|target| {
            free_scalar_machine(program, target)
                && program
                    .machine_states(target)
                    .first()
                    .is_some_and(|entry| entry.symbol == call.target_symbol)
        })
}

fn free_scalar_machine(program: &TypedTrees, machine: &Machine) -> bool {
    let states = program.machine_states(machine);
    machine.attached_data.is_none()
        && machine.type_parameters.is_empty()
        && machine.owned_data.is_empty()
        && !states.is_empty()
        && states.iter().all(|state| {
            let parameters = program.state_parameters(state);
            let mixed = parameters.iter().any(|parameter| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            });
            // Mixed scalar roots retain primitive borrows and whole owned inputs.
            // Structural state forwarding remains outside this graph slice;
            // exact computation and borrow custody are still checked downstream.
            if mixed && states.len() != 1 {
                return false;
            }
            program
                .primitive_type_reference(state.return_type)
                .is_some()
                && parameters.iter().all(|parameter| {
                    let reference = if mixed {
                        match program
                            .type_reference_table
                            .type_reference(parameter.type_reference)
                        {
                            typed_trees::types::TypeReferenceNode::Reference {
                                referee, ..
                            } => *referee,
                            _ => parameter.type_reference,
                        }
                    } else {
                        parameter.type_reference
                    };
                    !parameter.is_self
                        && !parameter.is_const
                        && (!parameter.is_mutable
                            || matches!(
                                program
                                    .type_reference_table
                                    .type_reference(parameter.type_reference),
                                typed_trees::types::TypeReferenceNode::Reference { .. }
                            ))
                        && (!mixed
                            || matches!(
                                program.type_reference_table.type_reference(reference),
                                typed_trees::types::TypeReferenceNode::Named { .. }
                            ))
                        && (program.primitive_type_reference(reference).is_some()
                            || (reference == parameter.type_reference
                                && matches!(
                                    program.type_multiplicity(reference),
                                    language_semantics::Multiplicity::Affine
                                        | language_semantics::Multiplicity::Unrestricted
                                )
                                && crate::has_plain_owned_contents_with_numeric_constraints(
                                    program, reference,
                                )))
                })
        })
}

/// A LET/ASSIGNMENT-bound value call whose ARGUMENT nests another machine call
/// (`let out = self.double(self.inc(3))`) reads a garbage inner result: the
/// inner callee's frame locals cannot materialize inside the outer call's
/// argument context in the VALUE sink (some consumer shapes fence loudly with
/// "needs stack/local storage lowering", but the guard-consumer shape slipped
/// through and natively bound 0). STATEMENT-call arguments take a different
/// materialization path and legitimately nest (the dungeon's
/// `self.append_exit(.., self.direction_command(self.opposite(d)), ..)`), so
/// this check runs ONLY on the value expression of a local/assignment.
pub(crate) fn report_nested_call_in_bound_value_call(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !value.is_valid() {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    // A BUILTIN outer call (`let v = max(self.range(..), floor)`) composes:
    // builtin arguments materialize as operands through the call-result-local
    // machinery (canaried). Only a MACHINE outer call's argument context is
    // broken.
    if matches!(
        call.target.as_str(),
        "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
    ) {
        return;
    }
    for argument in program.expression_table.expression_handles(call.arguments) {
        if let Some(inner) = first_non_builtin_call(program, *argument) {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}`: a value-call argument cannot itself \
                 be a machine call yet (`{inner}(..)` nested in `{}(..)` would read a \
                 garbage result) -- bind the inner call to a local first, then pass \
                 the local.",
                machine.name,
                call.target.as_str(),
            )));
            return;
        }
    }
}

/// The first NON-BUILTIN machine call nested anywhere inside `expression`
/// (its target name, for the diagnostic), or None. Reserved value builtins
/// (`min`/`max`/`sqrt`) and the view builtins (`as_slice`/`as_mut_slice`/
/// `as_view`/`bytes`) compose in arguments and are exempt.
fn first_non_builtin_call(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<Identifier> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => first_non_builtin_call(program, atomic.value),
        ExpressionNode::Call(call) => {
            if !matches!(
                call.target.as_str(),
                "min" | "max" | "sqrt" | "as_slice" | "as_mut_slice" | "as_view" | "bytes"
            ) {
                return Some(call.target.clone());
            }
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .find_map(|argument| first_non_builtin_call(program, *argument))
        }
        ExpressionNode::Binary(binary) => first_non_builtin_call(program, binary.left)
            .or_else(|| first_non_builtin_call(program, binary.right)),
        ExpressionNode::Unary(unary) => first_non_builtin_call(program, unary.operand),
        ExpressionNode::Cast(cast) => first_non_builtin_call(program, cast.value),
        ExpressionNode::Borrow(inner) => first_non_builtin_call(program, inner.target),
        ExpressionNode::Indexed(indexed) => first_non_builtin_call(program, indexed.collection)
            .or_else(|| first_non_builtin_call(program, indexed.index)),
        ExpressionNode::Member(member) => first_non_builtin_call(program, member.receiver),
        _ => None,
    }
}

/// A VOID callee in VALUE position used to compile and silently bind 0 (ZII)
/// -- and native/interp DIVERGED on the bound value. "Void" means: no declared
/// return type on the resolved state (the parser now lands `-> T` written
/// after the machine clauses too; it used to be silently dropped) AND no state
/// of the callee machine produces a value through a transition VALUE arm --
/// undeclared-return value machines (`transition r > 0 { true -> self.f(r-1)
/// false -> 0 }`, the termination-canary surface) stay callable.
pub(super) fn report_void_value_callee(
    program: &TypedTrees,
    callee_machine: &Machine,
    current_machine: &Machine,
    current_state: &State,
    callee_state: &State,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if callee_state.return_type.is_valid()
        || crate::calls::unit_statement_call_is_supported(
            program,
            current_machine,
            current_state,
            expression,
        )
    {
        return;
    }
    let produces_value = program.machine_states(callee_machine).iter().any(|state| {
        state.return_type.is_valid()
            || program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    let StatementNode::Transition(transition) = statement else {
                        return false;
                    };
                    [transition.target, transition.continuation]
                        .iter()
                        .any(|handle| {
                            handle.is_valid()
                                && matches!(
                                    program.statement_table.transition_target(*handle),
                                    TransitionTargetNode::Value(_)
                                )
                        })
                })
    });
    if produces_value {
        return;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return;
    };
    let callee_display = call.target.as_str();
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}`: `{callee_display}(..)` does not return a value but is \
         used in a VALUE position -- it would silently bind 0 (ZII) at runtime. Declare \
         a return type on the callee (`-> T`) or call it as a statement.",
        current_machine.name,
        current_state.name.as_str(),
    )));
}

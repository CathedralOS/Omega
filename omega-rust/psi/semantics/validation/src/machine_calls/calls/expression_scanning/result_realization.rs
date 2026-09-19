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
    if scalar_computation_call(program, machine, value, true)
        || result_initializer_call_is_supported(program, machine, value)
        || reference_result_nested_operand_call_is_supported(program, machine, value)
    {
        // This exempts a destination, not its semantics. Ordinary call checks
        // still validate every argument; unsupported computation nodes or call
        // custody fail lowering before any Terminal artifact can be published.
        return;
    }
    report_nested_call_in_bound_value_call(program, machine, state_name, value, diagnostics);
}

/// A bound bare `&mut` result local may take its projected record operand
/// from a nested call's anonymous result: `let held: &mut i32 =
/// select(forward_outer(outer).inner)` — the nested-operand form of the
/// named-carrier lane. This predicate only mirrors the checked route's
/// authored shape: the outer callee is a checked-body bare reference result
/// selecting one declared leaf of an owned record parameter, the projected
/// operand's carrier is one nested checked-body call returning that record's
/// owner type, and the nested result's whole returned leaf roster lives
/// under the projected edge with each leaf's actual a bare earlier local.
/// Captured-loan custody, activity, and last-use are still re-proven exactly
/// in `reference_result_custody` and the checked call builder, which fail
/// closed on anything this shape check lets through.
fn reference_result_nested_operand_call_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    // The bound local: the same statement walk
    // `unit_result_initializer_call_is_supported` performs.
    let mut bindings = program.machine_states(machine).iter().filter_map(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .find_map(|(index, statement)| match statement {
                StatementNode::LocalData(local) if local.initial_value == value => {
                    Some((state, index, local))
                }
                _ => None,
            })
    });
    let Some((state, statement_index, local)) = bindings.next() else {
        return false;
    };
    if bindings.next().is_some()
        || local.is_mutable
        || crate::machine_calls::reference_result_custody::parts(program, local.type_reference)
            .is_none()
    {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    if !call.target_symbol.is_valid()
        || call.receiver.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return false;
    }
    let mut owners = program.machines().iter().filter(|owner| {
        owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && program
                .machine_states(owner)
                .iter()
                .any(|candidate| candidate.symbol == call.target_symbol)
    });
    let Some(owner) = owners.next() else {
        return false;
    };
    if owners.next().is_some() {
        return false;
    }
    let Some(destination) = program
        .machine_states(owner)
        .iter()
        .find(|candidate| candidate.symbol == call.target_symbol)
    else {
        return false;
    };
    if crate::machine_calls::reference_result_custody::parts(program, destination.return_type)
        .is_none()
    {
        return false;
    }
    // The callee result must select one declared leaf of an owned record
    // parameter (`select(value: View) -> &mut i32 { value.body }`).
    let Some((position, leaf_source)) =
        crate::machine_calls::reference_result_custody::source_leaf(program, destination)
    else {
        return false;
    };
    let [
        leaf_path @ ..,
        checked_trees::CheckedUnitStructuralPathSegment::Referent,
    ] = leaf_source.path.as_slice()
    else {
        return false;
    };
    if leaf_path.is_empty() {
        return false;
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    let Some(&argument) = arguments.get(position) else {
        return false;
    };
    // Only the projected operand position may carry the nested call; a nested
    // operand at any other argument keeps the bound value-call fence.
    if arguments.iter().enumerate().any(|(index, argument)| {
        index != position && first_non_builtin_call(program, *argument).is_some()
    }) {
        return false;
    }
    // The projected operand: a declared-field chain over one nested call.
    let mut members = Vec::new();
    let mut root = argument;
    while let ExpressionNode::Member(member) = program.expression_table.expression(root) {
        if member.case_variant.is_some() {
            return false;
        }
        members.push(member);
        root = member.receiver;
    }
    if members.is_empty() {
        return false;
    }
    let ExpressionNode::Call(nested) = program.expression_table.expression(root) else {
        return false;
    };
    if !nested.target_symbol.is_valid()
        || nested.receiver.is_valid()
        || !nested.machine_arguments.is_empty()
        || !nested.evidence_arguments.is_empty()
        || nested.static_requirement_dispatch.is_some()
        || nested.quotient_operation.is_some()
        || nested.private_layout_operation.is_some()
    {
        return false;
    }
    let mut nested_owners = program.machines().iter().filter(|owner| {
        owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && program
                .machine_states(owner)
                .iter()
                .any(|candidate| candidate.symbol == nested.target_symbol)
    });
    let Some(nested_owner) = nested_owners.next() else {
        return false;
    };
    if nested_owners.next().is_some() {
        return false;
    }
    let Some(nested_destination) = program
        .machine_states(nested_owner)
        .iter()
        .find(|candidate| candidate.symbol == nested.target_symbol)
    else {
        return false;
    };
    if !crate::machine_calls::reference_result_custody::is_reference_record(
        program,
        nested_destination.return_type,
    ) {
        return false;
    }
    // The projected edge must resolve to exactly the callee parameter's
    // declared type; the checked side re-proves it against the result loan.
    let Some(parameter) = program.state_parameters(destination).get(position) else {
        return false;
    };
    let Some((edge, projected)) =
        crate::machine_calls::reference_result_custody::declared_field_path(
            program,
            nested_destination.return_type,
            members.iter().rev().copied(),
        )
    else {
        return false;
    };
    if program.normalized_type_identity(projected)
        != program.normalized_type_identity(parameter.type_reference)
    {
        return false;
    }
    // The nested result's whole returned leaf roster must live under the
    // projected edge, each leaf replaying a parameter-referent ingress whose
    // actual is a bare earlier local of the exact declared type.
    let Some(sources) = crate::machine_calls::reference_result_custody::returned_record_sources(
        program,
        nested_destination,
    ) else {
        return false;
    };
    !sources.is_empty()
        && sources.iter().all(|source| {
            if !source.path.starts_with(&edge) {
                return false;
            }
            let [
                ..,
                checked_trees::CheckedUnitStructuralPathSegment::Referent,
            ] = source.source.path.as_slice()
            else {
                return false;
            };
            let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } = source.source.source
            else {
                return false;
            };
            let Some((actual_position, nested_parameter)) = program
                .state_parameters(nested_destination)
                .iter()
                .enumerate()
                .filter(|(_, parameter)| {
                    !parameter.is_const
                        && program
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
                .nth(parameter_index as usize)
            else {
                return false;
            };
            if !crate::machine_calls::reference_result_custody::is_reference_record(
                program,
                nested_parameter.type_reference,
            ) {
                return false;
            }
            let Some(&actual) = program
                .expression_table
                .expression_handles(nested.arguments)
                .get(actual_position)
            else {
                return false;
            };
            let ExpressionNode::Name(name) = program.expression_table.expression(actual) else {
                return false;
            };
            if name.head_symbol != name.symbol
                || program
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
            {
                return false;
            }
            program
                .statement_table
                .statements(state.statement_nodes)
                .get(..statement_index)
                .is_some_and(|prior| {
                    prior.iter().any(|statement| {
                        matches!(statement, StatementNode::LocalData(local)
                            if local.symbol == name.symbol
                                && !local.is_mutable
                                && crate::machine_calls::reference_result_custody::is_reference_record(
                                    program,
                                    local.type_reference,
                                )
                                && program.normalized_type_identity(local.type_reference)
                                    == program
                                        .normalized_type_identity(nested_parameter.type_reference))
                    })
                })
        })
}

/// Result destinations handled by the ordinary checked statement sequence.
/// Typing and ordinary call validation still own result, argument and contract
/// compatibility; this predicate does not supply those semantic judgments.
/// Scalar and owned structural results use the authored statement sequence.
/// Structural ownership and cleanup are checked by that sequence's producer.
pub fn unit_result_initializer_call_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    let mut states = program.machine_states(machine).iter().filter(|state| {
        program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.initial_value == value)
                || matches!(statement, StatementNode::Expression(expression) if *expression == value)
        })
    });
    let Some(state) = states.next() else {
        return false;
    };
    if states.next().is_some()
        || !(unit_type(program, state.return_type)
            || ordinary_structural_result_type(program, state.return_type))
        || !program.expression_table.expression_is_valid(value)
    {
        return false;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    // A final array call has the state's result destination, not a synthetic
    // local. Its operands still use the same checked computation evaluator.
    if matches!(statements.last(), Some(StatementNode::Expression(expression)) if *expression == value)
        && crate::is_closed_primitive_array_type(program, state.return_type)
        && program.machine_states(machine).len() == 1
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
    let has_ordinary_structural_initializer =
        ordinary_structural_initializer(program, local.initial_value, local.type_reference);
    if initializers.next().is_some()
        // Scalar locals in state graphs keep their whole-call computation
        // owner. Ordinary structural results already use statement sequencing
        // in each state, including the constructor's nested operands.
        || (program.machine_states(machine).len() != 1
            && !has_ordinary_structural_initializer)
        // Mutable plain results retain the ordinary constructor's storage.
        // Primitive initialization, linear custody, and boundary results keep
        // their separate receiving contracts.
        || (local.is_mutable
            && (program.type_multiplicity(local.type_reference)
                == language_semantics::Multiplicity::Linear
                || !has_ordinary_structural_initializer))
        || (statement_index != 0
            && program
                .primitive_type_reference(local.type_reference)
                .is_none()
            && !has_ordinary_structural_initializer
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
    if !ordinary_structural_result_type(program, result_type) {
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
                    && ordinary_structural_result_type(program, target.return_type)
            })
    })
}

/// The same statement sequence owns these results in Unit and structural-return
/// bodies. Classifying a linear destination does not establish its claims:
/// ordinary call/return custody must still reconstruct the exact input lineage.
/// Reference destinations reject here; boundary results retain their existing
/// target admission and separate custody validation.
fn ordinary_structural_result_type(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_multiplicity(reference) {
        language_semantics::Multiplicity::Linear => {
            crate::structural_result_qualifications(program, reference).is_ok()
        }
        language_semantics::Multiplicity::Affine => {
            crate::has_plain_owned_contents(program, reference)
        }
        language_semantics::Multiplicity::Unrestricted => {
            crate::is_closed_primitive_array_type(program, reference)
        }
    }
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

/// Only an exact mutable scalar local is an assignment computation destination
/// in the scalar graph. The unit statement sequence owns the same write when
/// the destination is a mutable primitive local, a whole borrowed primitive
/// parameter, or a member path through a borrowed structural parameter or the
/// attached receiver: its `AssignmentValue` computation root evaluates the
/// outer call and every nested operand in authored order before the store.
/// Indexed or ranged carriers, record locals, case-qualified hops, shared or
/// constrained carriers, and generic or boundary callers keep the realization
/// fence until their stores sequence nested operands through that same checked
/// path.
pub(crate) fn report_nested_call_in_local_assignment(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    state_name: &str,
    assignment: &TableAssignment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(state) = state {
        if let ExpressionNode::Name(path) = program.expression_table.expression(assignment.target)
            && path.symbol.is_valid()
            && program
                .expression_table
                .name_path_members(path.members)
                .len()
                == 1
        {
            if program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(statement, StatementNode::LocalData(local)
                        if local.symbol == path.symbol
                            && local.is_mutable
                            && program.primitive_type_reference(local.type_reference).is_some())
                })
                && scalar_computation_call(program, machine, assignment.value, false)
            {
                return;
            }
            if unit_scalar_store_assignment_is_supported(
                program,
                machine,
                state,
                assignment,
                path.symbol,
            ) {
                return;
            }
        }
        if unit_member_scalar_store_assignment_is_supported(program, machine, state, assignment) {
            return;
        }
    }
    report_nested_call_in_bound_value_call(
        program,
        machine,
        state_name,
        assignment.value,
        diagnostics,
    );
}

/// The call-shape gate every unit store assignment shares: the outer call is a
/// direct, ordinary checked-body call to a free scalar callee. The evaluator
/// recurses into each argument itself, so this admission intentionally covers
/// only the root call shape — argument custody, structural operands, and flow
/// call ordinals still resolve in the checked builder, which rejects anything
/// it cannot sequence. Returns the callee's declared result primitive, which
/// each destination family then matches against its own stored type.
fn unit_store_call_result_primitive(
    program: &TypedTrees,
    machine: &Machine,
    assignment: &TableAssignment,
) -> Option<typed_trees::types::PrimitiveType> {
    // Generic, owned-data, and bound callers do not have a proven unit store
    // plan for this destination family.
    if !machine.type_parameters.is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || !machine.owned_data.is_empty()
        || !assignment.value.is_valid()
    {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(assignment.value) else {
        return None;
    };
    if call.receiver.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    let entry = program
        .machines()
        .iter()
        .find(|owner| {
            owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && free_scalar_machine(program, owner)
                && program
                    .machine_states(owner)
                    .first()
                    .is_some_and(|entry| entry.symbol == call.target_symbol)
        })
        .and_then(|owner| program.machine_states(owner).first())?;
    program.primitive_type_reference(entry.return_type)
}

/// Mirrors the checked `AssignmentValue` computation admission for a call
/// stored through the unit statement sequence: the outer call is a direct,
/// ordinary checked-body call to a free scalar callee whose declared result
/// primitive is exactly the destination's stored primitive.
fn unit_scalar_store_assignment_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
    target_symbol: symbols::SymbolHandle,
) -> bool {
    let Some(result_primitive) = unit_store_call_result_primitive(program, machine, assignment)
    else {
        return false;
    };
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == target_symbol)
    {
        // A bare `target = call(..)` writes through the whole borrowed place.
        // Only `&mut`/`&writeonly` primitives are whole-value store
        // destinations; shared borrows, plain scalars, and structural
        // referees keep the fence.
        return !parameter.is_self
            && parameter.is_mutable
            && !parameter.is_const
            && matches!(
                program
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                typed_trees::types::TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::Mutable
                        | language_semantics::ReferenceAccess::WriteOnly,
                    referee,
                    ..
                } if matches!(
                    program.type_reference_table.type_reference(*referee),
                    typed_trees::types::TypeReferenceNode::Named { .. }
                ) && program.primitive_type_reference(*referee) == Some(result_primitive)
            );
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if local.symbol == target_symbol
                    && local.is_mutable
                    && program.expression_table.expression_is_valid(local.initial_value)
                    && matches!(
                        program.type_reference_table.type_reference(local.type_reference),
                        typed_trees::types::TypeReferenceNode::Named { .. }
                    )
                    && program.primitive_type_reference(local.type_reference)
                        == Some(result_primitive))
        })
}

/// A member-target store (`param.field = call(..)`, `self.field = call(..)`)
/// sequences through the same checked structural field-store producer as an
/// authored scalar source: the statement sequence emits the call and a store
/// consuming its SSA result, or carries the whole call tree as the store's
/// `AssignmentValue` computation. This predicate mirrors
/// `build_structural_field_store_at`'s typed-tree-visible destination walk —
/// the canonical member path roots at the state's mutable borrowed structural
/// parameter or the attached `&mut self` receiver, every carrier resolves an
/// exact relevant field on a plain record owner, and the leaf is a
/// domain-unconstrained scalar whose declared primitive is exactly the
/// callee's result. Write-frame agreement, ABI position accounting, call
/// ordinals, and the computation rows are flow-side and still fail closed
/// downstream; indexed or ranged carriers, case-qualified hops, record locals,
/// shared borrows, domain-constrained or generic carriers, and non-scalar
/// leaves keep the fence here.
fn unit_member_scalar_store_assignment_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
) -> bool {
    let Some(result_primitive) = unit_store_call_result_primitive(program, machine, assignment)
    else {
        return false;
    };
    let Some((parameter, hops)) = member_store_destination(program, state, assignment.target)
    else {
        return false;
    };
    member_store_signature_is_supported(program, machine, state)
        && member_store_path_is_supported(program, machine, parameter, &hops, result_primitive)
}

/// One member hop of a store target: the authored member name plus the symbol
/// the expression retained for it. `effective_member_symbol` re-resolves the
/// name against the reached owner's data first and uses the retained symbol
/// only as a fallback, so the path walk below does the same.
struct MemberStoreHop {
    name: Identifier,
    symbol: symbols::SymbolHandle,
}

/// The store target's canonical member path as `(root name, root symbol,
/// single-member root, field hops in authored order)`. `Member` hops collect
/// leaf-first until a `Name` root; a multi-member `Name` path contributes its
/// leading members before them. `self` receivers, case-qualified members,
/// indexed or ranged carriers, and non-name roots produce no path — the
/// checked producer cannot build a field store for them either.
fn member_store_target_path(
    program: &TypedTrees,
    target: ExpressionHandle,
) -> Option<(Identifier, symbols::SymbolHandle, bool, Vec<MemberStoreHop>)> {
    let mut trailing = Vec::new();
    let mut handle = target;
    loop {
        match program.expression_table.expression(handle) {
            ExpressionNode::Borrow(inner) => handle = inner.target,
            ExpressionNode::Member(member) => {
                // A case-qualified hop canonicalizes through a Case segment,
                // which the field-store carrier walk cannot carry.
                if member.case_variant.is_some() {
                    return None;
                }
                trailing.push(MemberStoreHop {
                    name: member.member.clone(),
                    symbol: member.member_symbol,
                });
                handle = member.receiver;
            }
            ExpressionNode::Name(path) => {
                let members = program.expression_table.name_path_members(path.members);
                let member_symbols = program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols);
                let (root, fields) = members.split_first()?;
                if fields.is_empty() && trailing.is_empty() {
                    return None;
                }
                // `first_valid_name_path_symbol`'s order: the first member's
                // retained symbol, then the path's head and whole-path symbol.
                let root_symbol = member_symbols
                    .first()
                    .copied()
                    .filter(|symbol| symbol.is_valid())
                    .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
                    .or_else(|| path.symbol.is_valid().then_some(path.symbol))
                    .unwrap_or_else(symbols::SymbolHandle::invalid);
                let mut hops = Vec::with_capacity(fields.len() + trailing.len());
                for (index, member) in fields.iter().enumerate() {
                    hops.push(MemberStoreHop {
                        name: member.clone(),
                        symbol: member_symbols
                            .get(index + 1)
                            .copied()
                            .unwrap_or_else(symbols::SymbolHandle::invalid),
                    });
                }
                hops.extend(trailing.into_iter().rev());
                return Some((root.clone(), root_symbol, fields.is_empty(), hops));
            }
            _ => return None,
        }
    }
}

/// The state parameter a member-target store roots at, mirroring
/// `resolve_contextual_name_path_root`: a lone `self` names the receiver
/// parameter directly; other roots bind their retained symbol, with the
/// contextual fallback to the non-self parameter a single-member path spells.
/// A record local's symbol matches no state parameter here, so local member
/// stores keep the fence with the producer's parameter-only destinations.
fn member_store_destination<'a>(
    program: &'a TypedTrees,
    state: &'a State,
    target: ExpressionHandle,
) -> Option<(
    &'a typed_trees::signature::StateParameter,
    Vec<MemberStoreHop>,
)> {
    let (root_name, root_symbol, single_member, hops) = member_store_target_path(program, target)?;
    let parameter = if single_member && root_name.as_str() == "self" {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
    } else if root_symbol.is_valid() {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == root_symbol)
    } else if single_member {
        program
            .state_parameters(state)
            .iter()
            .find(|parameter| !parameter.is_self && parameter.name.as_str() == root_name.as_str())
    } else {
        None
    }?;
    Some((parameter, hops))
}

/// The unit signature's per-parameter early exits, minus shape-collector work:
/// erased self/const/mutable bindings refuse the plan; a `self` receiver needs
/// the machine attachment; primitive siblings must fit the immutable scalar
/// lane; bound-service parameters route the state through the fused-service
/// signature, which cannot host a mutable borrowed destination at all; and
/// structural siblings need a representable access and a valid domain spine.
/// A state with a service parameter reaches `fused_service_scalar_signature`,
/// which admits no mutable destination at all, and a free operator-spelled
/// machine may reach `free_selected_operator_structural_signature`, which
/// rejects mutable parameters outright — both keep the fence rather than
/// re-deriving their dispatch here. Under `structural_signature` — the
/// attached/no-scalar-parameter variant — a direct `Placed` reference sibling
/// is skipped by the plan and the bounded shared-observer leaf cannot apply
/// to a state with statements, so a shared borrow of a primitive is
/// unplannable there. Shape identities and projected qualifications still
/// build or fail inside the checker.
fn member_store_signature_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
) -> bool {
    let attached = machine.attached_data.as_ref().is_some_and(|name| {
        program
            .data_definitions()
            .iter()
            .any(|data| data.name == *name)
    });
    if machine.attached_data.is_none() && machine.spelling.is_some() {
        return false;
    }
    let parameters = program.state_parameters(state);
    let carries_scalar_parameter = parameters.iter().any(|parameter| {
        !parameter.is_self
            && !parameter.relevance.is_erased()
            && program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
    });
    let partial_affine_signature = attached && !carries_scalar_parameter;
    parameters.iter().all(|parameter| {
        if parameter.relevance.is_erased() {
            return !parameter.is_self && !parameter.is_const && !parameter.is_mutable;
        }
        if parameter.is_self && !attached {
            return false;
        }
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            return !parameter.is_self && !parameter.is_const && !parameter.is_mutable;
        }
        if parameter.is_const {
            return false;
        }
        if typed_trees::service::exact_bound_service_requirement(program, parameter.type_reference)
            .is_some()
        {
            return false;
        }
        if parameter.is_self && type_is_reference(program, parameter.type_reference) {
            // A borrowed receiver is ambient in the first signature attempt
            // and retained as a structural parameter on retry; either keeps
            // the state plannable for a member store rooted at `self` or at a
            // sibling parameter.
            return true;
        }
        if partial_affine_signature
            && let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = program
                .type_reference_table
                .type_reference(parameter.type_reference)
        {
            // A `Placed` view has no runtime carrier and is skipped by the
            // signature, leaving the ABI count short; a shared borrow of a
            // primitive is plannable only as the bounded observer leaf of an
            // empty state.
            if program
                .placed_view_plan_for_type_reference(*referee)
                .is_some()
            {
                return false;
            }
            if program.primitive_type_reference(*referee).is_some() {
                return mutable_store_referee(program, parameter.type_reference).is_some();
            }
        }
        structural_store_access_supported(program, parameter.type_reference)
            && parameter_domain_spine_supported(program, parameter.type_reference)
    })
}

/// `build_structural_field_store_at`'s destination walk on typed trees: the
/// root parameter must be a mutable `&mut`/`&writeonly` borrow of a carrier
/// whose record owner resolves through the retained type name (or the machine
/// attachment for `self`), each carrier resolves an exact relevant
/// domain-unconstrained field on a plain record, and the leaf field's declared
/// primitive is exactly the call's result. The producer's linear-destination
/// gate is subsumed here: `type_multiplicity` reports every reference as
/// `Unrestricted`, so only a non-reference destination could be `Linear` — and
/// `mutable_store_referee` already refuses those.
fn member_store_path_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    parameter: &typed_trees::signature::StateParameter,
    hops: &[MemberStoreHop],
    result_primitive: typed_trees::types::PrimitiveType,
) -> bool {
    if !parameter.is_mutable || parameter.is_const {
        return false;
    }
    let Some(referee) = mutable_store_referee(program, parameter.type_reference) else {
        return false;
    };
    let owner = if parameter.is_self {
        crate::value_custody::places::machine_attached_data(program, machine)
    } else {
        field_store_owner(program, referee)
    };
    let Some(mut owner) = owner else {
        return false;
    };
    if !structural_store_plain_record(program, owner) {
        return false;
    }
    let Some((leaf, carriers)) = hops.split_last() else {
        return false;
    };
    for carrier in carriers {
        let Some(field) = store_member_field(program, owner, carrier) else {
            return false;
        };
        if !field_store_domain_free(program, field.type_reference) {
            return false;
        }
        let Some(next) = field_store_owner(program, field.type_reference) else {
            return false;
        };
        if !structural_store_plain_record(program, next) {
            return false;
        }
        owner = next;
    }
    let Some(field) = store_member_field(program, owner, leaf) else {
        return false;
    };
    if !field_store_domain_free(program, field.type_reference) {
        return false;
    }
    let Some(primitive) = program.primitive_type_reference(field.type_reference) else {
        return false;
    };
    (matches!(
        primitive,
        typed_trees::types::PrimitiveType::Bool
            | typed_trees::types::PrimitiveType::F32
            | typed_trees::types::PrimitiveType::F64
    ) || (primitive.accepts_integer_literal()
        && primitive != typed_trees::types::PrimitiveType::Addr))
        && primitive == result_primitive
}

/// The destination parameter's carrier: one `&mut`/`&writeonly` reference with
/// no constraint anywhere on its `Constrained`/`Reference` spine — a domain on
/// the parameter surfaces in the structural plan's qualifications, which the
/// field-store producer requires empty — and a non-reference referee, which
/// `structural_access_for_type_reference` needs to plan it at all.
fn mutable_store_referee(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let mut referee = None;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Constrained { .. } => return None,
            typed_trees::types::TypeReferenceNode::Reference {
                access,
                referee: next,
                ..
            } => {
                if referee.is_some()
                    || !matches!(
                        access,
                        language_semantics::ReferenceAccess::Mutable
                            | language_semantics::ReferenceAccess::WriteOnly
                    )
                {
                    return None;
                }
                referee = Some(*next);
                type_reference = *next;
            }
            _ => break,
        }
    }
    referee
}

/// `is_reference`: true when a type bottoms out at a `Reference` node through
/// `Constrained` shells.
fn type_is_reference(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}

/// `structural_access_for_type_reference` admits every carrier except a
/// reference to a reference; the access kind itself does not matter for a
/// sibling that is not the store destination.
fn structural_store_access_supported(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                return !type_is_reference(program, *referee);
            }
            _ => return true,
        }
    }
}

/// `parameter_qualifications` yields a plan only when every constraint on the
/// `Constrained`/`Reference` spine is a declared domain with a resolved
/// semantic identity.
fn parameter_domain_spine_supported(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if !program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .all(|constraint| {
                        matches!(
                            constraint,
                            typed_trees::types::TypeConstraintNode::Domain(domain)
                                if domain.semantic_id.is_valid()
                        )
                    })
                {
                    return false;
                }
                type_reference = *base_type;
            }
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                type_reference = *referee
            }
            _ => return true,
        }
    }
}

/// `data_definition_for_field_type`: the record a field's type names, looking
/// through reference and constraint shells to a `Named` node. `Generic`
/// applications, primitives, arrays, and slices name no field-store owner.
fn field_store_owner(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            typed_trees::types::TypeReferenceNode::Named { name, .. } => {
                return program
                    .data_definitions()
                    .iter()
                    .find(|data| data.name == *name);
            }
            typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
                type_reference = *referee
            }
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type
            }
            _ => return None,
        }
    }
}

/// `plain_record` plus `retained_record_owner_application`: a checked-shape
/// record with no parameters, lifetimes, quotient, where facts, or zero gating
/// — or an already-generated closed instance of such a template.
fn structural_store_plain_record(
    program: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && retained_record_owner(program, data)
        && data.quotient.is_none()
        && data.where_facts.is_empty()
        && !data.zero_gated
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == typed_trees::data::DataShapeKind::Record
}

/// `retained_record_owner_application`: a generated instance must retain the
/// exact closed application of a plain template — never a substituted
/// declaration re-derived by name.
fn retained_record_owner(program: &TypedTrees, data: &typed_trees::data::DataDefinition) -> bool {
    let Some(application) = data.generic_instance else {
        return true;
    };
    let typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(application)
    else {
        return false;
    };
    let Some(template) = program
        .data_definitions()
        .iter()
        .find(|template| template.symbol == *base_symbol)
    else {
        return false;
    };
    let parameters = program.data_type_parameters(template);
    base_symbol.is_valid()
        && *base_symbol != data.symbol
        && template.generic_instance.is_none()
        && template.lifetime_parameters.is_empty()
        && lifetime_arguments.is_empty()
        && !parameters.is_empty()
        && parameters.len()
            == program
                .type_reference_table
                .type_reference_handles(*arguments)
                .len()
}

/// `effective_member_symbol` followed by the producer's `exact_relevant_field`
/// gate: the authored name re-resolves against the reached owner's members —
/// a field, a variant, or a variant payload field — and the retained member
/// symbol fills in only when name resolution yields nothing. The resolved
/// symbol must then name exactly one non-erased `Field` on that owner; a
/// resolved variant (or payload field) symbol matches none and keeps the
/// fence.
fn store_member_field<'a>(
    program: &'a TypedTrees,
    owner: &'a typed_trees::data::DataDefinition,
    hop: &MemberStoreHop,
) -> Option<&'a typed_trees::data::DataField> {
    let resolved = program
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if field.name.as_str() == hop.name.as_str() =>
            {
                Some(field.symbol)
            }
            typed_trees::data::DataMember::Variant(variant) => (variant.name.as_str()
                == hop.name.as_str())
            .then_some(variant.symbol)
            .or_else(|| {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find_map(|field| {
                        (field.name.as_str() == hop.name.as_str()).then_some(field.symbol)
                    })
            }),
            _ => None,
        })
        .or_else(|| hop.symbol.is_valid().then_some(hop.symbol))?;
    let mut fields = program
        .data_members(owner)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if !field.relevance.is_erased() && field.symbol == resolved =>
            {
                Some(field)
            }
            _ => None,
        });
    let field = fields.next()?;
    fields.next().is_none().then_some(field)
}

/// `domain_constraint_symbols(..).is_empty()`: no declared domain constraint
/// with a resolved symbol on the type's `Constrained`/`Reference` spine.
/// Arithmetic-policy constraints are a distinct node and stay admitted, as on
/// the checked side.
fn field_store_domain_free(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            field_store_domain_free(program, *referee)
        }
        typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .all(|constraint| {
                !matches!(
                    constraint,
                    typed_trees::types::TypeConstraintNode::Domain(domain)
                        if domain.symbol.is_valid()
                )
            }),
        _ => true,
    }
}

fn scalar_computation_call(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
    allow_static_local: bool,
) -> bool {
    if !value.is_valid() {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    // Receiver calls retain their whole expression in the scalar computation
    // graph. Both immutable and storage initializers use that ordered evaluator;
    // this admission must not reclassify the call as a result operation.
    if allow_static_local
        && call.receiver.is_valid()
        && call.machine_arguments.is_empty()
        && call.evidence_arguments.is_empty()
        && call.static_requirement_dispatch.is_none()
        && call.quotient_operation.is_none()
        && call.private_layout_operation.is_none()
        && program.machines().iter().any(|target| {
            let Some(entry) = program.machine_states(target).first() else {
                return false;
            };
            let Some(result_type) = program.primitive_type_reference(entry.return_type) else {
                return false;
            };
            let parameters = program.state_parameters(entry);
            target.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                && entry.symbol == call.target_symbol
                && parameters.first().is_some_and(|parameter| {
                    parameter.is_self
                        && !parameter.is_const
                        && matches!(program.type_reference_table.type_reference(parameter.type_reference),
                            typed_trees::types::TypeReferenceNode::Reference {
                                access: language_semantics::ReferenceAccess::Shared,
                                referee, ..
                            } if matches!(program.type_reference_table.type_reference(*referee),
                                typed_trees::types::TypeReferenceNode::Named { .. }))
                })
                && parameters.iter().filter(|parameter| parameter.is_self).count() == 1
                && program.machine_states(machine).iter().any(|state| {
                    program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
                        matches!(statement, StatementNode::LocalData(local)
                            if local.initial_value == value
                                && program.primitive_type_reference(local.type_reference) == Some(result_type))
                    })
                })
        })
    {
        return true;
    }
    (free_scalar_machine(program, machine)
        || (allow_static_local && static_scalar_local(program, machine, value)))
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

fn static_scalar_local(program: &TypedTrees, machine: &Machine, value: ExpressionHandle) -> bool {
    let [state] = program.machine_states(machine) else {
        return false;
    };
    // This is only an admission check. Do not classify the destination as a
    // result operation: nested calls retain their existing whole-call graph.
    if program
        .primitive_type_reference(state.return_type)
        .is_none()
        || !machine.type_parameters.is_empty()
        || !machine.lifetime_parameters.is_empty()
        || !machine.conformance_bounds.is_empty()
        || !machine.owned_data.is_empty()
        || program.state_parameters(state).iter().any(|parameter| {
            parameter.is_self
                || parameter.is_const
                || parameter.is_mutable
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
    {
        return false;
    }
    let mut locals = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.initial_value == value).then_some(local)
        });
    let Some(local) = locals.next() else {
        return false;
    };
    locals.next().is_none()
        && !local.is_mutable
        && program
            .primitive_type_reference(local.type_reference)
            .is_some()
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

/// Destinations without ordinary computation or result-operand evaluation must
/// retain this fence: the legacy value sink cannot materialize an inner callee's
/// result in the outer argument frame. Supported local computations are admitted
/// above and sequence nested calls explicitly; statement-call operands have their
/// own ordered materialization path. This check only handles the remaining local
/// and assignment destinations, not every bound nested call.
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
        || crate::machine_calls::calls::unit_statement_call_is_supported(
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

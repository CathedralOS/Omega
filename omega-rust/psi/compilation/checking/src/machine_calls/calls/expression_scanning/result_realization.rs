//! Fail-closed fences for value calls whose result cannot yet be realized.
//!
//! These checks are separate from target resolution and argument validation:
//! they reject call shapes that otherwise silently bind zero or read an
//! unmaterialized nested result at runtime.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::CollectionViewOperation;
use symbols::BuiltinFunction;
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

/// Scalar initializers retain their entire ordinary call in the computation
/// graph. Result operations and reference-result custody keep their own admission;
/// other destinations retain the fence until they have ordered evaluation.
pub(crate) fn report_nested_call_in_local_initializer(
    program: &TypedTrees,
    machine: &Machine,
    state_name: &str,
    value: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if scalar_initializer_computation_is_supported(program, machine, value)
        || result_initializer_call_is_supported(program, machine, value)
        || reference_result_nested_operand_call_is_supported(program, machine, value)
        || boundary_application_initializer(program, machine, value)
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
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return false;
    }
    // State spans are disjoint append-only ranges: a state symbol's holder
    // is unique, so a second owner cannot exist.
    let Some(owner) = program
        .machine_holding_state(call.target_symbol)
        .filter(|owner| owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody)
    else {
        return false;
    };
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
        || !nested.carries_only_positional_arguments()
        || !nested.selects_only_nominal_route()
    {
        return false;
    }
    let Some(nested_owner) = program
        .machine_holding_state(nested.target_symbol)
        .filter(|owner| owner.supply_mode == language_semantics::MachineSupplyMode::CheckedBody)
    else {
        return false;
    };
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
                .state_local_declaration(state, name.symbol)
                .is_some_and(|(ordinal, local)| {
                    ordinal < statement_index
                        && !local.is_mutable
                        && crate::machine_calls::reference_result_custody::is_reference_record(
                            program,
                            local.type_reference,
                        )
                        && program.normalized_type_identity(local.type_reference)
                            == program.normalized_type_identity(nested_parameter.type_reference)
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
    if states.next().is_some() || !program.expression_table.expression_is_valid(value) {
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
    let has_boundary_scalar_initializer = program
        .primitive_type_reference(local.type_reference)
        .is_some()
        && initializer_target_is_supported(
            program,
            machine,
            local.initial_value,
            local.type_reference,
            false,
            false,
        );
    if initializers.next().is_some()
        // A structural local belongs to the statement sequence regardless of
        // what the surrounding machine eventually returns. Scalar locals in
        // scalar-return bodies retain their whole-call computation owner.
        || (!has_ordinary_structural_initializer
            && !(unit_type(program, state.return_type)
                || ordinary_structural_result_type(program, state.return_type)))
        // Ordinary scalar helpers in state graphs keep their whole-call
        // computation owner. Bodyless boundaries instead retain their result
        // operation, independently of the surrounding state's successors.
        || (program.machine_states(machine).len() != 1
            && !has_ordinary_structural_initializer
            && !has_boundary_scalar_initializer)
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
    ) || self_place_receiver_scalar_call(
        program,
        machine,
        local.initial_value,
        local.type_reference,
    )
}

/// A scalar local bound directly to a method call on the caller's own
/// receiver or a field path beneath it (`let f: i32 = self.check.flag(ok)`)
/// is the same ordinary result operation as a free call. The Unit call lane
/// lends that place as the callee's borrowed receiver operand in one
/// operation; the whole-call computation would instead stage every operand
/// in its own block. Owned receivers, receivers reached through calls or
/// indexing, boundary callees and structural results keep their routes.
fn self_place_receiver_scalar_call(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return false;
    };
    if !call.target_symbol.is_valid()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
        || !program.expression_table.expression_is_valid(call.receiver)
        || program.primitive_type_reference(result_type).is_none()
    {
        return false;
    }
    let Some((owner, target)) =
        program
            .machine_holding_state(call.target_symbol)
            .and_then(|owner| {
                program
                    .machine_states(owner)
                    .first()
                    .filter(|entry| entry.symbol == call.target_symbol)
                    .map(|entry| (owner, entry))
            })
    else {
        return false;
    };
    let parameters = program.state_parameters(target);
    let Some(receiver) = parameters.first() else {
        return false;
    };
    if owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !owner.attached_data_symbol.is_valid()
        || !receiver.is_self
        || parameters
            .iter()
            .filter(|parameter| parameter.is_self)
            .count()
            != 1
        || !matches!(
            program
                .type_reference_table
                .type_reference(receiver.type_reference),
            typed_trees::types::TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared
                    | language_semantics::ReferenceAccess::Mutable,
                ..
            }
        )
        || program
            .primitive_type_reference(target.return_type)
            .is_none()
    {
        return false;
    }
    let mut cursor = call.receiver;
    let mut visited = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return false;
        }
        visited.push(cursor);
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                cursor = member.receiver;
            }
            ExpressionNode::Name(name) => {
                return name.symbol == machine.symbol
                    && name.head_symbol == machine.symbol
                    && matches!(program.expression_table.name_path_members(name.members),
                        [only] if only.is_self_receiver());
            }
            _ => return false,
        }
    }
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
    program
        .machine_holding_state(call.target_symbol)
        .is_some_and(|owner| {
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
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
    {
        return false;
    }
    // The target entry is unique: state spans are disjoint, so at most one
    // machine can hold `call.target_symbol` as its first state.
    let target_pair = program
        .machine_holding_state(call.target_symbol)
        .and_then(|owner| {
            program
                .machine_states(owner)
                .first()
                .filter(|entry| entry.symbol == call.target_symbol)
                .map(|entry| (owner, entry))
        });
    if let Some((owner, target)) = target_pair {
        return (program.call_has_no_runtime_receiver(call, owner, target)
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
/// parameter, or a scalar member destination: its `AssignmentValue` computation
/// root evaluates the outer call and every nested operand before the store.
/// Member stores defer storage/ABI eligibility to that checked producer instead
/// of predicting it from the source signature here. Other destinations retain
/// their existing fence until they use the same computation path.
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
                .state_local_declaration(state, path.symbol)
                .is_some_and(|(_, local)| {
                    local.is_mutable
                        && program
                            .primitive_type_reference(local.type_reference)
                            .is_some()
                        && program.primitive_type_reference(local.type_reference)
                            == ordinary_scalar_call_result_primitive(program, assignment.value)
                })
                && program
                    .primitive_type_reference(state.return_type)
                    .is_some()
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
    // A field store whose value is a tree of scalar boundary applications
    // (`self.f = a * b + c`) keeps its outer application as the statement's
    // result operation; every nested application is an ordered computation
    // operand of it. Other nested calls, and local-name destinations, keep
    // the fence until their destinations plan the same composition.
    if matches!(
        program.expression_table.expression(assignment.target),
        ExpressionNode::Member(_)
    ) && nested_boundary_applications_compose(program, assignment.value)
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

/// The call-shape gate every unit store assignment shares: the outer call is a
/// direct, ordinary checked-body call with a primitive result. The evaluator
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
    ordinary_scalar_call_result_primitive(program, assignment.value)
}

/// Mirrors the checked `AssignmentValue` computation admission for a call
/// stored through the unit statement sequence: the outer call is a direct,
/// ordinary checked-body call whose declared result
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
        .state_local_declaration(state, target_symbol)
        .is_some_and(|(_, local)| {
            local.is_mutable
                && program
                    .expression_table
                    .expression_is_valid(local.initial_value)
                && matches!(
                    program
                        .type_reference_table
                        .type_reference(local.type_reference),
                    typed_trees::types::TypeReferenceNode::Named { .. }
                )
                && program.primitive_type_reference(local.type_reference) == Some(result_primitive)
        })
}

/// A scalar member assignment and its call tree meet at the declared leaf
/// primitive, not at an enumerated destination/signature family. The checked
/// builder owns storage selection and the ordered `AssignmentValue` computation;
/// source replay independently binds that computation to this assignment and
/// its live destination. Duplicating those planners here excluded owned locals
/// even after both stages supported them.
///
/// This is not write authority or a claim that an executable plan exists.
/// Ordinary assignment/call checks still enforce access, types, and effects;
/// unavailable computations or storage must reject during product construction.
fn unit_member_scalar_store_assignment_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    assignment: &TableAssignment,
) -> bool {
    if !matches!(
        program.expression_table.expression(assignment.target),
        ExpressionNode::Member(_)
    ) {
        return false;
    }
    let Some(result_primitive) = ordinary_scalar_call_result_primitive(program, assignment.value)
    else {
        return false;
    };
    crate::value_custody::places::declared_place_type_raw(
        program,
        machine,
        Some(state),
        assignment.target,
    )
    .and_then(|reference| program.primitive_type_reference(reference))
        == Some(result_primitive)
}

/// The computation evaluator owns every ordinary scalar call, including calls
/// through borrowed receivers. This queries the exact declared result, not the
/// callee's incidental parameter mix or the receiver's access spelling. Operand
/// support and authority are independently established by computation planning
/// and source replay; a missing computation cannot supply a result.
///
/// Bodyless boundary/external calls remain result operations, so broadening this
/// query must not change `result_initializer_call_is_supported`'s ownership test.
fn ordinary_scalar_call_result_primitive(
    program: &TypedTrees,
    value: ExpressionHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    if !program.expression_table.expression_is_valid(value) {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return None;
    };
    if !call.carries_only_positional_arguments() || !call.selects_only_nominal_route() {
        return None;
    }
    let owner = program.machine_holding_state(call.target_symbol)?;
    if owner.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return None;
    }
    let entry = program.machine_states(owner).first()?;
    (entry.symbol == call.target_symbol)
        .then(|| program.primitive_type_reference(entry.return_type))
        .flatten()
}

fn scalar_initializer_computation_is_supported(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    let Some(result_primitive) = ordinary_scalar_call_result_primitive(program, value) else {
        return false;
    };
    program.machine_states(machine).iter().any(|state| {
        program.statement_table.statements(state.statement_nodes).iter().any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if local.initial_value == value
                    && program.primitive_type_reference(local.type_reference) == Some(result_primitive))
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
    if call_spelling_is_composing_builtin(call.target.as_str()) {
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

/// A scalar boundary application tree evaluates its operands in order for
/// either local destination. Immutable bindings retain a result operation;
/// mutable storage retains the whole computation before establishing its slot.
/// Mutability does not change the calls' signatures or operand evaluation.
fn boundary_application_initializer(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    nested_boundary_applications_compose(program, value)
        && program.machine_states(machine).iter().any(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(statement, StatementNode::LocalData(local)
                        if local.initial_value == value)
                })
        })
}

/// True when `value` is a bodyless scalar boundary application whose every
/// nested non-builtin call is also one. Such a tree composes as one statement
/// result operation over ordered nested application operands; a checked-body
/// or structural call anywhere inside still needs the fence.
pub fn nested_boundary_applications_compose(program: &TypedTrees, value: ExpressionHandle) -> bool {
    fn scalar_parameters(
        program: &TypedTrees,
        parameters: &[typed_trees::signature::StateParameter],
    ) -> bool {
        parameters.iter().all(|parameter| {
            !parameter.is_self
                && !parameter.is_const
                && !parameter.is_mutable
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
    }
    fn scalar_boundary_application(
        program: &TypedTrees,
        call: &typed_trees::expression::TableCallExpression,
    ) -> bool {
        // A nongeneric legacy named operator (`F64::from_i32`): its qualifier
        // is not an operand.
        if let Some(operator) = program
            .operators()
            .iter()
            .find(|operator| operator.symbol == call.target_symbol && operator.is_boundary)
        {
            return program.operator_type_parameters(operator).is_empty()
                && program
                    .primitive_type_reference(operator.return_type)
                    .is_some()
                && scalar_parameters(program, program.operator_parameters(operator));
        }
        let Some(owner) = program.machine_holding_state(call.target_symbol) else {
            return false;
        };
        let Some(entry) = program.machine_states(owner).first() else {
            return false;
        };
        owner.supply_mode.is_boundary_declaration()
            && !owner.body_is_present
            && entry.symbol == call.target_symbol
            && !call.receiver.is_valid()
            && program
                .primitive_type_reference(entry.return_type)
                .is_some()
            && scalar_parameters(program, program.state_parameters(entry))
    }
    fn composes(program: &TypedTrees, expression: ExpressionHandle) -> bool {
        if !expression.is_valid() {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Call(call) => {
                (call_spelling_is_composing_builtin(call.target.as_str())
                    || scalar_boundary_application(program, call))
                    && program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .all(|argument| composes(program, *argument))
            }
            ExpressionNode::Atomic(atomic) => composes(program, atomic.value),
            ExpressionNode::Binary(binary) => {
                composes(program, binary.left) && composes(program, binary.right)
            }
            ExpressionNode::Unary(unary) => composes(program, unary.operand),
            ExpressionNode::Cast(cast) => composes(program, cast.value),
            ExpressionNode::Borrow(inner) => composes(program, inner.target),
            ExpressionNode::Indexed(indexed) => {
                composes(program, indexed.collection) && composes(program, indexed.index)
            }
            ExpressionNode::Member(member) => composes(program, member.receiver),
            _ => true,
        }
    }
    matches!(program.expression_table.expression(value), ExpressionNode::Call(call)
        if scalar_boundary_application(program, call))
        && composes(program, value)
}

/// True when a call spelling names a builtin whose result composes inside an
/// argument position: a reserved value builtin (`min`/`max`/`sqrt`) or a
/// compiler-owned collection/text view. Both vocabularies are asked for their
/// own names -- this scan runs before checking selects either one, so the
/// spelling is all it has, and it must not carry a second copy of the lists.
fn call_spelling_is_composing_builtin(target: &str) -> bool {
    BuiltinFunction::from_name(target).is_some_and(BuiltinFunction::is_scalar_computation)
        || CollectionViewOperation::from_authored_spelling(target).is_some()
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
            if !call_spelling_is_composing_builtin(call.target.as_str()) {
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

//! Nominal and partial-affine Unit cleanup planning.
//!
//! This file retains nominal caller metadata from exact checked dispositions.
//! `nominal_requirements.rs` derives nominal cleanup requirements and their
//! diagnostics, `partial_affine_cleanup.rs` builds the partial affine
//! cleanup machine and `cleanup_evidence.rs` recognizes content evidence,
//! empty service reach and exact affine discards.
//! `actions` selects the ordered live complement and hooks from contextual
//! dispositions. `normal_edges` uses that selection at state exits;
//! `temporary_actions` completes call-continuation rosters after the executable
//! target catalog exists. Neither caller reconstructs a separate cleanup policy.

mod actions;
pub(in crate::execution::terminal_unit) use actions::checked_path_segments;
pub(super) mod anonymous;
pub(crate) use anonymous::projected_temporary_cleanup;
mod cleanup_evidence;
mod nominal_requirements;
pub(super) mod normal_edges;
mod order;
mod partial_affine_cleanup;
mod replacements;
mod residuals;
pub(super) mod temporary_actions;
pub(crate) use temporary_actions::finalize_scalar_actions;

pub(crate) use checked_trees::is_bounded_nominal_cleanup_record;
pub(crate) use cleanup_evidence::machine_has_content_evidence;
pub(crate) use nominal_requirements::{
    checked_contract_expressions, checked_requires_expressions, direct_boolean_field_requirement,
    nominal_cleanup_boolean_requirements, nominal_cleanup_caller_boolean_requirements,
    nominal_cleanup_missing_requirement, nominal_cleanup_missing_requirement_diagnostic,
};
pub(crate) use partial_affine_cleanup::partial_affine_residuals;

use super::{
    BTreeMap, CheckFacts, CheckedNominalAffineUnitCleanupMachinePlan, CheckedStructuralAccess,
    CheckedUnitEffectOperationPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, Diagnostic, MachineSupplyMode,
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, TypeReferenceNode,
};
use crate::execution::terminal_unit::calls::{
    entry_claims, free_structural_scalar_signature, structural_signature,
};
use crate::execution::terminal_unit::types::{
    ShapeCollector, is_unit, machine_binders, state_flow,
};
use checked_trees::CheckedUnitCallableEntry;

/// Construction may temporarily retain both body descriptions of one entry.
/// They must agree on the signature; ordinary overlap resolution and closure
/// pruning still choose exactly one complete body before publication.
fn callable_target<'plan>(
    entries: &[CheckedUnitCallableEntry<'plan>],
    machine: symbols::SymbolHandle,
) -> Option<CheckedUnitCallableEntry<'plan>> {
    let mut matches = entries.iter().filter(|entry| entry.machine == machine);
    let entry = *matches.next()?;
    matches
        .all(|candidate| *candidate == entry)
        .then_some(entry)
}

/// `plans` borrows the published ordinary and composed callable entries.
/// Exact hook identity and caller prerequisites are retained as metadata;
/// execution continues to use the source body's shared normal-exit owner.
pub(super) fn build_nominal_affine_unit_cleanup_machine(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    plans: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CheckedNominalAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    if !machine.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(machine).is_empty()
        || !program.machine_owned_data(machine).is_empty()
        || !program.machine_trait_conformances(machine).is_empty()
        || !machine.conformance_bounds.is_empty()
        || !program.machine_invokes(machine).is_empty()
        || machine.suspends
        || machine.blocks
        || !is_unit(program, state.return_type)
        || !program
            .statement_table
            .statements(state.statement_nodes)
            .is_empty()
    {
        return None;
    }

    let binders = machine_binders(program, machine);
    // An attached caller keeps its empty-record attachment. A free consuming
    // machine — the generic `drop<T>` specialization — carries no attachment;
    // its signature is the free structural roster instead.
    let (attachment_type_identity, structural_parameters) = if machine.attached_data.is_none() {
        let (structural, scalar) =
            free_structural_scalar_signature(program, shapes, state, &binders)?;
        if !scalar.is_empty() {
            return None;
        }
        (None, structural)
    } else {
        structural_signature(program, shapes, state, &binders)?
    };
    let source_parameters = program.state_parameters(state);
    if source_parameters.is_empty() || source_parameters.len() != structural_parameters.len() {
        return None;
    }
    for (position, (source_parameter, checked_parameter)) in source_parameters
        .iter()
        .zip(&structural_parameters)
        .enumerate()
    {
        let parameter_data = program.closed_data_definition(source_parameter.type_reference)?;
        let parameter_shape = shapes.types.get(&checked_parameter.type_identity)?;
        if (source_parameter.is_const)
            || source_parameter.is_mutable
            || usize::try_from(checked_parameter.position).ok()? != position
            || !matches!(
                checked_parameter.multiplicity,
                Multiplicity::Affine | Multiplicity::Linear
            )
            || !checked_parameter.qualifications.is_empty()
            || !program.data_type_parameters(parameter_data).is_empty()
            || !is_bounded_nominal_cleanup_record(&parameter_shape.shape)
        {
            return None;
        }
    }
    if let Some(attachment_type_identity) = &attachment_type_identity {
        let attachment_shape = shapes.types.get(attachment_type_identity)?;
        if !matches!(
            &attachment_shape.shape,
            CheckedUnitStructuralTypeShape::Record { fields, .. } if fields.is_empty()
        ) {
            return None;
        }
    }

    let entry_claims = entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        &structural_parameters,
        program.state_parameters(state),
    )?;
    let state_flow = state_flow(facts, machine.symbol, state.symbol)?;
    if facts
        .qualifications
        .for_machine(machine.symbol)
        .is_some_and(|fact| !fact.body_committed.is_empty())
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || !facts
            .flow
            .control
            .calls
            .span_or_empty(state_flow.calls)
            .is_empty()
        || !source_parameters
            .iter()
            .enumerate()
            .all(|(index, parameter)| {
                cleanup_evidence::has_exact_root_nominal_disposition(
                    facts,
                    machine,
                    state,
                    parameter,
                    structural_parameters[index].multiplicity,
                    index as u32,
                    &entry_claims,
                )
            })
    {
        return None;
    }
    // The consuming body has no authored calls, but its selected cleanup
    // actions are executable dependencies. Their reach belongs to the ordinary
    // effect graph prepared before these plans; requiring an empty service row
    // here would discard that dependency instead of checking it. The published
    // source body already retains its inferred state reach and callable contract.
    let caller_requirements = nominal_cleanup_caller_boolean_requirements(
        program,
        facts,
        machine,
        state,
        source_parameters,
    )?;
    for (source_parameter, checked_parameter) in
        source_parameters.iter().zip(&structural_parameters).rev()
    {
        let Some((cleanup_machine, cleanup_state, cleanup_receiver)) =
            nominal_cleanup_shape(program, source_parameter.type_reference)
        else {
            return None;
        };
        let cleanup_target = callable_target(plans, cleanup_machine.symbol)?;
        if !nominal_cleanup_target_matches(
            program,
            cleanup_machine,
            cleanup_state,
            &cleanup_target,
            checked_parameter.type_identity,
        ) {
            return None;
        }
        let cleanup_requirements = nominal_cleanup_boolean_requirements(
            program,
            facts,
            cleanup_machine,
            cleanup_state,
            cleanup_receiver,
        )?;
        if let Some(missing) = nominal_cleanup_missing_requirement(
            checked_parameter.position,
            &caller_requirements,
            &cleanup_requirements,
        ) {
            diagnostics.push(nominal_cleanup_missing_requirement_diagnostic(
                program,
                machine,
                state,
                source_parameter,
                cleanup_machine,
                missing,
            ));
            return None;
        }
    }

    Some(CheckedNominalAffineUnitCleanupMachinePlan {
        machine: machine.symbol,
        state: state.symbol,
        attachment_type_identity,
        caller_requirements,
    })
}

/// The nominal `::drop` machine selected for a parameter's data type and its
/// sole explicit parameter: the closed carrier owns exactly one designated
/// checked-body cleanup machine. Regions do not alter its layout or replace
/// the hook identity; type arguments must already have a closed definition.
pub(crate) fn nominal_cleanup_shape<'a>(
    program: &'a dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<(
    &'a typed_trees::machine::Machine,
    &'a typed_trees::state::State,
    &'a typed_trees::signature::StateParameter,
)> {
    let cleanup_machine =
        validation::nominal_cleanup_machine_for_type(program, type_reference, &[])?;
    if cleanup_machine.supply_mode != MachineSupplyMode::CheckedBody {
        return None;
    }
    let cleanup_state = program.machine_states(cleanup_machine).first()?;
    let [cleanup_receiver] = program.state_parameters(cleanup_state) else {
        return None;
    };
    Some((cleanup_machine, cleanup_state, cleanup_receiver))
}

/// The selected owner hook is an ordinary Unit callable, whether its body is
/// a sequence or a state graph. Source admission checks finite normal completion
/// and propagates its actual reach; retaining a borrowed subject or effectful
/// body does not create a different cleanup operation.
pub(crate) fn nominal_cleanup_target_matches(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    plan: &CheckedUnitCallableEntry<'_>,
    type_identity: typed_trees::type_identity::TypeIdentityId,
) -> bool {
    let [receiver] = program.state_parameters(state) else {
        return false;
    };
    let TypeReferenceNode::Reference { access, .. } = program
        .type_reference_table
        .type_reference(receiver.type_reference)
    else {
        return false;
    };
    access.is_readable()
        && access.is_exclusive()
        && program.parameter_declares_attached_data(machine, receiver)
        && !receiver.is_const
        && machine.lifetime_parameters.is_empty()
        && program.machine_type_parameters(machine).is_empty()
        && program.machine_owned_data(machine).is_empty()
        && program.machine_trait_conformances(machine).is_empty()
        && machine.conformance_bounds.is_empty()
        && program.machine_invokes(machine).is_empty()
        && !machine.suspends
        && !machine.blocks
        && is_unit(program, state.return_type)
        && plan.returns_unit
        && plan.machine == machine.symbol
        && plan.state == state.symbol
        && plan
            .attachment_type_identity
            .is_none_or(|identity| identity == type_identity)
        && plan.scalar_parameters.is_empty()
        && plan.structural_parameters.len() <= 1
        && plan.structural_parameters.iter().all(|parameter| {
            parameter.position == 0
                && matches!(
                    parameter.access,
                    CheckedStructuralAccess::MutableBorrow
                        | CheckedStructuralAccess::WriteOnlyBorrow
                )
                && parameter.type_identity == type_identity
        })
}

//! Nominal and partial-affine Unit cleanup planning.
//!
//! This file builds the nominal affine unit cleanup machine.
//! `nominal_requirements.rs` derives nominal cleanup requirements and their
//! diagnostics, `partial_affine_cleanup.rs` builds the partial affine
//! cleanup machine and `cleanup_evidence.rs` recognizes content evidence,
//! empty service reach and exact affine discards.

pub(super) mod anonymous;
mod cleanup_evidence;
mod nominal_requirements;
mod partial_affine_cleanup;
mod residuals;

pub(crate) use cleanup_evidence::{
    has_exact_root_affine_discard, machine_has_content_evidence, service_reach_is_empty,
    service_reach_plan_is_empty,
};
pub(crate) use nominal_requirements::{
    checked_requires_expressions, is_bounded_nominal_cleanup_record,
    nominal_cleanup_boolean_requirements, nominal_cleanup_caller_boolean_requirements,
    nominal_cleanup_missing_requirement, nominal_cleanup_missing_requirement_diagnostic,
    nominal_scalar_caller_requirements, scalar_nominal_cleanup_missing_requirement_diagnostic,
};
pub(crate) use partial_affine_cleanup::build_partial_affine_unit_cleanup_machine;

use super::{
    BTreeMap, CheckFacts, CheckedNominalAffineUnitCleanupMachinePlan, CheckedStructuralAccess,
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitNominalAffineCleanupPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, Diagnostic, MachineSupplyMode, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, StatementNode,
    TypeReferenceNode, TypedTrees,
};
use crate::execution::terminal_unit::{
    ShapeCollector, control, entry_claims, free_structural_scalar_signature, is_unit,
    machine_binders, state_flow, structural_signature,
};

/// `plans` is the checked ordinary-Unit roster this plan may join: the
/// completed `terminal_unit_effects.machines` at finalize time, or the live
/// candidate pool while closure pruning still needs the generic consuming
/// machine as a resolvable call target.
pub(super) fn build_nominal_affine_unit_cleanup_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    plans: &[CheckedUnitEffectMachinePlan],
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
        let (attachment, structural) =
            structural_signature(program, shapes, machine, state, &binders, false)?;
        (Some(attachment), structural)
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
        let TypeReferenceNode::Named {
            symbol: parameter_data_symbol,
            ..
        } = program
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return None;
        };
        let parameter_data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *parameter_data_symbol)?;
        let parameter_shape = shapes.types.get(&checked_parameter.type_identity)?;
        if source_parameter.is_self
            || source_parameter.is_const
            || source_parameter.is_mutable
            || checked_parameter.is_self
            || usize::try_from(checked_parameter.position).ok()? != position
            || checked_parameter.multiplicity != Multiplicity::Affine
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
            CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty()
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
    let contract = facts.contract_plans.for_machine(machine.symbol)?;
    if !entry_claims.is_empty()
        || facts
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
        || !service_reach_is_empty(facts, state_flow.service_reach)
        || !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(machine.symbol)?,
        )
        || !source_parameters
            .iter()
            .all(|parameter| has_exact_root_affine_discard(facts, machine, state, parameter))
    {
        return None;
    }
    let caller_requirements = nominal_cleanup_caller_boolean_requirements(
        program,
        facts,
        machine,
        state,
        source_parameters,
    )?;
    let mut cleanups = Vec::with_capacity(source_parameters.len());
    for (source_parameter, checked_parameter) in
        source_parameters.iter().zip(&structural_parameters).rev()
    {
        let TypeReferenceNode::Named {
            symbol: parameter_data_symbol,
            ..
        } = program
            .type_reference_table
            .type_reference(source_parameter.type_reference)
        else {
            return None;
        };
        let parameter_data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *parameter_data_symbol)?;
        let cleanup_machines = program
            .machines()
            .iter()
            .filter(|candidate| {
                candidate.supply_mode == MachineSupplyMode::CheckedBody
                    && candidate.name.as_str().ends_with("::drop")
                    && candidate.attached_data_symbol == parameter_data.symbol
            })
            .collect::<Vec<_>>();
        let [cleanup_machine] = cleanup_machines.as_slice() else {
            return None;
        };
        let [cleanup_state] = program.machine_states(cleanup_machine) else {
            return None;
        };
        let [cleanup_receiver] = program.state_parameters(cleanup_state) else {
            return None;
        };
        let TypeReferenceNode::Reference { access, .. } = program
            .type_reference_table
            .type_reference(cleanup_receiver.type_reference)
        else {
            return None;
        };
        if !access.is_readable() || !access.is_exclusive() {
            return None;
        }
        if !cleanup_receiver.is_self
            || cleanup_receiver.is_const
            || !cleanup_machine.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(cleanup_machine).is_empty()
            || !program.machine_owned_data(cleanup_machine).is_empty()
            || !program
                .machine_trait_conformances(cleanup_machine)
                .is_empty()
            || !cleanup_machine.conformance_bounds.is_empty()
            || !program.machine_invokes(cleanup_machine).is_empty()
            || cleanup_machine.suspends
            || cleanup_machine.blocks
            || !is_unit(program, cleanup_state.return_type)
        {
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
        let cleanup_statements = program
            .statement_table
            .statements(cleanup_state.statement_nodes);
        if cleanup_statements
            .iter()
            .any(|statement| !matches!(statement, StatementNode::Call(_)))
        {
            return None;
        }
        let cleanup_target = plans
            .iter()
            .find(|candidate| candidate.machine == cleanup_machine.symbol)?;
        let (cleanup_return, cleanup_calls) = cleanup_target.operations.split_last()?;
        let CheckedUnitEffectOperationPlan::Complete {
            statement_index,
            trivial_affine_local_discard_ordinals,
            trivial_affine_discards,
        } = cleanup_return
        else {
            return None;
        };
        if usize::try_from(*statement_index).ok()? != cleanup_calls.len()
            || cleanup_calls.len() != cleanup_statements.len()
            || !trivial_affine_local_discard_ordinals.is_empty()
            || !trivial_affine_discards.is_empty()
            || cleanup_target.attachment_type_identity.as_deref()
                != Some(checked_parameter.type_identity.as_str())
            || !cleanup_target.structural_parameters.is_empty()
            || !cleanup_target.trivial_affine_locals.is_empty()
            || !cleanup_target.entry_claims.is_empty()
            || !cleanup_target.body_qualifications.is_empty()
            || !service_reach_is_empty(facts, cleanup_target.service_reach)
            || !service_reach_plan_is_empty(facts, cleanup_target.contract_service_reach)
        {
            return None;
        }
        let mut cleanup_helpers = Vec::with_capacity(cleanup_calls.len());
        for (statement_index, operation) in cleanup_calls.iter().enumerate() {
            let CheckedUnitEffectOperationPlan::CallUnit {
                coordinate,
                target_machine,
                target_state,
                target_contract_report_fingerprint,
                service_reach,
                scalar_arguments,
                structural_arguments,
                claim_transfers,
            } = operation
            else {
                return None;
            };
            if usize::try_from(coordinate.statement_index).ok()? != statement_index
                || coordinate.call_ordinal != 0
                || *target_machine == cleanup_machine.symbol
                || cleanup_helpers
                    .iter()
                    .any(|(helper, _, _)| helper == target_machine)
                || !service_reach_is_empty(facts, *service_reach)
                || !scalar_arguments.is_empty()
                || !structural_arguments.is_empty()
                || !claim_transfers.is_empty()
            {
                return None;
            }
            cleanup_helpers.push((
                *target_machine,
                *target_state,
                *target_contract_report_fingerprint,
            ));
        }
        for (helper_machine, helper_state, helper_fingerprint) in cleanup_helpers {
            let helper = plans
                .iter()
                .find(|candidate| candidate.machine == helper_machine)?;
            let helper_shape = shapes
                .types
                .get(helper.attachment_type_identity.as_ref()?)?;
            if helper.machine == machine.symbol
                || helper.machine == cleanup_machine.symbol
                || helper.state != helper_state
                || helper.contract_report_fingerprint != helper_fingerprint
                || !matches!(&helper_shape.shape, CheckedUnitStructuralTypeShape::Record { fields } if fields.is_empty())
                || !helper.structural_parameters.is_empty()
                || !helper.trivial_affine_locals.is_empty()
                || !helper.entry_claims.is_empty()
                || !helper.body_qualifications.is_empty()
                || !service_reach_is_empty(facts, helper.service_reach)
                || !service_reach_plan_is_empty(facts, helper.contract_service_reach)
                || !matches!(helper.operations.as_slice(), [CheckedUnitEffectOperationPlan::Complete { statement_index: 0, trivial_affine_local_discard_ordinals, trivial_affine_discards }] if trivial_affine_local_discard_ordinals.is_empty() && trivial_affine_discards.is_empty())
            {
                return None;
            }
        }
        cleanups.push(CheckedUnitNominalAffineCleanupPlan {
            source_parameter_index: checked_parameter.position,
            type_identity: checked_parameter.type_identity.clone(),
            cleanup_machine: cleanup_machine.symbol,
            cleanup_state: cleanup_state.symbol,
            cleanup_contract_report_fingerprint: cleanup_target.contract_report_fingerprint,
            requirements: cleanup_requirements,
        });
    }

    Some(CheckedNominalAffineUnitCleanupMachinePlan {
        machine: CheckedUnitEffectMachinePlan {
            scalar_result: None,
            scalar_control: None,
            structural_result: None,
            machine: machine.symbol,
            state: state.symbol,
            attachment_type_identity,
            structural_parameters,
            scalar_parameters: Vec::new(),
            provider_attachment_requirements: Vec::new(),
            trivial_affine_locals: Vec::new(),
            entry_claims: Vec::new(),
            body_qualifications: Vec::new(),
            contract_report_fingerprint: contract.report_fingerprint,
            contract_commitment: contract.commitment,
            contract_service_reach: facts.service_reaches.plan_for_machine(machine.symbol)?,
            service_reach: state_flow.service_reach,
            operations: vec![CheckedUnitEffectOperationPlan::Complete {
                statement_index: 0,
                trivial_affine_local_discard_ordinals: Vec::new(),
                trivial_affine_discards: Vec::new(),
            }],
        },
        caller_requirements,
        cleanups,
    })
}

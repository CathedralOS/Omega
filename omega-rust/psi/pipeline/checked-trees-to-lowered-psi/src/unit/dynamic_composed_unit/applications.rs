//! Lowering the initial rebound and exact conformance applications with
//! their contracts and service summaries.

use crate::unit::dynamic_composed_unit::dynamic_lanes::LoweredDynamicRealization;
use crate::unit::{
    CheckedTrees, LoweringError, PrimitiveType, checked_unit_target_reach_matches,
    collect_service_summary, evidence_lowering, lower_installation_machine_service_ceiling,
    unsupported,
};
use checked_trees::{CheckedDynamicScalarCallPlan, CheckedDynamicSelectionPlan};
use language_semantics::ServiceReachSummary;
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceCallableResult,
    ClosedConformanceRealizationCallable, ClosedConformanceRow,
    closed_conformance_application_commitment, closed_conformance_application_report_fingerprint,
};

pub(crate) fn lower_initial_rebound_application(
    checked: &CheckedTrees,
    target_trait_symbol: symbols::SymbolHandle,
    initial: &CheckedDynamicSelectionPlan,
    owner: semantic_vocabulary::MachineId,
) -> Result<ClosedConformanceApplication, LoweringError> {
    let conformance_symbol = initial.fact.conformance.ok_or(LoweringError::Unsupported(
        "initial dynamic selection has no named conformance",
    ))?;
    let conformances = checked
        .typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == conformance_symbol)
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return unsupported("initial dynamic selection lost its exact conformance declaration");
    };
    let traits = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == target_trait_symbol)
        .collect::<Vec<_>>();
    let [target_trait] = traits.as_slice() else {
        return unsupported("initial dynamic selection lost its exact target trait");
    };
    if initial.fact.target_trait != target_trait_symbol
        || conformance.carrier_symbol != initial.fact.source_data
        || conformance.trait_symbol != initial.fact.target_trait
        || !conformance.lifetime_parameters.is_empty()
        || !checked
            .typed
            .conformance_type_parameters(conformance)
            .is_empty()
        || !checked
            .typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .is_empty()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !target_trait.lifetime_parameters.is_empty()
        || !checked.typed.trait_type_parameters(target_trait).is_empty()
    {
        return unsupported("generic initial dynamic conformance requires a later producer");
    }
    let closed_rows =
        checked
            .typed
            .closed_conformance_rows(conformance)
            .ok_or(LoweringError::Unsupported(
                "initial dynamic selection is not a closed conformance",
            ))?;
    if closed_rows.len() != initial.fact.rows.len() {
        return unsupported("initial dynamic selection row map is incomplete");
    }
    let rows = closed_rows
        .iter()
        .zip(&initial.fact.rows)
        .map(|(closed, retained)| {
            let requirement_identity = evidence_lowering::checked_evidence_requirement_identity(
                checked,
                closed.declaring_trait,
                closed.requirement,
            )?;
            let realization_identity = evidence_lowering::checked_evidence_machine_identity(
                checked,
                closed.realization_machine,
            )?;
            let family_tuple = evidence_lowering::checked_requirement_family_tuple(
                checked,
                closed.declaring_trait,
                closed.requirement,
            )?;
            if closed.declaring_trait != retained.declaring_trait
                || closed.requirement != retained.requirement
                || closed.realization_machine != retained.realization_machine
                || closed.realization_state != retained.realization_state
                || requirement_identity != retained.requirement_identity
                || realization_identity != retained.realization_identity
            {
                return unsupported("initial dynamic selection row map drifted from checking");
            }
            Ok(ClosedConformanceRow {
                declaring_trait_identity: checked
                    .symbols
                    .display_path(closed.declaring_trait, "::"),
                public_requirement_identity: requirement_identity,
                family_tuple,
                requirement_identity: checked.symbols.display_path(closed.requirement, "::"),
                realization_identity: checked.symbols.display_path(closed.realization_state, "::"),
                realization_callable_identity: None,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut application = ClosedConformanceApplication {
        owner,
        declaration_identity: checked.symbols.display_path(conformance_symbol, "::"),
        telescope: Vec::new(),
        subject_identity: Some(initial.type_identity.clone()),
        trait_identity: checked.symbols.display_path(target_trait_symbol, "::"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables: Vec::new(),
        rows,
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    application.report_fingerprint =
        closed_conformance_application_report_fingerprint(&application);
    application.commitment = closed_conformance_application_commitment(&application);
    Ok(application)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_exact_application(
    checked: &CheckedTrees,
    plan: &CheckedDynamicScalarCallPlan,
    owner: semantic_vocabulary::MachineId,
    lowered_realizations: &[LoweredDynamicRealization],
) -> Result<(ClosedConformanceApplication, ClosedConformanceRow), LoweringError> {
    let conformances = checked
        .typed
        .conformances()
        .iter()
        .filter(|conformance| conformance.symbol == plan.selected_conformance)
        .collect::<Vec<_>>();
    let [conformance] = conformances.as_slice() else {
        return unsupported("direct dynamic selection lost its exact conformance declaration");
    };
    let traits = checked
        .typed
        .traits()
        .iter()
        .filter(|definition| definition.symbol == plan.target_trait)
        .collect::<Vec<_>>();
    let [target_trait] = traits.as_slice() else {
        return unsupported("direct dynamic selection lost its exact target trait");
    };
    if conformance.carrier_symbol != plan.selection.source_data
        || conformance.trait_symbol != plan.target_trait
        || !conformance.lifetime_parameters.is_empty()
        || !checked
            .typed
            .conformance_type_parameters(conformance)
            .is_empty()
        || !checked
            .typed
            .type_reference_table
            .type_reference_handles(conformance.arguments)
            .is_empty()
        || !conformance.trait_lifetime_arguments.is_empty()
        || !target_trait.lifetime_parameters.is_empty()
        || !checked.typed.trait_type_parameters(target_trait).is_empty()
    {
        return unsupported("generic dynamic conformance applications require a later producer");
    }
    let closed_rows =
        checked
            .typed
            .closed_conformance_rows(conformance)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic selection is not a closed conformance",
            ))?;
    if closed_rows.len() != plan.selection.rows.len() {
        return unsupported("direct dynamic selection row map is incomplete");
    }

    let mut rows = Vec::with_capacity(closed_rows.len());
    let mut selected_row = None;
    for (closed, retained) in closed_rows.iter().zip(&plan.selection.rows) {
        let requirement_identity = evidence_lowering::checked_evidence_requirement_identity(
            checked,
            closed.declaring_trait,
            closed.requirement,
        )?;
        let realization_identity = evidence_lowering::checked_evidence_machine_identity(
            checked,
            closed.realization_machine,
        )?;
        let family_tuple = evidence_lowering::checked_requirement_family_tuple(
            checked,
            closed.declaring_trait,
            closed.requirement,
        )?;
        if closed.declaring_trait != retained.declaring_trait
            || closed.requirement != retained.requirement
            || closed.realization_machine != retained.realization_machine
            || closed.realization_state != retained.realization_state
            || requirement_identity != retained.requirement_identity
            || realization_identity != retained.realization_identity
        {
            return unsupported("direct dynamic selection row map drifted from checking");
        }
        let selected = closed.declaring_trait == plan.declaring_trait
            && closed.requirement == plan.requirement
            && closed.realization_machine == plan.realization_machine
            && closed.realization_state == plan.realization_state;
        let matching_realizations = lowered_realizations
            .iter()
            .filter(|candidate| {
                candidate.source_machine == closed.realization_machine
                    && candidate.source_state == closed.realization_state
                    && candidate.callable_identity == realization_identity
            })
            .collect::<Vec<_>>();
        let matching_realization = match matching_realizations.as_slice() {
            [] if !selected => None,
            [matching] => Some(*matching),
            _ => return unsupported("dynamic conformance row callable is absent or ambiguous"),
        };
        let row = ClosedConformanceRow {
            declaring_trait_identity: checked.symbols.display_path(closed.declaring_trait, "::"),
            public_requirement_identity: requirement_identity,
            family_tuple,
            requirement_identity: checked.symbols.display_path(closed.requirement, "::"),
            realization_identity: checked.symbols.display_path(closed.realization_state, "::"),
            realization_callable_identity: matching_realization
                .map(|matching| matching.callable_identity.clone()),
        };
        if selected && selected_row.replace(row.clone()).is_some() {
            return unsupported("direct dynamic selected row is duplicated");
        }
        rows.push(row);
    }
    let selected_row = selected_row.ok_or(LoweringError::Unsupported(
        "direct dynamic selected row is absent",
    ))?;
    if selected_row.public_requirement_identity != plan.requirement_identity {
        return unsupported("direct dynamic public requirement identity drifted");
    }

    let mut realization_callables = lowered_realizations
        .iter()
        .map(|callable| ClosedConformanceRealizationCallable {
            source_callable_identity: callable.callable_identity.clone(),
            machine: callable.machine,
            result: callable.result,
        })
        .collect::<Vec<_>>();
    realization_callables.sort();
    realization_callables.dedup();
    if realization_callables.len() != lowered_realizations.len() {
        return unsupported("dynamic realization callable registry is not one-to-one");
    }
    let mut application = ClosedConformanceApplication {
        owner,
        declaration_identity: checked
            .symbols
            .display_path(plan.selected_conformance, "::"),
        telescope: Vec::new(),
        subject_identity: Some(plan.source_type_identity.clone()),
        trait_identity: checked.symbols.display_path(plan.target_trait, "::"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        realization_callables,
        rows,
        report_fingerprint: 0,
        commitment: Default::default(),
    };
    application.report_fingerprint =
        closed_conformance_application_report_fingerprint(&application);
    application.commitment = closed_conformance_application_commitment(&application);
    Ok((application, selected_row))
}

pub(crate) fn validate_empty_contract(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    report_fingerprint: u64,
    commitment: checked_trees::MachineContractCommitment,
) -> Result<(), LoweringError> {
    let contract =
        checked
            .facts
            .contract_plans
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic machine has no exact checked contract",
            ))?;
    if report_fingerprint == 0
        || commitment.is_zero()
        || contract.report_fingerprint != report_fingerprint
        || contract.commitment != commitment
        || !contract.closed_scalar_values.requires().is_empty()
        || !contract.closed_scalar_values.ensures().is_empty()
        || contract.closed_scalar_values.has_crash_clauses()
        || contract.closed_scalar_values.has_outcome_specific_clauses()
        || !contract.crash.published().is_empty()
    {
        return unsupported("direct dynamic machine requires an unsupported contract lane");
    }
    Ok(())
}

pub(crate) fn validate_empty_service_summary(
    checked: &CheckedTrees,
    summary: ServiceReachSummary,
) -> Result<(), LoweringError> {
    let mut services = Vec::new();
    collect_service_summary(&checked.facts.service_reaches.rows, summary, &mut services)?;
    if !services.is_empty() {
        return unsupported("direct dynamic scalar call with service reach is unsupported");
    }
    Ok(())
}

pub(crate) fn exact_machine_service_summary(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<ServiceReachSummary, LoweringError> {
    let fact =
        checked
            .facts
            .service_reaches
            .for_machine(machine)
            .ok_or(LoweringError::Unsupported(
                "direct dynamic machine has no checked service reach",
            ))?;
    Ok(ServiceReachSummary {
        direct: fact.inferred_direct,
        transitive: fact.inferred_transitive,
    })
}

pub(crate) fn exact_empty_machine_service_ceiling(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    call: ServiceReachSummary,
) -> Result<Vec<semantic_vocabulary::ServiceId>, LoweringError> {
    let summary = exact_machine_service_summary(checked, machine)?;
    validate_empty_service_summary(checked, summary)?;
    let contract = checked
        .facts
        .service_reaches
        .plan_for_machine(machine)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic realization has no service contract",
        ))?;
    if !checked_unit_target_reach_matches(call, contract) {
        return unsupported("direct dynamic call reach drifted from its selected realization");
    }
    lower_installation_machine_service_ceiling(checked, machine, contract, summary, &[])
}

pub(crate) fn terminal_callable_result(
    primitive: PrimitiveType,
) -> Result<ClosedConformanceCallableResult, LoweringError> {
    match primitive {
        PrimitiveType::Bool => Ok(ClosedConformanceCallableResult::Bool),
        PrimitiveType::I32 => Ok(ClosedConformanceCallableResult::I32),
        _ => unsupported("direct dynamic Terminal custody currently admits only Bool and i32"),
    }
}

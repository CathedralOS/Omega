use super::behavior::{
    project_capability_flow, project_crash, project_installation_reaches, project_mutation,
    project_service_row, project_synchronous_invocations, project_termination,
};
use super::contracts::checked::facts::project_callable_contracts;
use super::contracts::checked::operations::canonical_checked_invocation_targets;
use super::providers::callable_conformances::project_callable_conformances;
use super::semantics::conformances::project_conformance_bounds;
use super::semantics::facts::exactly_one;
use super::semantics::signatures::parameters::project_type_parameters;
use super::semantics::types::review_signature_type_identity_with_binders;
use crate::evidence::package::ProjectedReviewRow;
use crate::evidence::{
    CheckedPackageCallableReview, PackageReviewCallableParameter, PackageReviewCallableRole,
    PackageReviewCallableSupply, PackageReviewCheckedServiceReach,
    PackageReviewExternalExecutableSupply, PackageReviewNominalIdentity,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;

pub(crate) fn project_callable(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
) -> Result<
    (
        CheckedPackageCallableReview,
        Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
    ),
    Vec<Diagnostic>,
> {
    let subject = identity.path.as_str();
    let Some(entry) = compilation.machine_states(machine).first() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no canonical entry signature"
        ))]);
    };
    let machine_type_parameters = compilation.machine_type_parameters(machine);
    let (binders, type_parameters) = project_type_parameters(
        compilation,
        machine_type_parameters,
        "callable",
        subject,
        &machine.lifetime_parameters,
    )?;
    let conformance_bounds = project_conformance_bounds(
        compilation,
        &machine.conformance_bounds,
        machine_type_parameters,
        &binders,
        &machine.lifetime_parameters,
        "reviewed callable",
        subject,
    )?;
    let parameters = compilation
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            Ok(PackageReviewCallableParameter {
                name: parameter.name.as_str().to_owned(),
                type_identity: review_signature_type_identity_with_binders(
                    compilation,
                    parameter.type_reference,
                    &binders,
                    &machine.lifetime_parameters,
                )?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let return_type = review_signature_type_identity_with_binders(
        compilation,
        entry.return_type,
        &binders,
        &machine.lifetime_parameters,
    )?;
    let (conformances, operator_realizations, external_executable_supply) =
        project_callable_conformances(compilation, machine, &identity, &binders, true)?;
    let contracts = project_callable_contracts(compilation, machine, entry, &binders)?;
    let service_reach = exactly_one(
        compilation
            .facts
            .service_reaches
            .machines()
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        subject,
        "service-reach",
    )?;
    let realized = exactly_one(
        compilation
            .facts
            .contract_plans
            .realized_envelopes
            .iter()
            .filter(|envelope| envelope.machine == machine.symbol),
        subject,
        "realized contract envelope",
    )?;
    let checked_invocation = exactly_one(
        compilation
            .facts
            .synchronous_invocations
            .machines
            .iter()
            .filter(|fact| fact.machine == machine.symbol),
        subject,
        "synchronous-invocation",
    )?;
    let canonical_published =
        canonical_checked_invocation_targets(compilation, &checked_invocation.published_targets)?;
    let canonical_checked_inferred = canonical_checked_invocation_targets(
        compilation,
        &checked_invocation.checked_inferred_targets,
    )?;
    if checked_invocation.plan.published != canonical_published
        || checked_invocation.plan.checked_inferred != canonical_checked_inferred
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has contradictory exact and rendered synchronous-invocation facts"
        ))]);
    }
    let suspension = compilation
        .facts
        .suspensions
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no exact suspension fact"
            ))]
        })?;
    let blocking = compilation
        .facts
        .blocking
        .for_machine(machine.symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no exact blocking fact"
            ))]
        })?;

    let declared_service_reach = match service_reach.interface {
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(row) => {
            Some(project_service_row(compilation, row)?)
        }
        psi_language_semantics::ServiceReachInterface::InternalInferred
            if role == PackageReviewCallableRole::Build =>
        {
            None
        }
        psi_language_semantics::ServiceReachInterface::InternalInferred => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no published service-reach ceiling"
            ))]);
        }
    };
    if role != PackageReviewCallableRole::Build
        && matches!(
            suspension.interface,
            psi_language_semantics::SuspensionInterface::InternalInferred
        )
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no published suspension ceiling"
        ))]);
    }
    if role != PackageReviewCallableRole::Build
        && matches!(
            blocking.interface,
            psi_language_semantics::BlockingInterface::InternalInferred
        )
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no published blocking ceiling"
        ))]);
    }
    if suspension.checked_may_suspend != realized.checked_may_suspend
        || blocking.checked_may_block != realized.checked_may_block
        || checked_invocation.plan.checked_inferred != realized.effective_synchronous_invocations
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` operational facts do not equal its exact realized contract envelope"
        ))]);
    }
    match machine.supply_mode {
        MachineSupplyMode::CheckedBody if !machine.body_is_present => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` is classified as checked supply but has no retained body"
            ))]);
        }
        MachineSupplyMode::Accepted
        | MachineSupplyMode::Requirement
        | MachineSupplyMode::ExternalRealization { .. }
            if machine.body_is_present =>
        {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has bodyless supply but retains a body"
            ))]);
        }
        MachineSupplyMode::CheckedBody
        | MachineSupplyMode::Boundary
        | MachineSupplyMode::Accepted
        | MachineSupplyMode::Requirement
        | MachineSupplyMode::ExternalRealization { .. } => {}
    }
    let has_checked_body = machine.body_is_present
        && matches!(
            machine.supply_mode,
            MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
        );
    let checked_service_reach = if has_checked_body {
        let realized = project_service_row(compilation, service_reach.inferred_transitive)?;
        let concrete = project_service_row(compilation, service_reach.concrete_transitive)?;
        PackageReviewCheckedServiceReach::CheckedBody { realized, concrete }
    } else {
        PackageReviewCheckedServiceReach::NoCheckedBody
    };
    let declared_synchronous_invocations = match checked_invocation.plan.interface {
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling => Some(
            project_synchronous_invocations(compilation, &checked_invocation.published_targets)?,
        ),
        psi_language_semantics::SynchronousInvocationInterface::InternalInferred
            if role == PackageReviewCallableRole::Build =>
        {
            None
        }
        psi_language_semantics::SynchronousInvocationInterface::InternalInferred => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed callable `{subject}` has no published synchronous-invocation ceiling"
            ))]);
        }
    };
    let realized_synchronous_invocations =
        project_synchronous_invocations(compilation, &checked_invocation.checked_inferred_targets)?;
    let mut capability_flows = realized
        .capabilities
        .iter()
        .map(|flow| project_capability_flow(compilation, flow))
        .collect::<Result<Vec<_>, _>>()?;
    capability_flows.sort_by(|left, right| {
        left.capability
            .cmp(&right.capability)
            .then(left.kind.as_str().cmp(right.kind.as_str()))
            .then(left.state.cmp(&right.state))
            .then(left.statement_index.cmp(&right.statement_index))
            .then(left.call_ordinal.cmp(&right.call_ordinal))
            .then(left.via_state.cmp(&right.via_state))
    });

    let supply = match machine.supply_mode {
        MachineSupplyMode::CheckedBody => PackageReviewCallableSupply::CheckedBody,
        MachineSupplyMode::Requirement => PackageReviewCallableSupply::Requirement,
        MachineSupplyMode::Boundary => PackageReviewCallableSupply::Boundary,
        MachineSupplyMode::Accepted => PackageReviewCallableSupply::Accepted,
        MachineSupplyMode::ExternalRealization { .. } => {
            PackageReviewCallableSupply::ExternalRealization
        }
    };

    Ok((
        CheckedPackageCallableReview {
            role,
            identity,
            supply,
            lifetime_parameter_count: machine.lifetime_parameters.len(),
            type_parameters,
            conformance_bounds,
            parameters,
            return_type,
            conformances,
            operator_realizations,
            contracts,
            declared_service_reach,
            checked_service_reach,
            unresolved_installation_reaches: project_installation_reaches(
                compilation,
                &service_reach.unresolved_installation_reaches,
            )?,
            declared_synchronous_invocations,
            realized_synchronous_invocations,
            capability_flows,
            checked_may_suspend: realized.checked_may_suspend,
            checked_may_block: realized.checked_may_block,
            checked_termination: project_termination(compilation, &realized.checked_termination)?,
            checked_crash: project_crash(compilation, &realized.checked_crash)?,
            mutation: project_mutation(compilation, &realized.mutation)?,
        },
        external_executable_supply,
    ))
}

pub(crate) fn project_private_external_executable_supply(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    identity: &PackageReviewNominalIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>, Vec<Diagnostic>> {
    let machine_type_parameters = compilation.machine_type_parameters(machine);
    let (binders, _) = project_type_parameters(
        compilation,
        machine_type_parameters,
        "external executable supply",
        identity.path.as_str(),
        &machine.lifetime_parameters,
    )?;
    let (_, _, supply) =
        project_callable_conformances(compilation, machine, identity, &binders, false)?;
    Ok(supply)
}

//! Shared exact callable associations, before evidence or policy behavior projection.

use super::super::behavior::{
    canonical_checked_invocation_targets, project_installation_reaches, project_service_row,
    project_synchronous_invocations,
};
use super::super::contracts::facts::{
    project_callable_contracts, project_callable_contracts_with_exposure,
};
use super::super::semantics::conformances::project_conformance_bounds;
use super::super::semantics::facts::exactly_one;
use super::super::semantics::signatures::parameters::project_type_parameters;
use super::super::semantics::types::review_signature_type_identity_with_binders;
use super::conformances::project_callable_conformances;
use super::signatures::project_external_callable_signature;
use crate::capture::source::ProjectedReviewRow;
use crate::record::*;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;
use psi_symbols::SymbolHandle;

pub(super) struct CallableSurface {
    pub(super) identity: PackageReviewNominalIdentity,
    pub(super) supply: PackageReviewCallableSupply,
    pub(super) lifetime_parameter_count: usize,
    pub(super) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(super) policy_type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(super) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(super) parameters: Vec<PackageReviewCallableParameter>,
    pub(super) return_type: PackageReviewTypeIdentity,
    pub(super) conformances: Vec<PackageReviewCallableConformance>,
    pub(super) policy_conformances: Vec<PackagePolicyCallableConformance>,
    pub(super) operator_realizations: Vec<PackageReviewOperatorRealization>,
    pub(super) contracts: Vec<PackageReviewCallableContract>,
    /// `Some` preserves a published ceiling, including an explicitly empty
    /// one. `None` is retained for the current ordinary build-machine form;
    /// admission must not silently reinterpret it as a public empty promise.
    pub(super) declared_service_reach: Option<Vec<PackageReviewNominalIdentity>>,
    pub(super) checked_service_reach: PackageReviewCheckedServiceReach,
    pub(super) unresolved_installation_reaches: Vec<PackageReviewInstallationReach>,
    /// `Some` preserves a published direct synchronous-invocation ceiling,
    /// including an explicitly empty one. Targets retain parameter ordinals
    /// or package-qualified service identities, never display strings.
    pub(super) declared_synchronous_invocations: Option<Vec<PackageReviewSynchronousInvocation>>,
    pub(super) realized_synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    /// Exact checked operational summary. Published callable surfaces expose
    /// their authored may-ceiling; the build-machine lane may remain inferred.
    pub(super) checked_may_suspend: bool,
    pub(super) checked_may_block: bool,
    pub(super) declared_may_suspend: Option<bool>,
    pub(super) declared_may_block: Option<bool>,
}

pub(super) struct ProjectedSurface<'a> {
    pub surface: CallableSurface,
    pub external_executable_supply: Vec<ProjectedReviewRow<PackageReviewExternalExecutableSupply>>,
    pub entry: &'a psi_typed_trees::state::State,
    pub realized: &'a psi_checked_trees::RealizedMachineContractEnvelope,
    pub binders: Vec<(SymbolHandle, String)>,
}

pub(super) fn project<'a>(
    compilation: &'a CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    role: PackageReviewCallableRole,
    identity: PackageReviewNominalIdentity,
    policy_mode: bool,
) -> Result<ProjectedSurface<'a>, Vec<Diagnostic>> {
    let subject = identity.path.as_str();
    let Some(entry) = compilation.machine_states(machine).first() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no canonical entry signature"
        ))]);
    };
    let machine_type_parameters = compilation.machine_type_parameters(machine);
    if policy_mode {
        crate::capture::source::service_reach::validate_machine_service_reach(
            compilation,
            machine,
        )?;
        crate::capture::source::invocations::validate_machine_invocations(compilation, machine)?;
        crate::capture::source::suspension::validate_machine_operational(compilation, machine)?;
    }
    // Match the typed authored-selection owner: accepted claims use the
    // boundary interface even when their source name is not public.
    let publishes_interface = machine.is_public || machine.supply_mode.is_boundary_declaration();
    let (binders, type_parameters, policy_type_parameters) = if policy_mode {
        let (binders, parameters) = super::policy_parameters::type_parameters(
            compilation,
            machine,
            subject,
            publishes_interface,
        )?;
        (binders, Vec::new(), parameters)
    } else {
        let (binders, parameters) = project_type_parameters(
            compilation,
            machine_type_parameters,
            "callable",
            subject,
            &machine.lifetime_parameters,
        )?;
        (binders, parameters, Vec::new())
    };
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
    let external_signature = (!policy_mode
        && matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization { .. }
        ))
    .then(|| project_external_callable_signature(compilation, machine, &binders))
    .transpose()?;
    let mut policy_conformances = Vec::new();
    let (conformances, operator_realizations, external_executable_supply) = if policy_mode
        && matches!(
            machine.supply_mode,
            MachineSupplyMode::ExternalRealization { .. }
        ) {
        let supplies = super::project_checked_external_supply_policy(compilation, machine.symbol)?;
        let mut operators = Vec::new();
        for supply in supplies {
            match supply.requirement {
                PackagePolicyExternalRequirement::Trait(conformance) => {
                    policy_conformances.push(conformance);
                }
                PackagePolicyExternalRequirement::Operator { coordinate, alias } => {
                    operators.push(PackageReviewOperatorRealization { coordinate, alias });
                }
                PackagePolicyExternalRequirement::TopLevelRequirement { .. } => {}
            }
        }
        (Vec::new(), operators, Vec::new())
    } else {
        project_callable_conformances(
            compilation,
            machine,
            &identity,
            &binders,
            external_signature.as_ref(),
            true,
            policy_mode.then_some(&mut policy_conformances),
        )?
    };
    let contracts = if policy_mode && !publishes_interface {
        project_callable_contracts_with_exposure(compilation, machine, entry, &binders,
        psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
    )?
    } else {
        project_callable_contracts(compilation, machine, entry, &binders)?
    };
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
        MachineSupplyMode::AdmissionClaim
        | MachineSupplyMode::TopLevelRequirement
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
        | MachineSupplyMode::AdmissionClaim
        | MachineSupplyMode::TopLevelRequirement
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
    let unresolved_installation_reaches =
        project_installation_reaches(compilation, &service_reach.unresolved_installation_reaches)?;
    if role == PackageReviewCallableRole::Public && !unresolved_installation_reaches.is_empty() {
        let requirements = unresolved_installation_reaches
            .iter()
            .map(|reach| format!("`{}`", reach.requirement().path()))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(vec![Diagnostic::error(format!(
            "ordinary public callable `{subject}` cannot export unresolved installation-reach requirement(s) {requirements}; bind each provider before crossing the package boundary"
        ))]);
    }
    let supply = match machine.supply_mode {
        MachineSupplyMode::CheckedBody => PackageReviewCallableSupply::CheckedBody,
        MachineSupplyMode::Requirement => PackageReviewCallableSupply::Requirement,
        MachineSupplyMode::TopLevelRequirement => PackageReviewCallableSupply::TopLevelRequirement,
        MachineSupplyMode::Boundary => PackageReviewCallableSupply::Boundary,
        MachineSupplyMode::AdmissionClaim => PackageReviewCallableSupply::AdmissionClaim,
        MachineSupplyMode::ExternalRealization { .. } => {
            PackageReviewCallableSupply::ExternalRealization
        }
    };

    Ok(ProjectedSurface {
        surface: CallableSurface {
            identity,
            supply,
            lifetime_parameter_count: machine.lifetime_parameters.len(),
            type_parameters,
            policy_type_parameters,
            conformance_bounds,
            parameters,
            return_type,
            conformances,
            policy_conformances,
            operator_realizations,
            contracts,
            declared_service_reach,
            checked_service_reach,
            unresolved_installation_reaches,
            declared_synchronous_invocations,
            realized_synchronous_invocations,
            checked_may_suspend: realized.checked_may_suspend,
            checked_may_block: realized.checked_may_block,
            declared_may_suspend: match suspension.interface {
                psi_language_semantics::SuspensionInterface::PublishedMaySuspend(ceiling) => {
                    Some(ceiling)
                }
                psi_language_semantics::SuspensionInterface::InternalInferred => None,
            },
            declared_may_block: match blocking.interface {
                psi_language_semantics::BlockingInterface::PublishedMayBlock(ceiling) => {
                    Some(ceiling)
                }
                psi_language_semantics::BlockingInterface::InternalInferred => None,
            },
        },
        external_executable_supply,
        binders,
        entry,
        realized,
    })
}

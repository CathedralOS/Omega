//! Complete calling-plan scalar debts without reopening Source.

use semantic_vocabulary::ScalarApplicationId;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_installation::{
    AdmittedProviderSelections, AdmittedScalarApplications, CallingAbiStorageDemand,
    ScalarAddressModel, ScalarApplicationClosure, ScalarApplicationObservations,
};

/// Invoking a policy owes its checked entry. Describing a default ABI does not
/// invoke the boundary and cannot certify its ordinary body or Product formation.
pub(super) enum CallingScalarScope<'request> {
    Invocation {
        declaration: terminal_psi::ProofDeclarationId,
        applications: &'request [ScalarApplicationId],
    },
    AbiStorage(&'request CallingAbiStorageDemand),
}

/// Both purposes use the same dependency-ready scalar evaluator and supplied
/// provider custody. Neither can install host imports or inherit Product grants.
pub(super) fn complete_calling_scalar_inputs(
    artifact: &CanonicalTerminalArtifact,
    primary: target::TargetProfile,
    request: CallingScalarScope<'_>,
    providers: &AdmittedProviderSelections,
    meter: &mut TerminalFuelMeter,
) -> Result<AdmittedScalarApplications, String> {
    let (mut closure, requested) = match &request {
        CallingScalarScope::Invocation {
            declaration,
            applications,
        } => (
            ScalarApplicationClosure::for_invocation(
                artifact.view(),
                primary.target_name(),
                &[*declaration],
            ),
            *applications,
        ),
        CallingScalarScope::AbiStorage(demand) => (
            ScalarApplicationClosure::for_abi_storage(artifact.view(), demand)?,
            demand.applications(),
        ),
    };
    let scopes = closure.scopes().map(str::to_owned).collect::<Vec<_>>();
    for scope in scopes {
        let profile = target::TargetProfile::from_canonical_target_name(&scope)
            .map_err(|error| error.message)?;
        let selected =
            terminal_installation::admit_target_machine_selections(artifact.view(), &scope);
        let facts = profile
            .layout_target_facts()
            .ok_or("calling scalar target has no immutable layout catalogue")?;
        let observations =
            ScalarApplicationObservations::new(artifact.view(), &selected, providers, &facts)?
                .with_target_carriers(
                    profile
                        .native_realization()
                        .map(runtime_abi::target_carrier_layout),
                );
        let mut observations = match &request {
            CallingScalarScope::Invocation { declaration, .. } => {
                observations.with_invocation_roots(&[*declaration])?
            }
            CallingScalarScope::AbiStorage(demand) => observations.with_abi_storage(demand)?,
        };
        let demand = if scope == primary.target_name() {
            requested
        } else {
            &[]
        };
        let conditions = build_time_evaluation::evaluate_scalar_applications(
            &mut observations,
            primary.target_name(),
            demand,
            meter,
        )
        .map_err(|error| format!("calling scalar conditions: {error}"))?;
        let address_model =
            target::TargetSemantics::close_address_model(profile).map(|model| ScalarAddressModel {
                target: scope,
                carrier: model.carrier(),
                semantics_version: model.semantics_version(),
                commitment: model.commitment(),
            });
        closure.close_scope(observations, conditions, Ok(address_model))?;
    }
    closure.finish()
}

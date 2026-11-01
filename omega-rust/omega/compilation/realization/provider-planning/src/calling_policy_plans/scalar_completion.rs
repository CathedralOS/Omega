//! Complete retained declaration or invocation debts without source.

use semantic_vocabulary::ScalarApplicationId;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::TerminalFuelMeter;
use terminal_installation::{
    AdmittedProviderSelections, AdmittedScalarApplications, ScalarAddressModel,
    ScalarApplicationClosure, ScalarApplicationObservations,
};

enum ScalarCompletionScope {
    DeclarationFormation,
    Invocation(terminal_psi::ProofDeclarationId),
}

/// Default ABI geometry owes the Product's retained declaration conditions,
/// even when its declaration has no calling-policy entry to invoke.
pub(super) fn complete_declaration_scalar_inputs(
    artifact: &CanonicalTerminalArtifact,
    target: target::TargetProfile,
    requested: &[ScalarApplicationId],
    providers: &AdmittedProviderSelections,
    meter: &mut TerminalFuelMeter,
) -> Result<AdmittedScalarApplications, String> {
    complete_scalar_inputs(
        artifact,
        target,
        requested,
        providers,
        meter,
        ScalarCompletionScope::DeclarationFormation,
    )
}

/// All work uses the invocation's existing meter. Neither a selected entry nor
/// a target layout supplies the retained predicates that must be re-decided.
/// Policies have the exact empty provider selection used by their activation;
/// completion cannot install a host import or borrow another Product's grants.
pub(super) fn complete_policy_scalar_inputs(
    artifact: &CanonicalTerminalArtifact,
    primary: target::TargetProfile,
    entry: terminal_psi::TargetMachineCallable,
    requested: &[ScalarApplicationId],
    providers: &AdmittedProviderSelections,
    meter: &mut TerminalFuelMeter,
) -> Result<AdmittedScalarApplications, String> {
    let root = artifact
        .view()
        .module()
        .scalar_applications
        .proof_scope
        .callable_declaration(entry)
        .ok_or("calling policy entry has no exact checked proof declaration")?;
    complete_scalar_inputs(
        artifact,
        primary,
        requested,
        providers,
        meter,
        ScalarCompletionScope::Invocation(root),
    )
}

/// Scope selection governs both coverage and observations. An empty invocation
/// cannot stand in for declaration formation or discharge its retained debt.
fn complete_scalar_inputs(
    artifact: &CanonicalTerminalArtifact,
    primary: target::TargetProfile,
    requested: &[ScalarApplicationId],
    providers: &AdmittedProviderSelections,
    meter: &mut TerminalFuelMeter,
    completion_scope: ScalarCompletionScope,
) -> Result<AdmittedScalarApplications, String> {
    let (mut closure, invocation_roots, context) = match completion_scope {
        ScalarCompletionScope::DeclarationFormation => (
            ScalarApplicationClosure::new(artifact.view(), primary.target_name()),
            None,
            "calling declaration",
        ),
        ScalarCompletionScope::Invocation(root) => (
            ScalarApplicationClosure::for_invocation(
                artifact.view(),
                primary.target_name(),
                &[root],
            ),
            Some([root]),
            "calling policy",
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
            .ok_or_else(|| format!("{context} scalar target has no immutable layout catalogue"))?;
        let mut observations =
            ScalarApplicationObservations::new(artifact.view(), &selected, providers, &facts)?
                .with_target_carriers(
                    profile
                        .native_realization()
                        .map(runtime_abi::target_carrier_layout),
                );
        if let Some(roots) = &invocation_roots {
            observations = observations.with_invocation_roots(roots)?;
        }
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
        .map_err(|error| format!("{context} scalar conditions: {error}"))?;
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

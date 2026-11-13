/*
We prepare source calling requests and activate retained Terminal calling-policy
machines. The source vocabulary lives in `source/library/std/calling.omg`, while
this subsystem acts as the compiler-owned decoder that keeps that open policy
surface behind the closed normalized-plan validator.

Boundary signatures associate geometry with Source review coordinates, and
`source_preparation` captures original request/review correspondence and target
helper candidates once beside the shared Terminal publication. `plan_computation`
publishes per-target plans from that owned preparation and canonical storage
without reopening Source, while `selected_providers` applies the canonical
invocation-scoped provider choice and `calling_program` owns finite invocation
accounting and carries Build evaluation's two executors
(`CallingPolicyEvaluator`).

We also fold the source-free policy activation request (`policy_invocation`) and
dependency-ready calling-plan scalar completion (`scalar_completion`) directly
into this module entrypoint. Receiving admission precedes the compiler-evaluation
floor, and both policy invocation and default ABI description use the same
dependency-ready scalar evaluator and supplied provider custody without
reopening Source or inheriting Product grants. Scalar completion decides which
target scopes and applications to close; the evaluator that runs each scope's
conditions is Build evaluation's, supplied by the caller.
*/

mod boundary_signatures;
mod build_time_decoding;
mod callback_bindings;
mod callback_layout_catalog;
mod calling_program;
mod opaque_representations;
mod plan_computation;
mod policy_signature;
mod selected_application;
mod terminal_signature;

pub use boundary_signatures::{
    BoundaryCallbackBinder, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, BoundaryValueField, BoundaryValueShape, MaterializedBoundarySignature,
};
pub(crate) use boundary_signatures::{BoundaryDirectCallbackParameter, BoundaryNativeParameter};
pub use build_time_decoding::decode_boundary_plan_result;
pub use callback_bindings::BoundaryCallingPlanRealization;
pub use callback_layout_catalog::{
    BoundaryCallbackLayoutEntry, CallbackFieldReviewSource, CallbackLayoutReviewSource,
    CallbackRecordReviewSource, CallbackSlotReviewSource,
};
pub use calling_program::CallingProgram;
pub use opaque_representations::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
    BoundaryOpaqueRepresentationUse,
};
pub(crate) use plan_computation::evaluate_default_calling_applications;
pub use policy_signature::CallingPolicySignature;
pub use policy_signature::{encode_calling_policy_signature, validate_calling_policy_result};
pub use scalar_completion::{CallingScalarScope, complete_calling_scalar_inputs};
pub use selected_application::component_calling_applications;
pub(crate) use selected_application::selected_calling_applications;
pub use terminal_signature::{CanonicalCallingSignature, CanonicalOpaqueUse};

use calling_conventions::ValidatedBoundaryEntryPlan;
use evaluation_accounting::EvaluationInvocationAccount;
use psi_work::WorkUsage;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::TargetMachineCallable;

pub type TerminalCallingPolicyAdmission = terminal_installation::AdmittedCompilerEvaluation;

pub fn admit_terminal_calling_policy(
    artifact: &CanonicalTerminalArtifact,
    declaration: terminal_psi::ProofDeclarationId,
    target: target::TargetProfile,
    providers: &[backend_plan::TerminalTargetProviderPlan],
    requester: semantic_vocabulary::compiler_evaluation::DeclarationCustody,
    authority: Option<&dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority>,
) -> Result<TerminalCallingPolicyAdmission, String> {
    let providers = backend_plan::TerminalTargetProviderSelection::from_plans(
        providers.to_vec(),
        artifact.view(),
    )?
    .admit(artifact.view(), target.target_name())?;
    terminal_installation::admit_compiler_evaluation(
        artifact.view(),
        target.target_name(),
        declaration,
        providers,
        requester,
        authority,
    )
}

pub struct TerminalCallingPolicyInvocation<'input> {
    pub artifact: &'input CanonicalTerminalArtifact,
    pub entry: TargetMachineCallable,
    pub target: target::TargetProfile,
    pub signature: &'input CallingPolicySignature,
    pub account: EvaluationInvocationAccount,
    pub admission: TerminalCallingPolicyAdmission,
}

pub type CallingPolicyExecutor = fn(
    TerminalCallingPolicyInvocation<'_>,
) -> Result<(ValidatedBoundaryEntryPlan, WorkUsage), String>;

/// Runs one target scope's retained scalar conditions: the observations to
/// evaluate under, the scope whose values are wanted, the applications
/// requested there, the scope's address model, and the work meter to charge.
pub type CallingScalarEvaluator =
    for<'artifact> fn(
        &mut terminal_installation::ScalarApplicationObservations<'artifact>,
        &str,
        &[semantic_vocabulary::ScalarApplicationId],
        Option<&terminal_installation::ScalarAddressModel>,
        &mut psi_work::WorkMeter,
    )
        -> Result<terminal_installation::ScalarConditionSelection<'artifact>, String>;

/// Build evaluation's executors, supplied by the compiler coordinator.
///
/// Provider planning prepares calling-policy invocations and decides which
/// scalar applications a calling signature needs, but it does not run
/// build-time evaluation itself. Both runs go through these two functions:
/// `policy` executes a retained calling-policy machine, and
/// `scalar_conditions` evaluates the scalar conditions of one target scope.
#[derive(Debug, Clone, Copy)]
pub struct CallingPolicyEvaluator {
    pub policy: CallingPolicyExecutor,
    pub scalar_conditions: CallingScalarEvaluator,
}

pub(super) mod scalar_completion {
    use psi_work::WorkMeter;
    use semantic_vocabulary::ScalarApplicationId;
    use terminal_codec::CanonicalTerminalArtifact;
    use terminal_installation::{
        AdmittedProviderSelections, AdmittedScalarApplications, CallingAbiStorageDemand,
        ScalarAddressModel, ScalarApplicationClosure, ScalarApplicationObservations,
    };

    pub enum CallingScalarScope<'request> {
        Invocation {
            declaration: terminal_psi::ProofDeclarationId,
            applications: &'request [ScalarApplicationId],
        },
        AbiStorage(&'request CallingAbiStorageDemand),
    }

    pub fn complete_calling_scalar_inputs(
        artifact: &CanonicalTerminalArtifact,
        primary: target::TargetProfile,
        request: CallingScalarScope<'_>,
        providers: &AdmittedProviderSelections,
        evaluate: super::CallingScalarEvaluator,
        meter: &mut WorkMeter,
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
            let address_model =
                target::TargetSemantics::close_address_model(profile).map(|model| {
                    ScalarAddressModel {
                        target: scope.clone(),
                        carrier: model.carrier(),
                        semantics_version: model.semantics_version(),
                        commitment: model.commitment(),
                    }
                });
            let conditions = evaluate(
                &mut observations,
                primary.target_name(),
                demand,
                address_model.as_ref(),
                meter,
            )
            .map_err(|error| format!("calling scalar conditions: {error}"))?;
            closure.close_scope(observations, conditions, Ok(address_model))?;
        }
        closure.finish()
    }
}

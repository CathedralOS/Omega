// Canonical calling signatures, validated ABI plans and actual Terminal policy
// execution/storage readers. The old source request/review preparation and its
// checked-owner correspondence have been removed; these data contracts do not
// supply a replacement source-plan producer.

mod boundary_signatures;
mod build_time_decoding;
mod callback_layouts;
mod calling_program;
mod execution;
mod opaque_representations;
mod plan_computation;
mod policy_signature;
mod selected_application;
mod terminal_signature;
mod values;

pub use boundary_signatures::{
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, BoundaryValueClass,
    BoundaryValueField, BoundaryValueShape,
};
pub use build_time_decoding::decode_boundary_plan_result;
pub use calling_program::CallingProgram;
pub use opaque_representations::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
};
pub use policy_signature::CallingPolicySignature;
pub use policy_signature::{encode_calling_policy_signature, validate_calling_policy_result};
pub use scalar_completion::{CallingScalarScope, complete_calling_scalar_inputs};
pub(crate) use selected_application::{
    NativeCallingResults, component_calling_application, native_calling_results,
};
pub use terminal_signature::{CanonicalCallingSignature, CanonicalOpaqueUse};

use evaluation_accounting::EvaluationInvocationAccount;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi::TargetMachineCallable;

pub type TerminalCallingPolicyAdmission = evaluation_admission::AdmittedCompilerEvaluation;

pub struct TerminalCallingPolicyInvocation<'input> {
    pub artifact: &'input CanonicalTerminalArtifact,
    /// The artifact's callable projection for the admission's providers,
    /// shared by every policy evaluated under them.
    pub analysis: &'input terminal_interpreter::TerminalCallableAnalysis<'input, 'input>,
    pub entry: TargetMachineCallable,
    pub target: target::TargetProfile,
    pub signature: &'input CallingPolicySignature,
    pub account: EvaluationInvocationAccount,
    pub admission: TerminalCallingPolicyAdmission,
}

pub(super) mod scalar_completion {
    use psi_provider_selections::AdmittedProviderSelections;
    use psi_scalar_applications::CallingAbiStorageDemand;
    use semantic_vocabulary::ScalarAddressModel;
    use semantic_vocabulary::ScalarApplicationId;
    use terminal_codec::CanonicalTerminalArtifact;
    use psi_scalar_applications::{
        AdmittedScalarApplications, ScalarApplicationClosure, ScalarApplicationObservations,
    };
    use evaluation_work::EvaluationWorkMeter;

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
        meter: &mut EvaluationWorkMeter,
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
        let scopes = closure
            .scopes()
            .map(|scope| {
                scope
                    .map(str::to_owned)
                    .ok_or("calling scalar scope has no deployment selection")
            })
            .collect::<Result<Vec<_>, _>>()?;
        for scope in scopes {
            let profile = target::TargetProfile::from_canonical_target_name(&scope)
                .map_err(|error| error.message)?;
            let selected = psi_target_layouts::terminal_target_context(artifact.view(), &scope);
            let facts = profile
                .layout_target_facts()
                .ok_or("calling scalar target has no immutable layout catalogue")?;
            let observations =
                ScalarApplicationObservations::new(artifact.view(), &selected, providers, &facts)?
                    .with_target_carriers(
                        profile
                            .native_realization()
                            .map(runtime_abi::target_carrier_layout),
                    )?;
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
            let conditions = build_scalar_evaluation::evaluate_scalar_applications(
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

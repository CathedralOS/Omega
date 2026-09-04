use crate::realization::diagnostics::realization_error;
use crate::realization::model::{
    NativeRealizationCoreRequest, NativeRealizationInput, PostTerminalOptimizationContinuation,
};
use psi_diagnostics::Diagnostic;

/// Reusable target-neutral lowering of one exact canonical Terminal artifact.
///
/// Construction binds the full artifact identity, exact proof-admission
/// profile, and exact post-Terminal optimization selection. Target selection,
/// provider settlement, authority policy, callbacks, FMA admission, and every
/// physical lowering input remain in each request's source-free core.
#[derive(Debug, Clone)]
pub struct PreparedNativeRealizationInput {
    terminal_artifact_identity: psi_terminal_codec::TerminalArtifactIdentity,
    profile: psi_proof_admission::AdmissionProfile,
    optimization_selection: omega_optimization_core::OptimizationSelectionIdentity,
    input: NativeRealizationInput,
}

impl PreparedNativeRealizationInput {
    pub const fn terminal_artifact_identity(&self) -> psi_terminal_codec::TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub fn admission_profile(&self) -> &psi_proof_admission::AdmissionProfile {
        &self.profile
    }

    pub fn is_optimized(&self) -> bool {
        self.input.optimization_continuation().selected().is_some()
    }

    pub fn matches(
        &self,
        terminal_artifact_identity: psi_terminal_codec::TerminalArtifactIdentity,
        profile: &psi_proof_admission::AdmissionProfile,
        optimization_selections: &omega_optimization_core::PostTerminalOptimizationSelections,
    ) -> bool {
        self.terminal_artifact_identity == terminal_artifact_identity
            && self.profile == *profile
            && self.optimization_selection == optimization_selections.identity()
    }

    fn reopen(
        &self,
        artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
        request: &NativeRealizationCoreRequest<'_>,
    ) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
        if !self.matches(
            artifact.manifest().identity(),
            request.profile,
            request.optimization_selections,
        ) {
            return Err(realization_error(
                "prepared native input",
                "Terminal artifact identity, proof-admission profile, or exact optimization selection changed",
            ));
        }
        Ok(self.input.clone())
    }
}

/// Decode, verify, and lower one canonical Terminal artifact into the reusable
/// target-neutral native input frontier.
pub fn prepare_native_realization_input(
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::PostTerminalOptimizationSelections,
) -> Result<PreparedNativeRealizationInput, Vec<Diagnostic>> {
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let input = lower_realization_input(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        profile,
        optimization_selections,
    )?;
    Ok(PreparedNativeRealizationInput {
        terminal_artifact_identity: artifact.manifest().identity(),
        profile: profile.clone(),
        optimization_selection: optimization_selections.identity(),
        input,
    })
}

pub(crate) fn lower_realization_input(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::PostTerminalOptimizationSelections,
) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
    let native = omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
        semantic_bytes,
        proof_bytes,
        profile,
    )
    .map_err(|error| realization_error("native artifact lowering", error))?;
    let optimization_input =
        omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
            semantic_bytes,
            proof_bytes,
            profile,
        )
        .map_err(|error| realization_error("verified optimizer artifact lowering", error))?;
    let optimization_continuation = if optimization_selections.is_empty() {
        PostTerminalOptimizationContinuation::Identity(optimization_input)
    } else {
        PostTerminalOptimizationContinuation::Selected(optimization_input)
    };
    NativeRealizationInput::new(native, optimization_continuation)
        .map_err(|error| realization_error("native abstract-stage join", error))
}

pub(crate) fn reopen_prepared_native_realization_input(
    prepared: &PreparedNativeRealizationInput,
    artifact: &psi_terminal_codec::CanonicalTerminalArtifact,
    request: &NativeRealizationCoreRequest<'_>,
) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
    prepared.reopen(artifact, request)
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::{AdmissionSiteId, EvidenceIdentity, ProfileDecisionId};
    use psi_proof_admission::{AdmissionAcceptance, AdmissionProfile};

    fn artifact_fixture() -> psi_terminal_codec::CanonicalTerminalArtifact {
        let checked = crate::tests::fixtures::checked_source::checked(
            "data Main {} machine Main::launch() {}",
        );
        let produced = psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked,
            "Main::launch",
        )
        .expect("produce Terminal fixture");
        produced.into_parts().0
    }

    fn alternate_artifact_fixture() -> psi_terminal_codec::CanonicalTerminalArtifact {
        let checked = crate::tests::fixtures::checked_source::checked(
            r#"
                data Empty {}
                data Root {}
                machine Root::cleanup_prefix() {
                    let mut values: [Empty; 3];
                    values[0] = Empty {};
                    values[1] = Empty {};
                }
            "#,
        );
        psi_checked_trees_to_terminal::produce_terminal_artifact(&checked, "Root::cleanup_prefix")
            .expect("produce alternate Terminal fixture")
    }

    fn nonempty_profile() -> AdmissionProfile {
        AdmissionProfile::from_acceptances([AdmissionAcceptance {
            site: AdmissionSiteId::new(1).expect("site"),
            evidence_identity: EvidenceIdentity::new(2).expect("evidence"),
            profile_decision: ProfileDecisionId::new(3).expect("profile decision"),
        }])
    }

    #[test]
    fn prepared_input_is_bound_to_artifact_profile_and_entrance() {
        let artifact = artifact_fixture();
        let profile = AdmissionProfile::default();
        let empty = omega_optimization_core::PostTerminalOptimizationSelections::default();
        let prepared = prepare_native_realization_input(&artifact, &profile, &empty)
            .expect("prepare target-neutral input");
        assert!(matches!(
            prepared.input.optimization_continuation(),
            PostTerminalOptimizationContinuation::Identity(_)
        ));
        assert!(prepared.matches(artifact.manifest().identity(), &profile, &empty));
        assert!(!prepared.matches(
            alternate_artifact_fixture().manifest().identity(),
            &profile,
            &empty,
        ));
        assert!(!prepared.matches(artifact.manifest().identity(), &nonempty_profile(), &empty,));
        let selected = omega_optimization_core::PostTerminalOptimizationSelections::new(
            omega_optimization_core::OptimizationSelections::new([
                omega_optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .expect("one optimization"),
        )
        .expect("one post-Terminal optimization");
        assert!(!prepared.matches(artifact.manifest().identity(), &profile, &selected,));
    }

    #[test]
    fn post_terminal_selection_type_rejects_preterminal_reselection() {
        let selections = omega_optimization_core::OptimizationSelections::new([
            omega_optimization_core::Optimization::ControlFlowCleanup,
        ])
        .expect("one optimization");
        let error = omega_optimization_core::PostTerminalOptimizationSelections::new(selections)
            .expect_err("a resumed lowerer cannot represent a Psi optimization");
        assert_eq!(
            error.0,
            omega_optimization_core::Optimization::ControlFlowCleanup
        );
    }

    #[test]
    fn prepared_input_retains_native_authority_and_exact_physical_selection() {
        let artifact = artifact_fixture();
        let profile = AdmissionProfile::default();
        let selected = omega_optimization_core::PostTerminalOptimizationSelections::new(
            omega_optimization_core::OptimizationSelections::new([
                omega_optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .expect("one physical optimization"),
        )
        .expect("one post-Terminal optimization");
        let substituted = omega_optimization_core::PostTerminalOptimizationSelections::new(
            omega_optimization_core::OptimizationSelections::new([
                omega_optimization_core::Optimization::SelectedIncomingU12ExactSubtractImmediate,
            ])
            .expect("a different physical optimization"),
        )
        .expect("a different post-Terminal optimization");
        let prepared = prepare_native_realization_input(&artifact, &profile, &selected)
            .expect("prepare the unconditional native stage plus selected physical context");

        assert!(matches!(
            prepared.input.native(),
            omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(_)
        ));
        assert!(matches!(
            prepared.input.optimization_continuation(),
            PostTerminalOptimizationContinuation::Selected(_)
        ));
        assert!(prepared.matches(artifact.manifest().identity(), &profile, &selected));
        assert!(!prepared.matches(artifact.manifest().identity(), &profile, &substituted,));
    }
}

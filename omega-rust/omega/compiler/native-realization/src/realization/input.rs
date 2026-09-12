use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationInput, NativeRealizationRequest};
use diagnostics::Diagnostic;

/// Reusable target-neutral lowering of one exact canonical Terminal artifact.
///
/// Construction binds the full artifact identity, exact proof-admission
/// profile, and exact post-Terminal optimization selection. Target selection,
/// provider settlement, authority policy, callbacks, FMA admission, and every
/// physical lowering input remain in each realization request.
#[derive(Debug, Clone)]
pub struct PreparedNativeRealizationInput {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    profile: proof_admission::AdmissionProfile,
    optimization_selections: optimization_core::PostTerminalOptimizationSelections,
    input: NativeRealizationInput,
}

impl PreparedNativeRealizationInput {
    pub const fn terminal_artifact_identity(&self) -> terminal_codec::TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub fn admission_profile(&self) -> &proof_admission::AdmissionProfile {
        &self.profile
    }

    pub fn is_optimized(&self) -> bool {
        !self.optimization_selections.is_empty()
    }

    pub fn matches(
        &self,
        terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
        profile: &proof_admission::AdmissionProfile,
        optimization_selections: &optimization_core::PostTerminalOptimizationSelections,
    ) -> bool {
        self.terminal_artifact_identity == terminal_artifact_identity
            && self.profile == *profile
            && self.optimization_selections == *optimization_selections
    }

    pub(crate) fn reopen(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        request: &NativeRealizationRequest<'_>,
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
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    profile: &proof_admission::AdmissionProfile,
    optimization_selections: &optimization_core::PostTerminalOptimizationSelections,
) -> Result<PreparedNativeRealizationInput, Vec<Diagnostic>> {
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let input =
        lower_realization_input(artifact.semantic_bytes(), artifact.proof_bytes(), profile)?;
    Ok(PreparedNativeRealizationInput {
        terminal_artifact_identity: artifact.manifest().identity(),
        profile: profile.clone(),
        optimization_selections: optimization_selections.clone(),
        input,
    })
}

pub(crate) fn lower_realization_input(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
    terminal_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
        semantic_bytes,
        proof_bytes,
        profile,
    )
    .map_err(|error| realization_error("native artifact lowering", error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::{AdmissionAcceptance, AdmissionProfile};
    use semantic_vocabulary::{AdmissionSiteId, EvidenceIdentity, ProfileDecisionId};

    fn artifact_fixture() -> terminal_codec::CanonicalTerminalArtifact {
        let checked = crate::tests::fixtures::checked_source::checked(
            "data Main {} machine Main::launch() {}",
        );
        let produced =
            terminal_production::TerminalProductionRequest::new(&checked, "Main::launch")
                .produce_checked_artifact()
                .expect("produce Terminal fixture");
        produced.into_parts().0
    }

    fn alternate_artifact_fixture() -> terminal_codec::CanonicalTerminalArtifact {
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
        terminal_production::TerminalProductionRequest::new(&checked, "Root::cleanup_prefix")
            .produce_artifact()
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
        let empty = optimization_core::PostTerminalOptimizationSelections::default();
        let prepared = prepare_native_realization_input(&artifact, &profile, &empty)
            .expect("prepare target-neutral input");
        assert!(!prepared.is_optimized());
        assert!(prepared.matches(artifact.manifest().identity(), &profile, &empty));
        assert!(!prepared.matches(
            alternate_artifact_fixture().manifest().identity(),
            &profile,
            &empty,
        ));
        assert!(!prepared.matches(artifact.manifest().identity(), &nonempty_profile(), &empty,));
        let selected = optimization_core::PostTerminalOptimizationSelections::new(
            optimization_core::OptimizationSelections::new([
                optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .expect("one optimization"),
        )
        .expect("one post-Terminal optimization");
        assert!(!prepared.matches(artifact.manifest().identity(), &profile, &selected,));
    }

    #[test]
    fn native_input_retains_one_canonical_program_and_its_proof_context() {
        let profile = AdmissionProfile::default();
        for artifact in [artifact_fixture(), alternate_artifact_fixture()] {
            let input = lower_realization_input(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
            )
            .expect("native input");
            let expected = terminal_psi_to_abstract_operations::lower_artifact_sections(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
            )
            .expect("independent ordinary lowering");
            assert_eq!(input.plan(), &expected);
            let input = input.into_optimization_input();
            assert_eq!(input.plan(), &expected);
            assert_eq!(
                input.context().module(),
                &terminal_codec::decode_module(artifact.semantic_bytes()).unwrap()
            );
            assert_eq!(
                input.context().proof_bundle(),
                &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap()
            );
        }
    }

    #[test]
    fn post_terminal_selection_type_rejects_preterminal_reselection() {
        let selections = optimization_core::OptimizationSelections::new([
            optimization_core::Optimization::ControlFlowCleanup,
        ])
        .expect("one optimization");
        let error = optimization_core::PostTerminalOptimizationSelections::new(selections)
            .expect_err("a resumed lowerer cannot represent a Psi optimization");
        assert_eq!(error.0, optimization_core::Optimization::ControlFlowCleanup);
    }

    #[test]
    fn native_input_rejects_malformed_semantic_or_proof_sections() {
        let artifact = artifact_fixture();
        let profile = AdmissionProfile::default();
        assert!(lower_realization_input(&[], artifact.proof_bytes(), &profile).is_err());
        assert!(lower_realization_input(artifact.semantic_bytes(), &[], &profile).is_err());
    }

    #[test]
    fn empty_and_selected_preparation_retain_the_same_current_program_and_authority() {
        let artifact = artifact_fixture();
        let profile = AdmissionProfile::default();
        let empty = optimization_core::PostTerminalOptimizationSelections::default();
        let selected = optimization_core::PostTerminalOptimizationSelections::new(
            optimization_core::OptimizationSelections::new([
                optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .unwrap(),
        )
        .unwrap();
        let ordinary = prepare_native_realization_input(&artifact, &profile, &empty).unwrap();
        let selected_input =
            prepare_native_realization_input(&artifact, &profile, &selected).unwrap();
        assert_eq!(ordinary.input.plan(), selected_input.input.plan());
        assert!(!ordinary.is_optimized());
        assert!(selected_input.is_optimized());
    }

    #[test]
    fn prepared_input_retains_native_authority_and_exact_physical_selection() {
        let artifact = artifact_fixture();
        let profile = AdmissionProfile::default();
        let selected = optimization_core::PostTerminalOptimizationSelections::new(
            optimization_core::OptimizationSelections::new([
                optimization_core::Optimization::SelectedIncomingU12ExactAddImmediate,
            ])
            .expect("one physical optimization"),
        )
        .expect("one post-Terminal optimization");
        let substituted = optimization_core::PostTerminalOptimizationSelections::new(
            optimization_core::OptimizationSelections::new([
                optimization_core::Optimization::SelectedIncomingU12ExactSubtractImmediate,
            ])
            .expect("a different physical optimization"),
        )
        .expect("a different post-Terminal optimization");
        let prepared = prepare_native_realization_input(&artifact, &profile, &selected)
            .expect("prepare the unconditional native stage plus selected physical context");

        assert!(prepared.is_optimized());
        assert!(prepared.matches(artifact.manifest().identity(), &profile, &selected));
        assert!(!prepared.matches(artifact.manifest().identity(), &profile, &substituted,));
    }
}

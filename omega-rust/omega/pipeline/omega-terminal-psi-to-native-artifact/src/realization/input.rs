use crate::realization::diagnostics::realization_error;
use crate::realization::model::{NativeRealizationCoreRequest, NativeRealizationInput};
use psi_diagnostics::Diagnostic;

/// Reusable target-neutral lowering of one exact canonical Terminal artifact.
///
/// Construction binds the full artifact identity, exact proof-admission
/// profile, and the optimized/unoptimized entrance. Target selection,
/// provider settlement, authority policy, callbacks, FMA admission, and every
/// physical lowering input remain in each request's source-free core.
#[derive(Debug, Clone)]
pub struct PreparedNativeRealizationInput {
    terminal_artifact_identity: psi_terminal_codec::TerminalArtifactIdentity,
    profile: psi_proof_admission::AdmissionProfile,
    mode: NativeRealizationPreparationMode,
    input: NativeRealizationInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeRealizationPreparationMode {
    Unoptimized,
    ExplicitOptimization,
}

impl NativeRealizationPreparationMode {
    fn from_selections(selections: &omega_optimization_core::OptimizationSelections) -> Self {
        if selections.is_empty() {
            Self::Unoptimized
        } else {
            Self::ExplicitOptimization
        }
    }
}

impl PreparedNativeRealizationInput {
    pub const fn terminal_artifact_identity(&self) -> psi_terminal_codec::TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub fn admission_profile(&self) -> &psi_proof_admission::AdmissionProfile {
        &self.profile
    }

    pub fn is_optimized(&self) -> bool {
        self.mode == NativeRealizationPreparationMode::ExplicitOptimization
    }

    pub fn matches(
        &self,
        terminal_artifact_identity: psi_terminal_codec::TerminalArtifactIdentity,
        profile: &psi_proof_admission::AdmissionProfile,
        optimization_selections: &omega_optimization_core::OptimizationSelections,
    ) -> bool {
        self.terminal_artifact_identity == terminal_artifact_identity
            && self.profile == *profile
            && self.mode
                == NativeRealizationPreparationMode::from_selections(optimization_selections)
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
                "Terminal artifact identity, proof-admission profile, or optimization entrance changed",
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
    optimization_selections: &omega_optimization_core::OptimizationSelections,
) -> Result<PreparedNativeRealizationInput, Vec<Diagnostic>> {
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let mode = NativeRealizationPreparationMode::from_selections(optimization_selections);
    let input = lower_realization_input(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        profile,
        optimization_selections,
    )?;
    Ok(PreparedNativeRealizationInput {
        terminal_artifact_identity: artifact.manifest().identity(),
        profile: profile.clone(),
        mode,
        input,
    })
}

fn reject_pre_terminal_selections(
    optimization_selections: &omega_optimization_core::OptimizationSelections,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(selection) = optimization_selections.as_slice().iter().find(|selection| {
        matches!(
            selection.execution_phase(),
            omega_optimization_core::OptimizationExecutionPhase::CheckedTrees
                | omega_optimization_core::OptimizationExecutionPhase::Psi
        )
    }) {
        return Err(realization_error(
            "sealed Terminal optimization custody",
            format!(
                "pre-Terminal optimization `{}` cannot be selected by a Terminal-to-native lowerer; consume the sealed artifact's optimization execution record",
                selection.build_case_name()
            ),
        ));
    }
    Ok(())
}

pub(crate) fn lower_realization_input(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &psi_proof_admission::AdmissionProfile,
    optimization_selections: &omega_optimization_core::OptimizationSelections,
) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
    reject_pre_terminal_selections(optimization_selections)?;
    if optimization_selections.is_empty() {
        Ok(NativeRealizationInput::Unoptimized(
            omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
                semantic_bytes,
                proof_bytes,
                profile,
            )
            .map_err(|error| realization_error("native artifact lowering", error))?,
        ))
    } else {
        Ok(NativeRealizationInput::ExplicitOptimization(
            omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                semantic_bytes,
                proof_bytes,
                profile,
            )
            .map_err(|error| realization_error("verified optimizer artifact lowering", error))?,
        ))
    }
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
        let empty = omega_optimization_core::OptimizationSelections::default();
        let prepared = prepare_native_realization_input(&artifact, &profile, &empty)
            .expect("prepare target-neutral input");
        assert!(prepared.matches(artifact.manifest().identity(), &profile, &empty));
        assert!(!prepared.matches(
            alternate_artifact_fixture().manifest().identity(),
            &profile,
            &empty,
        ));
        assert!(!prepared.matches(artifact.manifest().identity(), &nonempty_profile(), &empty,));
        let selected = omega_optimization_core::OptimizationSelections::new([
            omega_optimization_core::Optimization::ControlFlowCleanup,
        ])
        .expect("one optimization");
        assert!(!prepared.matches(artifact.manifest().identity(), &profile, &selected,));
    }

    #[test]
    fn sealed_terminal_artifact_rejects_preterminal_reselection() {
        let artifact = artifact_fixture();
        let selections = omega_optimization_core::OptimizationSelections::new([
            omega_optimization_core::Optimization::ControlFlowCleanup,
        ])
        .expect("one optimization");
        let diagnostics =
            prepare_native_realization_input(&artifact, &AdmissionProfile::default(), &selections)
                .expect_err("a resumed lowerer must not rerun a Psi optimization");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("pre-Terminal optimization"));
        assert!(diagnostics[0].message.contains("ControlFlowCleanup"));
    }
}

//! Prepared native compilation and exact Terminal-input reuse identity.

use super::{admission, realization};
use crate::CompileReport;
use crate::compiler::optimization::rollback;
use crate::compiler::request::ValidatedTargetCompilation;
use diagnostics::Diagnostic;

#[derive(Clone, PartialEq, Eq)]
pub(in crate::compiler) struct NativeInputReuseKey {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    admission_profile: proof_admission::AdmissionProfile,
    post_terminal_optimizations: optimization_core::PostTerminalOptimizationSelections,
}

pub(in crate::compiler) struct PreparedNativeCompilation {
    pub(super) request: ValidatedTargetCompilation,
    pub(super) checked: crate::pipeline::CheckedCompilation,
    pub(super) admission: admission::NativeCompilationAdmission,
    pub(super) rollback: rollback::OptimizationRollbackSettlement,
    pub(super) terminal: realization::PreparedTerminalNativeArtifact,
    pub(super) production_subject: Option<crate::ProductionCompilationSubject>,
    pub(super) source_file_count: usize,
}

impl PreparedNativeCompilation {
    pub(in crate::compiler::native) fn new(
        request: ValidatedTargetCompilation,
        checked: crate::pipeline::CheckedCompilation,
        admission: admission::NativeCompilationAdmission,
        rollback: rollback::OptimizationRollbackSettlement,
        terminal: realization::PreparedTerminalNativeArtifact,
        production_subject: Option<crate::ProductionCompilationSubject>,
        source_file_count: usize,
    ) -> Self {
        Self {
            request,
            checked,
            admission,
            rollback,
            terminal,
            production_subject,
            source_file_count,
        }
    }

    pub(in crate::compiler) fn reuse_key(&self) -> NativeInputReuseKey {
        let post_terminal = self.rollback.effective().project_post_terminal();
        NativeInputReuseKey {
            terminal_artifact_identity: self.terminal.artifact().manifest().identity(),
            admission_profile: self
                .request
                .configuration
                .terminal_admission_profile
                .clone(),
            post_terminal_optimizations: post_terminal.selections().clone(),
        }
    }
}

impl PreparedNativeCompilation {
    pub(in crate::compiler) fn prepare_reusable_input(
        &self,
    ) -> Result<::native_realization::PreparedNativeRealizationInput, Vec<Diagnostic>> {
        let post_terminal = self.rollback.effective().project_post_terminal();
        ::native_realization::prepare_native_realization_input(
            self.terminal.artifact(),
            &self.request.configuration.terminal_admission_profile,
            post_terminal.selections(),
        )
    }

    pub(in crate::compiler) fn finish(
        self,
        prepared_input: &::native_realization::PreparedNativeRealizationInput,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let Self {
            request,
            checked,
            admission,
            rollback,
            terminal,
            production_subject,
            source_file_count,
        } = self;
        let options = request.options;
        let crate::TargetCompileConfiguration {
            terminal_admission_profile,
            terminal_authority_permission_policy,
            ..
        } = request.configuration;
        let post_terminal = rollback.effective().project_post_terminal();
        let artifact = realization::realize(
            &checked,
            &admission,
            &terminal_admission_profile,
            terminal_authority_permission_policy,
            post_terminal.selections(),
            terminal,
            prepared_input,
        )?;
        let report = CompileReport::from_retained_native_artifact(
            options.root_path,
            source_file_count,
            artifact,
            rollback.into_receipt(),
            production_subject,
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        crate::compiler::native_checked::NativeCompilationWithCheckedReceipt::new(checked, report)
            .map(crate::compiler::native_checked::NativeCompilationWithCheckedReceipt::into_report)
            .map_err(|message| vec![Diagnostic::error(message)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_key_distinguishes_two_nonempty_optimization_suites() {
        use optimization_core::{Optimization, OptimizationSelections};
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(4)
            .expect("repository root");
        let report = crate::compile(
            crate::CompileRequest::new(crate::CompileOptions {
                root_path: repository
                    .join("tests/omega/pass/optimizer/no_selection_empty_entry/main.omg"),
                target_name: Some("linux_x86_64".to_owned()),
                build_dir: None,
            })
            .with_requested_product(crate::RequestedCompileProduct::NativeArtifact)
            .with_artifact_policy(crate::ArtifactEmissionPolicy::OutputOnly),
        )
        .and_then(crate::CompileOutcomes::into_single_report)
        .expect("produce an exact Terminal identity");
        let identity = report
            .retained_native_artifact()
            .unwrap()
            .psi_artifact()
            .manifest()
            .identity();
        let key = |optimization| NativeInputReuseKey {
            terminal_artifact_identity: identity,
            admission_profile: proof_admission::AdmissionProfile::default(),
            post_terminal_optimizations: OptimizationSelections::new([optimization])
                .unwrap()
                .project_post_terminal()
                .selections()
                .clone(),
        };
        let first = key(Optimization::SelectedIncomingU12ExactAddImmediate);
        let second = key(Optimization::SelectedIncomingU12ExactSubtractImmediate);
        assert!(!first.post_terminal_optimizations.is_empty());
        assert!(!second.post_terminal_optimizations.is_empty());
        assert!(first != second);
        assert!(first == key(Optimization::SelectedIncomingU12ExactAddImmediate));
    }
}

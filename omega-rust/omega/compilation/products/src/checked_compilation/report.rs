//! Admit the requested stop and retain its report companions across lowering.
//!
//! Check and artifact-only builds finish here. Terminal and native production
//! remain caller-owned stage sequences; their reports rejoin the exact build
//! companions, trust settlement and timing snapshot retained before lowering.

use super::CheckedCompilation;
use crate::{CompileOutputKind, CompileReport, RequestedCompileProduct, RetainedBuildOutputs};
use artifacts::compile_timings::PhaseTiming;
use diagnostics::Diagnostic;
use optimization_core::OptimizationRollback;
use package_compilation::BuildObservationIdentity;
use std::path::PathBuf;
use trust_model::TrustAdmissionSettlement;

/// Product admission chooses which stage sequence remains, or returns the
/// completed report when the requested stop needs no artifact lowering.
pub enum CheckedProductReport {
    Complete(CompileReport),
    Terminal(CompileReportSettlement),
    Native(CompileReportSettlement),
}

/// One-use report companions retained independently of artifact production.
/// This grants no proof, package acceptance or receiver authority.
pub struct CompileReportSettlement {
    expected_kind: CompileOutputKind,
    build_outputs: Option<RetainedBuildOutputs>,
    build_observation: Option<BuildObservationIdentity>,
    trust: TrustAdmissionSettlement,
    timings: Vec<PhaseTiming>,
}

impl CheckedCompilation {
    /// Rejoin a realized image with this checked source's product metadata.
    /// The production subject was sealed before lowering; the artifact,
    /// manifest and source metadata still undergo their independent checks.
    /// A dynamically linked ELF output keeps its own report variant, so it
    /// never satisfies direct-image custody.
    pub fn native_report(
        &self,
        root_path: PathBuf,
        artifact: native_artifact::RequestedNativeArtifact,
        rollback: Option<optimization_core::OptimizationRollbackReceipt>,
        production_subject: Option<crate::ProductionCompilationSubject>,
        profile: proof_admission::AdmissionProfile,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let execution = self.execution();
        let source_file_count = self.sources().source_file_count();
        let application = self.build_admission().native_application_metadata(
            &self.build().config.settings,
            execution.selected_target_profile,
        );
        let report = CompileReport::from_requested_native_artifact(
            root_path,
            source_file_count,
            artifact,
            rollback,
            production_subject,
        )
        .and_then(|report| report.with_pcc_context(self.build().config.settings.pcc, profile))
        .and_then(|report| report.with_application_metadata(application.clone()))
        .map_err(diagnostics)?;
        report
            .verify_native_source_metadata(
                source_file_count,
                execution.selected_target_profile,
                execution.selected_native_target(),
                &application,
            )
            .map_err(diagnostics)?;
        Ok(report.with_timings(self.timings().phases().to_vec()))
    }

    /// Construct a check-only report under the same evaluated-Build admission
    /// as ordinary compilation. A check never acquires completed output files.
    pub fn check_report(
        &self,
        root_path: PathBuf,
        rollback: &OptimizationRollback,
        trust: TrustAdmissionSettlement,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        RequestedCompileProduct::Check
            .validate_optimization_rollback(rollback)
            .map_err(|error| vec![error])?;
        RequestedCompileProduct::Check.admit_build_requirements(
            self.build_admission().artifact_only(),
            self.build().config.settings.pcc,
            rollback,
        )?;
        CompileReport::check_only(root_path, self.sources().source_file_count())
            .map(|report| {
                report
                    .with_trust_admission_settlement(trust)
                    .with_timings(self.timings().phases().to_vec())
            })
            .map_err(diagnostics)
    }

    /// Settle completed files and refuse incompatible Build requests before
    /// dispatching any Terminal or native production. Trust has already been
    /// independently reconstructed by the caller; admission does not infer it.
    pub fn prepare_product_report(
        &self,
        root_path: PathBuf,
        requested: RequestedCompileProduct,
        rollback: &OptimizationRollback,
        trust: TrustAdmissionSettlement,
    ) -> Result<CheckedProductReport, Vec<Diagnostic>> {
        if requested == RequestedCompileProduct::Check {
            return self
                .check_report(root_path, rollback, trust)
                .map(CheckedProductReport::Complete);
        }
        requested
            .validate_optimization_rollback(rollback)
            .map_err(|error| vec![error])?;
        // Preserve the checked route's refusal order: completed-file custody
        // precedes evaluated proof/rollback eligibility for producing stops.
        let build_outputs = self.build().completed_outputs()?;
        let artifact_only = self.build_admission().artifact_only();
        requested.admit_build_requirements(
            artifact_only,
            self.build().config.settings.pcc,
            rollback,
        )?;
        let timings = self.timings().phases().to_vec();
        if artifact_only {
            let outputs = build_outputs
                .ok_or_else(|| diagnostics("artifact-only build completed no files"))?;
            return CompileReport::from_build_outputs(
                root_path,
                self.sources().source_file_count(),
                outputs,
                self.production_subject()?,
            )
            .map(|report| {
                CheckedProductReport::Complete(
                    report
                        .with_trust_admission_settlement(trust)
                        .with_timings(timings),
                )
            })
            .map_err(diagnostics);
        }
        let terminal = requested == RequestedCompileProduct::TerminalArtifact;
        if !terminal {
            // Final composition stops at build-bound progress that no
            // installed provider occurrence has discharged.
            provider_planning::component_progress::reject_undischarged_build_bound_progress(
                self.execution().component_progress.as_ref(),
            )?;
        }
        let settlement = CompileReportSettlement {
            expected_kind: if terminal {
                CompileOutputKind::TerminalArtifact
            } else {
                CompileOutputKind::RetainedNativeArtifact
            },
            build_outputs,
            build_observation: self.build().observation_identity(),
            trust,
            timings,
        };
        Ok(if terminal {
            CheckedProductReport::Terminal(settlement)
        } else {
            CheckedProductReport::Native(settlement)
        })
    }
}

impl CompileReportSettlement {
    /// Production legs that extend the checked record's ladder after this
    /// settlement's snapshot replace it, so the finished report carries the
    /// post-production row set.
    pub fn with_timings(mut self, timings: Vec<artifacts::compile_timings::PhaseTiming>) -> Self {
        self.timings = timings;
        self
    }

    /// Rejoin companions only after the admitted primary product succeeds.
    /// Existing report constructors and attachment replay still validate all
    /// artifact, rollback, proof-product and build-observation custody.
    pub fn finish(self, report: CompileReport) -> Result<CompileReport, Vec<Diagnostic>> {
        if report.output_kind() != self.expected_kind {
            return Err(diagnostics(
                "produced report differs from the admitted product kind",
            ));
        }
        report
            .with_build_outputs(self.build_outputs, self.build_observation)
            .map(|report| {
                let timings = if report.timings().is_empty() {
                    self.timings
                } else {
                    report.timings().to_vec()
                };
                report
                    .with_trust_admission_settlement(self.trust)
                    .with_timings(timings)
            })
            .map_err(diagnostics)
    }
}

fn diagnostics(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

#[cfg(test)]
mod tests;

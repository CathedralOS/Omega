//! Admit the requested stop and construct every route's product report.
//!
//! One settlement call owns both halves: `prepare_product_report` refuses an
//! incompatible request before any lowering runs, and the settlement it hands
//! back is the only way the Terminal and native stage sequences turn their
//! produced artifact into a report. Check and artifact-only builds finish
//! inside the admission itself. Nothing downstream re-admits the stop or
//! re-decorates the report: the production subject, application metadata,
//! proof-product context, completed build outputs and
//! timing snapshot are all captured here, before the stage sequence consumes
//! the checked record.

use super::CheckedCompilation;
use crate::{
    CompileOutputKind, CompileReport, ProductionCompilationSubject, RequestedCompileProduct,
    RetainedBuildOutputs, RetainedTerminalArtifact,
};
use artifacts::compile_timings::PhaseTiming;
use diagnostics::Diagnostic;
use optimization_core::{OptimizationRollback, OptimizationRollbackReceipt};
use package_compilation::BuildObservationIdentity;
use package_compilation::build_execution::build_value::PccRequests;
use std::path::PathBuf;
use target::NativeApplicationMetadata;

/// Product admission chooses which stage sequence remains, or returns the
/// completed report when the requested stop needs no artifact lowering.
pub enum CheckedProductReport {
    Complete(CompileReport),
    Terminal(CompileReportSettlement),
    Native(CompileReportSettlement),
}

/// One-use report construction retained independently of artifact production.
/// This grants no proof, package acceptance or receiver authority.
pub struct CompileReportSettlement {
    expected_kind: CompileOutputKind,
    root_path: PathBuf,
    source_file_count: usize,
    /// The package production subject settled before lowering, or `None` for a
    /// compilation without package custody. Production never reconstructs it
    /// from the checked record it is consuming.
    production_subject: Option<ProductionCompilationSubject>,
    pcc_requests: PccRequests,
    application: NativeApplicationMetadata,
    build_outputs: Option<RetainedBuildOutputs>,
    build_observation: Option<BuildObservationIdentity>,
    timings: Vec<PhaseTiming>,
}

impl CheckedCompilation {
    /// Settle completed files, refuse incompatible Build and release-rollback
    /// requests, and capture the report companions before dispatching any
    /// Terminal or native production. The caller has already resolved the
    /// root's trust grants.
    pub fn prepare_product_report(
        &self,
        root_path: PathBuf,
        requested: RequestedCompileProduct,
        rollback: &OptimizationRollback,
    ) -> Result<CheckedProductReport, Vec<Diagnostic>> {
        // A rollback naming stages this stop never executes refuses here, for
        // every route, before production or publication can begin
        // (wiki/spec/build/optimizations.md#release-rollback-and-promotion).
        requested
            .validate_optimization_rollback(rollback)
            .map_err(|error| vec![error])?;
        let pcc_requests = self.build().config.settings.pcc;
        let source_file_count = self.sources().source_file_count();
        let timings = self.timings().phases().to_vec();
        if requested == RequestedCompileProduct::Check {
            requested.admit_build_requirements(
                self.build_admission().artifact_only(),
                pcc_requests,
                rollback,
            )?;
            // A check never acquires completed output files.
            return CompileReport::check_only(root_path, source_file_count)
                .map(|report| {
                    CheckedProductReport::Complete(
                        report
                            .with_timings(timings),
                    )
                })
                .map_err(diagnostics);
        }
        // Preserve the checked route's refusal order: completed-file custody
        // precedes evaluated proof/rollback eligibility for producing stops.
        let build_outputs = self.build().completed_outputs()?;
        let artifact_only = self.build_admission().artifact_only();
        requested.admit_build_requirements(artifact_only, pcc_requests, rollback)?;
        if artifact_only {
            let outputs = build_outputs
                .ok_or_else(|| diagnostics("artifact-only build completed no files"))?;
            return CompileReport::from_build_outputs(
                root_path,
                source_file_count,
                outputs,
                self.production_subject()?,
            )
            .map(|report| {
                CheckedProductReport::Complete(
                    report
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
            root_path,
            source_file_count,
            // Package production custody is required before the artifact
            // exists, not after a produced report fails to carry a manifest.
            production_subject: self.production_subject()?,
            pcc_requests,
            application: self.build_admission().native_application_metadata(
                &self.build().config.settings,
                self.execution().selected_target_profile,
            ),
            build_outputs,
            build_observation: self.build().observation_identity(),
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
    /// The production subject this admission settled. A route that assembles
    /// its own intermediate product reuses this exact custody rather than
    /// reconstructing it from the checked record production has consumed.
    pub const fn production_subject(&self) -> Option<&ProductionCompilationSubject> {
        self.production_subject.as_ref()
    }

    /// Production legs that extend the checked record's ladder after this
    /// settlement's snapshot replace it, so the finished report carries the
    /// post-production row set.
    pub fn with_timings(mut self, timings: Vec<PhaseTiming>) -> Self {
        self.timings = timings;
        self
    }

    /// Assemble the admitted Terminal product with the source subject captured
    /// before lowering and the exact effective rollback receipt.
    pub fn finish_terminal(
        self,
        artifact: RetainedTerminalArtifact,
        rollback: Option<OptimizationRollbackReceipt>,
        profile: proof_admission::AdmissionProfile,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let report = CompileReport::from_retained_terminal_artifact(
            self.root_path.clone(),
            self.source_file_count,
            artifact,
            self.production_subject.clone(),
        )
        .and_then(|report| report.with_terminal_optimization_rollback(rollback))
        .and_then(|report| report.with_pcc_context(self.pcc_requests, profile))
        .map_err(diagnostics)?;
        self.finish(report)
    }

    /// Assemble the admitted retained-native product with this compilation's
    /// application metadata. A dynamically linked ELF output keeps its own
    /// report variant, so it never satisfies direct-image custody.
    pub fn finish_native(
        self,
        artifact: native_artifact::RequestedNativeArtifact,
        rollback: Option<OptimizationRollbackReceipt>,
        profile: proof_admission::AdmissionProfile,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let report = CompileReport::from_requested_native_artifact(
            self.root_path.clone(),
            self.source_file_count,
            artifact,
            rollback,
            self.production_subject.clone(),
        )
        .and_then(|report| report.with_pcc_context(self.pcc_requests, profile))
        .and_then(|report| report.with_application_metadata(self.application.clone()))
        .map_err(diagnostics)?;
        self.finish(report)
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
                    .with_timings(timings)
            })
            .map_err(diagnostics)
    }
}

fn diagnostics(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

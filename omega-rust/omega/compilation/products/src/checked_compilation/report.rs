//! Admit the requested stop and construct every route's product report.
//!
//! One settlement call owns both halves: `prepare_product_report` refuses an
//! incompatible request before any lowering runs, and the settlement it hands
//! back is the only way the Terminal and native stage sequences turn their
//! produced artifact into a report. Check and artifact-only builds finish
//! inside the admission itself. Nothing downstream re-admits the stop or
//! re-decorates the report: application metadata, completed build outputs and
//! timing snapshot are all captured here, before the stage sequence consumes
//! the checked record.

use super::CheckedCompilation;
use crate::{
    CompileOutputKind, CompileReport, RequestedCompileProduct, RetainedBuildOutputs,
    RetainedTerminalArtifact,
};
use artifacts::compile_timings::PhaseTiming;
use diagnostics::Diagnostic;
use optimization_core::{OptimizationRollback, OptimizationRollbackReceipt};
use package_compilation::BuildObservationIdentity;
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
    source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    expected_kind: CompileOutputKind,
    root_path: PathBuf,
    source_file_count: usize,
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
        let source_file_count = self.sources().source_file_count();
        let timings = self.timings().phases().to_vec();
        if requested == RequestedCompileProduct::Check {
            requested.admit_build_requirements(self.build_admission().artifact_only(), rollback)?;
            // A check never acquires completed output files.
            return CompileReport::check_only(root_path, source_file_count)
                .map(|report| {
                    CheckedProductReport::Complete(
                        report
                            .with_source_completion_accounting(
                                self.source_completion_accounting().clone(),
                            )
                            .with_timings(timings),
                    )
                })
                .map_err(diagnostics);
        }
        // Preserve the checked route's refusal order: completed-file custody
        // precedes evaluated proof/rollback eligibility for producing stops.
        let build_outputs = self.build().completed_outputs()?;
        let artifact_only = self.build_admission().artifact_only();
        requested.admit_build_requirements(artifact_only, rollback)?;
        if artifact_only {
            let outputs = build_outputs
                .ok_or_else(|| diagnostics("artifact-only build completed no files"))?;
            return CompileReport::from_build_outputs(root_path, source_file_count, outputs)
                .map(|report| {
                    CheckedProductReport::Complete(
                        report
                            .with_source_completion_accounting(
                                self.source_completion_accounting().clone(),
                            )
                            .with_timings(timings),
                    )
                })
                .map_err(diagnostics);
        }
        let terminal = requested == RequestedCompileProduct::TerminalArtifact;
        let settlement = CompileReportSettlement {
            source_completion_accounting: self.source_completion_accounting().clone(),
            expected_kind: if terminal {
                CompileOutputKind::TerminalArtifact
            } else {
                CompileOutputKind::RetainedNativeArtifact
            },
            root_path,
            source_file_count,
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
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let report = CompileReport::from_retained_terminal_artifact(
            self.root_path.clone(),
            self.source_file_count,
            artifact,
        )
        .and_then(|report| report.with_terminal_optimization_rollback(rollback))
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
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let report = CompileReport::from_requested_native_artifact(
            self.root_path.clone(),
            self.source_file_count,
            artifact,
            rollback,
        )
        .and_then(|report| report.with_application_metadata(self.application.clone()))
        .map_err(diagnostics)?;
        self.finish(report)
    }

    /// Rejoin companions only after the admitted primary product succeeds.
    /// Existing report constructors and attachment replay still validate all
    /// artifact, rollback and build-observation custody.
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
                    .with_source_completion_accounting(self.source_completion_accounting)
                    .with_timings(timings)
            })
            .map_err(diagnostics)
    }
}

fn diagnostics(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

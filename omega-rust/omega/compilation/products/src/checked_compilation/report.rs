//! Admit the requested stop and construct every route's product report.
//!
//! One plan owns both halves: `ProductReportPlan` retains the requested stop and
//! settles optimization choices once. After installation, `prepare` refuses an
//! incompatible request before application completion, and the settlement it hands
//! back is the only way the Terminal and native stage sequences turn their
//! produced artifact into a report. Check and artifact-only builds finish
//! inside the admission itself. Nothing downstream re-admits the stop or
//! re-decorates the report: application metadata, completed build outputs and
//! timing snapshot are all captured here, before the stage sequence consumes
//! the checked record.

use super::CheckedCompilation;
use crate::{
    BuildAdmission, CompileOutputKind, CompileReport, ComputedBuildConfig, NativeProductRequest,
    RequestedCompileProduct, RetainedBuildOutputs, RetainedTerminalArtifact,
};
use artifacts::compile_timings::PhaseTiming;
use diagnostics::Diagnostic;
use optimization_core::{
    OptimizationRollback, OptimizationRollbackReceipt, OptimizationRollbackSettlement,
    OptimizationSelections,
};
use package_compilation::BuildObservationIdentity;
use std::path::PathBuf;
use target::NativeApplicationMetadata;

/// Product admission chooses which stage sequence remains, or returns the
/// completed report when the requested stop needs no artifact lowering.
pub enum CheckedProductReport {
    Complete(CompileReport),
    Terminal {
        settlement: CompileReportSettlement,
        rollback: OptimizationRollbackSettlement,
    },
    Native {
        settlement: CompileReportSettlement,
        request: NativeProductRequest,
        rollback: OptimizationRollbackSettlement,
    },
}

/// A product's output and production inputs are one request. In particular,
/// a native request cannot be paired with a Terminal-only report settlement.
pub enum ProductReportRequest {
    Check,
    Terminal(OptimizationRollback),
    Native(NativeProductRequest),
}

impl ProductReportRequest {
    fn kind(&self) -> RequestedCompileProduct {
        match self {
            Self::Check => RequestedCompileProduct::Check,
            Self::Terminal(_) => RequestedCompileProduct::TerminalArtifact,
            Self::Native(_) => RequestedCompileProduct::NativeArtifact,
        }
    }

    fn rollback(&self) -> OptimizationRollback {
        match self {
            Self::Check => OptimizationRollback::default(),
            Self::Terminal(rollback) => rollback.clone(),
            Self::Native(request) => request.optimization_rollback.clone(),
        }
    }
}

/// Planning grants no authority. It fixes the output and effective optimization
/// choices before shared Terminal work; admission still consumes the installed
/// checked product and its exact permissions at the existing boundary.
pub struct ProductReportPlan {
    root_path: PathBuf,
    requested: ProductReportRequest,
    rollback: OptimizationRollbackSettlement,
}

impl ProductReportPlan {
    pub fn new(
        root_path: PathBuf,
        requested: ProductReportRequest,
        build: &ComputedBuildConfig,
    ) -> Self {
        let rollback = requested
            .rollback()
            .settle(&build.config.settings.optimizations);
        Self {
            root_path,
            requested,
            rollback,
        }
    }

    pub fn needs_terminal(&self, admission: &BuildAdmission) -> bool {
        !matches!(self.requested, ProductReportRequest::Check) && !admission.artifact_only()
    }

    pub fn is_native(&self) -> bool {
        matches!(self.requested, ProductReportRequest::Native(_))
    }

    pub fn optimizations(&self) -> &OptimizationSelections {
        self.rollback.effective()
    }
}

/// One-use report construction retained independently of artifact production.
/// This grants no proof, package acceptance or receiver authority.
pub struct CompileReportSettlement {
    expected_kind: CompileOutputKind,
    root_path: PathBuf,
    source_file_count: usize,
    application: NativeApplicationMetadata,
    build_outputs: Option<RetainedBuildOutputs>,
    build_observation: Option<BuildObservationIdentity>,
    timings: Vec<PhaseTiming>,
}

impl ProductReportPlan {
    /// Settle completed files, refuse incompatible Build and release-rollback
    /// requests, and capture the report companions before completing the
    /// Terminal application or realizing native output. The caller has resolved the
    /// root's trust grants.
    pub fn prepare(
        self,
        checked: &CheckedCompilation,
    ) -> Result<CheckedProductReport, Vec<Diagnostic>> {
        let Self {
            root_path,
            requested: product,
            rollback: settled_rollback,
        } = self;
        let requested = product.kind();
        let rollback = product.rollback();
        // A rollback naming stages this stop never executes refuses here, for
        // every route, before application completion or native publication
        // (wiki/spec/build/optimizations.md#release-rollback-and-promotion).
        requested
            .validate_optimization_rollback(&rollback)
            .map_err(|error| vec![error])?;
        let source_file_count = checked.sources().source_file_count();
        let timings = checked.timings().phases().to_vec();
        if requested == RequestedCompileProduct::Check {
            requested
                .admit_build_requirements(checked.build_admission().artifact_only(), &rollback)?;
            // A check never acquires completed output files.
            return CompileReport::check_only(root_path, source_file_count)
                .map(|report| CheckedProductReport::Complete(report.with_timings(timings)))
                .map_err(diagnostics);
        }
        // Preserve the checked route's refusal order: completed-file custody
        // precedes evaluated proof/rollback eligibility for producing stops.
        let build_outputs = checked.build().completed_outputs()?;
        let artifact_only = checked.build_admission().artifact_only();
        requested.admit_build_requirements(artifact_only, &rollback)?;
        if artifact_only {
            let outputs = build_outputs
                .ok_or_else(|| diagnostics("artifact-only build completed no files"))?;
            return CompileReport::from_build_outputs(root_path, source_file_count, outputs)
                .map(|report| CheckedProductReport::Complete(report.with_timings(timings)))
                .map_err(diagnostics);
        }
        let terminal = requested == RequestedCompileProduct::TerminalArtifact;
        let settlement = CompileReportSettlement {
            expected_kind: if terminal {
                CompileOutputKind::TerminalArtifact
            } else {
                CompileOutputKind::RetainedNativeArtifact
            },
            root_path,
            source_file_count,
            application: checked.build_admission().native_application_metadata(
                &checked.build().config.settings,
                checked.execution().selected_target_profile,
            ),
            build_outputs,
            build_observation: checked.build().observation_identity(),
            timings,
        };
        match product {
            ProductReportRequest::Terminal(_) => Ok(CheckedProductReport::Terminal {
                settlement,
                rollback: settled_rollback,
            }),
            ProductReportRequest::Native(request) => Ok(CheckedProductReport::Native {
                settlement,
                request,
                rollback: settled_rollback,
            }),
            ProductReportRequest::Check => {
                Err(diagnostics("check product requested artifact settlement"))
            }
        }
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
                report.with_timings(timings)
            })
            .map_err(diagnostics)
    }
}

fn diagnostics(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

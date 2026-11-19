// Admit the requested stop and construct every route's product report.
// `ProductReportPlan` retains the requested stop and settles optimization
// choices once before shared Terminal work; `prepare` refuses incompatible
// Build or release-rollback requests (checking completed-file custody before
// proof/rollback eligibility on producing stops) and captures application
// metadata, completed build outputs, and timing snapshots into a one-use
// `CompileReportSettlement` before the stage sequence consumes the checked
// record.

use super::CheckedCompilation;
use crate::{
    BuildAdmission, CompileOutputKind, CompileReport, ComputedBuildConfig, NativeProductRequest,
    RequestedCompileProduct, RetainedBuildOutputs, RetainedTerminalArtifact,
};
use artifacts::compile_timings::PhaseTiming;
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use optimization_requests::{
    OptimizationRollback, OptimizationRollbackReceipt, OptimizationRollbackSettlement,
};
use package_compilation::BuildObservationIdentity;
use product_publication::{CompletedProduct, FinishedNativeImage};
use std::path::PathBuf;
use target::NativeApplicationMetadata;

pub enum CheckedProductReport {
    Complete(Box<CompileReport>),
    PublishedPsi {
        settlement: CompileReportSettlement,
        rollback: OptimizationRollbackSettlement,
    },
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

pub enum ProductReportRequest {
    Check,
    Terminal(OptimizationRollback),
    PublishedPsi(OptimizationRollback),
    Native(NativeProductRequest),
}

impl ProductReportRequest {
    fn kind(&self) -> RequestedCompileProduct {
        match self {
            Self::Check => RequestedCompileProduct::Check,
            Self::Terminal(_) => RequestedCompileProduct::TerminalArtifact,
            Self::PublishedPsi(_) => RequestedCompileProduct::PublishedPsi,
            Self::Native(_) => RequestedCompileProduct::NativeArtifact,
        }
    }

    fn rollback(&self) -> OptimizationRollback {
        match self {
            Self::Check => OptimizationRollback::default(),
            Self::Terminal(rollback) | Self::PublishedPsi(rollback) => rollback.clone(),
            Self::Native(request) => request.optimization_rollback.clone(),
        }
    }
}

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
        !Self::reports_advisories(self.requested.kind(), admission)
    }

    pub fn reports_advisories(
        request: RequestedCompileProduct,
        admission: &BuildAdmission,
    ) -> bool {
        request == RequestedCompileProduct::Check || admission.artifact_only()
    }

    pub fn optimizations(&self) -> &OptimizationSelections {
        self.rollback.effective()
    }
}

pub struct CompileReportSettlement {
    source_completion_accounting: source_compiler::evaluation::SourceCompletionAccounting,
    expected_kind: CompileOutputKind,
    root_path: PathBuf,
    source_file_count: usize,
    application: NativeApplicationMetadata,
    build_outputs: Option<RetainedBuildOutputs>,
    build_observation: Option<BuildObservationIdentity>,
    timings: Vec<PhaseTiming>,
}

impl ProductReportPlan {
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
        requested
            .validate_optimization_rollback(&rollback)
            .map_err(|error| vec![error])?;
        let source_file_count = checked.sources().source_file_count();
        let timings = checked.timings().phases().to_vec();
        if requested == RequestedCompileProduct::Check {
            requested
                .admit_build_requirements(checked.build_admission().artifact_only(), &rollback)?;
            return CompileReport::check_only(root_path, source_file_count)
                .map(|report| {
                    CheckedProductReport::Complete(Box::new(
                        report
                            .with_source_completion_accounting(
                                checked.source_completion_accounting().clone(),
                            )
                            .with_timings(timings),
                    ))
                })
                .map_err(diagnostics);
        }
        let build_outputs = checked.build().completed_outputs()?;
        let artifact_only = checked.build_admission().artifact_only();
        requested.admit_build_requirements(artifact_only, &rollback)?;
        if artifact_only && requested == RequestedCompileProduct::PublishedPsi {
            return Err(diagnostics(
                "this build only produces declared files and has no portable program to save",
            ));
        }
        if artifact_only {
            let outputs = build_outputs
                .ok_or_else(|| diagnostics("artifact-only build completed no files"))?;
            return CompileReport::from_build_outputs(root_path, source_file_count, outputs)
                .map(|report| {
                    CheckedProductReport::Complete(Box::new(
                        report
                            .with_source_completion_accounting(
                                checked.source_completion_accounting().clone(),
                            )
                            .with_timings(timings),
                    ))
                })
                .map_err(diagnostics);
        }
        let terminal = matches!(
            requested,
            RequestedCompileProduct::TerminalArtifact | RequestedCompileProduct::PublishedPsi
        );
        let settlement = CompileReportSettlement {
            source_completion_accounting: checked.source_completion_accounting().clone(),
            expected_kind: if terminal {
                CompileOutputKind::TerminalArtifact
            } else {
                CompileOutputKind::RetainedNativeArtifact
            },
            root_path,
            source_file_count,
            application: checked.build_admission().native_application_metadata(
                &checked.build().config.settings,
                checked.execution().target_profile(),
            ),
            build_outputs: if requested == RequestedCompileProduct::PublishedPsi {
                None
            } else {
                build_outputs
            },
            build_observation: checked.build().observation_identity(),
            timings,
        };
        match product {
            ProductReportRequest::PublishedPsi(_) => Ok(CheckedProductReport::PublishedPsi {
                settlement,
                rollback: settled_rollback,
            }),
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
    pub const fn application(&self) -> &NativeApplicationMetadata {
        &self.application
    }

    pub fn source_completion_accounting_mut(
        &mut self,
    ) -> &mut source_compiler::evaluation::SourceCompletionAccounting {
        &mut self.source_completion_accounting
    }

    pub fn with_timings(mut self, timings: Vec<PhaseTiming>) -> Self {
        self.timings = timings;
        self
    }

    pub fn finish_terminal(
        self,
        artifact: RetainedTerminalArtifact,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let product = CompletedProduct::from_terminal(artifact, rollback).map_err(diagnostics)?;
        self.finish_product(product)
    }

    pub fn finish_terminal_runtime(
        mut self,
        program: product_publication::AdmittedRuntimeProgram,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        if self.expected_kind != CompileOutputKind::RetainedNativeArtifact {
            return Err(diagnostics(
                "Terminal runtime admission requires an admitted native request",
            ));
        }
        self.expected_kind = CompileOutputKind::TerminalArtifact;
        let product = CompletedProduct::from_runtime(program, rollback);
        self.finish_product(product)
    }

    pub fn finish_published_psi(
        self,
        artifact: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        self.finish_product(
            CompletedProduct::from_published_psi(artifact, rollback).map_err(diagnostics)?,
        )
    }

    pub fn finish_native(
        self,
        image: FinishedNativeImage,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let product = CompletedProduct::from_native(image, rollback, self.application.clone());
        self.finish_product(product)
    }

    fn finish_product(
        mut self,
        product: CompletedProduct,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        let report = CompileReport::from_completed_product(
            std::mem::take(&mut self.root_path),
            self.source_file_count,
            product,
        );
        self.finish(report)
    }

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

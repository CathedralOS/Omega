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
use optimization_core::{
    OptimizationRollback, OptimizationRollbackReceipt, OptimizationRollbackSettlement,
    OptimizationSelections,
};
use package_compilation::BuildObservationIdentity;
use std::path::PathBuf;
use target::NativeApplicationMetadata;

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

pub struct CompileReportSettlement {
    source_completion_accounting: build_time_evaluation::SourceCompletionAccounting,
    expected_kind: CompileOutputKind,
    root_path: PathBuf,
    source_file_count: usize,
    application: NativeApplicationMetadata,
    build_outputs: Option<RetainedBuildOutputs>,
    build_observation: Option<BuildObservationIdentity>,
    timings: Vec<PhaseTiming>,
    float_realizations: Vec<([u8; 32], String)>,
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
                    CheckedProductReport::Complete(
                        report
                            .with_source_completion_accounting(
                                checked.source_completion_accounting().clone(),
                            )
                            .with_timings(timings),
                    )
                })
                .map_err(diagnostics);
        }
        let build_outputs = checked.build().completed_outputs()?;
        let artifact_only = checked.build_admission().artifact_only();
        requested.admit_build_requirements(artifact_only, &rollback)?;
        if artifact_only {
            let outputs = build_outputs
                .ok_or_else(|| diagnostics("artifact-only build completed no files"))?;
            return CompileReport::from_build_outputs(root_path, source_file_count, outputs)
                .map(|report| {
                    CheckedProductReport::Complete(
                        report
                            .with_source_completion_accounting(
                                checked.source_completion_accounting().clone(),
                            )
                            .with_timings(timings),
                    )
                })
                .map_err(diagnostics);
        }
        let terminal = requested == RequestedCompileProduct::TerminalArtifact;
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
                checked.execution().selected_target_profile,
            ),
            build_outputs,
            build_observation: checked.build().observation_identity(),
            timings,
            float_realizations: selected_float_realizations(checked)?,
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
    pub fn source_completion_accounting_mut(
        &mut self,
    ) -> &mut build_time_evaluation::SourceCompletionAccounting {
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
        let report = CompileReport::from_retained_terminal_artifact(
            self.root_path.clone(),
            self.source_file_count,
            artifact,
        )
        .and_then(|report| report.with_terminal_optimization_rollback(rollback))
        .map_err(diagnostics)?;
        self.finish(report)
    }

    pub fn finish_terminal_runtime(
        mut self,
        artifact: RetainedTerminalArtifact,
        provisions: Option<native_artifact::TerminalRuntimeProvisions>,
        rollback: Option<OptimizationRollbackReceipt>,
    ) -> Result<CompileReport, Vec<Diagnostic>> {
        if self.expected_kind != CompileOutputKind::RetainedNativeArtifact {
            return Err(diagnostics(
                "Terminal runtime admission requires an admitted native request",
            ));
        }
        self.expected_kind = CompileOutputKind::TerminalArtifact;
        let report = CompileReport::from_retained_terminal_artifact(
            self.root_path.clone(),
            self.source_file_count,
            artifact,
        )
        .and_then(|report| report.with_terminal_runtime_provisions(provisions))
        .and_then(|report| report.with_terminal_optimization_rollback(rollback))
        .map_err(diagnostics)?;
        self.finish(report)
    }

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

    pub fn finish(self, report: CompileReport) -> Result<CompileReport, Vec<Diagnostic>> {
        if report.output_kind() != self.expected_kind {
            return Err(diagnostics(
                "produced report differs from the admitted product kind",
            ));
        }
        // Selection checks the complete canonical map. Execution observations
        // instead follow the finished product's exact operator coverage, which
        // also retains operations inside selected checked bodies. Catalog rows
        // that supplied no emitted operator are not observed realizations.
        let coverage = report
            .retained_terminal_artifact()
            .and_then(|artifact| artifact.native_realization_proposal())
            .map(|proposal| &proposal.boundary_applications().coverage)
            .or_else(|| {
                report
                    .retained_native_artifact()
                    .and_then(|artifact| artifact.boundary_application_coverage())
            })
            .or_else(|| {
                report
                    .retained_dynamic_elf_artifact()
                    .and_then(|artifact| artifact.boundary_application_coverage())
            });
        let used = coverage
            .into_iter()
            .flat_map(|coverage| coverage.realizations().rows())
            .map(|realization| *realization.selected_plan_digest())
            .collect::<std::collections::HashSet<_>>();
        let float_realizations = self
            .float_realizations
            .into_iter()
            .filter_map(|(plan, description)| used.contains(&plan).then_some(description))
            .collect();
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
                    .with_float_realizations(float_realizations)
                    .with_timings(timings)
            })
            .map_err(diagnostics)
    }
}

fn diagnostics(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(message)]
}

// Capture descriptions against exact selected-plan identities while the
// selection provenance is available. The finished artifact determines use.
fn selected_float_realizations(
    checked: &CheckedCompilation,
) -> Result<Vec<([u8; 32], String)>, Vec<Diagnostic>> {
    use effects::provider_plan::ProviderBinding;
    use provider_planning::ProviderSelectionProvenance;
    use target::CanonicalFloatRealization;
    let mut rows = Vec::new();
    for selected in &checked.execution().selected_provider_provenance {
        let ProviderSelectionProvenance::TargetRealization(realization) = selected.selected_by
        else {
            continue;
        };
        let [row] = selected.plan.rows.as_slice() else {
            return Err(diagnostics(
                "canonical float realization report lost its exact row",
            ));
        };
        let (kind, implementation, isa, controls) = match (realization, &row.binding) {
            (
                CanonicalFloatRealization::CatalogIntrinsic { isa, controls },
                ProviderBinding::CompilerIntrinsic { machine },
            ) => ("catalog-intrinsic", machine.as_str(), isa, controls),
            (
                CanonicalFloatRealization::CheckedMachine { isa, controls },
                ProviderBinding::CheckedAdapter {
                    machine_identity, ..
                },
            ) => (
                "checked-machine",
                selected.plan.identity_text(*machine_identity),
                isa,
                controls,
            ),
            _ => {
                return Err(diagnostics(
                    "canonical float realization report differs from its selected binding",
                ));
            }
        };
        let plan = *selected.plan.identity_digest().as_bytes();
        rows.push((plan, format!("float realization: {}; kind={kind}; implementation={implementation}; target={}; isa={}; controls={}; plan={plan:02x?}",
            selected.plan.identity_text(row.requirement_identity), selected.plan.target,
            isa.report_name(), controls.report_name())));
    }
    Ok(rows)
}

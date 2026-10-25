//! Plan a requested stop, then complete its target-local installation and output.
//!
//! The compiler owns batch ordering and shared Terminal production. This owner
//! owns the work remaining for one product: nonexecuting stops finish directly,
//! while executable stops carry one product-owned report/optimization plan
//! across Terminal production. Permission and trust admission stay after selected
//! execution installation; planning is not authority to execute or publish.

use crate::execution_settlement::SelectedProductTarget;
use crate::native::produce_native_product;
use crate::terminal::{
    ProducedTerminalApplication, TerminalProductRequest, TerminalProductionView,
    finish_terminal_application,
};
use crate::{CheckedCompilation, CompileReport};
use compilation_products::{
    CheckedProductReport, NativeProductRequest, ProductReportPlan, ProductReportRequest,
};
use diagnostics::Diagnostic;
use package_evidence::permission_acceptance::TerminalPermissionAcceptance;
use std::path::PathBuf;

/// Native authority belongs only to a native request, never a checking stop.
pub enum CompilationProduct {
    Check,
    Terminal(crate::OptimizationRollback),
    Native(NativeProductRequest),
    /// Describe a named checked machine without granting executable authority.
    Inspection(String),
}

impl CompilationProduct {
    pub fn optimization_rollback(&self) -> crate::OptimizationRollback {
        match self {
            Self::Terminal(rollback) => rollback.clone(),
            Self::Native(request) => request.optimization_rollback.clone(),
            Self::Check | Self::Inspection(_) => Default::default(),
        }
    }
}

/// Finished output; checked source and continuation state stay in the compiler.
pub enum CompilationOutput {
    Report(CompileReport),
    Inspection(terminal_codec::CanonicalTerminalArtifact),
}

impl CompilationOutput {
    pub fn into_report(self) -> Result<CompileReport, CompilationProductError> {
        match self {
            Self::Report(report) => Ok(report),
            Self::Inspection(_) => Err(CompilationProductError::Production(vec![
                Diagnostic::error("requested a report from an inspection product"),
            ])),
        }
    }

    pub fn into_inspection(
        self,
    ) -> Result<terminal_codec::CanonicalTerminalArtifact, CompilationProductError> {
        match self {
            Self::Inspection(artifact) => Ok(artifact),
            Self::Report(_) => Err(CompilationProductError::Production(vec![
                Diagnostic::error("requested an inspection from a compilation report"),
            ])),
        }
    }
}

#[derive(Debug)]
pub enum CompilationProductError {
    TrustAdmission(Vec<Diagnostic>),
    Production(Vec<Diagnostic>),
}

impl From<Vec<Diagnostic>> for CompilationProductError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Production(diagnostics)
    }
}

impl CompilationProductError {
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::TrustAdmission(diagnostics) | Self::Production(diagnostics) => diagnostics,
        }
    }
}

/// The selection and exact request travel together, including occurrence consent.
pub(crate) struct ProductRequest {
    pub realization: SelectedProductTarget,
    pub root: PathBuf,
    pub product: CompilationProduct,
    pub permissions: Option<TerminalPermissionAcceptance>,
}

pub(crate) enum CompilationWork {
    Complete(CompilationOutput),
    Terminal(TerminalCompilation),
}

/// Only work that actually needs Terminal production enters the shared batch.
/// The report plan owns the native request and its settled optimization choices;
/// there is no independent requested-mode/report-mode pair to reconcile later.
pub(crate) struct TerminalCompilation {
    realization: SelectedProductTarget,
    report: ProductReportPlan,
    permissions: Option<TerminalPermissionAcceptance>,
}

impl ProductRequest {
    pub(crate) fn plan(self) -> Result<CompilationWork, CompilationProductError> {
        let Self {
            realization,
            root,
            product,
            permissions,
        } = self;
        let requested = match product {
            CompilationProduct::Inspection(machine) => {
                let checked = realization.install()?;
                return crate::inspection::inspect_terminal(checked, &machine)
                    .map(CompilationOutput::Inspection)
                    .map(CompilationWork::Complete)
                    .map_err(CompilationProductError::Production);
            }
            CompilationProduct::Check => ProductReportRequest::Check,
            CompilationProduct::Terminal(rollback) => ProductReportRequest::Terminal(rollback),
            CompilationProduct::Native(request) => ProductReportRequest::Native(request),
        };
        let report = ProductReportPlan::new(root, requested, &realization.custody.build);
        if report.needs_terminal(&realization.custody.build_admission) {
            Ok(CompilationWork::Terminal(TerminalCompilation {
                realization,
                report,
                permissions,
            }))
        } else {
            let checked = realization.install()?;
            admit_permissions(&checked, permissions.as_ref(), false)?;
            checked
                .admit_trust()
                .map_err(CompilationProductError::TrustAdmission)?;
            match report.prepare(&checked)? {
                CheckedProductReport::Complete(report) => {
                    Ok(CompilationWork::Complete(CompilationOutput::Report(report)))
                }
                _ => Err(vec![Diagnostic::error(
                    "non-executable product requested Terminal production",
                )]
                .into()),
            }
        }
    }
}

impl TerminalCompilation {
    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest {
            view: TerminalProductionView::selection(
                &mut self.realization.custody,
                self.report
                    .is_native()
                    .then_some(self.realization.independent_component_products.as_slice()),
            ),
            selections: self.report.optimizations(),
        }
    }

    pub(crate) fn finish(
        self,
        produced: ProducedTerminalApplication,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let checked = self.realization.install()?;
        admit_permissions(&checked, self.permissions.as_ref(), true)?;
        checked
            .admit_trust()
            .map_err(CompilationProductError::TrustAdmission)?;
        // Product eligibility precedes application completion, as it did before
        // batching. Both production halves consume the same settled choices.
        let product = self.report.prepare(&checked)?;
        let report = match product {
            CheckedProductReport::Complete(_) => {
                return Err(vec![Diagnostic::error(
                    "executable product unexpectedly completed without an artifact",
                )]
                .into());
            }
            CheckedProductReport::Terminal {
                settlement,
                rollback,
            } => {
                let terminal =
                    finish_terminal_application(&checked, produced, rollback.effective(), &[])?;
                settlement
                    .with_timings(checked.timings().phases().to_vec())
                    .finish_terminal(terminal, rollback.into_receipt())?
            }
            CheckedProductReport::Native {
                settlement,
                request,
                rollback,
            } => {
                let terminal =
                    finish_terminal_application(&checked, produced, rollback.effective(), &[])?;
                let produced =
                    produce_native_product(request, &checked, terminal, rollback.effective())?;
                settlement
                    .with_timings(checked.timings().phases().to_vec())
                    .finish_native(produced.artifact, rollback.into_receipt())?
                    .with_runtime_provisions(produced.runtime_provisions)
                    .map_err(|message| vec![Diagnostic::error(message)])?
            }
        };
        Ok(CompilationOutput::Report(report))
    }
}

fn admit_permissions(
    checked: &CheckedCompilation,
    permissions: Option<&TerminalPermissionAcceptance>,
    runtime_product: bool,
) -> Result<(), CompilationProductError> {
    if runtime_product
        && permissions.is_none()
        && checked.sources().package_identity().is_some()
        && !checked
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings()
            .is_empty()
    {
        return Err(CompilationProductError::TrustAdmission(vec![
            Diagnostic::error(
                "assumed binding requires explicit accepted root Product risk rows; run omega update",
            ),
        ]));
    }
    if let Some(permissions) = permissions {
        permissions
            .validate(checked)
            .map_err(CompilationProductError::TrustAdmission)?;
    }
    Ok(())
}

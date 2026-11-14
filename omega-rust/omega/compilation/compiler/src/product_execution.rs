// One product path publishes the accepted program, checks its target conditions,
// and admits package permissions. Executable stops then join its actual entry,
// arrival plans and settled provider declarations; application completion owns
// native compilation and publication.

mod executable;
mod inspection;

use crate::execution_settlement::SelectedProductTarget;
use crate::terminal::{ProducedTerminalProduct, TerminalProductRequest};
use crate::{CheckedCompilation, CompileReport};
use compilation_products::{
    CheckedProductReport, NativeProductRequest, ProductReportPlan, ProductReportRequest,
};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use package_evidence::permission_acceptance::TerminalPermissionAcceptance;
use std::path::PathBuf;

#[derive(Clone)]
pub enum CompilationProduct {
    Check,
    Terminal(crate::OptimizationRollback),
    Native(NativeProductRequest),
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

pub enum CompilationOutput {
    Native(Box<compilation_products::PendingNativeProduct>),
    Report(CompileReport),
    Inspection(std::sync::Arc<psi_compiler::PortableTerminalProgram>),
    Component(compilation_products::IndependentComponentProduct),
}

impl CompilationOutput {
    pub fn into_report(self) -> Result<CompileReport, CompilationProductError> {
        match self {
            Self::Report(report) => Ok(report),
            Self::Native(_) => Err(vec![Diagnostic::error(
                "native product requires application completion",
            )]
            .into()),
            Self::Inspection(_) => Err(CompilationProductError::Production(vec![
                Diagnostic::error("requested a report from an inspection product"),
            ])),
            Self::Component(_) => Err(vec![Diagnostic::error(
                "requested a report from a component product",
            )]
            .into()),
        }
    }

    pub fn into_inspection(
        self,
    ) -> Result<std::sync::Arc<psi_compiler::PortableTerminalProgram>, CompilationProductError>
    {
        match self {
            Self::Inspection(artifact) => Ok(artifact),
            Self::Report(_) | Self::Native(_) => Err(CompilationProductError::Production(vec![
                Diagnostic::error("requested an inspection from a compilation report"),
            ])),
            Self::Component(_) => Err(vec![Diagnostic::error(
                "requested an inspection from a component product",
            )]
            .into()),
        }
    }

    pub(crate) fn into_component(
        self,
    ) -> Result<compilation_products::IndependentComponentProduct, CompilationProductError> {
        match self {
            Self::Component(product) => Ok(product),
            Self::Report(_) | Self::Inspection(_) | Self::Native(_) => {
                Err(vec![Diagnostic::error(
                    "requested a component from a non-component product",
                )]
                .into())
            }
        }
    }
}

pub struct TargetProductOutcome {
    pub target: Option<target::TargetProfile>,
    pub result: Result<CompilationOutput, CompilationProductError>,
}

#[derive(Debug)]
pub enum CompilationProductError {
    TrustAdmission(Vec<Diagnostic>),
    Production(Vec<Diagnostic>),
    ProductionAttempt {
        diagnostics: Vec<Diagnostic>,
        accounting: build_time_evaluation::SourceCompletionAccounting,
    },
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
            Self::ProductionAttempt { diagnostics, .. } => diagnostics,
        }
    }

    pub fn into_target_outcome(
        self,
        target: Option<target::TargetProfile>,
    ) -> crate::CompileTargetOutcome {
        match self {
            Self::ProductionAttempt {
                diagnostics,
                accounting,
            } => crate::CompileTargetOutcome::new(target, Err(diagnostics))
                .with_failed_source_completion_accounting(accounting),
            error => crate::CompileTargetOutcome::new(target, Err(error.into_diagnostics())),
        }
    }
}

pub(crate) enum ProductRequest {
    Requested {
        realization: SelectedProductTarget,
        root: PathBuf,
        product: CompilationProduct,
        permissions: Option<TerminalPermissionAcceptance>,
    },
    Component(CheckedCompilation),
}

pub(crate) enum TerminalCompilation {
    Product(TargetCompilation),
    Inspection(inspection::InspectionCompilation),
}

pub(crate) struct TargetCompilation {
    realization: SelectedProductTarget,
    report: ProductReportPlan,
    permissions: Option<TerminalPermissionAcceptance>,
    selections: OptimizationSelections,
}

impl TargetCompilation {
    pub(crate) fn new(
        realization: SelectedProductTarget,
        report: ProductReportPlan,
        permissions: Option<TerminalPermissionAcceptance>,
    ) -> Self {
        let selections = report.optimizations().clone();
        Self {
            realization,
            report,
            permissions,
            selections,
        }
    }

    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest::declarations(&mut self.realization.custody, &self.selections)
    }

    pub(crate) fn finish(
        mut self,
        produced: ProducedTerminalProduct,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let completed = produced
            .check_conditions(&mut self.realization.custody)
            .and_then(|()| {
                self.realization
                    .check_declaration_requirements(produced.module())
            });
        if let Err(diagnostics) = completed {
            return Err(CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting: std::mem::take(
                    &mut self.realization.custody.source_completion_accounting,
                ),
            });
        }
        let runtime_product = self
            .report
            .needs_terminal(&self.realization.custody.build_admission);
        let runtime_policy = std::sync::Arc::clone(&self.realization.runtime_policy);
        let checked = self.realization.into_checked_compilation()?;
        let accounting = checked.source_completion_accounting().clone();
        let report = (|| {
            admit_permissions(
                &checked,
                &runtime_policy,
                self.permissions.as_ref(),
                runtime_product,
            )?;
            checked
                .admit_trust()
                .map_err(CompilationProductError::TrustAdmission)?;
            match self.report.prepare(&checked)? {
                CheckedProductReport::Complete(report) => Ok(CompilationOutput::Report(report)),
                CheckedProductReport::Terminal {
                    settlement,
                    rollback,
                } => {
                    let (terminal, _) = executable::prepare(&checked, produced, &self.selections)?;
                    settlement
                        .finish_terminal(terminal, rollback.into_receipt())
                        .map(CompilationOutput::Report)
                        .map_err(Into::into)
                }
                CheckedProductReport::Native {
                    settlement,
                    request,
                    rollback,
                } => {
                    let (terminal, permissions) =
                        executable::prepare(&checked, produced, &self.selections)?;
                    if checked.execution().realization().composition_modes().any(|mode| {
                        mode == language_semantics::declaration_selection::CompositionMode::Independent
                    }) {
                        return Err(CompilationProductError::Production(vec![Diagnostic::error(
                            "native composition needs the selected independent components' verified products",
                        )]));
                    }
                    Ok(CompilationOutput::Native(Box::new(
                        compilation_products::PendingNativeProduct {
                            terminal,
                            retained_native_inputs: None,
                            components: Ok(Vec::new()),
                            accepted_package_permissions: permissions,
                            request,
                            rollback,
                            subsystem: checked.build().config.settings.subsystem,
                            settlement,
                            tests: Vec::new(),
                            test_notes: Vec::new(),
                        },
                    )))
                }
            }
        })();
        report.map_err(|error: CompilationProductError| {
            CompilationProductError::ProductionAttempt {
                diagnostics: error.into_diagnostics(),
                accounting,
            }
        })
    }
}

impl ProductRequest {
    pub(crate) fn plan(self) -> Result<TerminalCompilation, CompilationProductError> {
        let (realization, root, product, permissions) = match self {
            Self::Requested {
                realization,
                root,
                product,
                permissions,
            } => (realization, root, product, permissions),
            Self::Component(checked) => {
                if checked.sources().package_identity().is_none() {
                    return Err(vec![Diagnostic::error(
                        "component publication requires package-aware checked custody for the dependency compiled as its own component",
                    )]
                    .into());
                }
                return Err(refuse_executable_product("a component").into());
            }
        };
        let requested = match product {
            CompilationProduct::Inspection(machine) => {
                return Ok(TerminalCompilation::Inspection(
                    inspection::InspectionCompilation::new(realization, machine),
                ));
            }
            CompilationProduct::Check => ProductReportRequest::Check,
            CompilationProduct::Terminal(rollback) => ProductReportRequest::Terminal(rollback),
            CompilationProduct::Native(request) => ProductReportRequest::Native(request),
        };
        let report = ProductReportPlan::new(root, requested, &realization.custody.build);
        // The accepted Terminal module always exists on this route, so every
        // non-executing stop checks its target-local conditions against it.
        Ok(TerminalCompilation::Product(TargetCompilation::new(
            realization,
            report,
            permissions,
        )))
    }
}

fn refuse_executable_product(product: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "{product} product needs its canonical export roots and verified component evidence before publication"
    ))]
}

impl TerminalCompilation {
    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        match self {
            Self::Product(product) => product.terminal_request(),
            Self::Inspection(product) => product.terminal_request(),
        }
    }

    pub(crate) fn finish(
        self,
        produced: crate::terminal::ProducedTerminalProduct,
    ) -> Result<CompilationOutput, CompilationProductError> {
        match self {
            Self::Product(product) => product.finish(produced),
            Self::Inspection(product) => product.finish(produced),
        }
    }
}

fn admit_permissions(
    checked: &CheckedCompilation,
    declarations: &package_evidence::permission_acceptance::PreparedRuntimePolicy,
    permissions: Option<&TerminalPermissionAcceptance>,
    runtime_product: bool,
) -> Result<(), CompilationProductError> {
    let input = package_evidence::permission_acceptance::RuntimePermissionInput::from_compilation(
        declarations,
        checked,
    );
    let evaluated_rows = if runtime_product
        && permissions.is_none()
        && checked.sources().package_identity().is_some()
        && !checked.execution().plans().assumed_bindings().is_empty()
    {
        Some(
            TerminalPermissionAcceptance::from_public_build(&input)
                .map_err(CompilationProductError::TrustAdmission)?,
        )
    } else {
        None
    };
    let permissions = evaluated_rows.as_ref().or(permissions);
    if let Some(permissions) = permissions {
        permissions
            .validate(&input)
            .map_err(CompilationProductError::TrustAdmission)?;
    }
    Ok(())
}

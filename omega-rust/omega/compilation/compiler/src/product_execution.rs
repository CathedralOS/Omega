/*
We plan each requested compilation stop (`Check`, `Terminal`, `Native`,
`Inspection`, or `Component`) and finish its product.

The compiler owns batch ordering and shared Terminal production. A `Check` or
`Inspection` stop joins the shared portable Terminal batch
(`TerminalProductUse::Declarations` / `Inspection`): the accepted program is
produced once, its target-local scalar conditions and task activations are
checked against the target's settled provider plans, and permission and trust
admission run on the sealed `CheckedCompilation`, so planning alone never
grants authority to publish.

A `Terminal`, `Native` or `Component` stop is an executable product. It needs
the selected program entry, callback roots and the native declarations of its
provider plans, formed from the accepted artifact and the settled plans.
Nothing on the lowered route produces those yet, so `ProductRequest::plan`
refuses these stops by name instead of reading a retained copy of them.
*/

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

impl From<crate::CompileFailure> for CompilationProductError {
    fn from(failure: crate::CompileFailure) -> Self {
        match failure {
            crate::CompileFailure::SourceCompletion {
                diagnostics,
                accounting,
            } => Self::ProductionAttempt {
                diagnostics,
                accounting,
            },
            other => Self::Production(other.into_diagnostics()),
        }
    }
}

impl From<CompilationProductError> for crate::CompileFailure {
    fn from(error: CompilationProductError) -> Self {
        match error {
            CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting,
            } => Self::source_completion(diagnostics, accounting),
            other => Self::Diagnostics(other.into_diagnostics()),
        }
    }
}

impl CompilationProductError {
    pub fn with_source_completion_accounting(
        self,
        mut accounting: build_time_evaluation::SourceCompletionAccounting,
    ) -> Self {
        match self {
            Self::ProductionAttempt {
                diagnostics,
                accounting: attempted,
            } => {
                accounting.append(attempted);
                Self::ProductionAttempt {
                    diagnostics,
                    accounting,
                }
            }
            other if accounting.receipts().is_empty() => other,
            other => Self::ProductionAttempt {
                diagnostics: other.into_diagnostics(),
                accounting,
            },
        }
    }

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
        package_inputs: Option<package_compilation::PackageCompilationInputs>,
        root: PathBuf,
        product: CompilationProduct,
        permissions: Option<TerminalPermissionAcceptance>,
    },
    Component(CheckedCompilation),
}

pub(crate) enum TerminalCompilation {
    Checking(CheckingCompilation),
    Inspection(inspection::InspectionCompilation),
}

pub(crate) struct CheckingCompilation {
    realization: SelectedProductTarget,
    package_inputs: Option<package_compilation::PackageCompilationInputs>,
    report: ProductReportPlan,
    permissions: Option<TerminalPermissionAcceptance>,
    selections: OptimizationSelections,
}

impl CheckingCompilation {
    pub(crate) fn new(
        realization: SelectedProductTarget,
        package_inputs: Option<package_compilation::PackageCompilationInputs>,
        report: ProductReportPlan,
        permissions: Option<TerminalPermissionAcceptance>,
    ) -> Self {
        let selections = realization
            .custody
            .terminal_publication
            .optimizations()
            .clone();
        Self {
            realization,
            package_inputs,
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
        let runtime_policy = std::sync::Arc::clone(&self.realization.runtime_policy);
        let checked = self.realization.into_checked_compilation()?;
        let mut accounting = checked.source_completion_accounting().clone();
        let report = (|| {
            admit_permissions(
                &checked,
                &runtime_policy,
                self.permissions.as_ref(),
                self.report.needs_terminal(checked.build_admission()),
            )?;
            checked
                .admit_trust()
                .map_err(CompilationProductError::TrustAdmission)?;
            match self.report.prepare(&checked)? {
                CheckedProductReport::Complete(report) => Ok(CompilationOutput::Report(
                    report.with_advisories(advisories(&checked)),
                )),
                CheckedProductReport::Terminal {
                    mut settlement,
                    rollback,
                } => {
                    let (signature, application) =
                        crate::terminal::prepare_program_entry(&checked)?;
                    let plans = crate::terminal::prepare_entry_calling_plans(
                        &checked,
                        &produced.program,
                        self.package_inputs.as_ref(),
                        &signature,
                        &mut accounting,
                    )?;
                    let (terminal, _, _) = crate::terminal::prepare_native_application(
                        &checked,
                        produced,
                        native_compiler::NativeProgramEntrySettlement::new(
                            &signature,
                            Some((
                                &plans.semantic_calling_application,
                                &plans.physical_calling_application,
                                &plans.storage_entry,
                            )),
                        ),
                        application,
                        settlement.application().clone(),
                        rollback.effective(),
                        None,
                    )?;
                    *settlement.source_completion_accounting_mut() = accounting.clone();
                    settlement
                        .finish_terminal(terminal, rollback.into_receipt())
                        .map(CompilationOutput::Report)
                        .map_err(Into::into)
                }
                CheckedProductReport::Native {
                    mut settlement,
                    request,
                    rollback,
                } => {
                    let (signature, application) =
                        crate::terminal::prepare_program_entry(&checked)?;
                    let plans = crate::terminal::prepare_entry_calling_plans(
                        &checked,
                        &produced.program,
                        self.package_inputs.as_ref(),
                        &signature,
                        &mut accounting,
                    )?;
                    let (terminal, accepted_package_permissions, retained_native_inputs) =
                        crate::terminal::prepare_native_application(
                            &checked,
                            produced,
                            native_compiler::NativeProgramEntrySettlement::new(
                                &signature,
                                Some((
                                    &plans.semantic_calling_application,
                                    &plans.physical_calling_application,
                                    &plans.storage_entry,
                                )),
                            ),
                            application,
                            settlement.application().clone(),
                            rollback.effective(),
                            Some(&request),
                        )?;
                    *settlement.source_completion_accounting_mut() = accounting.clone();
                    Ok(CompilationOutput::Native(Box::new(
                        compilation_products::PendingNativeProduct {
                            terminal,
                            retained_native_inputs,
                            components: Ok(Vec::new()),
                            accepted_package_permissions,
                            request,
                            rollback,
                            subsystem: settlement.application().subsystem,
                            settlement,
                            tests: Vec::new(),
                            test_notes: Vec::new(),
                        },
                    )))
                }
            }
        })();
        report.map_err(|error: CompilationProductError| {
            error.with_source_completion_accounting(accounting)
        })
    }
}

/// The advisory diagnostics of the accepted source this compilation checked
/// (`AcceptedTerminalSource::advisories`), each placed in its source file.
/// They are warnings about a program that was accepted, so a compilation
/// with no retained judgment simply has none.
fn advisories(checked: &compilation_products::CheckedCompilation) -> Vec<Diagnostic> {
    let Ok(accepted) = checked.terminal_publication().accepted() else {
        return Vec::new();
    };
    let mut advisories = accepted.advisories().to_vec();
    if let Some(sources) = accepted.source().symbols.source_map() {
        crate::checked::checking::attribute_diagnostics(&mut advisories, &sources, None);
    }
    advisories
}

impl ProductRequest {
    pub(crate) fn plan(self) -> Result<TerminalCompilation, CompilationProductError> {
        let (mut realization, package_inputs, root, product, permissions) = match self {
            Self::Requested {
                realization,
                package_inputs,
                root,
                product,
                permissions,
            } => (realization, package_inputs, root, product, permissions),
            Self::Component(checked) => {
                let accounting = checked.source_completion_accounting().clone();
                if checked.sources().package_identity().is_none() {
                    return Err(CompilationProductError::from(vec![Diagnostic::error(
                        "component publication requires package-aware checked custody for the dependency compiled as its own component",
                    )])
                    .with_source_completion_accounting(accounting));
                }
                return Err(CompilationProductError::from(refuse_executable_product(
                    "a component",
                ))
                .with_source_completion_accounting(accounting));
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
        if report.needs_terminal(&realization.custody.build_admission)
            && let Err(diagnostics) = crate::terminal::prepare_entry_metadata(
                &mut realization.custody,
                package_inputs.as_ref(),
            )
        {
            return Err(CompilationProductError::from(diagnostics)
                .with_source_completion_accounting(
                    realization.custody.source_completion_accounting,
                ));
        }
        // The accepted Terminal module always exists on this route, so every
        // non-executing stop checks its target-local conditions against it.
        Ok(TerminalCompilation::Checking(CheckingCompilation::new(
            realization,
            package_inputs,
            report,
            permissions,
        )))
    }
}

fn refuse_executable_product(product: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "no product route yet: {product} product needs separately accepted component exports and native realization inputs"
    ))]
}

impl TerminalCompilation {
    pub(crate) fn source_completion_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        match self {
            Self::Checking(product) => &product.realization.custody.source_completion_accounting,
            Self::Inspection(product) => product.source_completion_accounting(),
        }
    }

    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        match self {
            Self::Checking(product) => product.terminal_request(),
            Self::Inspection(product) => product.terminal_request(),
        }
    }

    pub(crate) fn finish(
        self,
        produced: crate::terminal::ProducedTerminalProduct,
    ) -> Result<CompilationOutput, CompilationProductError> {
        match self {
            Self::Checking(product) => product.finish(produced),
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

//! Plan a requested stop, then complete its target-local installation and output.
//!
//! The compiler owns batch ordering and shared Terminal production. This owner
//! owns the work remaining for one product: nonexecuting stops finish directly,
//! while executable stops carry one product-owned report/optimization plan
//! across Terminal production. Permission and trust admission stay after selected
//! execution installation; planning is not authority to execute or publish.

mod checking;
mod component;
mod inspection;
mod native_inputs;

use crate::execution_settlement::SelectedProductTarget;
use crate::terminal::{TerminalProductRequest, TerminalProductionView};
use crate::{CheckedCompilation, CompileReport};
use compilation_products::{
    CheckedProductReport, NativeProductRequest, ProductReportPlan, ProductReportRequest,
};
use component::ComponentPublication;
use diagnostics::Diagnostic;
use native_production::TerminalApplication;
use package_evidence::permission_acceptance::TerminalPermissionAcceptance;
use std::path::PathBuf;

/// Native authority belongs only to a native request, never a checking stop.
#[derive(Clone)]
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
    Inspection(std::sync::Arc<psi_compiler::PortableTerminalProgram>),
    Component(compilation_products::IndependentComponentProduct),
}

impl CompilationOutput {
    pub fn into_report(self) -> Result<CompileReport, CompilationProductError> {
        match self {
            Self::Report(report) => Ok(report),
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
            Self::Report(_) => Err(CompilationProductError::Production(vec![
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
            Self::Report(_) | Self::Inspection(_) => Err(vec![Diagnostic::error(
                "requested a component from a non-component product",
            )]
            .into()),
        }
    }
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

    pub(crate) fn into_target_outcome(
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

/// The selection and exact request travel together, including occurrence consent.
pub(crate) enum ProductRequest {
    Requested {
        realization: SelectedProductTarget,
        root: PathBuf,
        product: CompilationProduct,
        permissions: Option<TerminalPermissionAcceptance>,
    },
    /// Review and dependency demands own Psi-checked, target-selected custody.
    /// Declaration obligations complete against the shared produced artifact,
    /// before publishing a component description or admitting it to the cache.
    Component {
        checked: CheckedCompilation,
        retain_native_inputs: bool,
    },
}

pub(crate) enum CompilationWork {
    Complete(CompilationOutput),
    Terminal(TerminalCompilation),
}

/// Only work that actually needs Terminal production enters the shared batch.
/// The report plan owns the native request and its settled optimization choices;
/// there is no independent requested-mode/report-mode pair to reconcile later.
pub(crate) enum TerminalCompilation {
    Requested(RequestedTerminalCompilation),
    Checking(checking::CheckingCompilation),
    Component(ComponentPublication),
    Inspection(inspection::InspectionCompilation),
}

pub(crate) struct RequestedTerminalCompilation {
    realization: SelectedProductTarget,
    report: ProductReportPlan,
    permissions: Option<TerminalPermissionAcceptance>,
}

impl ProductRequest {
    pub(crate) fn plan(self) -> Result<CompilationWork, CompilationProductError> {
        let (realization, root, product, permissions) = match self {
            Self::Requested {
                realization,
                root,
                product,
                permissions,
            } => (realization, root, product, permissions),
            Self::Component {
                checked,
                retain_native_inputs,
            } => {
                let component = ComponentPublication::prepare(checked, retain_native_inputs)?;
                return Ok(CompilationWork::Terminal(TerminalCompilation::Component(
                    component,
                )));
            }
        };
        let requested = match product {
            CompilationProduct::Inspection(machine) => {
                return Ok(CompilationWork::Terminal(TerminalCompilation::Inspection(
                    inspection::InspectionCompilation::new(realization, machine),
                )));
            }
            CompilationProduct::Check => ProductReportRequest::Check,
            CompilationProduct::Terminal(rollback) => ProductReportRequest::Terminal(rollback),
            CompilationProduct::Native(request) => ProductReportRequest::Native(request),
        };
        let report = ProductReportPlan::new(root, requested, &realization.custody.build);
        if report.needs_terminal(&realization.custody.build_admission) {
            Ok(CompilationWork::Terminal(TerminalCompilation::Requested(
                RequestedTerminalCompilation {
                    realization,
                    report,
                    permissions,
                },
            )))
        } else {
            if realization
                .custody
                .execution
                .program
                .facts
                .proof_applications
                .has_obligations
            {
                return Ok(CompilationWork::Terminal(TerminalCompilation::Checking(
                    checking::CheckingCompilation::new(realization, report, permissions),
                )));
            }
            let checked = realization.finish_check()?;
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
        match self {
            Self::Requested(product) => product.terminal_request(),
            Self::Checking(product) => product.terminal_request(),
            Self::Component(product) => product.terminal_request(),
            Self::Inspection(product) => product.terminal_request(),
        }
    }

    pub(crate) fn finish(
        self,
        produced: crate::terminal::ProducedTerminalProduct,
    ) -> Result<CompilationOutput, CompilationProductError> {
        match self {
            Self::Requested(product) => product.finish(produced.into_application()?),
            Self::Checking(product) => product.finish(produced),
            Self::Inspection(product) => product.finish(produced),
            Self::Component(product) => product
                .finish(produced.into_application()?)
                .map(CompilationOutput::Component),
        }
    }
}

impl RequestedTerminalCompilation {
    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest {
            view: TerminalProductionView::selection(
                &mut self.realization.custody,
                self.report
                    .is_native()
                    .then_some(&self.realization.independent_components),
            ),
            selections: self.report.optimizations(),
            use_kind: crate::terminal::TerminalProductUse::Application,
        }
    }

    pub(crate) fn finish(
        self,
        produced: TerminalApplication,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let checked = self.realization.into_checked_compilation();
        phase_profile::scope(
            "06.0.b_admit_permissions_and_trust",
            || -> Result<_, CompilationProductError> {
                admit_permissions(&checked, self.permissions.as_ref(), true)?;
                checked
                    .admit_trust()
                    .map_err(CompilationProductError::TrustAdmission)
            },
        )?;
        // Product eligibility precedes application completion, as it did before
        // batching. Both production halves consume the same settled choices.
        let product =
            phase_profile::scope("06.0.c_report_prepare", || self.report.prepare(&checked))?;
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
                let request =
                    checked.terminal_native_proposal_request(rollback.effective(), &[])?;
                let terminal = produced.prepare(request)?;
                settlement
                    .with_timings(checked.timings().phases().to_vec())
                    .finish_terminal(terminal, rollback.into_receipt())?
            }
            CheckedProductReport::Native {
                mut settlement,
                request,
                rollback,
            } => {
                let terminal = phase_profile::scope("06.0.d_finish_terminal_application", || {
                    let request =
                        checked.terminal_native_proposal_request(rollback.effective(), &[])?;
                    produced.prepare(request)
                })?;
                let produced = phase_profile::scope("06.0.e_produce_native_product", || {
                    let accepted_package_terminal_authority_permission_policy =
                        native_inputs::accepted_terminal_authority_permission_policy(&checked)?;
                    let components =
                        native_inputs::independent_components(checked.independent_components())?;
                    let post_terminal = rollback.effective().project_post_terminal();
                    let request = native_production::NativeCompileRequest {
                        private_entry_ram: &request.private_entry_ram,
                        optimization_selections: post_terminal.selections(),
                        terminal_authority_policy: request.terminal_authority_policy,
                        accepted_package_terminal_authority_permission_policy,
                        terminal_authority_permission_policy: request
                            .terminal_authority_permission_policy,
                        image_request: image_emission::ExecutableImageEmissionRequest::direct(
                            checked.build().config.settings.subsystem,
                        ),
                        imports: &[],
                        components: &components,
                    };
                    let mut prepared = native_production::prepare(&terminal, request)
                        .map_err(|(_, diagnostics)| diagnostics)?;
                    for completion in prepared.scalar_completions() {
                        let completed = crate::terminal::scalar_completion::complete(
                            completion.artifact(),
                            completion.target(),
                            completion.demand(),
                            completion.provider_proposals(),
                            settlement.source_completion_accounting_mut(),
                        )
                        .map_err(|reason| {
                            vec![Diagnostic::error(format!("constant checking: {reason}"))]
                        })?;
                        completion
                            .complete(completed)
                            .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    }
                    native_production::compile(prepared).map_err(|(_, diagnostics)| diagnostics)
                });
                let produced = match produced {
                    Ok(produced) => produced,
                    Err(diagnostics) => {
                        return Err(CompilationProductError::ProductionAttempt {
                            diagnostics,
                            accounting: std::mem::take(
                                settlement.source_completion_accounting_mut(),
                            ),
                        });
                    }
                };
                let accounting = settlement.source_completion_accounting_mut().clone();
                phase_profile::scope(
                    "06.0.f_settlement_finish_native",
                    || -> Result<_, CompilationProductError> {
                        Ok(settlement
                            .with_timings(checked.timings().phases().to_vec())
                            .finish_native(produced.artifact, rollback.into_receipt())?
                            .with_runtime_provisions(produced.runtime_provisions)
                            .map_err(|message| vec![Diagnostic::error(message)])?)
                    },
                )
                .map_err(|error| CompilationProductError::ProductionAttempt {
                    diagnostics: error.into_diagnostics(),
                    accounting,
                })?
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
    // A retained lock is compared as historical consent. An activation with no
    // lock still has to carry the root's Product risk rows, and those rows come
    // from the Build this compilation already evaluated. Writing `omega.lock`
    // here would accept one source graph as a side effect and leave every other
    // place of the same fact without consent.
    let evaluated_rows = if runtime_product
        && permissions.is_none()
        && checked.sources().package_identity().is_some()
        && !checked
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings()
            .is_empty()
    {
        Some(
            TerminalPermissionAcceptance::from_public_build(checked)
                .map_err(CompilationProductError::TrustAdmission)?,
        )
    } else {
        None
    };
    let permissions = evaluated_rows.as_ref().or(permissions);
    if let Some(permissions) = permissions {
        permissions
            .validate(checked)
            .map_err(CompilationProductError::TrustAdmission)?;
    }
    Ok(())
}

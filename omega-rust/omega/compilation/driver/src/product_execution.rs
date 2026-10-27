//! Plan a requested stop, then complete its target-local installation and output.
//!
//! The compiler owns batch ordering and shared Terminal production. This owner
//! owns the work remaining for one product: nonexecuting stops finish directly,
//! while executable stops carry one product-owned report/optimization plan
//! across Terminal production. Permission and trust admission stay after selected
//! execution installation; planning is not authority to execute or publish.

mod component;
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
    Inspection(psi_compiler::ProducedTerminalProgram),
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
    ) -> Result<psi_compiler::ProducedTerminalProgram, CompilationProductError> {
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
pub(crate) enum ProductRequest {
    Requested {
        realization: SelectedProductTarget,
        root: PathBuf,
        product: CompilationProduct,
        permissions: Option<TerminalPermissionAcceptance>,
    },
    /// Review and dependency demands already own installed checked custody.
    /// They publish a component without selecting or installing it again.
    Component(CheckedCompilation),
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
    Component(ComponentPublication),
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
            Self::Component(checked) => {
                let component = ComponentPublication::prepare(checked)?;
                return Ok(CompilationWork::Terminal(TerminalCompilation::Component(
                    component,
                )));
            }
        };
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
            Ok(CompilationWork::Terminal(TerminalCompilation::Requested(
                RequestedTerminalCompilation {
                    realization,
                    report,
                    permissions,
                },
            )))
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
        match self {
            Self::Requested(product) => product.terminal_request(),
            Self::Component(product) => product.terminal_request(),
        }
    }

    pub(crate) fn finish(
        self,
        produced: TerminalApplication,
    ) -> Result<CompilationOutput, CompilationProductError> {
        match self {
            Self::Requested(product) => product.finish(produced),
            Self::Component(product) => product
                .finish(produced)
                .map(CompilationOutput::Component)
                .map_err(CompilationProductError::Production),
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
                    .then_some(self.realization.independent_component_products.as_slice()),
            ),
            selections: self.report.optimizations(),
        }
    }

    pub(crate) fn finish(
        self,
        produced: TerminalApplication,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let checked =
            phase_profile::scope("06.0.a_realization_install", || self.realization.install())?;
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
                settlement,
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
                    let components = native_inputs::independent_components(&checked, &terminal)?;
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
                    native_production::compile(terminal, request)
                        .map_err(|(_, diagnostics)| diagnostics)
                })?;
                phase_profile::scope(
                    "06.0.f_settlement_finish_native",
                    || -> Result<_, CompilationProductError> {
                        Ok(settlement
                            .with_timings(checked.timings().phases().to_vec())
                            .finish_native(produced.artifact, rollback.into_receipt())?
                            .with_runtime_provisions(produced.runtime_provisions)
                            .map_err(|message| vec![Diagnostic::error(message)])?)
                    },
                )?
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

/*
We plan each requested compilation stop (`Check`, `Terminal`, `Native`,
`Inspection`, or `Component`) and finish its admitted Terminal Psi product.

The compiler owns batch ordering and shared Terminal production; this module
finishes non-native stops directly and packages native work into a source-free
`PendingNativeProduct` for application Build completion. Permission and trust
admission run after selected execution settlement, ensuring that planning alone
never grants authority to execute or publish.
*/

/*
When a non-executing `Check` request still owes scalar proof conditions
(`has_proof_obligations`), `CheckingCompilation` joins the shared portable
Terminal batch under `TerminalProductUse::Declarations` to evaluate target-local
scalar conditions and task activations without requiring a `ProgramEntry` or
native permissions.

When a native or component product is requested, we project and validate the
checked package's accepted Terminal authority permission rows
(`accepted_terminal_authority_permission_policy`) and optionally capture
deterministic native derivation inputs before preparing the source-free
`RetainedTerminalArtifact`.
*/

mod component;
mod inspection;

use crate::execution_settlement::SelectedProductTarget;
use crate::terminal::{
    ProducedTerminalProduct, TerminalProductRequest, TerminalProductUse, TerminalProductionView,
};
use crate::{CheckedCompilation, CompileReport};
use compilation_products::{
    CheckedProductReport, NativeProductRequest, ProductReportPlan, ProductReportRequest,
};
use component::ComponentPublication;
use diagnostics::Diagnostic;
use native_compiler::TerminalApplication;
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
    Component {
        checked: CheckedCompilation,
        retain_native_inputs: bool,
    },
}

pub(crate) enum CompilationWork {
    Complete(CompilationOutput),
    Terminal(TerminalCompilation),
}

pub(crate) enum TerminalCompilation {
    Requested(RequestedTerminalCompilation),
    Checking(CheckingCompilation),
    Component(ComponentPublication),
    Inspection(inspection::InspectionCompilation),
}

pub(crate) struct RequestedTerminalCompilation {
    realization: SelectedProductTarget,
    report: ProductReportPlan,
    permissions: Option<TerminalPermissionAcceptance>,
}

pub(crate) struct CheckingCompilation {
    realization: SelectedProductTarget,
    report: ProductReportPlan,
    permissions: Option<TerminalPermissionAcceptance>,
    selections: OptimizationSelections,
}

impl CheckingCompilation {
    pub(crate) fn new(
        realization: SelectedProductTarget,
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
            report,
            permissions,
            selections,
        }
    }

    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest {
            view: TerminalProductionView::selection(&mut self.realization.custody, None),
            selections: &self.selections,
            use_kind: TerminalProductUse::Declarations,
        }
    }

    pub(crate) fn finish(
        mut self,
        produced: ProducedTerminalProduct,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let completed = produced
            .check_conditions(&mut self.realization.custody)
            .and_then(|()| {
                self.realization
                    .check_declaration_requirements(Some(produced.module()))
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
        let checked = self.realization.into_checked_compilation();
        let accounting = checked.source_completion_accounting().clone();
        let report = (|| {
            admit_permissions(&checked, &runtime_policy, self.permissions.as_ref(), false)?;
            checked
                .admit_trust()
                .map_err(CompilationProductError::TrustAdmission)?;
            match self.report.prepare(&checked)? {
                CheckedProductReport::Complete(report) => Ok(CompilationOutput::Report(report)),
                _ => Err(vec![Diagnostic::error(
                    "declaration checking unexpectedly requested executable production",
                )]
                .into()),
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
            if realization.custody.execution.has_proof_obligations {
                return Ok(CompilationWork::Terminal(TerminalCompilation::Checking(
                    CheckingCompilation::new(realization, report, permissions),
                )));
            }
            let runtime_policy = std::sync::Arc::clone(&realization.runtime_policy);
            let checked = realization.finish_check()?;
            admit_permissions(&checked, &runtime_policy, permissions.as_ref(), false)?;
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
            use_kind: TerminalProductUse::Application,
        }
    }

    pub(crate) fn finish(
        self,
        produced: TerminalApplication,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let runtime_policy = std::sync::Arc::clone(&self.realization.runtime_policy);
        let checked = self.realization.into_checked_compilation();
        (|| -> Result<_, CompilationProductError> {
            admit_permissions(&checked, &runtime_policy, self.permissions.as_ref(), true)?;
            checked
                .admit_trust()
                .map_err(CompilationProductError::TrustAdmission)
        })()?;
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
                let native_request =
                    checked.terminal_native_proposal_request(rollback.effective(), &[])?;
                let captured_inputs = request.retain_native_inputs.then(|| {
                    let permissions = accepted_terminal_authority_permission_policy(&checked)?;
                    let bytes = native_compiler::retain_native_derivation_inputs(
                        &produced.receipt,
                        produced.physical_entry_adapter,
                        &produced.provider_proposals,
                        &native_request,
                        &permissions,
                        &request.installation_assumptions,
                    )
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
                    Ok::<_, Vec<Diagnostic>>((bytes, permissions))
                });
                let terminal = produced.prepare(native_request)?;
                let inputs = match captured_inputs {
                    Some(captured) => {
                        captured.map(|(bytes, permissions)| (Some(bytes), permissions))
                    }
                    None => accepted_terminal_authority_permission_policy(&checked)
                        .map(|permissions| (None, permissions)),
                };
                let (retained_native_inputs, accepted_package_permissions) =
                    inputs.map_err(|diagnostics| CompilationProductError::ProductionAttempt {
                        diagnostics,
                        accounting: std::mem::take(settlement.source_completion_accounting_mut()),
                    })?;
                let subsystem = checked.build().config.settings.subsystem;
                let settlement = settlement.with_timings(checked.timings().phases().to_vec());
                return Ok(CompilationOutput::Native(Box::new(
                    compilation_products::PendingNativeProduct {
                        terminal,
                        retained_native_inputs,
                        components: checked.into_independent_components(),
                        accepted_package_permissions,
                        request,
                        rollback,
                        subsystem,
                        settlement,
                        tests: Vec::new(),
                    },
                )));
            }
        };
        Ok(CompilationOutput::Report(report))
    }
}

pub(crate) fn accepted_terminal_authority_permission_policy(
    checked: &CheckedCompilation,
) -> Result<native_compiler::TerminalAuthorityPermissionPolicy, Vec<Diagnostic>> {
    let (permissions, permission_identities) = checked.execution().terminal_authority_permissions();
    native_compiler::admit_package_permissions(permissions, permission_identities)
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

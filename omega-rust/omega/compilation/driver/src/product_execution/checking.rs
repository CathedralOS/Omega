//! Finish a nonexecuting product whose source still owes scalar conditions.
//!
//! Declaration production joins the same portable batch as other products, but
//! needs no ProgramEntry or native permissions. Only scalar evaluation is
//! target-local; task checking, permission admission and reporting follow it.

use super::{CompilationOutput, CompilationProductError, admit_permissions};
use crate::execution_settlement::SelectedProductTarget;
use crate::terminal::{
    ProducedTerminalProduct, TerminalProductRequest, TerminalProductUse, TerminalProductionView,
};
use compilation_products::{CheckedProductReport, ProductReportPlan};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;
use package_evidence::permission_acceptance::TerminalPermissionAcceptance;

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
        Self {
            realization,
            report,
            permissions,
            selections: OptimizationSelections::default(),
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
            .and_then(|()| self.realization.check_declaration_requirements());
        if let Err(diagnostics) = completed {
            return Err(CompilationProductError::ProductionAttempt {
                diagnostics,
                accounting: std::mem::take(
                    &mut self.realization.custody.source_completion_accounting,
                ),
            });
        }
        let checked = self.realization.into_checked_compilation();
        let accounting = checked.source_completion_accounting().clone();
        let report = (|| {
            admit_permissions(&checked, self.permissions.as_ref(), false)?;
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

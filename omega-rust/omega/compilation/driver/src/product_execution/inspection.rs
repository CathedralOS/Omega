//! Complete a named, nonexecuting inspection from the shared Terminal batch.
//!
//! Inspection preserves declaration conditions and task obligations but does not
//! require a ProgramEntry or acquire native permissions. Exact source-issued
//! Build companions remain available to explicit inspection execution consumers;
//! they cannot be reconstructed from externally loaded artifact bytes.

use super::{CompilationOutput, CompilationProductError};
use crate::execution_settlement::SelectedProductTarget;
use crate::terminal::{
    ProducedTerminalProduct, TerminalProductRequest, TerminalProductUse, TerminalProductionView,
};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;

pub(crate) struct InspectionCompilation {
    realization: SelectedProductTarget,
    machine: String,
    selections: OptimizationSelections,
}

impl InspectionCompilation {
    pub(crate) fn new(realization: SelectedProductTarget, machine: String) -> Self {
        Self {
            realization,
            machine,
            selections: OptimizationSelections::default(),
        }
    }

    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest {
            view: TerminalProductionView::selection(&mut self.realization.custody, None),
            selections: &self.selections,
            use_kind: TerminalProductUse::Inspection(&self.machine),
        }
    }

    pub(crate) fn finish(
        mut self,
        produced: ProducedTerminalProduct,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let completed = (|| {
            if self
                .realization
                .custody
                .execution
                .program
                .facts
                .proof_applications
                .has_obligations
            {
                produced.check_conditions(&mut self.realization.custody)?;
            }
            self.realization.check_declaration_requirements()?;
            match produced {
                ProducedTerminalProduct::Descriptive { program, .. } => {
                    Ok(CompilationOutput::Inspection(program))
                }
                _ => Err(vec![Diagnostic::error(
                    "inspection lost its source-issued Terminal production",
                )]),
            }
        })();
        completed.map_err(|diagnostics| CompilationProductError::ProductionAttempt {
            diagnostics,
            accounting: self.realization.custody.source_completion_accounting,
        })
    }
}

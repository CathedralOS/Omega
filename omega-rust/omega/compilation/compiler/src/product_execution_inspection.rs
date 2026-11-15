// We complete a named, non-executing machine inspection (`InspectionCompilation`)
// from the shared Terminal batch. Inspection evaluates declaration proof
// conditions and task obligations without requiring a `ProgramEntry` or native
// permissions, preserving source-issued Build companions for explicit inspection
// execution consumers.

use super::{CompilationOutput, CompilationProductError};
use crate::execution_settlement::SelectedProductTarget;
use crate::terminal::{ProducedTerminalProduct, TerminalProductRequest};
use diagnostics::Diagnostic;
use optimization_core::OptimizationSelections;

pub(crate) struct InspectionCompilation {
    realization: SelectedProductTarget,
    selections: OptimizationSelections,
}

impl InspectionCompilation {
    pub(crate) fn new(realization: SelectedProductTarget) -> Self {
        let selections = realization
            .custody
            .terminal_publication
            .optimizations()
            .clone();
        Self {
            realization,
            selections,
        }
    }

    pub(crate) fn source_completion_accounting(
        &self,
    ) -> &build_time_evaluation::SourceCompletionAccounting {
        &self.realization.custody.source_completion_accounting
    }

    pub(crate) fn terminal_request(&mut self) -> TerminalProductRequest<'_> {
        TerminalProductRequest::new(&mut self.realization.custody, &self.selections)
    }

    pub(crate) fn finish(
        mut self,
        produced: ProducedTerminalProduct,
    ) -> Result<CompilationOutput, CompilationProductError> {
        let completed = (|| {
            produced.check_conditions(&mut self.realization.custody)?;
            self.realization
                .check_declaration_requirements(produced.module())?;
            Ok::<_, Vec<Diagnostic>>(CompilationOutput::Inspection(produced.program))
        })();
        completed.map_err(|diagnostics| CompilationProductError::ProductionAttempt {
            diagnostics,
            accounting: self.realization.custody.source_completion_accounting,
        })
    }
}

//! A safe calculation may depend on proving that an earlier helper left its
//! operands unchanged. Retain unresolved arithmetic obligations until the
//! completed storage history can prove that preservation. Every obligation
//! must succeed before checked publication.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// This is an obligation, not permission to execute the operation. Early
/// validation may continue conditionally, but publication must discharge every
/// entry, including a child whose enclosing expression cancels to zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingIntegerFormation {
    pub(super) machine: SymbolHandle,
    pub(super) state: SymbolHandle,
    pub(super) expression: ExpressionHandle,
    pub(super) other_conditions_proven: bool,
    pub(super) diagnostic: String,
}

pub(crate) fn discharge(
    program: &dyn crate::CheckingInput,
    pending: &[PendingIntegerFormation],
) -> Result<Vec<ExpressionHandle>, Vec<Diagnostic>> {
    let mut proven = Vec::new();
    let mut diagnostics = Vec::new();
    for obligation in pending {
        let fits = program.machine(obligation.machine).is_some_and(|machine| {
            program
                .state_in_machine(machine, obligation.state)
                .is_some_and(|state| {
                    super::super::contract_entailment::exact_binary_formation_at_occurrence(
                        program,
                        machine,
                        state,
                        obligation.expression,
                    )
                    .proven
                })
        });
        if !fits {
            let mut diagnostic = Diagnostic::error(obligation.diagnostic.clone());
            let expression_span = program.expression_table.source_span(obligation.expression);
            diagnostic.source_span = (expression_span != source::SourceSpan::default())
                .then_some(expression_span)
                .or_else(|| {
                    program
                        .symbols
                        .symbol_provenance_source_span(obligation.machine)
                });
            diagnostics.push(diagnostic);
        } else if obligation.other_conditions_proven {
            proven.push(obligation.expression);
        }
    }
    if diagnostics.is_empty() {
        Ok(proven)
    } else {
        Err(diagnostics)
    }
}

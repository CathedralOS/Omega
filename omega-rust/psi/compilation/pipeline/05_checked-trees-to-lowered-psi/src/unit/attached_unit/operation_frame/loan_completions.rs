//! Statement calls use the same normal-return custody join as computed calls.
use super::OperationFrame;
use crate::emission::operation_emission::buffer::SourceCallCoordinate;
use crate::lowering_error::LoweringError;

impl OperationFrame<'_, '_> {
    pub(super) fn publish_loan_completions(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        call: &checked_trees::CheckedUnitCallPlan,
    ) -> Result<(), LoweringError> {
        let coordinate = SourceCallCoordinate {
            state: self.state,
            statement_index: call.coordinate.statement_index as usize,
            call_ordinal: call.coordinate.call_ordinal as usize,
        };
        let (_, state) = crate::declarations::state_declaration(self.checked, self.state)?;
        let prepared = crate::emission::loan_completions::prepare(
            identities,
            self.checked,
            self.machine,
            coordinate,
            self.structural_types.declarations(),
            |root| {
                crate::expression_preparation::bindings::owned_structural_home(
                    root,
                    self.checked.state_parameters(state),
                    &self.evaluation.structural_parameters,
                    &self.evaluation.structural_locals,
                )
            },
        )?;
        crate::emission::loan_completions::emit(
            &prepared,
            coordinate,
            self.operations,
            self.calls,
            &mut self.evaluation.current_claims,
        )
    }
}

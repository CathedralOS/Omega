//! A completed initializer transfers into one stable local home. The ordinary
//! owned binding carries its claims and memberships; it does not open a
//! borrowed restoration window over the new home's whole value.

use super::super::structural_values;
use super::OperationFrame;
use crate::unit::LoweringError;

impl OperationFrame<'_, '_> {
    pub(super) fn initialize_owned_local(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        result: &checked_trees::CheckedUnitStructuralResultBindingPlan,
        initial_value: &checked_trees::CheckedStructuralLocalInitialization,
        discard_on_return: bool,
    ) -> Result<(), LoweringError> {
        let source = self
            .results
            .value_storage(initial_value.source_binding_ordinal(), self.operations)?;
        let (mut environment, emission) = self.reborrow().split();
        let structural_values::ValueOutcome::Normal(completed) =
            structural_values::receive_completed(
                identities,
                initial_value,
                result,
                source,
                &mut environment,
                emission.calls,
                emission.evaluation,
                emission.values,
                emission.next_value,
                emission.next_block,
                emission.next_edge,
                emission.operations,
            )?
        else {
            return Ok(());
        };
        emission.evaluation.establish_structural_binding(
            result,
            completed.binding,
            environment.structural_types,
            emission.operations,
        )?;
        environment.results.publish_value_binding(
            result,
            completed.declaration,
            discard_on_return,
            emission.operations,
        )
    }
}

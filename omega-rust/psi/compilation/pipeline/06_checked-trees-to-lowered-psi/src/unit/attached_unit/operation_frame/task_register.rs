//! Task instructions consume the same completed borrows as ordinary calls.

use super::{CallInputs, OperationFrame};
use crate::unit::{
    LoweringError, Operation, OperationResult, ValueDeclaration, allocate_dense, lookup_service_id,
    terminal_scalar_type, value_id,
};
use checked_trees::CheckedUnitCallPlan;
use language_core::task_register::TaskRegisterOperation;

impl OperationFrame<'_, '_> {
    pub(super) fn task_register(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        call: &CheckedUnitCallPlan,
        action: TaskRegisterOperation,
        service: language_semantics::ServiceReachId,
        result_type: typed_trees::types::PrimitiveType,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
        let earlier = self
            .results
            .earlier(&call.structural_arguments, self.operations, self.evaluation)?
            .into_owned();
        let arguments = self.lower_runtime_structural_arguments(
            identities,
            &call.structural_arguments,
            &earlier,
            inputs.byte_places,
            inputs.evaluated_selectors,
            inputs.completed_structural_arguments,
        )?;
        let service = lookup_service_id(self.service_ids, service)?;
        let kind = crate::emission::operation_emission::task_register::operation_kind(
            action,
            service,
            inputs.evaluated_scalar_arguments.unwrap_or_default(),
            arguments,
        )?;
        let value = ValueDeclaration {
            id: value_id(allocate_dense(self.next_value)?),
            scalar_type: terminal_scalar_type(result_type)?,
            qualifications: Default::default(),
        };
        let id = self.operations.allocate();
        self.record_source_call(call.coordinate, call.source_site(), id, call.target.machine)?;
        self.operations.push(Operation::unsuspended(
            id,
            OperationResult::Scalar(value),
            kind,
        ));
        if call.scalar_result().is_some() {
            self.bind_scalar_result(identities, call.coordinate, value)?;
        }
        Ok(())
    }
}

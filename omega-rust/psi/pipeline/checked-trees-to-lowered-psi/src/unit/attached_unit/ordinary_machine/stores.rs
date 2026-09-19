//! Stores the body performs: port writes, primitive and structural scalar
//! stores, and byte-sequence field and index writes.

use super::super::primitive_locals;
use super::{MachineEmission, StepInputs};
use crate::unit::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, LoweringError, OperationKind,
    lookup_service_id, unsupported,
};

impl MachineEmission<'_> {
    pub(super) fn port_write(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let CheckedUnitEffectOperationPlan::PortWrite {
            service_reach,
            port,
            value,
            ..
        } = operation
        else {
            unreachable!("dispatched port_write")
        };
        let direct = checked
            .facts
            .service_reaches
            .rows
            .services(service_reach.direct);
        let [port_service] = direct else {
            return unsupported(
                "port output does not carry the unique exact checked PortIo service",
            );
        };
        if !checked
            .facts
            .service_reaches
            .rows
            .services(service_reach.transitive)
            .contains(port_service)
        {
            return unsupported(
                "port output does not carry the unique exact checked PortIo service",
            );
        }
        Ok(Some(OperationKind::PortWrite {
            // `CheckedUnitEffectOperationPlan::PortWrite` is minted only for the
            // exact checked asm-port-out builtin. Its singleton direct row is
            // therefore the symbol-backed PortIo authority; no spelling lookup is
            // repeated here.
            service: lookup_service_id(self.service_ids, *port_service)?,
            port: *port,
            value: *value,
        }))
    }

    pub(super) fn write_only_primitive_store(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            path,
            value,
        } = operation
        else {
            unreachable!("dispatched write_only_primitive_store")
        };
        let destination = match destination {
            checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index } => {
                let parameter = self.parameters.get(*parameter_index as usize).ok_or(
                    LoweringError::Unsupported("primitive store parameter is absent"),
                )?;
                crate::emission::primitive_store::parameter_destination(
                    parameter,
                    path,
                    self.structural_types,
                )?
            }
            checked_trees::CheckedPrimitiveStoreDestination::Local { symbol } => {
                if !path.is_empty() {
                    return unsupported("primitive local store has a projected destination");
                }
                let local = primitive_locals::find(&self.primitive_local_places, *symbol)?;
                crate::emission::primitive_store::Destination {
                    place: local.declaration.id,
                    path: Vec::new(),
                    scalar_type: local.scalar_type,
                }
            }
        };
        let kind = crate::emission::primitive_store::emit_assignment(
            checked,
            plan.machine,
            plan.state,
            *statement_index,
            destination,
            value,
            &mut self.evaluation,
            step.source_value_count,
            &mut self.scalar_result_values,
            &mut self.next_value_identity,
            &mut self.next_block,
            &mut self.next_edge,
            &mut self.operations,
            &mut self.scalar_calls,
        )?;
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        Ok(Some(kind))
    }

    pub(super) fn structural_byte_sequence_field_store(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) = operation
        else {
            unreachable!("dispatched structural_byte_sequence_field_store")
        };
        crate::emission::structural_byte_sequence_store::literal_view_type(self.structural_types)?;
        Ok(Some(crate::emission::structural_byte_sequence_store::emit(
            store,
            self.parameters,
            self.structural_types,
            &mut self.literal_places,
            &mut self.next_place,
            &mut self.next_value_identity,
            &mut self.next_call_obligation,
            &mut self.operations,
        )?))
    }

    pub(super) fn structural_byte_sequence_field_byte_store(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) = operation
        else {
            unreachable!("dispatched structural_byte_sequence_field_byte_store")
        };
        let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
            self.scalar_result_values.len(),
        )
        .with_primitive_storage(&self.evaluation.primitive_storage)
        .with_structural_parameters(&self.evaluation.structural_parameters)
        .with_resolved_structural_observations(
            &self.evaluation.structural_fields,
            &self.evaluation.structural_cases,
        );
        let index = bindings.expression_at(
            checked,
            plan.state,
            store.statement_index,
            CheckedScalarExpressionRole::AssignmentIndex,
        )?;
        let value = crate::emission::byte_store_scalar_value(
            &bindings,
            checked,
            plan.state,
            store.statement_index,
            &store.value,
            &self.scalar_result_values,
        )?;
        Ok(Some(
            crate::emission::structural_byte_sequence_index_store::emit(
                store,
                self.parameters,
                self.structural_types,
                &index,
                &value,
                &self.scalar_result_values,
                &mut self.next_value_identity,
                &mut self.next_call_obligation,
                &mut self.operations,
            )?,
        ))
    }

    pub(super) fn byte_sequence_write(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) = operation else {
            unreachable!("dispatched byte_sequence_write")
        };
        let bindings = crate::expression_preparation::bindings::ScalarBindings::new(
            self.scalar_result_values.len(),
        )
        .with_primitive_storage(&self.evaluation.primitive_storage)
        .with_structural_parameters(&self.evaluation.structural_parameters);
        let index = bindings.expression_at(
            checked,
            plan.state,
            write.statement_index,
            CheckedScalarExpressionRole::AssignmentIndex,
        )?;
        let value = crate::emission::byte_store_scalar_value(
            &bindings,
            checked,
            plan.state,
            write.statement_index,
            &write.value,
            &self.scalar_result_values,
        )?;
        Ok(Some(crate::emission::byte_sequence_write::emit(
            write,
            self.parameters,
            self.structural_types,
            &index,
            &value,
            &self.scalar_result_values,
            &mut self.next_value_identity,
            &mut self.next_call_obligation,
            &mut self.operations,
        )?))
    }

    pub(super) fn structural_scalar_field_store(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) = operation else {
            unreachable!("dispatched structural_scalar_field_store")
        };
        let destination = self
            .parameters
            .iter()
            .find(|parameter| Some(parameter.position) == store.destination.parameter_position())
            .ok_or(LoweringError::Unsupported(
                "structural scalar store names an unknown parameter",
            ))?;
        let lowered =
            crate::emission::structural_scalar_store::lower_structural_scalar_store_place(
                store,
                store.statement_index,
                destination,
                self.structural_types,
                crate::emission::structural_scalar_store::StoreAccessPolicy::Exclusive,
            )?;
        let value = self.evaluation.field_assignment_value(
            checked,
            plan.machine,
            plan.state,
            store,
            &mut self.scalar_result_values,
            &mut self.next_value_identity,
            &mut self.next_block,
            &mut self.next_edge,
            &mut self.operations,
            &mut self.scalar_calls,
        )?;
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        if value.scalar_type != lowered.scalar_type {
            return unsupported("structural scalar store RHS differs from its field type");
        }
        Ok(Some(lowered.into_operation(
            destination.place,
            value.id,
            &mut self.next_call_obligation,
        )?))
    }
}

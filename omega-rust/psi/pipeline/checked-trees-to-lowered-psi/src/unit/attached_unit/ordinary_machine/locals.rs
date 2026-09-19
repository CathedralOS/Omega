//! Places and values a body establishes for itself: structural values,
//! primitive and trivial affine locals, references, scalar arrays and
//! scalar locals.

use super::super::{
    ordinary_calls, parameters, primitive_locals, reference_results, scalar_arrays, signatures,
    structural_values,
};
use super::{MachineEmission, StepInputs};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::unit::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, LoweringError, OperationKind,
    StructuralPlaceKind, StructuralTypeShape, ValueDeclaration, lookup_machine_id, lookup_type_id,
    terminal_scalar_type, unsupported,
};

impl MachineEmission<'_> {
    pub(super) fn establish_structural_value(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let plans = self.plans;
        let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
            result,
            discard_result_on_return,
            ..
        } = operation
        else {
            unreachable!("dispatched establish_structural_value")
        };
        // The operand closure holds an immutable view of the caller's scalar
        // namespace while the emitter mutates `scalar_result_values` around it.
        let operand_scalar_values = self.scalar_result_values.clone();
        let mut emit_operand_call = |operand: &CheckedUnitEffectOperationPlan,
                                     evaluated: Option<&[ValueDeclaration]>,
                                     call_context: &mut CallEmissionContext<'_>,
                                     output: &mut OperationBuffer,
                                     place_counter: &mut u64| {
            let CheckedUnitEffectOperationPlan::StructuralCall {
                target_machine,
                result,
                ..
            } = operand
            else {
                return unsupported("record operand is not a structural call");
            };
            if result.binding_ordinal as usize != self.structural_result_places.len() {
                return unsupported("structural operand result binding is not dense");
            }
            let prepared = ordinary_calls::prepare(
                checked,
                plans,
                operand,
                signatures::find(self.machine_signatures, *target_machine)?.call_target(),
                evaluated,
                &operand_scalar_values,
                &signatures::find(self.machine_signatures, plan.machine)?.erased_scalar_parameters,
                self.parameters,
                &self.local_places,
                &self.structural_result_places,
                &self.primitive_local_places,
                self.type_ids,
                self.structural_types,
                &[],
                call_context,
            )?;
            let declaration = ordinary_calls::emit_structural(
                checked,
                plan.state,
                operand,
                prepared,
                lookup_machine_id(self.machine_ids, *target_machine)?,
                self.type_ids,
                self.domain_ids,
                &self.claim_bindings,
                true,
                place_counter,
                output,
            )?;
            self.structural_result_places.push((declaration, false));
            Ok(declaration)
        };
        let declaration = structural_values::emit(
            checked,
            plan.machine,
            plan.state,
            operation,
            self.structural_types,
            self.type_ids,
            &mut self.next_place,
            &mut self.structural_value_temporaries,
            &mut emit_operand_call,
            &mut self.scalar_calls,
            &mut self.evaluation,
            &mut self.scalar_result_values,
            &mut self.next_value_identity,
            &mut self.next_block,
            &mut self.next_edge,
            &mut self.operations,
        )?;
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        if result.binding_ordinal as usize != self.structural_result_places.len() {
            return unsupported("structural value result binding is not dense");
        }
        self.structural_result_places
            .push((declaration, *discard_result_on_return));
        Ok(None)
    }

    pub(super) fn establish_primitive_local(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            statement_index,
            symbol,
            type_identity,
            primitive_type,
            ..
        } = operation
        else {
            unreachable!("dispatched establish_primitive_local")
        };
        if self
            .primitive_local_places
            .iter()
            .any(|local| local.symbol == *symbol)
        {
            return unsupported("primitive local is established more than once");
        }
        let value =
            crate::expression_preparation::bindings::ScalarBindings::new(step.source_value_count)
                .with_primitive_storage(&self.evaluation.primitive_storage)
                .expression_at(
                    checked,
                    plan.state,
                    *statement_index,
                    CheckedScalarExpressionRole::StorageInitializer,
                )?;
        let structural_type = lookup_type_id(self.type_ids, type_identity)?;
        if value.scalar_type() != terminal_scalar_type(*primitive_type)?
            || !self.structural_types.iter().any(|declaration| {
                declaration.id == structural_type
                    && declaration.shape
                        == StructuralTypeShape::PrimitiveScalar(value.scalar_type())
            })
        {
            return unsupported("primitive local initializer and referent types disagree");
        }
        let local = primitive_locals::emit(
            *symbol,
            structural_type,
            &value,
            &self.scalar_result_values,
            &mut self.next_place,
            &mut self.next_value_identity,
            &mut self.operations,
        )?;
        self.evaluation.primitive_storage.push((
            local.symbol,
            local.declaration.id,
            local.scalar_type,
        ));
        self.primitive_local_places.push(local);
        Ok(None)
    }

    pub(super) fn establish_reference(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishReference { result, source } = operation
        else {
            unreachable!("dispatched establish_reference")
        };
        if result.binding_ordinal as usize != self.structural_result_places.len() {
            return unsupported("reference result binding is not dense");
        }
        let arguments = parameters::lower_structural_arguments(
            std::slice::from_ref(source),
            self.parameters,
            &self.local_places,
            &self.structural_result_places,
            &[],
            &self.primitive_local_places,
        )?;
        let declaration = reference_results::emit(
            result,
            arguments[0].clone(),
            self.type_ids,
            &mut self.next_place,
            &mut self.operations,
        )?;
        self.structural_result_places.push((declaration, false));
        Ok(None)
    }

    pub(super) fn release_reference(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            binding_ordinal, ..
        } = operation
        else {
            unreachable!("dispatched release_reference")
        };
        let source = self
            .structural_result_places
            .get(*binding_ordinal as usize)
            .ok_or(LoweringError::Unsupported(
                "released reference result is absent",
            ))?
            .0
            .id;
        Ok(Some(OperationKind::ReleaseReference {
            source: self.evaluation.current_structural_place(source),
        }))
    }

    pub(super) fn establish_scalar_array(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source,
            result,
            elements,
        } = operation
        else {
            unreachable!("dispatched establish_scalar_array")
        };
        if result.binding_ordinal as usize != self.structural_result_places.len() {
            return unsupported("array result binding is not dense");
        }
        // Earlier arguments are private staging slots, not source
        // bindings. Keep them live through every leaf's control
        // joins, then remove only this constructor's leaf tail.
        let leaf_start = self.scalar_result_values.len();
        let source_value_count = self.retained_scalar_prefix.unwrap_or(leaf_start);
        for (ordinal, element) in elements.iter().enumerate() {
            let element_ordinal = u32::try_from(ordinal)
                .map_err(|_| LoweringError::Unsupported("array element ordinal exceeds u32"))?;
            let value = self.evaluation.source_value(
                checked,
                plan.machine,
                plan.state,
                result.statement_index,
                CheckedScalarExpressionRole::ArrayElement {
                    source: *source,
                    element_ordinal,
                },
                element,
                source_value_count,
                &mut self.scalar_result_values,
                &mut self.next_value_identity,
                &mut self.next_block,
                &mut self.next_edge,
                &mut self.operations,
                &mut self.scalar_calls,
            )?;
            self.scalar_result_values.push(value);
        }
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        // Private control joins may replace every scalar identity.
        // Read the completed leaves only after the final element.
        let declaration = scalar_arrays::emit(
            result,
            &self.scalar_result_values[leaf_start..],
            self.type_ids,
            &mut self.next_place,
            &mut self.operations,
        )?;
        self.scalar_result_values.truncate(leaf_start);
        self.structural_result_places.push((declaration, false));
        if *source == checked_trees::CheckedArrayConstructionSource::Statement {
            structural_values::bind_local(
                checked,
                plan,
                operation,
                declaration.id,
                &mut self.evaluation.structural_locals,
            )?;
        }
        Ok(None)
    }

    pub(super) fn establish_trivial_affine_local(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            declaration_ordinal,
            type_identity,
            ..
        } = operation
        else {
            unreachable!("dispatched establish_trivial_affine_local")
        };
        let local = self
            .local_places
            .get(
                usize::try_from(*declaration_ordinal)
                    .map_err(|_| LoweringError::Unsupported("Unit local ordinal exceeds usize"))?,
            )
            .ok_or(LoweringError::Unsupported(
                "Unit local ordinal is not dense",
            ))?;
        if !matches!(
            local.kind,
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal: ordinal,
                structural_type,
                ..
            } if ordinal == *declaration_ordinal
                && structural_type == lookup_type_id(self.type_ids, type_identity)?
        ) {
            return unsupported("Unit local declaration drifted from checked custody");
        }
        Ok(Some(OperationKind::EstablishTrivialAffineLocal {
            destination: local.id,
        }))
    }

    pub(super) fn establish_scalar_local(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        step: &StepInputs,
    ) -> Result<Option<OperationKind>, LoweringError> {
        let checked = self.checked;
        let plan = self.plan;
        let CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value } = operation
        else {
            unreachable!("dispatched establish_scalar_local")
        };
        if usize::try_from(result.binding_ordinal)
            .ok()
            .and_then(|ordinal| ordinal.checked_add(self.scalar_parameter_count))
            != Some(self.scalar_result_values.len())
        {
            return unsupported("Unit scalar expression local binding drifted from source order");
        }
        let role = if plan.scalar_result.as_ref() == Some(result) {
            CheckedScalarExpressionRole::Return
        } else {
            CheckedScalarExpressionRole::LocalInitializer {
                binding_ordinal: result.binding_ordinal,
            }
        };
        let lowered = self.evaluation.source_value(
            checked,
            plan.machine,
            plan.state,
            result.statement_index,
            role,
            value,
            step.source_value_count,
            &mut self.scalar_result_values,
            &mut self.next_value_identity,
            &mut self.next_block,
            &mut self.next_edge,
            &mut self.operations,
            &mut self.scalar_calls,
        )?;
        if lowered.scalar_type != terminal_scalar_type(result.primitive_type)? {
            return unsupported("Unit scalar expression local type disagrees with its binding");
        }
        self.next_call_obligation = self.scalar_calls.next_obligation_identity;
        self.scalar_result_values.push(lowered);
        Ok(None)
    }
}

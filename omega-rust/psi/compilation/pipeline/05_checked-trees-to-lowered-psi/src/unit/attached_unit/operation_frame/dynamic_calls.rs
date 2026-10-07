//! Selector calls (`Trait::member(selector, data...)`): one `CallDynamic`
//! naming the interface slot, with the selector as its dispatch operand.
//!
//! The route evaluates the selector as the call's first scalar operand, then
//! the data operands, so `evaluated_scalar_arguments` arrives as
//! `[selector, data...]`. Structural arguments and claim transfers lower
//! exactly as an ordinary call's do: every selectable implementation is a
//! checked body that receives them under checked-body custody.

use super::super::parameters::{emitted_claim_transfers, lower_structural_arguments};
use super::OperationFrame;
use super::calls::CallInputs;
use crate::unit::{
    CheckedUnitEffectOperationPlan, LoweringError, Operation, OperationKind, OperationResult,
    ValueDeclaration, allocate_dense, terminal_scalar_type, unsupported, value_id,
};
use checked_trees::{CheckedUnitCallResult, CheckedUnitCallRoute};

impl OperationFrame<'_, '_> {
    pub(super) fn dynamic_call(
        &mut self,
        operation: &CheckedUnitEffectOperationPlan,
        inputs: &CallInputs<'_>,
    ) -> Result<Option<ValueDeclaration>, LoweringError> {
        let Some(call) = operation.call() else {
            return unsupported("selector call custody drifted before emission");
        };
        let CheckedUnitCallRoute::Dynamic {
            interface_identity, ..
        } = call.route()
        else {
            return unsupported("selector call lost its dispatch route");
        };
        if inputs.staged {
            return unsupported("a staged argument group cannot yet carry a selector call");
        }
        let Some((selector, data)) = inputs
            .evaluated_scalar_arguments
            .and_then(|evaluated| evaluated.split_first())
        else {
            return unsupported("selector call has no evaluated selector operand");
        };
        let checked = self.checked;
        let Some(slot_plan) = checked
            .facts
            .flow
            .terminal_unit_effects
            .dynamic_slots
            .iter()
            .find(|plan| plan.slot == call.target.machine)
        else {
            return unsupported("selector call names a slot with no checked call shape");
        };
        // The data operands match the slot's scalar formals after the selector.
        let data_types = slot_plan
            .scalar_parameters
            .get(1..)
            .unwrap_or_default()
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        if data.len() != data_types.len()
            || data
                .iter()
                .zip(&data_types)
                .any(|(value, expected)| value.scalar_type != *expected)
        {
            return unsupported("selector call operands disagree with its slot");
        }
        let dispatch = &self.operations.dynamic_dispatch;
        let interface =
            dispatch
                .interface(interface_identity)
                .ok_or(LoweringError::Unsupported(
                    "selector call names an interface no implementation is selected for",
                ))?;
        let slot = dispatch
            .slot(interface, &slot_plan.requirement_identity)
            .ok_or(LoweringError::Unsupported(
                "selector call names a requirement absent from its interface",
            ))?;
        let structural_arguments = &call.structural_arguments;
        let earlier =
            self.results
                .earlier(structural_arguments, self.operations, self.evaluation)?;
        let mut selector_values = super::super::parameters::mint_runtime_index_selectors(
            structural_arguments,
            &self.evaluation.structural_fields,
            self.values,
            self.next_value,
            self.operations,
        )?
        .into_iter();
        let structural_arguments = lower_structural_arguments(
            structural_arguments,
            self.structural_types.declarations(),
            self.parameters,
            &[],
            &earlier,
            &self.operations.structural_values.clone(),
            inputs.byte_places,
            self.primitive_locals,
            &self.evaluation.structural_locals,
            Some(self.values),
            &mut selector_values,
            Some(&mut *self.calls),
            Some((self.next_value, self.operations)),
        )?;
        drop(earlier);
        let claim_transfers =
            emitted_claim_transfers(call.claim_transfers(), self.calls.claims.bindings())?;
        let result = match call.result() {
            CheckedUnitCallResult::Unit => None,
            CheckedUnitCallResult::Scalar(result) => Some(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(self.next_value)?),
                scalar_type: terminal_scalar_type(result.primitive_type)?,
            }),
            CheckedUnitCallResult::Structural { .. } => {
                return unsupported("a selector call's structural result is not yet planned");
            }
        };
        let id = self.operations.allocate();
        self.record_source_call(call.coordinate, None, id, call.target.machine)?;
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: result.map_or(OperationResult::Unit, OperationResult::Scalar),
            kind: OperationKind::CallDynamic {
                selector: selector.id,
                interface,
                slot,
                arguments: data.iter().map(|value| value.id).collect(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments,
                claim_transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        match result {
            Some(value) => self.bind_scalar_result(call.coordinate, value, inputs.staged),
            None => Ok(None),
        }
    }
}

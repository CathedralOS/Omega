//! Selector calls (`Trait::member(selector, data...)`): one `CallDynamic`
//! naming the interface slot, with the selector as its dispatch operand.
//!
//! The route evaluates the selector as the call's first scalar operand, then
//! the data operands, so `evaluated_scalar_arguments` arrives as
//! `[selector, data...]`. Structural arguments and claim transfers lower
//! exactly as an ordinary call's do: every selectable implementation is a
//! checked body that receives them under checked-body custody.

use super::calls::CallInputs;
use super::{OperationFrame, StructuralResults};
use crate::unit::{
    LoweringError, Operation, OperationKind, OperationResult, ValueDeclaration, allocate_dense,
    terminal_scalar_type, unsupported, value_id,
};
use checked_trees::{CheckedUnitCallPlan, CheckedUnitCallResult};

impl OperationFrame<'_, '_> {
    pub(super) fn dynamic_call(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        operation: &checked_trees::CheckedUnitEffectOperationPlan,
        call: &CheckedUnitCallPlan,
        interface_identity: &typed_trees::type_identity::TypeIdentityId,
        inputs: &CallInputs<'_>,
    ) -> Result<(), LoweringError> {
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
        let earlier = self
            .results
            .earlier(structural_arguments, self.operations, self.evaluation)?
            .into_owned();
        let structural_arguments = self.lower_runtime_structural_arguments(
            identities,
            structural_arguments,
            &earlier,
            inputs.byte_places,
            inputs.evaluated_selectors,
        )?;
        let prepared_claims = crate::emission::claim_bindings::prepare_checked_claim_transfers(
            identities,
            self.checked,
            call.claim_transfers(),
            &slot_plan.structural_parameters,
            self.structural_types.declarations(),
        )?;
        let claim_transfers = crate::emission::claim_bindings::emit_prepared_claim_transfers(
            &prepared_claims,
            self.calls.claims.bindings(),
        )?;
        let shape = slot_plan.result.clone();
        let id = self.operations.allocate();
        let (result, declaration) = match call.result() {
            CheckedUnitCallResult::Unit => (OperationResult::Unit, None),
            CheckedUnitCallResult::Scalar(result) => (
                OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value_id(allocate_dense(self.next_value)?),
                    scalar_type: terminal_scalar_type(result.primitive_type)?,
                }),
                None,
            ),
            CheckedUnitCallResult::Structural { .. } => {
                let (returned, declaration) =
                    self.prepare_structural_call_result(identities, call, &shape, id)?;
                (OperationResult::Structural(returned), Some(declaration))
            }
        };
        // A finite-family call's row-selection actuals lower through the
        // same scalar evaluator every ordinary operand uses: the slot's base
        // position names the first tuple slot, and dispatch matches these
        // values against each family slot's declared tuple identities.
        let mut family_actuals = Vec::new();
        let mut family_actual_types = Vec::new();
        for operand in match call.route() {
            checked_trees::CheckedUnitCallRoute::Dynamic {
                family_operands, ..
            } => family_operands.as_slice(),
            _ => &[],
        } {
            let expression = match operand {
                checked_trees::CheckedCallScalarArgument::Pure(expression) => expression,
                _ => {
                    return unsupported(
                        "dynamic family operand is not a pure scalar expression",
                    )
                }
            };
            let lowered = self.evaluation.scalar_bindings.expression(
                identities,
                &checked.facts.proven_scalar_indexes,
                expression,
            )?;
            family_actual_types.push(lowered.scalar_type());
            family_actuals.push(
                crate::emission::operation_emission::expressions::emit_direct_expression(
                    &lowered,
                    self.values,
                    self.next_value,
                    self.operations,
                ),
            );
        }
        if !family_actuals.is_empty() {
            let Some(merge_result) = result.scalar() else {
                return unsupported(
                    "dynamic family dispatch does not yet cover non-scalar results",
                );
            };
            let dispatch = &self.operations.dynamic_dispatch;
            let roster: Vec<Vec<String>> = dispatch
                .interface_slots(interface)
                .ok_or(LoweringError::Unsupported(
                    "selector call names an interface with no slot roster",
                ))?
                .iter()
                .skip(slot as usize)
                .take_while(|(requirement, tuple)| {
                    *requirement == slot_plan.requirement_identity && !tuple.is_empty()
                })
                .map(|(_, tuple)| tuple.clone())
                .collect();
            let leaf_selector = selector.id;
            let leaf_arguments: Vec<_> = data.iter().map(|value| value.id).collect();
            let mut first_leaf = Some(id);
            self.record_source_call(call.coordinate, None, id, call.target.machine)?;
            let mut span = super::super::argument_evaluation::BlockEmissionSpan {
                id: self.evaluation.current,
                parameters: std::mem::take(&mut self.evaluation.parameters),
                structural_parameters: std::mem::take(
                    &mut self.evaluation.block_structural_parameters,
                ),
                operation_start: self.evaluation.operation_start,
            };
            let dispatch_result = super::super::argument_evaluation::emit_dynamic_family_dispatch(
                &mut span,
                &mut self.evaluation.blocks,
                self.operations,
                self.next_value,
                self.next_block,
                self.next_edge,
                &roster,
                &family_actuals,
                &family_actual_types,
                Some(merge_result),
                |operations, _next_value, tuple_index, produced| {
                    let operation = first_leaf
                        .take()
                        .unwrap_or_else(|| operations.allocate());
                    operations.push(Operation {
                        static_reach_binding: None,
                        suspension_crossing: None,
                        id: operation,
                        result: produced
                            .map(OperationResult::Scalar)
                            .unwrap_or(OperationResult::Unit),
                        kind: OperationKind::CallDynamic {
                            selector: leaf_selector,
                            interface,
                            slot: slot + u32::try_from(tuple_index).map_err(|_| {
                                LoweringError::Unsupported("dynamic family slot overflows")
                            })?,
                            arguments: leaf_arguments.clone(),
                            family_actuals: family_actuals.clone(),
                            erased_arguments: Vec::new(),
                            erased_proof_arguments: Vec::new(),
                            structural_arguments: structural_arguments.clone(),
                            claim_transfers: claim_transfers.clone(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    });
                    Ok(())
                },
            );
            self.evaluation.current = span.id;
            self.evaluation.parameters = span.parameters;
            self.evaluation.block_structural_parameters = span.structural_parameters;
            self.evaluation.operation_start = span.operation_start;
            dispatch_result?;
            return self.bind_scalar_result(identities, call.coordinate, merge_result);
        }
        if declaration.is_none() {
            self.record_source_call(call.coordinate, None, id, call.target.machine)?;
        }
        self.operations.push(Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id,
            result: result.clone(),
            kind: OperationKind::CallDynamic {
                selector: selector.id,
                interface,
                slot,
                arguments: data.iter().map(|value| value.id).collect(),
                family_actuals,
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments,
                claim_transfers,
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        });
        match (result, call.result(), declaration) {
            (OperationResult::Scalar(value), _, _) => {
                self.bind_scalar_result(identities, call.coordinate, value)
            }
            (
                OperationResult::Structural(returned),
                CheckedUnitCallResult::Structural {
                    binding,
                    discard_result_on_return,
                },
                Some(declaration),
            ) => {
                if matches!(self.results, StructuralResults::Dense(_)) {
                    self.operations
                        .structural_values
                        .push((binding.binding_ordinal, returned.into()));
                }
                self.publish_result(operation, binding, declaration, *discard_result_on_return)
            }
            (OperationResult::Unit, _, _) => Ok(()),
            _ => unsupported("dynamic result lost its checked publication shape"),
        }
    }
}

use crate::InstructionSelectionInput;
use omega_abstract_operations::{
    AbstractOperationKind, InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
    SelectedInstruction,
};
use omega_calling_conventions::{
    CallPlan, CallSignature, CallingPolicy, ValueClass, ValueShape, evaluate_call_plan,
};
use omega_control_flow::StateKey;
use omega_state_calls::{StateCall, StateCallDynamicReceiver, StateCallLowering};
use psi_arena::Arena;
use psi_diagnostics::Diagnostic;

use super::instruction_sink::SelectedInstructionSink;
use super::storage_places::{
    classify_scalar_value_type_in_table, resolve_runtime_storage_place_in_table,
    static_integer_value_in_table,
};

#[derive(Clone)]
pub(super) struct ValidatedDynamicCall {
    pub table_byte_offset: usize,
    pub requirement_identity: std::sync::Arc<str>,
    pub plan: CallPlan,
    pub descriptor_byte_offset: usize,
    pub result: Option<(usize, usize, ValueClass)>,
}

pub(super) fn validate_dynamic_calls(
    input: &InstructionSelectionInput<'_>,
) -> Result<(), Diagnostic> {
    validate_dynamic_result_ownership(input.runtime_bodies)?;
    for (_, call) in input.state_calls.calls.iter() {
        if call.lowering != StateCallLowering::IndirectDynamic {
            continue;
        }
        let dispatch_index = runtime_dispatch_index(input, call).ok_or_else(|| {
            Diagnostic::error("dynamic call source has no exact runtime dispatch body")
        })?;
        validated_dynamic_call(input, dispatch_index, call)?;
    }
    Ok(())
}

fn validate_dynamic_result_ownership(
    runtime_bodies: &omega_runtime_bodies::RuntimeDispatchBodyPlan,
) -> Result<(), Diagnostic> {
    use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
    for (_, body) in runtime_bodies.bodies.iter() {
        let Some(operations) = runtime_bodies.operations.paged_span(body.operations) else {
            continue;
        };
        for dynamic in operations.iter() {
            let RuntimeDispatchBodyOperationKind::DynamicStateCall {
                role,
                call_ordinal,
                target_key,
                ..
            } = dynamic.kind
            else {
                continue;
            };
            if operations.iter().any(|operation| {
                source_matches(operation.source_key, dynamic.source_key)
                    && operation.statement_index == dynamic.statement_index
                    && matches!(
                        operation.kind,
                        RuntimeDispatchBodyOperationKind::StateCallResult {
                            role: result_role,
                            call_ordinal: result_ordinal,
                            target_key: result_target,
                            ..
                        } if result_role == role
                            && result_ordinal == call_ordinal
                            && result_target == target_key
                    )
            }) {
                return Err(Diagnostic::error(
                    "indirect dynamic call retained a second direct/spliced result producer",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn select_dynamic_state_call(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
    operands: &mut Arena<InstructionOperand>,
    selected: &mut SelectedInstructionSink,
) -> bool {
    let mut matches = input.state_calls.calls.iter().filter_map(|(_, call)| {
        (source_matches(call.source_key, source_key)
            && call.statement_index == statement_index
            && call.call_ordinal == call_ordinal
            && call.lowering == StateCallLowering::IndirectDynamic)
            .then_some(call)
    });
    let Some(call) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let Ok(validated) = validated_dynamic_call(input, dispatch_index, call) else {
        return false;
    };

    let mut kinds = Vec::with_capacity(call.argument_count + 3);
    if let Some((byte_offset, byte_size, class)) = validated.result {
        kinds.push(match class {
            ValueClass::Float => InstructionOperandKind::RuntimeScalarFloat {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: byte_size,
            },
            _ => InstructionOperandKind::RuntimeScalarInteger {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset,
                byte_count: byte_size,
            },
        });
    }
    kinds.push(InstructionOperandKind::RuntimeScalarInteger {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: validated.descriptor_byte_offset + input.runtime_abi.pointer_size,
        byte_count: input.runtime_abi.pointer_size,
    });
    kinds.push(InstructionOperandKind::RuntimeScalarInteger {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: validated.descriptor_byte_offset,
        byte_count: input.runtime_abi.pointer_size,
    });

    let Some(arguments) = input.state_calls.arguments.span(call.arguments) else {
        return false;
    };
    for argument in arguments {
        let expression = argument.expression;
        if let Some(value) =
            static_integer_value_in_table(input.layouts, &input.state_calls.expressions, expression)
        {
            kinds.push(InstructionOperandKind::ImmediateInteger(value));
            continue;
        }
        let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            dispatch_index,
            call.source_key,
            &input.state_calls.expressions,
            expression,
        ) else {
            return false;
        };
        let class = classify_scalar_value_type_in_table(
            input,
            dispatch_index,
            call.source_key,
            &input.state_calls.expressions,
            expression,
        );
        kinds.push(
            if class.is_some_and(|primitive| {
                matches!(
                    primitive,
                    psi_checked_trees::types::PrimitiveType::F32
                        | psi_checked_trees::types::PrimitiveType::F64
                )
            }) {
                InstructionOperandKind::RuntimeScalarFloat {
                    region: place.region,
                    byte_offset: place.byte_offset,
                    byte_count: place.byte_count,
                }
            } else {
                InstructionOperandKind::RuntimeScalarInteger {
                    region: place.region,
                    byte_offset: place.byte_offset,
                    byte_count: place.byte_count,
                }
            },
        );
    }
    let operand_span =
        operands.insert_many(kinds.into_iter().map(|kind| InstructionOperand { kind }));
    selected.push(SelectedInstruction {
        kind: AbstractOperationKind::DynamicTableCall {
            byte_offset: validated.table_byte_offset,
            requirement_identity: validated.requirement_identity,
            result_present: validated.result.is_some(),
            call_plan: validated.plan,
            operands: operand_span,
        },
        source_key,
        source_statement: statement_index,
    });
    true
}

pub(super) fn dynamic_call_plan_for_realization(
    input: &InstructionSelectionInput<'_>,
    realization: StateKey,
) -> Result<CallPlan, Diagnostic> {
    let (parameters, result) = realization_signature(input, realization)?;
    evaluate_call_plan(
        CallingPolicy::native_for_target(input.target),
        &CallSignature { parameters, result },
    )
    .map_err(|error| {
        Diagnostic::error(format!("dynamic realization call plan is invalid: {error}"))
    })
}

fn validated_dynamic_call(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    call: &StateCall,
) -> Result<ValidatedDynamicCall, Diagnostic> {
    let dispatch = call.dynamic_dispatch.as_ref().ok_or_else(|| {
        Diagnostic::error("indirect dynamic call lost its exact checked descriptor identity")
    })?;
    let source_state = input
        .control_flow
        .state_by_key(call.source_key)
        .ok_or_else(|| Diagnostic::error("dynamic call source state is missing"))?;
    let (receiver_symbol, receiver_name, receiver_slot_kind) = match dispatch.receiver {
        StateCallDynamicReceiver::Parameter { symbol } => {
            let source_parameter = input
                .control_flow
                .state_parameters(source_state)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .ok_or_else(|| {
                    Diagnostic::error("dynamic receiver is not the exact checked source parameter")
                })?;
            if source_parameter.name != call.receiver_name
                || source_parameter.type_symbol != dispatch.target_trait
            {
                return Err(Diagnostic::error(
                    "dynamic receiver parameter identity or trait drifted",
                ));
            }
            (
                source_parameter.symbol,
                source_parameter.name.clone(),
                omega_runtime_storage::RuntimeFrameSlotKind::Parameter,
            )
        }
        StateCallDynamicReceiver::ReboundLocal {
            binding,
            selection_statement_index,
        } => {
            let facts = &input.control_flow.semantics.facts.dynamic_conformances;
            let selection = facts
                .at_statement(
                    call.source_key.machine,
                    call.source_key.state,
                    binding,
                    selection_statement_index,
                )
                .ok_or_else(|| {
                    Diagnostic::error("rebound dynamic receiver selection identity is missing")
                })?;
            let latest = facts
                .for_receiver(
                    call.source_key.machine,
                    call.source_key.state,
                    binding,
                    &selection.binding_name,
                    call.statement_index,
                )
                .ok_or_else(|| {
                    Diagnostic::error("rebound dynamic receiver has no live selection")
                })?;
            if selection.statement_index >= call.statement_index
                || latest.statement_index != selection.statement_index
                || latest.binding != binding
                || selection.binding_name != call.receiver_name
                || selection.target_trait != dispatch.target_trait
            {
                return Err(Diagnostic::error(
                    "rebound dynamic receiver selection identity drifted",
                ));
            }
            let Some(conformance) = selection.conformance else {
                return Err(Diagnostic::error(
                    "rebound dynamic receiver lost its exact conformance",
                ));
            };
            let [candidate] = dispatch.candidates.as_slice() else {
                return Err(Diagnostic::error(
                    "rebound dynamic receiver must retain exactly one table candidate",
                ));
            };
            if candidate.source_data != selection.source_data
                || candidate.conformance != conformance
                || candidate.rows != selection.rows
            {
                return Err(Diagnostic::error(
                    "rebound dynamic receiver candidate drifted from its checked selection",
                ));
            }
            (
                binding,
                selection.binding_name.clone(),
                omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage,
            )
        }
    };
    if dispatch.candidates.is_empty() {
        return Err(Diagnostic::error(
            "dynamic call has no retained complete table candidates",
        ));
    }
    let (parameters, result_shape) = realization_signature(input, call.target_key)?;
    if parameters.len() != call.argument_count + 1 {
        return Err(Diagnostic::error(
            "dynamic call arguments do not match the exact requirement signature",
        ));
    }
    let plan = evaluate_call_plan(
        CallingPolicy::native_for_target(input.target),
        &CallSignature {
            parameters,
            result: result_shape,
        },
    )
    .map_err(|error| Diagnostic::error(format!("dynamic call plan is invalid: {error}")))?;
    let mut common_slot = None;
    for candidate in &dispatch.candidates {
        let mut tables = input
            .data
            .dynamic_conformance_tables
            .iter()
            .filter_map(|(_, table)| {
                (table.target_trait == dispatch.target_trait
                    && table.conformance == candidate.conformance)
                    .then_some(table)
            });
        let table = tables.next().ok_or_else(|| {
            Diagnostic::error("dynamic call candidate has no exact private table")
        })?;
        if tables.next().is_some() || table.rows.len() != candidate.rows.len() {
            return Err(Diagnostic::error(
                "dynamic call candidate table is duplicated or row-incomplete",
            ));
        }
        for (physical, checked) in table.rows.iter().zip(&candidate.rows) {
            if physical.requirement_identity.as_ref() != checked.requirement_identity
                || physical.realization_identity.as_ref() != checked.realization_identity
                || physical.realization.machine != checked.realization_machine
                || physical.realization.state != checked.realization_state
                || physical.realization.segment_index != 0
            {
                return Err(Diagnostic::error(
                    "dynamic call candidate table identity drifted from checked rows",
                ));
            }
        }
        let mut rows =
            table.rows.iter().enumerate().filter(|(_, row)| {
                row.requirement_identity.as_ref() == dispatch.requirement_identity
            });
        let (slot, row) = rows.next().ok_or_else(|| {
            Diagnostic::error(
                "dynamic call exact requirement row is missing from a candidate table",
            )
        })?;
        if rows.next().is_some() {
            return Err(Diagnostic::error(
                "dynamic call exact requirement row is duplicated",
            ));
        }
        let candidate_plan = dynamic_call_plan_for_realization(input, row.realization)?;
        require_matching_candidate_plan(&plan, &candidate_plan)?;
        match common_slot {
            Some(expected) if expected != slot => {
                return Err(Diagnostic::error(
                    "dynamic call requirement row occupies mismatched candidate slots",
                ));
            }
            None => common_slot = Some(slot),
            _ => {}
        }
    }

    let descriptor = unique_frame_slot(input, |slot| {
        slot.dispatch_index == dispatch_index
            && slot.symbol == receiver_symbol
            && slot.name == receiver_name
            && slot.kind == receiver_slot_kind
            && slot.byte_size == input.runtime_abi.pointer_size.saturating_mul(2)
            && matches!(
                &slot.type_descriptor,
                omega_layout::TypeLayoutDescriptor::Reference { referee, is_mutable: false }
                    if matches!(
                        referee.as_ref(),
                        omega_layout::TypeLayoutDescriptor::DynamicTrait { symbol, .. }
                            if *symbol == dispatch.target_trait
                    )
            )
    })
    .map_err(|error| Diagnostic::error(format!("dynamic descriptor slot: {}", error.message)))?;
    if descriptor.byte_size != input.runtime_abi.pointer_size.saturating_mul(2) {
        return Err(Diagnostic::error(
            "dynamic receiver is not the exact two-word descriptor ABI",
        ));
    }

    let result = match result_shape {
        None => None,
        Some(shape) => {
            let slot = unique_frame_slot(input, |slot| {
                slot.dispatch_index == dispatch_index
                    && source_matches(slot.source_key, call.source_key)
                    && slot.statement_index == call.statement_index
                    && matches!(slot.kind, omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult {
                        role,
                        call_ordinal,
                        ..
                    } if role == call.role && call_ordinal == call.call_ordinal)
            })
            .map_err(|error| Diagnostic::error(format!("dynamic call result slot: {}", error.message)))?;
            if slot.byte_size != usize::from(shape.byte_size) || shape.byte_size > 8 {
                return Err(Diagnostic::error(
                    "dynamic scalar result slot does not match its calling plan",
                ));
            }
            Some((slot.byte_offset, slot.byte_size, shape.class))
        }
    };

    Ok(ValidatedDynamicCall {
        table_byte_offset: common_slot
            .and_then(|slot| slot.checked_mul(input.runtime_abi.pointer_size))
            .ok_or_else(|| Diagnostic::error("dynamic table slot byte offset overflow"))?,
        requirement_identity: dispatch.requirement_identity.clone().into(),
        plan,
        descriptor_byte_offset: descriptor.byte_offset,
        result,
    })
}

fn require_matching_candidate_plan(
    caller: &CallPlan,
    candidate: &CallPlan,
) -> Result<(), Diagnostic> {
    if candidate == caller {
        Ok(())
    } else {
        Err(Diagnostic::error(
            "dynamic call candidate requirement row has a mismatched realization calling plan",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{require_matching_candidate_plan, validate_dynamic_result_ownership};
    use omega_calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    use omega_runtime_bodies::{
        RuntimeDispatchBody, RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
        RuntimeDispatchBodyPlan,
    };
    use omega_state_calls::StateCallRole;

    #[test]
    fn rejects_second_result_producer_for_an_indirect_dynamic_call() {
        let source_key = omega_control_flow::StateKey::default();
        let target_key = omega_control_flow::StateKey {
            segment_index: 1,
            ..source_key
        };
        let mut plan = RuntimeDispatchBodyPlan::default();
        let operations = plan.operations.insert_many([
            RuntimeDispatchBodyOperation {
                kind: RuntimeDispatchBodyOperationKind::DynamicStateCall {
                    role: StateCallRole::AssignmentValue,
                    call_ordinal: 3,
                    target_key,
                    argument_count: 0,
                },
                source_key,
                statement_index: 5,
            },
            RuntimeDispatchBodyOperation {
                kind: RuntimeDispatchBodyOperationKind::StateCallResult {
                    role: StateCallRole::AssignmentValue,
                    call_ordinal: 3,
                    target_key,
                    value: psi_checked_trees::expression::ExpressionHandle::invalid(),
                },
                source_key,
                statement_index: 5,
            },
        ]);
        plan.bodies.insert(RuntimeDispatchBody {
            key: source_key,
            dispatch_index: 0,
            operations,
        });

        let diagnostic = validate_dynamic_result_ownership(&plan)
            .expect_err("indirect dynamic result ownership must be unique");
        assert!(
            diagnostic
                .message
                .contains("second direct/spliced result producer")
        );
    }

    #[test]
    fn rejects_candidate_parameter_or_result_physical_shape_drift() {
        let plan = |parameter: ValueShape, result: ValueShape| {
            evaluate_call_plan(
                CallingPolicy::SystemVAMD64,
                &CallSignature {
                    parameters: vec![ValueShape::integer(8, 8), parameter],
                    result: Some(result),
                },
            )
            .expect("scalar dynamic call plan")
        };
        let caller = plan(ValueShape::integer(4, 4), ValueShape::integer(4, 4));
        let parameter_drift = plan(ValueShape::integer(8, 8), ValueShape::integer(4, 4));
        let result_drift = plan(ValueShape::integer(4, 4), ValueShape::integer(8, 8));

        assert!(require_matching_candidate_plan(&caller, &parameter_drift).is_err());
        assert!(require_matching_candidate_plan(&caller, &result_drift).is_err());
    }
}

fn realization_signature(
    input: &InstructionSelectionInput<'_>,
    realization: StateKey,
) -> Result<(Vec<ValueShape>, Option<ValueShape>), Diagnostic> {
    let state = input
        .control_flow
        .state_by_key(realization)
        .ok_or_else(|| Diagnostic::error("dynamic realization has no exact control-flow state"))?;
    let mut parameters = vec![ValueShape::integer(
        input.runtime_abi.pointer_size as u16,
        input.runtime_abi.pointer_alignment as u16,
    )];
    for parameter in input.control_flow.state_parameters(state) {
        parameters.push(scalar_shape_for_type_reference(
            input,
            parameter.type_reference,
        )?);
    }
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == realization.machine)
        .ok_or_else(|| Diagnostic::error("dynamic realization machine is missing"))?;
    let checked_state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == realization.state)
        .ok_or_else(|| Diagnostic::error("dynamic realization checked state is missing"))?;
    let result = if input
        .program
        .type_reference_table
        .type_reference(checked_state.return_type)
        == &psi_checked_trees::types::TypeReferenceNode::Unit
    {
        None
    } else {
        Some(scalar_shape_for_type_reference(
            input,
            checked_state.return_type,
        )?)
    };
    Ok((parameters, result))
}

fn scalar_shape_for_type_reference(
    input: &InstructionSelectionInput<'_>,
    type_reference: psi_checked_trees::types::TypeReferenceHandle,
) -> Result<ValueShape, Diagnostic> {
    use psi_checked_trees::types::{PrimitiveType, TypeReferenceNode};
    match input
        .program
        .type_reference_table
        .type_reference(type_reference)
    {
        TypeReferenceNode::Reference { .. } => Ok(ValueShape::integer(
            input.runtime_abi.pointer_size as u16,
            input.runtime_abi.pointer_alignment as u16,
        )),
        _ => {
            let primitive = input
                .program
                .primitive_type_reference(type_reference)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "dynamic dispatch currently requires scalar parameters and results",
                    )
                })?;
            let byte_size = primitive
                .scalar_byte_size()
                .ok_or_else(|| Diagnostic::error("dynamic dispatch scalar has no physical width"))?
                as u16;
            Ok(match primitive {
                PrimitiveType::F32 | PrimitiveType::F64 => ValueShape::float(byte_size),
                _ => ValueShape::integer(byte_size, byte_size.max(1)),
            })
        }
    }
}

fn unique_frame_slot<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    predicate: impl Fn(&omega_runtime_storage::RuntimeFrameSlot) -> bool,
) -> Result<&'plan omega_runtime_storage::RuntimeFrameSlot, Diagnostic> {
    let mut slots = input
        .runtime_storage
        .frame_slots
        .iter()
        .filter_map(|(_, slot)| predicate(slot).then_some(slot));
    let slot = slots
        .next()
        .ok_or_else(|| Diagnostic::error("dynamic dispatch required frame slot is missing"))?;
    if slots.next().is_some() {
        return Err(Diagnostic::error(
            "dynamic dispatch required frame slot is duplicated",
        ));
    }
    Ok(slot)
}

fn runtime_dispatch_index(input: &InstructionSelectionInput<'_>, call: &StateCall) -> Option<u32> {
    let mut bodies = input.runtime_bodies.bodies.iter().filter(|(_, body)| {
        input
            .runtime_bodies
            .operations
            .paged_span(body.operations)
            .is_some_and(|operations| {
                operations.iter().any(|operation| {
                    source_matches(operation.source_key, call.source_key)
                        && operation.statement_index == call.statement_index
                        && matches!(
                            operation.kind,
                            omega_runtime_bodies::RuntimeDispatchBodyOperationKind::DynamicStateCall {
                                call_ordinal,
                                ..
                            } if call_ordinal == call.call_ordinal
                        )
                })
            })
    });
    let index = bodies.next().map(|(_, body)| body.dispatch_index)?;
    bodies.next().is_none().then_some(index)
}

fn source_matches(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

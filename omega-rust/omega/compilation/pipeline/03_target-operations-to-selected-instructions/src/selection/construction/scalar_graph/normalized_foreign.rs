//! Evaluated normalized foreign calls: scalar copies, source-rooted borrowed
//! pointers, the exact per-plan register row, and the foreign-call roster.
//!
//! Every retained coordinate — locator target, calling policy, plan,
//! argument placements, and scalar result home — is re-validated by
//! `scalar_call_abi::normalized_foreign::validate` before any instruction is
//! emitted, so a substituted plan or argument cannot reach a selected row.
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstructionKind, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::row;
use crate::selection::construction::scalar_graph::structural;
use calling_conventions::ValueLocation;
use legalized_operations::LegalizedScalarInstruction;
use selected_instructions::{
    LocalStorageSlotId, SelectedLocalStorageSlot, SelectedNormalizedForeignCall,
};
use semantic_vocabulary::BoundaryInvocationSite;

mod descriptor_input;

pub(super) enum Invocation<'source> {
    Operation(&'source LegalizedScalarInstruction),
    ExternalCompletion {
        edge: semantic_vocabulary::EdgeId,
        effect: optimization_unit::EffectLink,
        ownership: &'source [optimization_unit::OwnershipEvent],
        fuel: &'source [optimization_unit::FuelSettlement],
    },
}

pub(super) fn emit(
    function: usize,
    source: &LegalizedScalarFunction,
    operation: &LegalizedScalarInstruction,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::NormalizedForeignCall(call) = &operation.kind else {
        return Err(SelectedInstructionError::custody());
    };
    if operation.result.is_some() != call.result.scalar_home().is_some() {
        return Err(SelectedInstructionError::custody());
    }
    emit_invocation(
        function,
        source,
        call,
        Invocation::Operation(operation),
        environment,
        builder,
    )
}

/// Marshal the admitted ABI for either a returning operation or a terminal
/// invocation. Only a returning operation owns result capture and preservation
/// of floating controls; a completion carries its edge through the same call.
pub(super) fn emit_invocation(
    function: usize,
    source: &LegalizedScalarFunction,
    call: &legalized_operations::LegalizedNormalizedForeignCall,
    invocation: Invocation<'_>,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    builder: &mut Builder<'_>,
) -> Result<(), SelectedInstructionError> {
    let (invocation_site, operation, effect, ownership, fuel) = match invocation {
        Invocation::Operation(operation) => (
            operation.operation.into(),
            Some(operation),
            operation.effect,
            operation.ownership.as_slice(),
            operation.fuel.as_slice(),
        ),
        Invocation::ExternalCompletion {
            edge,
            effect,
            ownership,
            fuel,
        } => (
            BoundaryInvocationSite::ExternalCompletion(edge),
            None,
            effect,
            ownership,
            fuel,
        ),
    };
    let result = operation.and_then(|operation| operation.result);
    let constraint =
        crate::selection::scalar_call_abi::normalized_foreign::call_constraint(call, environment)
            .ok_or_else(|| SelectedInstructionError::custody())?;
    let key = constraint.key;
    if !builder
        .constraints
        .keys
        .call_normalized_foreign
        .contains(&key)
    {
        return Err(SelectedInstructionError::custody());
    }
    crate::selection::scalar_call_abi::normalized_foreign::validate(
        function,
        source,
        call,
        invocation_site,
        result,
        key,
        &constraint,
        environment,
    )?;
    if let Some(operation) = operation {
        descriptor_input::emit(source, call, operation.operation, builder)?;
    }
    // Encoding materializes private addresses inside the call row, but frame
    // demand belongs here alongside every other outgoing argument. A final
    // stack callback cannot borrow extent from an unrelated sibling operand.
    for callback in &call.callbacks {
        if matches!(
            callback.destination,
            calling_conventions::NativePlace::Parameter(_)
        ) && let [
            ValueLocation::Stack {
                stack_byte_offset,
                byte_size,
                alignment,
                ..
            },
        ] = callback.application.placement.locations.as_slice()
        {
            builder
                .transport
                .slots
                .push(selected_instructions::SelectedOutgoingArgumentSlot {
                    id: selected_instructions::OutgoingArgumentSlotId {
                        invocation_site,
                        argument_index: callback.application.native_ordinal,
                        role: selected_instructions::OutgoingArgumentSlotRole::Argument,
                    },
                    byte_size: *byte_size,
                    alignment: *alignment,
                    abi_stack_byte_offset: *stack_byte_offset,
                });
        }
    }
    // The private save home is disjoint from outgoing arguments and survives
    // the counterparty's register clobbers. Save after argument transport;
    // restore before interpreting its result. Both remain ordinary selected
    // instructions so allocation and byte replay see their complete effects.
    // Only returning user-space counterparties can change floating controls.
    // Supervisor entries retain the same argument/result sequence without a
    // private floating-control home or save/restore instructions.
    let saved_controls = call
        .binding
        .same_stack_contribution()
        .and(operation)
        .map(|operation| LocalStorageSlotId::Boundary {
            operation: operation.operation,
        });
    // Durable scalars retain their bits in GPR storage. Transfer floating
    // arguments into short ABI-bank homes only at the call boundary; stack
    // arguments store those same bits directly into their exact-width slots.
    let mut operands = Vec::new();
    for argument in &call.scalar_arguments {
        let value = argument.source.source_value();
        if super::scalar_stack::invocation_argument(
            builder,
            invocation_site,
            argument.parameter_index as usize,
            value,
            &argument.placement,
        )? {
            continue;
        }
        let (_, input, site, scalar_type) = builder
            .resolve(value)
            .ok_or_else(|| SelectedInstructionError::custody())?;
        if scalar_type != argument.source.scalar_type()
            || crate::selection::scalar_call_abi::scalar_shape(scalar_type)
                != Some(argument.placement.shape)
        {
            return Err(SelectedInstructionError::custody());
        }
        let output = builder.register(value, site, scalar_type)?;
        let (kind, transfer_key) = crate::selection::scalar_call_abi::outgoing_float_transfer(
            scalar_type,
            &builder.constraints.keys,
        )
        .unwrap_or((
            SelectedInstructionKind::CopyI64,
            builder.constraints.keys.copy_i64,
        ));
        builder.registers[output.0 as usize].class = row(builder.catalog, transfer_key)?
            .operands
            .get(1)
            .ok_or_else(|| SelectedInstructionError::custody())?
            .class;
        builder.emit(
            kind,
            transfer_key,
            &[input, output],
            SelectedInstructionProvenance {
                operations: invocation_site.operation().into_iter().collect(),
                edges: completion_edges(invocation_site),
                values: vec![value],
                ..Default::default()
            },
        )?;
        operands.push((argument.parameter_index, output));
    }
    for ((argument_index, argument), indices) in
        crate::selection::scalar_call_abi::normalized_foreign::structural_parameter_positions(call)
            .zip(&call.structural_arguments)
            .zip(&call.structural_indices)
    {
        // The retained target argument has already rejoined its source type,
        // access and projection. Reuse ordinary value/reference transport,
        // without manufacturing an internal call or changing its ownership.
        let semantic = terminal_psi::StructuralArgument {
            place: argument.place,
            path: argument.path.clone(),
            access: argument.access,
        };
        operands.extend(
            super::call_arguments::structural_argument(
                source,
                operation.ok_or(SelectedInstructionError::custody())?,
                argument_index,
                &semantic,
                argument,
                indices,
                builder,
            )?
            .into_iter()
            .map(|register| (argument_index as u32, register)),
        );
    }
    // Preserve parameter order and each placement's fragment order, using
    // the same bank ordering as retained-plan constraint reconstruction.
    operands.sort_by_key(|(position, _)| {
        (
            call.binding.boundary_entry_plan.call.parameters[*position as usize]
                .shape
                .class
                == calling_conventions::ValueClass::Float,
            *position,
        )
    });
    let mut operands: Vec<VirtualRegisterId> =
        operands.into_iter().map(|(_, register)| register).collect();
    let hidden = match operation {
        Some(operation) => structural::indirect_results::prepare_call(source, operation, builder)?,
        None => None,
    };
    // Marshaling only moves bytes and addresses; it does not change floating
    // controls. Save immediately around the counterparty, after preparation,
    // and before result registers acquire the call instruction's origin.
    if let Some(saved_controls) = saved_controls {
        builder
            .transport
            .local_slots
            .push(SelectedLocalStorageSlot {
                id: saved_controls,
                byte_size: 8,
                alignment: 8,
            });
        builder.emit(
            SelectedInstructionKind::SaveFloatingControl {
                slot: saved_controls,
            },
            builder
                .constraints
                .keys
                .save_floating_control
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[],
            SelectedInstructionProvenance {
                operations: invocation_site.operation().into_iter().collect(),
                ..Default::default()
            },
        )?;
    }
    let mut result_registers = Vec::new();
    if let Some(pointer) = hidden {
        operands.push(pointer);
    } else if let Some((result, placement, _)) = operation.and_then(|operation| {
        crate::selection::aggregate_result_input::call_completion(source, operation)
    }) {
        for location in &placement.locations {
            let ValueLocation::Register {
                value_byte_offset, ..
            } = location
            else {
                return Err(SelectedInstructionError::custody());
            };
            let register = super::structural_case::register(
                builder,
                result.place,
                *value_byte_offset,
                64,
                false,
            )?;
            builder.registers[register.0 as usize].class = constraint
                .operands
                .get(operands.len())
                .ok_or(SelectedInstructionError::custody())?
                .class;
            operands.push(register);
            result_registers.push(register);
        }
    }
    let short_result = if let Some(result) = result {
        let register =
            builder.register(result.value, result.definition_site, result.scalar_type)?;
        builder.registers[register.0 as usize].class = constraint
            .operands
            .last()
            .ok_or_else(|| SelectedInstructionError::custody())?
            .class;
        operands.push(register);
        Some(register)
    } else {
        None
    };
    let ordinal = builder.transport.normalized_foreign_calls.len();
    builder
        .transport
        .normalized_foreign_calls
        .push(SelectedNormalizedForeignCall {
            instruction: SelectedInstructionId(
                builder
                    .instructions
                    .len()
                    .try_into()
                    .map_err(|_| SelectedInstructionError::custody())?,
            ),
            invocation_site,
            call: call.clone(),
            effect,
            ownership: ownership.to_vec(),
        });
    builder.emit(
        SelectedInstructionKind::NormalizedForeignCall {
            boundary: call.boundary,
            ordinal: ordinal
                .try_into()
                .map_err(|_| SelectedInstructionError::custody())?,
        },
        key,
        &operands,
        SelectedInstructionProvenance {
            operations: invocation_site.operation().into_iter().collect(),
            edges: completion_edges(invocation_site),
            values: call
                .scalar_arguments
                .iter()
                .map(|argument| argument.source.source_value())
                .chain(result.map(|result| result.value))
                .collect(),
            fuel: fuel.to_vec(),
            ..Default::default()
        },
    )?;
    if let Some(saved_controls) = saved_controls {
        builder.emit(
            SelectedInstructionKind::RestoreFloatingControl {
                slot: saved_controls,
            },
            builder
                .constraints
                .keys
                .restore_floating_control
                .ok_or_else(|| SelectedInstructionError::custody())?,
            &[],
            SelectedInstructionProvenance {
                operations: invocation_site.operation().into_iter().collect(),
                ..Default::default()
            },
        )?;
    }
    if let Some(operation) = operation {
        if hidden.is_some() {
            structural::indirect_results::finish_call(source, operation, builder)?;
        } else {
            super::unit_call::retain_result(source, operation, &result_registers, builder)?;
        }
    }
    // After restoring machine controls, retain the exact result bits in the
    // ordinary scalar home. Integer and Boolean carriers normalize as before.
    if let (Some(short_result), Some(result)) = (short_result, result) {
        let output = builder.register(result.value, result.definition_site, result.scalar_type)?;
        let (kind, transfer_key) = crate::selection::scalar_call_abi::incoming_float_transfer(
            result.scalar_type,
            &builder.constraints.keys,
        )
        .unwrap_or((
            crate::selection::scalar_call_abi::integer_carrier_normalization(result.scalar_type),
            builder.constraints.keys.copy_i64,
        ));
        builder.emit(
            kind,
            transfer_key,
            &[short_result, output],
            SelectedInstructionProvenance {
                operations: invocation_site.operation().into_iter().collect(),
                values: vec![result.value],
                ..Default::default()
            },
        )?;
        builder.definitions.push((
            result.value,
            output,
            result.definition_site,
            result.scalar_type,
        ));
    }
    Ok(())
}

fn completion_edges(site: BoundaryInvocationSite) -> Vec<semantic_vocabulary::EdgeId> {
    match site {
        BoundaryInvocationSite::Operation(_) => Vec::new(),
        BoundaryInvocationSite::ExternalCompletion(edge) => vec![edge],
    }
}

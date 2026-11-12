/*
  Structural, ordinary, dynamic, and normalized foreign call contracts, ABI
  constraint instantiation, and frame storage slot identities.

  `ordinary` defines `SelectedCallContract`, `SelectedLocalStorageSlot`,
  `SelectedOutgoingArgumentSlot`, `SelectedMemoryAccess`, and
  `SelectedBoundarySettlement`; `normalized_foreign` and its `return_transport`
  queries bind evaluated foreign boundary-entry plans and result normalization;
  `dynamic` instantiates the dispatch selector register on dynamic calls; and
  `constraints` instantiates physical call operand constraints from retained
  evaluated placements.
*/

pub mod constraints {

    use calling_conventions::{
        CallSignature, IndirectPointerLocation, MachineRegister, NativePlace, ValueClass,
        ValueLocation, ValueShape,
    };
    use legalized_operations::LegalizedNormalizedForeignCall;
    use register_model::{
        RegisterConstraintFamily, RegisterInstructionConstraint, RegisterOperandAccess,
        RegisterOperandConstraint, RegisterView, RegisterViewId, ValidatedPhysicalRegisterModel,
    };
    use target_operations::NormalizedForeignCallMechanism;

    use crate::{
        SelectedFunction, SelectedInstruction, SelectedInstructionKind,
        SelectedNormalizedForeignCall,
    };
    use calling_conventions::validate_boundary_entry_plan_with_callback_materializations;

    pub fn foreign_call_record<'function>(
        function: &'function SelectedFunction,
        instruction: &SelectedInstruction,
    ) -> Option<&'function SelectedNormalizedForeignCall> {
        let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = instruction.kind
        else {
            return None;
        };
        let record = function
            .normalized_foreign_calls
            .get(usize::try_from(ordinal).ok()?)?;
        if record.instruction != instruction.id || record.call.boundary != boundary {
            return None;
        }
        let mut matching = function
            .normalized_foreign_calls
            .iter()
            .filter(|candidate| candidate.instruction == instruction.id);
        let unique = matching.next()?;
        (matching.next().is_none() && unique == record).then_some(record)
    }

    pub fn foreign_argument_registers(
        call: &LegalizedNormalizedForeignCall,
    ) -> Vec<(usize, MachineRegister)> {
        let operands = call.binding.operand_placements();
        let mut parameters = operands
            .iter()
            .enumerate()
            .filter(|(position, _)| {
                !call.callbacks.iter().any(|callback| {
                    usize::try_from(callback.application.native_ordinal)
                        .ok()
                        .map(|ordinal| ordinal + call.binding.dispatch_operand_count())
                        == Some(*position)
                        && matches!(callback.destination, NativePlace::Parameter(_))
                })
            })
            .collect::<Vec<_>>();
        parameters.sort_by_key(|(_, placement)| placement.shape.class == ValueClass::Float);
        parameters
            .into_iter()
            .flat_map(|(position, placement)| {
                placement.locations.iter().filter_map(move |location| {
                    location
                        .direct_register()
                        .map(|register| (position, register))
                })
            })
            .collect()
    }

    pub fn instantiate_foreign_call_constraint(
        call: &LegalizedNormalizedForeignCall,
        base: &RegisterInstructionConstraint,
        physical: &ValidatedPhysicalRegisterModel,
        fixed_register_view: impl Fn(MachineRegister) -> Option<RegisterViewId>,
    ) -> Option<RegisterInstructionConstraint> {
        let target = call.binding.native_target()?;
        let expected_family = match call.binding.mechanism {
            NormalizedForeignCallMechanism::Import { .. }
            | NormalizedForeignCallMechanism::Component { .. }
            | NormalizedForeignCallMechanism::Table { .. } => RegisterConstraintFamily::Call,
            NormalizedForeignCallMechanism::SystemCall(_) => RegisterConstraintFamily::SystemCall,
        };
        if !base.operands.is_empty()
            || base.key.family != expected_family
            || physical.model().architecture != target.architecture
            || !call.binding.has_valid_entry_mechanism()
        {
            return None;
        }
        let plan = &call.binding.boundary_entry_plan;
        let signature =
            CallSignature::of_placements(&plan.call.parameters, plan.call.result.as_ref());
        let validated = match call.callbacks.first() {
            Some(callback) => validate_boundary_entry_plan_with_callback_materializations(
                plan.clone(),
                &signature,
                &callback.registrar_context,
            ),
            None => calling_conventions::validate_boundary_entry_plan(plan.clone(), &signature),
        }
        .ok()?;
        if validated.plan() != plan
            || call.callbacks.len() != plan.call.callback_materializations.len()
        {
            return None;
        }

        let mut row = base.clone();
        let mut private_parameters = Vec::new();
        let mut memory_destination = false;
        let pointer_shape = ValueShape::integer(
            u32::try_from(target.pointer_size).ok()?,
            u16::try_from(target.pointer_alignment).ok()?,
        );
        for (callback_index, callback) in call.callbacks.iter().enumerate() {
            let parameter = usize::try_from(callback.application.native_ordinal).ok()?;
            if callback.registrar_boundary_entry_plan != *plan
                || callback.registrar_context != call.callbacks.first()?.registrar_context
                || callback.application.shape != pointer_shape
                || callback.application.placement.shape != callback.application.shape
                || plan.call.parameters.get(parameter) != Some(&callback.application.placement)
                || !plan
                    .call
                    .callback_materializations
                    .iter()
                    .any(|materialization| materialization.destination == callback.destination)
                || call.callbacks[..callback_index].iter().any(|previous| {
                    previous.destination == callback.destination
                        || previous.placement_index == callback.placement_index
                })
            {
                return None;
            }
            match callback.destination {
                NativePlace::Parameter(identity) => {
                    if identity != callback.application.parameter
                        || callback.field_placements.is_some()
                        || private_parameters.contains(&parameter)
                    {
                        return None;
                    }
                    private_parameters.push(parameter);
                    match callback.application.placement.locations.as_slice() {
                        [ValueLocation::Register { register, .. }] => {
                            row.implicit_uses.extend(
                                register_view(physical, *register, &fixed_register_view)?
                                    .units
                                    .iter()
                                    .copied(),
                            );
                        }
                        [ValueLocation::Stack { .. }] => memory_destination = true,
                        _ => return None,
                    }
                }
                NativePlace::Field {
                    parameter: identity,
                    ..
                } => {
                    if identity != callback.application.parameter
                        || callback.field_placements.is_none()
                        || !matches!(
                            callback.application.placement.locations.as_slice(),
                            [ValueLocation::Register { .. }]
                        )
                    {
                        return None;
                    }
                    memory_destination = true;
                }
            }
        }
        if memory_destination {
            let address_scratch = [plan.call.policy.callback_field_scratch()];
            let scratch_registers = if call
                .callbacks
                .iter()
                .any(|callback| callback.field_placements.is_some())
            {
                plan.call.policy.callback_fragment_scratch()
            } else {
                &address_scratch
            };
            for &scratch in scratch_registers {
                if plan.call.parameters.iter().any(|parameter| {
                    parameter
                        .locations
                        .iter()
                        .any(|location| location.direct_register() == Some(scratch))
                }) || plan.call.result.as_ref().is_some_and(|result| {
                    result.locations.iter().any(|location| {
                        matches!(location,
                    ValueLocation::Indirect { pointer: IndirectPointerLocation::Register(register), .. }
                        if *register == scratch)
                    })
                }) {
                    return None;
                }
                row.implicit_uses.extend(
                    register_view(physical, scratch, &fixed_register_view)?
                        .units
                        .iter()
                        .copied(),
                );
            }
        }
        row.implicit_uses.sort_unstable();
        row.implicit_uses.dedup();

        for (_, register) in foreign_argument_registers(call) {
            append_operand(
                &mut row,
                register_view(physical, register, &fixed_register_view)?,
                RegisterOperandAccess::Use,
            )?;
        }
        if let Some(result) = &plan.call.result {
            for location in &result.locations {
                let (register, access) = match location {
                    ValueLocation::Register { register, .. } => {
                        (*register, RegisterOperandAccess::Def)
                    }
                    ValueLocation::Indirect {
                        pointer: IndirectPointerLocation::Register(register),
                        ..
                    } => (*register, RegisterOperandAccess::Use),
                    _ => return None,
                };
                append_operand(
                    &mut row,
                    register_view(physical, register, &fixed_register_view)?,
                    access,
                )?;
            }
        }
        Some(row)
    }

    fn register_view<'physical>(
        physical: &'physical ValidatedPhysicalRegisterModel,
        register: MachineRegister,
        fixed_register_view: &impl Fn(MachineRegister) -> Option<RegisterViewId>,
    ) -> Option<&'physical RegisterView> {
        let identity = fixed_register_view(register)?;
        physical.model().view(identity)
    }

    fn append_operand(
        row: &mut RegisterInstructionConstraint,
        view: &RegisterView,
        access: RegisterOperandAccess,
    ) -> Option<()> {
        row.operands.push(RegisterOperandConstraint {
            operand: u16::try_from(row.operands.len()).ok()?,
            access,
            class: view.class,
            fixed_view: Some(view.id),
            tied_to: None,
            early_clobber: false,
        });
        if access == RegisterOperandAccess::Def {
            row.clobbers.retain(|unit| !view.write_units.contains(unit));
        }
        Some(())
    }
}
pub mod normalized_foreign {
    use crate::SelectedInstructionId;
    use optimization_unit::{EffectLink, OwnershipEvent};
    use semantic_vocabulary::InvocationSite;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedNormalizedForeignCall {
        pub instruction: SelectedInstructionId,
        pub invocation_site: InvocationSite,
        pub call: legalized_operations::LegalizedNormalizedForeignCall,
        pub effect: EffectLink,
        pub ownership: Vec<OwnershipEvent>,
    }

    pub mod return_transport {
        use crate::FrameStorageSlotId::Local;
        use crate::{
            LocalStorageSlotId, SelectedBlock, SelectedFunction, SelectedInstruction,
            SelectedInstructionKind, SelectedNormalizedForeignCall, VirtualRegisterId,
            VirtualRegisterOrigin,
        };
        use calling_conventions::ValueLocation::Indirect;
        use register_model::RegisterOperandAccess;
        use target_operations::TargetBoundaryResult::Unit;

        pub struct ReturnTransport {
            pub call_position: usize,
            pub restore_position: Option<usize>,
            pub normalization_position: usize,
            pub result_registers: Vec<VirtualRegisterId>,
        }

        pub fn resolve(
            function: &SelectedFunction,
            block: &SelectedBlock,
            record: &SelectedNormalizedForeignCall,
        ) -> Option<ReturnTransport> {
            let mut calls = block
                .instructions
                .iter()
                .enumerate()
                .filter(|(_, instruction)| instruction.id == record.instruction);
            let (call_position, call) = calls.next()?;
            if calls.next().is_some() {
                return None;
            }
            if let semantic_vocabulary::InvocationSite::ExternalCompletion(edge) =
                record.invocation_site
            {
                if !matches!(record.call.result, Unit)
                    || record.result_placement().is_some()
                    || !record.call.callbacks.is_empty()
                    || !matches!(&block.terminator,
                        crate::SelectedTerminator::HostedExitProcess { instruction, completion_edge }
                            if *completion_edge == edge
                            && instruction.kind == SelectedInstructionKind::Crash
                            && instruction.provenance.edges.as_slice() == [edge]
                            && instruction.provenance.operations.is_empty()
                            && instruction.provenance.values.is_empty()
                            && instruction.provenance.obligations.is_empty()
                            && instruction.provenance.work.is_empty()
                            && instruction.operands.is_empty())
                    || call_position.checked_add(1)? != block.instructions.len()
                {
                    return None;
                }
                return Some(ReturnTransport {
                    call_position,
                    restore_position: None,
                    normalization_position: call_position.checked_add(1)?,
                    result_registers: Vec::new(),
                });
            }
            let operation = record.invocation_site.operation()?;
            let indirect_scalar = record.call.result.scalar_home().is_some()
                && record.result_placement().is_some_and(|placement| {
                    matches!(placement.locations.as_slice(), [Indirect { .. }])
                });
            let mut result_registers = if indirect_scalar {
                validate_indirect_destination(function, block, record, call_position)?;
                Vec::new()
            } else if let Some(home) = record.call.result.scalar_home() {
                let output = call.operands.last()?;
                let value = function
                    .virtual_registers
                    .get(output.virtual_register.0 as usize)?;
                if output.access != RegisterOperandAccess::Def
                    || value.scalar_type != home.scalar_type
                    || value.origin
                        != (VirtualRegisterOrigin::InstructionResult {
                            instruction: call.id,
                            source_value: home.source_value,
                        })
                {
                    return None;
                }
                vec![output.virtual_register]
            } else if let target_operations::TargetBoundaryResult::Structural(home) =
                &record.call.result
            {
                use calling_conventions::ValueLocation;
                let placement = record.result_placement()?;
                match placement.locations.as_slice() {
                    [ValueLocation::Indirect { .. }] => Vec::new(),
                    locations if !locations.is_empty() => {
                        let outputs = call
                            .operands
                            .get(call.operands.len().checked_sub(locations.len())?..)?;
                        let mut registers = Vec::with_capacity(locations.len());
                        for (location, output) in locations.iter().zip(outputs) {
                            let ValueLocation::Register {
                                value_byte_offset, ..
                            } = location
                            else {
                                return None;
                            };
                            let value = function
                                .virtual_registers
                                .get(output.virtual_register.0 as usize)?;
                            if output.access != RegisterOperandAccess::Def
                                || value.origin
                                    != (VirtualRegisterOrigin::AbiTransport {
                                        instruction: call.id,
                                        place: home.place(),
                                        byte_offset: *value_byte_offset,
                                    })
                                || registers.contains(&output.virtual_register)
                            {
                                return None;
                            }
                            registers.push(output.virtual_register);
                        }
                        registers
                    }
                    _ => return None,
                }
            } else {
                Vec::new()
            };
            let mut cursor = call_position.checked_add(1)?;
            follow_copies(
                function,
                &block.instructions,
                &mut cursor,
                &mut result_registers,
            )?;
            let restore_position = if record.call.binding.same_stack_contribution().is_some() {
                let restore = block.instructions.get(cursor)?;
                if restore.kind
                    != (SelectedInstructionKind::RestoreFloatingControl {
                        slot: LocalStorageSlotId::Boundary { operation },
                    })
                    || restore.provenance.operations.as_slice() != [operation]
                {
                    return None;
                }
                let position = cursor;
                cursor = cursor.checked_add(1)?;
                follow_copies(
                    function,
                    &block.instructions,
                    &mut cursor,
                    &mut result_registers,
                )?;
                Some(position)
            } else {
                None
            };
            if indirect_scalar {
                let home = record.call.result.scalar_home()?;
                let address = block.instructions.get(cursor)?;
                let pointer =
                    result_buffer_address(function, address, operation, home.source_value)?;
                cursor = cursor.checked_add(1)?;
                let load = block.instructions.get(cursor)?;
                let expected = match home.shape.byte_size {
                    1 => SelectedInstructionKind::Load8 { byte_offset: 0 },
                    2 => SelectedInstructionKind::Load16 { byte_offset: 0 },
                    4 => SelectedInstructionKind::Load32 { byte_offset: 0 },
                    8 => SelectedInstructionKind::Load64 { byte_offset: 0 },
                    _ => return None,
                };
                let [input, output] = load.operands.as_slice() else {
                    return None;
                };
                let value = function.virtual_register(output.virtual_register)?;
                if load.kind != expected
                    || input.virtual_register != pointer
                    || input.access != RegisterOperandAccess::Use
                    || output.access != RegisterOperandAccess::Def
                    || load.provenance.operations.as_slice() != [operation]
                    || load.provenance.values.as_slice() != [home.source_value]
                    || value.scalar_type != home.scalar_type
                    || value.origin
                        != (VirtualRegisterOrigin::InstructionResult {
                            instruction: load.id,
                            source_value: home.source_value,
                        })
                {
                    return None;
                }
                result_registers.push(output.virtual_register);
                cursor = cursor.checked_add(1)?;
                follow_copies(
                    function,
                    &block.instructions,
                    &mut cursor,
                    &mut result_registers,
                )?;
            }
            Some(ReturnTransport {
                call_position,
                restore_position,
                normalization_position: cursor,
                result_registers,
            })
        }

        fn validate_indirect_destination(
            function: &SelectedFunction,
            block: &SelectedBlock,
            record: &SelectedNormalizedForeignCall,
            call_position: usize,
        ) -> Option<()> {
            let home = record.call.result.scalar_home()?;
            let operation = record.invocation_site.operation()?;
            let placement = record.result_placement()?;
            if placement.shape != home.shape || !placement.is_complete_scalar_result() {
                return None;
            }
            let mut slots = function
                .local_storage_slots
                .iter()
                .filter(|slot| slot.id == LocalStorageSlotId::BoundaryResult { operation });
            let slot = slots.next()?;
            if slots.next().is_some()
                || slot.byte_size != home.shape.byte_size
                || slot.alignment != home.shape.alignment
            {
                return None;
            }
            let operand = block.instructions.get(call_position)?.operands.last()?;
            if operand.access != RegisterOperandAccess::Use {
                return None;
            }
            let mut pointer = operand.virtual_register;
            let mut before = call_position;
            loop {
                let (position, definition) =
                    block.instructions[..before].iter().enumerate().rev().find(
                        |(_, instruction)| {
                            instruction.operands.iter().any(|operand| {
                                operand.virtual_register == pointer
                                    && operand.access == RegisterOperandAccess::Def
                            })
                        },
                    )?;
                if result_buffer_address(function, definition, operation, home.source_value)
                    == Some(pointer)
                {
                    return Some(());
                }
                let [input, output] = definition.operands.as_slice() else {
                    return None;
                };
                if definition.kind != SelectedInstructionKind::CopyI64
                    || input.access != RegisterOperandAccess::Use
                    || output.virtual_register != pointer
                    || function
                        .virtual_register(input.virtual_register)?
                        .scalar_type
                        != function.virtual_register(pointer)?.scalar_type
                {
                    return None;
                }
                pointer = input.virtual_register;
                before = position;
            }
        }

        fn result_buffer_address(
            function: &SelectedFunction,
            instruction: &SelectedInstruction,
            operation: semantic_vocabulary::OperationId,
            source_value: semantic_vocabulary::ValueId,
        ) -> Option<VirtualRegisterId> {
            let [output] = instruction.operands.as_slice() else {
                return None;
            };
            if instruction.kind
                != (SelectedInstructionKind::FrameAddress {
                    slot: Local(LocalStorageSlotId::BoundaryResult { operation }),
                    byte_offset: 0,
                })
                || output.access != RegisterOperandAccess::Def
                || instruction.provenance.operations.as_slice() != [operation]
                || instruction.provenance.values.as_slice() != [source_value]
                || function.virtual_register(output.virtual_register)?.origin
                    != (VirtualRegisterOrigin::ScalarAbiAddress {
                        instruction: instruction.id,
                        source_value,
                    })
            {
                return None;
            }
            Some(output.virtual_register)
        }

        fn follow_copies(
            function: &SelectedFunction,
            instructions: &[SelectedInstruction],
            cursor: &mut usize,
            result_registers: &mut [VirtualRegisterId],
        ) -> Option<()> {
            while let Some(instruction) = instructions.get(*cursor) {
                let Some(input) = instruction.operands.first() else {
                    break;
                };
                let Some(fragment) = result_registers
                    .iter()
                    .position(|register| *register == input.virtual_register)
                else {
                    break;
                };
                let Some(next) = copy_result(function, instruction, result_registers[fragment])
                else {
                    break;
                };
                if result_registers.contains(&next) {
                    return None;
                }
                result_registers[fragment] = next;
                *cursor = cursor.checked_add(1)?;
            }
            Some(())
        }

        fn copy_result(
            function: &SelectedFunction,
            instruction: &SelectedInstruction,
            source: VirtualRegisterId,
        ) -> Option<VirtualRegisterId> {
            forwarding_copy(function, instruction, source, None)
        }

        // An exact register-to-register forward of one value: no fixed view,
        // clobber or provenance of its own, and the output keeps the input's
        // origin with this copy as its producer. A fixed-occurrence split
        // before a call tags its copy with that call's operation, which
        // `owner` admits; nothing else may appear in the provenance.
        pub fn forwarding_copy(
            function: &SelectedFunction,
            instruction: &SelectedInstruction,
            source: VirtualRegisterId,
            owner: Option<semantic_vocabulary::OperationId>,
        ) -> Option<VirtualRegisterId> {
            let [input, output] = instruction.operands.as_slice() else {
                return None;
            };
            if instruction.kind != SelectedInstructionKind::CopyI64
                || input.virtual_register != source
                || input.access != RegisterOperandAccess::Use
                || output.access != RegisterOperandAccess::Def
                || !instruction.implicit_uses.is_empty()
                || !instruction.implicit_defs.is_empty()
                || !instruction.clobbers.is_empty()
                || !(instruction.provenance.operations.is_empty()
                    || owner.is_some_and(|owner| instruction.provenance.operations == [owner]))
                || !instruction.provenance.edges.is_empty()
                || !instruction.provenance.obligations.is_empty()
                || !instruction.provenance.work.is_empty()
                || instruction.operands.iter().any(|operand| {
                    operand.fixed_view.is_some()
                        || operand.tied_to.is_some()
                        || operand.early_clobber
                })
            {
                return None;
            }
            let input_value = function
                .virtual_registers
                .get(input.virtual_register.0 as usize)?;
            let output_value = function
                .virtual_registers
                .get(output.virtual_register.0 as usize)?;
            let expected_origin = match input_value.origin {
                VirtualRegisterOrigin::InstructionResult { source_value, .. } => {
                    if instruction.provenance.values.as_slice() != [source_value] {
                        return None;
                    }
                    VirtualRegisterOrigin::InstructionResult {
                        instruction: instruction.id,
                        source_value,
                    }
                }
                VirtualRegisterOrigin::AbiTransport {
                    place, byte_offset, ..
                } => {
                    if !instruction.provenance.values.is_empty() {
                        return None;
                    }
                    VirtualRegisterOrigin::AbiTransport {
                        instruction: instruction.id,
                        place,
                        byte_offset,
                    }
                }
                _ => return None,
            };
            if input_value.scalar_type != output_value.scalar_type
                || input_value.class != output_value.class
                || input_value.definition_site != output_value.definition_site
                || output_value.origin != expected_origin
            {
                return None;
            }
            Some(output.virtual_register)
        }

        pub fn save_position(
            function: &SelectedFunction,
            block: &SelectedBlock,
            record: &SelectedNormalizedForeignCall,
            call_position: usize,
        ) -> Option<usize> {
            let operation = record.invocation_site.operation()?;
            let slot = LocalStorageSlotId::Boundary { operation };
            let mut saves = block
                .instructions
                .iter()
                .enumerate()
                .filter(|(_, instruction)| {
                    instruction.kind == SelectedInstructionKind::SaveFloatingControl { slot }
                });
            let (position, save) = saves.next()?;
            if saves.next().is_some()
                || position >= call_position
                || save.provenance.operations.as_slice() != [operation]
                || !save.provenance.edges.is_empty()
                || !save.provenance.values.is_empty()
                || !save.provenance.obligations.is_empty()
                || !save.provenance.work.is_empty()
            {
                return None;
            }
            for (index, instruction) in block.instructions[position + 1..call_position]
                .iter()
                .enumerate()
            {
                if !instruction.provenance.operations.contains(&operation)
                    && !private_transport(function, block, position + 1 + index, instruction)
                {
                    return None;
                }
            }
            Some(position)
        }

        fn private_transport(
            function: &SelectedFunction,
            block: &SelectedBlock,
            position: usize,
            instruction: &SelectedInstruction,
        ) -> bool {
            let provenance = &instruction.provenance;
            if !provenance.operations.is_empty()
                || !provenance.edges.is_empty()
                || !provenance.obligations.is_empty()
                || !provenance.work.is_empty()
            {
                return false;
            }
            match instruction.kind {
                SelectedInstructionKind::CopyI64 => {
                    let [input, output] = instruction.operands.as_slice() else {
                        return false;
                    };
                    let (Some(input_value), Some(output_value)) = (
                        function.virtual_register(input.virtual_register),
                        function.virtual_register(output.virtual_register),
                    ) else {
                        return false;
                    };
                    input.access == RegisterOperandAccess::Use
                        && output.access == RegisterOperandAccess::Def
                        && input_value.scalar_type == output_value.scalar_type
                        && input_value.class == output_value.class
                        && instruction.implicit_uses.is_empty()
                        && instruction.implicit_defs.is_empty()
                        && instruction.clobbers.is_empty()
                        && instruction.operands.iter().all(|operand| {
                            operand.fixed_view.is_none()
                                && operand.tied_to.is_none()
                                && !operand.early_clobber
                        })
                }
                SelectedInstructionKind::FrameAddress {
                    slot:
                        Local(LocalStorageSlotId::Spill {
                            register: slot_register,
                        }),
                    byte_offset: 0,
                } => {
                    let [output] = instruction.operands.as_slice() else {
                        return false;
                    };
                    provenance.values.is_empty()
                        && output.access == RegisterOperandAccess::Def
                        && function
                            .virtual_register(output.virtual_register)
                            .is_some_and(|value| {
                                matches!(value.origin, VirtualRegisterOrigin::SpillAddress {
                                    instruction: address_instruction, ..
                                } if address_instruction == instruction.id)
                            })
                        && function.local_storage_slots.iter().any(|home| {
                            home.id
                                == LocalStorageSlotId::Spill {
                                    register: slot_register,
                                }
                                && home.byte_size == 8
                                && home.alignment == 8
                        })
                }
                SelectedInstructionKind::Load64 { byte_offset: 0 } => {
                    let [address, output] = instruction.operands.as_slice() else {
                        return false;
                    };
                    let Some(value) = function.virtual_register(address.virtual_register) else {
                        return false;
                    };
                    let VirtualRegisterOrigin::SpillAddress {
                        instruction: address_instruction,
                        ..
                    } = value.origin
                    else {
                        return false;
                    };
                    let Some(address_position) = block.instruction_index(address_instruction)
                    else {
                        return false;
                    };
                    provenance.values.is_empty()
                        && address.access == RegisterOperandAccess::Use
                        && output.access == RegisterOperandAccess::Def
                        && instruction.implicit_uses.is_empty()
                        && instruction.implicit_defs.is_empty()
                        && instruction.clobbers.is_empty()
                        && address_position < position
                        && block.instructions[address_position]
                            .operands
                            .first()
                            .is_some_and(|output| {
                                output.virtual_register == address.virtual_register
                            })
                        && private_transport(
                            function,
                            block,
                            address_position,
                            &block.instructions[address_position],
                        )
                }
                _ => false,
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::{
                FrameStorageSlotId, SelectedBlockId, SelectedInstructionId, SelectedOperand,
                VirtualRegister,
            };
            use register_model::{
                RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey,
            };
            use semantic_vocabulary::{
                IntegerSign, IntegerType, MachineId, OperationId, ScalarType, ValueId,
            };

            #[test]
            fn scalar_result_buffer_address_rejects_other_storage_and_false_lineage() {
                let operation = OperationId::new(17).unwrap();
                let source_value = ValueId::new(19).unwrap();
                let register = VirtualRegisterId(0);
                let mut function = SelectedFunction {
                    machine: MachineId::new(1).unwrap(),
                    attachment: None,
                    nominal_cleanup_subject: None,
                    provenance: Default::default(),
                    structural: None,
                    local_storage_slots: Vec::new(),
                    outgoing_arguments: Vec::new(),
                    calls: Vec::new(),
                    normalized_foreign_calls: Vec::new(),
                    memory_accesses: Vec::new(),
                    boundary_settlements: Vec::new(),
                    entry_block: SelectedBlockId(0),
                    blocks: (Vec::new()).into(),
                    virtual_registers: vec![VirtualRegister {
                        id: register,
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        ),
                        class: RegisterClassId(0),
                        origin: VirtualRegisterOrigin::ScalarAbiAddress {
                            instruction: SelectedInstructionId(3),
                            source_value,
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    }],
                };
                let address = SelectedInstruction {
                    id: SelectedInstructionId(3),
                    kind: SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(LocalStorageSlotId::BoundaryResult {
                            operation,
                        }),
                        byte_offset: 0,
                    },
                    constraint: RegisterConstraintKey {
                        family: RegisterConstraintFamily::Instruction,
                        variant: 0,
                    },
                    operands: vec![SelectedOperand {
                        operand: 0,
                        virtual_register: register,
                        access: RegisterOperandAccess::Def,
                        class: RegisterClassId(0),
                        fixed_view: None,
                        tied_to: None,
                        early_clobber: false,
                    }],
                    implicit_uses: Vec::new(),
                    implicit_defs: Vec::new(),
                    clobbers: Vec::new(),
                    provenance: crate::SelectedInstructionProvenance {
                        operations: vec![operation],
                        values: vec![source_value],
                        ..Default::default()
                    },
                };
                assert_eq!(
                    result_buffer_address(&function, &address, operation, source_value),
                    Some(register)
                );
                for slot in [
                    LocalStorageSlotId::Boundary { operation },
                    LocalStorageSlotId::BoundaryResult {
                        operation: OperationId::new(23).unwrap(),
                    },
                ] {
                    let mut changed = address.clone();
                    changed.kind = SelectedInstructionKind::FrameAddress {
                        slot: FrameStorageSlotId::Local(slot),
                        byte_offset: 0,
                    };
                    assert_eq!(
                        result_buffer_address(&function, &changed, operation, source_value),
                        None
                    );
                }
                let mut shifted = address.clone();
                shifted.kind = SelectedInstructionKind::FrameAddress {
                    slot: FrameStorageSlotId::Local(LocalStorageSlotId::BoundaryResult {
                        operation,
                    }),
                    byte_offset: 1,
                };
                assert_eq!(
                    result_buffer_address(&function, &shifted, operation, source_value),
                    None
                );
                function.virtual_registers[0].origin = VirtualRegisterOrigin::ScalarAbiAddress {
                    instruction: address.id,
                    source_value: ValueId::new(29).unwrap(),
                };
                assert_eq!(
                    result_buffer_address(&function, &address, operation, source_value),
                    None
                );
            }
        }
    }

    impl SelectedNormalizedForeignCall {
        fn result_placement(&self) -> Option<&calling_conventions::ValuePlacement> {
            self.call.binding.boundary_entry_plan.call.result.as_ref()
        }
    }
}
pub mod ordinary {
    use crate::{SelectedBlockId, SelectedInstructionId};
    use legalized_operations::encode_hosted_read_byte_identity;
    use legalized_operations::identity::encode_boundary_execution_identity;
    use optimization_unit::{EffectLink, OwnershipEvent};
    use semantic_vocabulary::InvocationSite::ExternalCompletion;
    use semantic_vocabulary::{BlockId, EdgeId, InvocationSite, OperationId, PlaceId};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct OutgoingArgumentSlotId {
        pub invocation_site: InvocationSite,
        pub argument_index: u32,
        pub role: OutgoingArgumentSlotRole,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum OutgoingArgumentSlotRole {
        Argument,
        ValueCopy,
    }

    impl OutgoingArgumentSlotId {
        pub fn encode_identity(self, bytes: &mut Vec<u8>) {
            self.invocation_site.encode_identity(bytes);
            crate::codec_bytes::push_encoded(bytes, self.argument_index.to_le_bytes());
            bytes.push(match self.role {
                OutgoingArgumentSlotRole::Argument => 0,
                OutgoingArgumentSlotRole::ValueCopy => 1,
            });
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum LocalStorageSlotId {
        InvocationResult {
            edge: EdgeId,
        },
        InvocationArgument {
            edge: EdgeId,
            argument_index: u32,
        },
        StructuralParameter {
            place: PlaceId,
        },
        StructuralBlockParameter {
            block: BlockId,
            place: PlaceId,
        },
        Spill {
            register: crate::VirtualRegisterId,
        },
        Structural {
            operation: OperationId,
            place: PlaceId,
        },
        StructuralCallArgument {
            operation: OperationId,
            argument_index: u32,
        },
        EdgeSnapshot {
            edge: semantic_vocabulary::EdgeId,
            binding_index: u32,
        },
        Boundary {
            operation: OperationId,
        },
        BoundaryResult {
            operation: OperationId,
        },
    }

    impl LocalStorageSlotId {
        pub const fn operation(self) -> Option<OperationId> {
            match self {
                Self::Structural { operation, .. }
                | Self::StructuralCallArgument { operation, .. }
                | Self::Boundary { operation }
                | Self::BoundaryResult { operation } => Some(operation),
                Self::InvocationResult { .. }
                | Self::InvocationArgument { .. }
                | Self::Spill { .. }
                | Self::EdgeSnapshot { .. }
                | Self::StructuralBlockParameter { .. }
                | Self::StructuralParameter { .. } => None,
            }
        }

        pub const fn structural_place(self) -> Option<PlaceId> {
            match self {
                Self::Structural { place, .. }
                | Self::StructuralBlockParameter { place, .. }
                | Self::StructuralParameter { place } => Some(place),
                Self::InvocationResult { .. }
                | Self::InvocationArgument { .. }
                | Self::Boundary { .. }
                | Self::BoundaryResult { .. }
                | Self::Spill { .. }
                | Self::StructuralCallArgument { .. }
                | Self::EdgeSnapshot { .. } => None,
            }
        }

        pub fn encode_identity(self, bytes: &mut Vec<u8>) {
            match self {
                Self::InvocationResult { edge } => {
                    bytes.push(8);
                    crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                }
                Self::InvocationArgument {
                    edge,
                    argument_index,
                } => {
                    bytes.push(9);
                    crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, argument_index.to_le_bytes());
                }
                Self::EdgeSnapshot {
                    edge,
                    binding_index,
                } => {
                    bytes.push(6);
                    crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, binding_index.to_le_bytes());
                }
                Self::StructuralCallArgument {
                    operation,
                    argument_index,
                } => {
                    bytes.push(5);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, argument_index.to_le_bytes());
                }
                Self::StructuralParameter { place } => {
                    bytes.push(4);
                    crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                }
                Self::StructuralBlockParameter { block, place } => {
                    bytes.push(3);
                    crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                }
                Self::Spill { register } => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
                }
                Self::Structural { operation, place } => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
                }
                Self::BoundaryResult { operation } => {
                    bytes.push(7);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                }
                Self::Boundary { operation } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum FrameStorageSlotId {
        ActivationBase,
        Incoming {
            parameter_index: u32,
            abi_stack_byte_offset: u32,
        },
        Outgoing(OutgoingArgumentSlotId),
        Local(LocalStorageSlotId),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedLocalStorageSlot {
        pub id: LocalStorageSlotId,
        pub byte_size: u32,
        pub alignment: u16,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedOutgoingArgumentSlot {
        pub id: OutgoingArgumentSlotId,
        pub byte_size: u32,
        pub alignment: u16,
        pub abi_stack_byte_offset: u32,
    }

    impl SelectedOutgoingArgumentSlot {
        pub fn reserved_byte_size(&self, stack_alignment: u16) -> Option<u32> {
            let padding = if self.id.role == OutgoingArgumentSlotRole::ValueCopy {
                u32::from(self.alignment.saturating_sub(stack_alignment))
            } else {
                0
            };
            self.byte_size.checked_add(padding)
        }

        pub fn reservation_alignment(&self, stack_alignment: u16) -> u16 {
            if self.id.role == OutgoingArgumentSlotRole::ValueCopy {
                self.alignment.min(stack_alignment)
            } else {
                self.alignment
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedCallContract {
        pub instruction: SelectedInstructionId,
        pub owner: target_operations::CallSiteOwner,
        pub call: legalized_operations::LegalizedScalarCall,
        pub effect: EffectLink,
        pub ownership: Vec<OwnershipEvent>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SelectedMemoryAccessRole {
        ReadReturnedView {
            producer: OperationId,
        },
        ReadByteSpan {
            length: semantic_vocabulary::ValueId,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        WriteByteSpan {
            length: semantic_vocabulary::ValueId,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        WriteByteSequence {
            index: semantic_vocabulary::ValueId,
            value: semantic_vocabulary::ValueId,
            length: semantic_vocabulary::ValueId,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        ReadByteSequence {
            index: semantic_vocabulary::ValueId,
            length: semantic_vocabulary::ValueId,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        ReadMeasuredSpan {
            length: semantic_vocabulary::ValueId,
        },
        WriteIndexedPrimitive {
            index: semantic_vocabulary::ValueId,
            value: semantic_vocabulary::ValueId,
            stride: u32,
            extent: Option<u64>,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        WriteIndexedStructural {
            index: semantic_vocabulary::ValueId,
            stride: u32,
            extent: Option<u64>,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        ReadIndexedPrimitive {
            index: semantic_vocabulary::ValueId,
            stride: u32,
            extent: Option<u64>,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        ReadElementView {
            index: semantic_vocabulary::ValueId,
            length: semantic_vocabulary::ValueId,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        WriteElementView {
            index: semantic_vocabulary::ValueId,
            length: semantic_vocabulary::ValueId,
            obligation: semantic_vocabulary::ObligationId,
            accepted_fact: optimization_core::AcceptedObligationFactIdentity,
        },
        ReadPlace,
        WritePlace,
        ReadBacking,
        WriteBacking,
        ReadBackingSpan,
        WriteBackingSpan,
        ReadLocal {
            slot: LocalStorageSlotId,
        },
        WriteLocal {
            slot: LocalStorageSlotId,
        },
        AddressLocal {
            slot: LocalStorageSlotId,
        },
        WriteOutgoing {
            slot: OutgoingArgumentSlotId,
        },
        AddressOutgoing {
            slot: OutgoingArgumentSlotId,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedMemoryAccess {
        pub instruction: SelectedInstructionId,
        pub origin: SelectedMemoryAccessOrigin,
        pub place: PlaceId,
        pub byte_offset: u32,
        pub byte_count: u32,
        pub role: SelectedMemoryAccessRole,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SelectedMemoryAccessOrigin {
        Operation(OperationId),
        Edge(EdgeId),
        Block(BlockId),
    }

    impl SelectedMemoryAccessOrigin {
        pub fn encode_identity(self, bytes: &mut Vec<u8>) {
            let (tag, identity) = match self {
                Self::Operation(identity) => (0, identity.get()),
                Self::Edge(identity) => (1, identity.get()),
                Self::Block(identity) => (2, identity.get()),
            };
            bytes.push(tag);
            crate::codec_bytes::push_encoded(bytes, identity.to_le_bytes());
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SelectedBoundarySettlement {
        pub block: SelectedBlockId,
        pub instruction_index: u32,
        pub settlement: SelectedBoundarySettlementPayload,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SelectedBoundarySettlementPayload {
        HostedReadByte {
            operation: OperationId,
            boundary: semantic_vocabulary::BoundaryMachineId,
            result: terminal_psi::StructuralOperationResult,
            layout: calling_conventions::ConventionalSumLayout,
        },
        HostedExitProcessI32 {
            edge: semantic_vocabulary::EdgeId,
            execution: target_operations::BoundaryExecutionBinding,
            boundary: semantic_vocabulary::BoundaryMachineId,
            source: semantic_vocabulary::ValueId,
        },
        ClaimCompletion(legalized_operations::LegalizedBoundarySettlement),
        HostedWriteByteI32 {
            operation: OperationId,
            boundary: semantic_vocabulary::BoundaryMachineId,
            source: semantic_vocabulary::ValueId,
        },
    }

    impl SelectedBoundarySettlementPayload {
        pub fn invocation_site(&self) -> semantic_vocabulary::InvocationSite {
            match self {
                Self::ClaimCompletion(settlement) => settlement.operation.into(),
                Self::HostedReadByte { operation, .. }
                | Self::HostedWriteByteI32 { operation, .. } => (*operation).into(),
                Self::HostedExitProcessI32 { edge, .. } => ExternalCompletion(*edge),
            }
        }

        pub fn canonical_bytes(&self) -> Vec<u8> {
            let mut bytes = Vec::new();
            match self {
                Self::HostedReadByte {
                    operation,
                    boundary,
                    result,
                    layout,
                } => {
                    bytes.push(3);
                    encode_hosted_read_byte_identity(
                        &mut bytes, *operation, *boundary, result, layout,
                    );
                }
                Self::ClaimCompletion(settlement) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&settlement.canonical_bytes());
                }
                Self::HostedExitProcessI32 {
                    edge,
                    execution,
                    boundary,
                    source,
                } => {
                    bytes.push(2);
                    crate::codec_bytes::push_encoded(&mut bytes, edge.get().to_le_bytes());
                    encode_boundary_execution_identity(&mut bytes, *execution);
                    crate::codec_bytes::push_encoded(&mut bytes, boundary.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, source.get().to_le_bytes());
                }
                Self::HostedWriteByteI32 {
                    operation,
                    boundary,
                    source,
                } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(&mut bytes, operation.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, boundary.get().to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, source.get().to_le_bytes());
                }
            }
            bytes
        }
    }
}

pub mod dynamic {

    use crate::{
        MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedMemoryEffect,
        MachineEncodedTrapBehavior, MachineMemoryEffect, MachineSizeKnowledge, MachineTrapBehavior,
        SelectedCallTarget, SelectedFunction, SelectedInstruction,
    };
    use register_model::{
        RegisterInstructionConstraint, RegisterOperandAccess, RegisterOperandConstraint,
        ValidatedPhysicalRegisterModel,
    };

    pub fn call_record<'function>(
        function: &'function SelectedFunction,
        instruction: &SelectedInstruction,
    ) -> Option<&'function crate::SelectedCallContract> {
        let target = instruction.kind.call_target()?;
        let mut records = function
            .calls
            .iter()
            .filter(|record| record.instruction == instruction.id);
        let record = records.next()?;
        (records.next().is_none() && SelectedCallTarget::from_native(record.call.target) == target)
            .then_some(record)
    }

    pub fn instantiate_constraint(
        base: &RegisterInstructionConstraint,
        physical: &ValidatedPhysicalRegisterModel,
    ) -> Option<RegisterInstructionConstraint> {
        let name = match physical.model().architecture {
            target::Architecture::Aarch64 => "x16",
            target::Architecture::X86_64 => "r11",
        };
        let selector = physical.model().view_named(name)?;
        if base
            .operands
            .iter()
            .any(|operand| operand.fixed_view == Some(selector.id))
        {
            return None;
        }
        let mut row = base.clone();
        let position = row
            .operands
            .iter()
            .take_while(|operand| operand.access == RegisterOperandAccess::Use)
            .count();
        row.operands.insert(
            position,
            RegisterOperandConstraint {
                operand: position.try_into().ok()?,
                access: RegisterOperandAccess::Use,
                class: selector.class,
                fixed_view: Some(selector.id),
                tied_to: None,
                early_clobber: false,
            },
        );
        for (position, operand) in row.operands.iter_mut().enumerate() {
            operand.operand = position.try_into().ok()?;
        }
        Some(row)
    }

    pub fn instantiate_declaration(
        base: &MachineEffectDeclaration,
        constraint: &RegisterInstructionConstraint,
        physical: &ValidatedPhysicalRegisterModel,
    ) -> Option<MachineEffectDeclaration> {
        let mut declaration = base.clone();
        let inputs = constraint
            .operands
            .iter()
            .take_while(|operand| operand.access == RegisterOperandAccess::Use)
            .count();
        inputs.checked_sub(1)?;
        declaration.memory = MachineMemoryEffect::ReadPointerV1;
        declaration.trap = MachineTrapBehavior::MayArchitecturalFaultV1;
        for alternative in &mut declaration.alternatives {
            alternative.key.variant = 1;
            alternative.size =
                MachineSizeKnowledge::ExactBytes(match physical.model().architecture {
                    target::Architecture::Aarch64 => 36,
                    target::Architecture::X86_64 => 24,
                });
            alternative.encoded =
                encoded_effects(&alternative.encoded, physical.model().architecture)?;
        }
        Some(declaration)
    }

    pub fn encoded_effects(
        base: &crate::MachineEncodedEffects,
        architecture: target::Architecture,
    ) -> Option<crate::MachineEncodedEffects> {
        let position = u16::try_from(base.external_operand_reads.len()).ok()?;
        let mut effects = base.clone();
        effects.external_operand_reads.push(position);
        for output in &mut effects.external_operand_writes {
            *output = output.checked_add(1)?;
        }
        effects.memory = MachineEncodedMemoryEffect::ReadPointerV1 {
            pointer_operand: position,
            byte_count: 8,
        };
        effects.control = MachineEncodedControlEffect::IndirectTableCallOrTrapV1;
        effects.trap = MachineEncodedTrapBehavior::MayArchitecturalFaultV1;
        let _ = architecture;
        Some(effects)
    }
}

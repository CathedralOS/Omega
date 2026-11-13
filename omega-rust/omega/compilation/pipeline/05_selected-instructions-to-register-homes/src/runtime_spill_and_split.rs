/*
We implement the runtime register-pressure recovery rewrites replayed during
allocation: runtime constant/address rematerialization (`runtime_rematerialization`),
stack-slot spill store/reload insertion (`runtime_spill`), and live-range copy
splitting around fixed-register pins (`runtime_split`).
*/

#[allow(unused_imports)]
pub(crate) use crate::rewrites::*;

pub(crate) mod runtime_rematerialization {
    pub(crate) mod admission {
        use optimization_core::OptimizationWorkBudget;
        use optimization_unit::ValueDefinitionSite;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{
            SelectedCasePayloadTransport, SelectedFunction, SelectedInstruction,
            SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance,
            SelectedStructuralTransport, SelectedValueTransport, VirtualRegister,
            VirtualRegisterId, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{IntegerValue, ScalarType, ValueId};

        use super::RuntimeRematerializationError;
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::runtime_spill::{
            control, next_instruction_id, next_register_id, require_dominated_uses,
        };
        use crate::rewrites::selected_operands::selected_operand;

        pub(crate) struct Admission<'source> {
            pub function: &'source SelectedFunction,
            pub use_blocks: Vec<usize>,
            pub victim: &'source VirtualRegister,
            pub source_value: ValueId,
            pub value: IntegerValue,
            pub first_instruction: u32,
            pub first_register: u32,
            pub materialize: &'source RegisterInstructionConstraint,
        }

        pub(crate) fn admit<'source>(
            source: &'source impl ValidatedSelectedAnalysis,
            function_index: usize,
            register: VirtualRegisterId,
            environment: &'source ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission<'source>, RuntimeRematerializationError> {
            let (plan, function) =
                crate::rewrites::plan_function(source, function_index, environment, || {
                    RuntimeRematerializationError::SourceMismatch
                })?;
            let victim = function
                .virtual_register(register)
                .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
            let VirtualRegisterOrigin::InstructionResult {
                instruction: definition,
                source_value,
            } = victim.origin
            else {
                return Err(RuntimeRematerializationError::UnsupportedValue);
            };
            let scalar_payload = match victim.scalar_type {
                ScalarType::Callable(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_) => true,
                ScalarType::Integer(integer) => {
                    !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64)
                }
            };
            if !scalar_payload
                || victim.entry_fixed_view.is_some()
                || !matches!(
                    victim.definition_site,
                    Some(ValueDefinitionSite::FunctionParameter(_))
                        | Some(ValueDefinitionSite::BlockParameter { .. })
                        | Some(ValueDefinitionSite::Node { .. })
                )
            {
                return Err(RuntimeRematerializationError::UnsupportedValue);
            }
            let check_materialize =
                || -> Result<&RegisterInstructionConstraint, RuntimeRematerializationError> {
                    let materialize = environment
                        .constraint(environment.selected_keys().materialize_i64)
                        .ok_or(RuntimeRematerializationError::ConstraintMismatch)?;
                    if materialize.operands.len() != 1
                        || !materialize.implicit_uses.is_empty()
                        || !materialize.implicit_defs.is_empty()
                        || !materialize.clobbers.is_empty()
                    {
                        return Err(RuntimeRematerializationError::ConstraintMismatch);
                    }
                    let row_operand = &materialize.operands[0];
                    if row_operand.access != RegisterOperandAccess::Def
                        || row_operand.class != victim.class
                        || row_operand.fixed_view.is_some()
                        || row_operand.tied_to.is_some()
                        || row_operand.early_clobber
                    {
                        return Err(RuntimeRematerializationError::ConstraintMismatch);
                    }
                    Ok(materialize)
                };
            let materialize = match check_materialize() {
                Ok(m) => m,
                Err(err) => {
                    function
                        .blocks
                        .instruction_block(definition)
                        .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
                    return Err(err);
                }
            };
            let row_operand = &materialize.operands[0];
            if !function.blocks.has_materialize_definition(register) {
                return Err(RuntimeRematerializationError::UnsupportedValue);
            }
            let block_index = function
                .blocks
                .instruction_block(definition)
                .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
            let defined_instruction = function
                .blocks
                .instruction_index_in_block(block_index, definition)
                .and_then(|idx| function.blocks[block_index].instructions.get(idx))
                .ok_or(RuntimeRematerializationError::UnsupportedValue)?;
            let SelectedInstructionKind::MaterializeI64 { value } = defined_instruction.kind else {
                return Err(RuntimeRematerializationError::UnsupportedValue);
            };
            if defined_instruction.constraint != materialize.key
                || defined_instruction.operands.len() != 1
                || defined_instruction.implicit_uses != materialize.implicit_uses
                || defined_instruction.implicit_defs != materialize.implicit_defs
                || defined_instruction.clobbers != materialize.clobbers
            {
                return Err(RuntimeRematerializationError::UnsupportedValue);
            }
            let def_operand = &defined_instruction.operands[0];
            if def_operand.virtual_register != register
                || def_operand.operand != row_operand.operand
                || def_operand.access != RegisterOperandAccess::Def
                || def_operand.class != victim.class
                || def_operand.fixed_view.is_some()
                || def_operand.tied_to.is_some()
                || def_operand.early_clobber
            {
                return Err(RuntimeRematerializationError::UnsupportedValue);
            }
            let mut defined = false;
            let mut uses = 0usize;
            let mut use_blocks = Vec::new();
            for (current_block_index, block) in function.blocks.iter().enumerate() {
                if !function
                    .blocks
                    .block_references_register(current_block_index, register)
                {
                    continue;
                }
                let previous_uses = uses;
                let (terminal, successors) = control(&block.terminator);
                for operand in &terminal.operands {
                    if operand.virtual_register != register {
                        continue;
                    }
                    match operand.access {
                        RegisterOperandAccess::Use
                            if operand.tied_to.is_none()
                                && !operand.early_clobber
                                && operand.class == victim.class =>
                        {
                            if terminal
                                .operands
                                .iter()
                                .any(|other| other.tied_to == Some(operand.operand))
                            {
                                return Err(RuntimeRematerializationError::UnsupportedUse);
                            }
                            uses = uses
                                .checked_add(1)
                                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                        }
                        _ => return Err(RuntimeRematerializationError::UnsupportedUse),
                    }
                }
                if successors.into_iter().flatten().any(|successor| {
                    successor.bindings.iter().any(|binding| {
                        matches!(binding.transport,
                        SelectedValueTransport::Registers { argument, parameter }
                            if argument == register || parameter == register)
                    }) || successor.structural_bindings.iter().any(|binding| {
                        matches!(binding.transport,
                        SelectedStructuralTransport::WholeValue { argument, .. }
                        | SelectedStructuralTransport::ScalarReference {
                            base: selected_instructions::SelectedAddressBase::Register(argument),
                            ..
                        }
                        | SelectedStructuralTransport::Address {
                            base: selected_instructions::SelectedAddressBase::Register(argument),
                            ..
                        } if argument == register)
                    }) || successor.structural_case.as_ref().is_some_and(|case| {
                        case.payloads.iter().any(|payload| match payload.transport {
                            SelectedCasePayloadTransport::Registers {
                                argument,
                                parameter,
                            } => argument == register || parameter == register,
                            SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                                parameter == register
                            }
                            SelectedCasePayloadTransport::Unused => false,
                        })
                    })
                }) {
                    return Err(RuntimeRematerializationError::UnsupportedUse);
                }
                if let Some((start_idx, end_idx)) = function
                    .blocks
                    .block_register_instruction_span(current_block_index, register)
                {
                    for instruction in &block.instructions[start_idx..=end_idx] {
                        for operand in &instruction.operands {
                            if operand.virtual_register != register {
                                continue;
                            }
                            match operand.access {
                                RegisterOperandAccess::Def
                                    if !defined
                                        && current_block_index == block_index
                                        && instruction.id == definition =>
                                {
                                    defined = true;
                                }
                                RegisterOperandAccess::Use
                                    if (defined || current_block_index != block_index)
                                        && instruction.id != definition
                                        && operand.fixed_view.is_none()
                                        && operand.tied_to.is_none()
                                        && !operand.early_clobber
                                        && operand.class == victim.class =>
                                {
                                    if instruction
                                        .operands
                                        .iter()
                                        .any(|other| other.tied_to == Some(operand.operand))
                                    {
                                        return Err(RuntimeRematerializationError::UnsupportedUse);
                                    }
                                    uses = uses
                                        .checked_add(1)
                                        .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                                }
                                _ => return Err(RuntimeRematerializationError::UnsupportedUse),
                            }
                        }
                    }
                }
                if uses != previous_uses {
                    use_blocks.push(current_block_index);
                }
            }
            if !defined || uses == 0 {
                return Err(RuntimeRematerializationError::UnsupportedValue);
            }
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| total.checked_add(uses.checked_mul(2)?))
                .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                crate::rewrites::required_rewrite_budget(plan, function, budget)
                    .ok_or(RuntimeRematerializationError::IdentityOverflow)?,
                || RuntimeRematerializationError::IdentityOverflow,
                || RuntimeRematerializationError::WorkBudgetExceeded,
            )?;
            require_dominated_uses(function, block_index, &use_blocks).map_err(
                |error| match error {
                    crate::RuntimeSpillError::SourceMismatch => {
                        RuntimeRematerializationError::SourceMismatch
                    }
                    _ => RuntimeRematerializationError::UnsupportedUse,
                },
            )?;
            let first_instruction = next_instruction_id(function)
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
            let first_register = next_register_id(function)
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
            Ok(Admission {
                function,
                use_blocks,
                victim,
                source_value,
                value,
                first_instruction,
                first_register,
                materialize,
            })
        }

        pub(crate) fn fresh(next: &mut u32) -> Result<u32, RuntimeRematerializationError> {
            let result = *next;
            *next = next
                .checked_add(1)
                .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
            Ok(result)
        }

        pub(crate) struct Materialization {
            pub register: VirtualRegister,
            pub instruction: SelectedInstruction,
        }

        pub(crate) fn materialization(
            admitted: &Admission<'_>,
            next_instruction: &mut u32,
            next_register: &mut u32,
        ) -> Result<Materialization, RuntimeRematerializationError> {
            let instruction = SelectedInstructionId(fresh(next_instruction)?);
            let register = VirtualRegisterId(fresh(next_register)?);
            Ok(Materialization {
                register: VirtualRegister {
                    id: register,
                    scalar_type: admitted.victim.scalar_type,
                    class: admitted.victim.class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction,
                        source_value: admitted.source_value,
                    },
                    definition_site: admitted.victim.definition_site,
                    entry_fixed_view: None,
                },
                instruction: SelectedInstruction {
                    id: instruction,
                    kind: SelectedInstructionKind::MaterializeI64 {
                        value: admitted.value,
                    },
                    constraint: admitted.materialize.key,
                    operands: vec![selected_operand(
                        &admitted.materialize.operands[0],
                        register,
                    )],
                    implicit_uses: admitted.materialize.implicit_uses.clone(),
                    implicit_defs: admitted.materialize.implicit_defs.clone(),
                    clobbers: admitted.materialize.clobbers.clone(),
                    provenance: SelectedInstructionProvenance {
                        values: vec![admitted.source_value],
                        ..Default::default()
                    },
                },
            })
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{SelectedInstruction, SelectedOperand, VirtualRegisterId};

        use super::{RuntimeRematerializationError, ValidatedRuntimeRematerialization, admission};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::runtime_spill::control_mut;
        use crate::rewrites::settlement_gaps::{gap_tables, push_position, remap};

        pub fn rematerialize_selected_runtime_value(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            register: VirtualRegisterId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedRuntimeRematerialization, RuntimeRematerializationError> {
            let admitted = admission::admit(source, function_index, register, environment, budget)?;
            let mut transformed = source.selected_plan().clone();
            transformed.functions.mutate_preserving_register_prefix(
                function_index,
                |_local_storage_slots,
                 virtual_registers,
                 boundary_settlements,
                 blocks|
                 -> Result<(), RuntimeRematerializationError> {
                    let mut next_instruction = admitted.first_instruction;
                    let mut next_register = admitted.first_register;
                    let mut rematerialize =
                        |operands: &mut [SelectedOperand],
                         instructions: &mut Vec<SelectedInstruction>|
                         -> Result<(), RuntimeRematerializationError> {
                            for operand in operands {
                                if operand.virtual_register != register
                                    || operand.access != RegisterOperandAccess::Use
                                {
                                    continue;
                                }
                                let materialization = admission::materialization(
                                    &admitted,
                                    &mut next_instruction,
                                    &mut next_register,
                                )?;
                                let regenerated = materialization.register.id;
                                virtual_registers.push(materialization.register);
                                instructions.push(materialization.instruction);
                                operand.virtual_register = regenerated;
                            }
                            Ok(())
                        };
                    for (block_index, block) in admitted.function.blocks.iter().enumerate() {
                        if !admitted.use_blocks.contains(&block_index) {
                            continue;
                        }
                        let has_settlements = boundary_settlements
                            .iter()
                            .any(|settlement| settlement.block == block.id);
                        let mut instructions = Vec::with_capacity(block.instructions.len() + 8);
                        let (mut boundaries, mut positions) = gap_tables(has_settlements, block);
                        let active_window = if has_settlements || block.instructions.is_empty() {
                            if block.instructions.is_empty() {
                                None
                            } else {
                                Some((0usize, block.instructions.len() - 1))
                            }
                        } else {
                            admitted
                                .function
                                .blocks
                                .block_register_instruction_span(block_index, register)
                        };
                        let unchanged_prefix =
                            active_window.map_or(block.instructions.len(), |(start, _)| start);
                        if let Some((active_start, active_end)) = active_window {
                            if active_start > 0 {
                                instructions.extend_from_slice(&block.instructions[..active_start]);
                            }
                            for original in &block.instructions[active_start..=active_end] {
                                push_position(&mut boundaries, has_settlements, instructions.len())
                                    .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                                let mut rewritten = original.clone();
                                rematerialize(&mut rewritten.operands, &mut instructions)?;
                                push_position(&mut positions, has_settlements, instructions.len())
                                    .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                                instructions.push(rewritten);
                            }
                            if active_end + 1 < block.instructions.len() {
                                instructions
                                    .extend_from_slice(&block.instructions[active_end + 1..]);
                            }
                        } else if !block.instructions.is_empty() {
                            instructions.extend_from_slice(&block.instructions);
                        }
                        let pre_terminator_len = instructions.len();
                        let mut terminator = block.terminator.clone();
                        rematerialize(
                            &mut control_mut(&mut terminator).operands,
                            &mut instructions,
                        )?;
                        push_position(&mut boundaries, has_settlements, instructions.len())
                            .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                        push_position(&mut positions, has_settlements, instructions.len())
                            .ok_or(RuntimeRematerializationError::IdentityOverflow)?;
                        if has_settlements {
                            remap(boundary_settlements, &boundaries, &positions, block.id)
                                .ok_or(RuntimeRematerializationError::SourceMismatch)?;
                        }
                        let unchanged_suffix = if instructions.len() == pre_terminator_len {
                            active_window
                                .map_or(block.instructions.len(), |(_, end)| {
                                    block.instructions.len() - (end + 1)
                                })
                                .min(block.instructions.len() - unchanged_prefix)
                        } else {
                            0
                        };
                        blocks.replace_block_contents(
                            block_index,
                            instructions,
                            terminator,
                            unchanged_prefix,
                            unchanged_suffix,
                        );
                    }
                    Ok(())
                },
            )?;
            super::validation::seal_runtime_rematerialization(source, transformed)
        }
    }
    pub(crate) mod validation {
        use selected_instructions::SelectedInstructionPlan;

        use super::{
            RuntimeRematerializationError, RuntimeRematerializationReceipt,
            ValidatedRuntimeRematerialization,
        };
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn seal_runtime_rematerialization(
            source: &impl ValidatedSelectedAnalysis,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedRuntimeRematerialization, RuntimeRematerializationError> {
            Ok(sealed!(
                ValidatedRuntimeRematerialization,
                RuntimeRematerializationReceipt,
                source,
                proposed
            ))
        }
        use crate::rewrites::sealed;
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
    use semantic_vocabulary::WorkScheduleIdentity;

    pub use rewrite::rematerialize_selected_runtime_value;

    validated_transformed_receipt!(
        ValidatedRuntimeRematerialization,
        RuntimeRematerializationReceipt
    );

    rewrite_receipt!(RuntimeRematerializationReceipt);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuntimeRematerializationError {
        SourceMismatch,
        UnsupportedValue,
        UnsupportedControlFlow,
        UnsupportedUse,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(
        RuntimeRematerializationError,
        "invalid runtime-value rematerialization"
    );
}

pub(crate) mod runtime_spill {
    pub(crate) mod admission {
        use selected_instructions::{
            FrameStorageSlotId, LocalStorageSlotId, SelectedInstruction, SelectedInstructionId,
            SelectedInstructionKind, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
        };

        use super::{RuntimeSpillDerivation, RuntimeSpillError, VictimLineage};

        pub(crate) fn fresh(next: &mut u32) -> Result<u32, RuntimeSpillError> {
            let result = *next;
            *next = next
                .checked_add(1)
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            Ok(result)
        }

        pub(crate) struct Reload {
            pub address_register: VirtualRegister,
            pub carrier_register: Option<VirtualRegister>,
            pub reload_register: VirtualRegister,
            pub address: SelectedInstruction,
            pub load: SelectedInstruction,
            pub convert: Option<SelectedInstruction>,
        }

        impl Reload {
            pub(crate) fn into_streams(self) -> (Vec<VirtualRegister>, Vec<SelectedInstruction>) {
                (
                    [
                        Some(self.address_register),
                        self.carrier_register,
                        Some(self.reload_register),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    [Some(self.address), Some(self.load), self.convert]
                        .into_iter()
                        .flatten()
                        .collect(),
                )
            }
        }

        pub(crate) fn reload(
            admitted: &RuntimeSpillDerivation<'_>,
            slot: LocalStorageSlotId,
            register: VirtualRegisterId,
            next_instruction: &mut u32,
            next_register: &mut u32,
        ) -> Result<Reload, RuntimeSpillError> {
            let conversion = admitted.bits_conversion;
            let address_instruction = SelectedInstructionId(fresh(next_instruction)?);
            let load_instruction = SelectedInstructionId(fresh(next_instruction)?);
            let convert_instruction = if conversion.is_some() {
                Some(SelectedInstructionId(fresh(next_instruction)?))
            } else {
                None
            };
            let address_register = VirtualRegisterId(fresh(next_register)?);
            let carrier_id = if conversion.is_some() {
                Some(VirtualRegisterId(fresh(next_register)?))
            } else {
                None
            };
            let reload_register = VirtualRegisterId(fresh(next_register)?);
            let loaded = carrier_id.unwrap_or(reload_register);
            let restating = convert_instruction.unwrap_or(load_instruction);
            Ok(Reload {
                address_register: VirtualRegister {
                    id: address_register,
                    scalar_type: admitted.address_scalar_type,
                    class: admitted.carrier_class,
                    origin: VirtualRegisterOrigin::SpillAddress {
                        instruction: address_instruction,
                        register,
                    },
                    definition_site: None,
                    entry_fixed_view: None,
                },
                carrier_register: carrier_id.map(|id| VirtualRegister {
                    id,
                    scalar_type: admitted.address_scalar_type,
                    class: admitted.carrier_class,
                    origin: match admitted.lineage {
                        VictimLineage::Scalar(source_value) => {
                            VirtualRegisterOrigin::InstructionResult {
                                instruction: load_instruction,
                                source_value,
                            }
                        }
                        VictimLineage::Structural { place, byte_offset } => {
                            VirtualRegisterOrigin::StructuralObservation {
                                instruction: load_instruction,
                                place,
                                byte_offset,
                            }
                        }
                    },
                    definition_site: admitted.victim.definition_site,
                    entry_fixed_view: None,
                }),
                reload_register: VirtualRegister {
                    id: reload_register,
                    scalar_type: admitted.victim.scalar_type,
                    class: admitted.victim.class,
                    origin: match admitted.lineage {
                        VictimLineage::Scalar(source_value) => {
                            VirtualRegisterOrigin::InstructionResult {
                                instruction: restating,
                                source_value,
                            }
                        }
                        VictimLineage::Structural { place, byte_offset } => {
                            VirtualRegisterOrigin::StructuralObservation {
                                instruction: restating,
                                place,
                                byte_offset,
                            }
                        }
                    },
                    definition_site: admitted.victim.definition_site,
                    entry_fixed_view: None,
                },
                address: crate::rewrites::instruction(
                    address_instruction,
                    SelectedInstructionKind::FrameAddress {
                        slot: frame(slot),
                        byte_offset: 0,
                    },
                    admitted.address,
                    &[address_register],
                ),
                load: crate::rewrites::instruction(
                    load_instruction,
                    SelectedInstructionKind::Load64 { byte_offset: 0 },
                    admitted.load,
                    &[address_register, loaded],
                ),
                convert: conversion.map(|conversion| {
                    crate::rewrites::instruction(
                        convert_instruction.unwrap_or(restating),
                        conversion.from_kind,
                        conversion.from_bits,
                        &[loaded, reload_register],
                    )
                }),
            })
        }

        pub(crate) struct Store {
            pub carrier_register: Option<VirtualRegister>,
            address_scratch: Option<VirtualRegister>,
            pub convert: Option<SelectedInstruction>,
            pub store: SelectedInstruction,
        }

        impl Store {
            pub(crate) fn into_streams(self) -> (Vec<VirtualRegister>, Vec<SelectedInstruction>) {
                (
                    self.carrier_register
                        .into_iter()
                        .chain(self.address_scratch)
                        .collect(),
                    [self.convert, Some(self.store)]
                        .into_iter()
                        .flatten()
                        .collect(),
                )
            }
        }

        pub(crate) fn store(
            admitted: &RuntimeSpillDerivation<'_>,
            slot: LocalStorageSlotId,
            stored: VirtualRegisterId,
            next_instruction: &mut u32,
            next_register: &mut u32,
        ) -> Result<Store, RuntimeSpillError> {
            let conversion = admitted.bits_conversion;
            let convert_instruction = if conversion.is_some() {
                Some(SelectedInstructionId(fresh(next_instruction)?))
            } else {
                None
            };
            let store_instruction = SelectedInstructionId(fresh(next_instruction)?);
            let carrier_id = if conversion.is_some() {
                Some(VirtualRegisterId(fresh(next_register)?))
            } else {
                None
            };
            let stored_register = carrier_id.unwrap_or(stored);
            let address_scratch = admitted
                .store
                .operands
                .get(1)
                .map(|operand| {
                    Ok::<_, RuntimeSpillError>(VirtualRegister {
                        id: VirtualRegisterId(fresh(next_register)?),
                        scalar_type: admitted.address_scalar_type,
                        class: operand.class,
                        origin: VirtualRegisterOrigin::InstructionScratch {
                            instruction: store_instruction,
                            operand: operand.operand,
                        },
                        definition_site: None,
                        entry_fixed_view: None,
                    })
                })
                .transpose()?;
            let store_registers = std::iter::once(stored_register)
                .chain(address_scratch.as_ref().map(|register| register.id))
                .collect::<Vec<_>>();
            Ok(Store {
                carrier_register: carrier_id.map(|id| VirtualRegister {
                    id,
                    scalar_type: admitted.address_scalar_type,
                    class: admitted.carrier_class,
                    origin: match admitted.lineage {
                        VictimLineage::Scalar(source_value) => {
                            VirtualRegisterOrigin::InstructionResult {
                                instruction: convert_instruction.unwrap_or(store_instruction),
                                source_value,
                            }
                        }
                        VictimLineage::Structural { place, byte_offset } => {
                            VirtualRegisterOrigin::StructuralObservation {
                                instruction: convert_instruction.unwrap_or(store_instruction),
                                place,
                                byte_offset,
                            }
                        }
                    },
                    definition_site: admitted.victim.definition_site,
                    entry_fixed_view: None,
                }),
                address_scratch,
                convert: conversion.map(|conversion| {
                    crate::rewrites::instruction(
                        convert_instruction.unwrap_or(store_instruction),
                        conversion.to_kind,
                        conversion.to_bits,
                        &[stored, stored_register],
                    )
                }),
                store: crate::rewrites::instruction(
                    store_instruction,
                    SelectedInstructionKind::Store64 {
                        slot: frame(slot),
                        byte_offset: 0,
                    },
                    admitted.store,
                    &store_registers,
                ),
            })
        }

        pub(crate) fn frame(slot: LocalStorageSlotId) -> FrameStorageSlotId {
            FrameStorageSlotId::Local(slot)
        }
    }
    pub(crate) mod case_payload_address {
        use selected_instructions::{SelectedFunction, SelectedInstructionKind};
        use semantic_vocabulary::EdgeId;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn case_payload_address_corresponds(
            function: &SelectedFunction,
            block_index: usize,
            load: &selected_instructions::SelectedInstruction,
            source: selected_instructions::SelectedCaseDispatchSource,
            edge: EdgeId,
            field_byte_offset: u32,
            byte_count: u32,
            displacement: u32,
        ) -> bool {
            use crate::rewrites::operand_probes::last_definition_before;
            use register_model::RegisterOperandAccess;
            use selected_instructions::{
                FrameStorageSlotId, SelectedCaseDispatchSource, SelectedMemoryAccessOrigin,
                SelectedMemoryAccessRole,
            };
            let Some(place) = source.place() else {
                return false;
            };
            let byte_bound = match source {
                SelectedCaseDispatchSource::Local { slot } => {
                    let Some(slot) = function
                        .local_storage_slots
                        .iter()
                        .find(|row| row.id == slot)
                    else {
                        return false;
                    };
                    slot.byte_size
                }
                SelectedCaseDispatchSource::Borrowed { byte_size, .. } => byte_size,
            };
            if field_byte_offset
                .checked_add(byte_count)
                .is_none_or(|end| end > byte_bound)
                || !field_byte_offset.is_multiple_of(byte_count)
            {
                return false;
            }
            let mut accesses = function
                .memory_accesses
                .iter()
                .filter(|row| row.instruction == load.id);
            let Some(access) = accesses.next() else {
                return false;
            };
            if accesses.next().is_some()
                || access.origin != SelectedMemoryAccessOrigin::Edge(edge)
                || access.role != SelectedMemoryAccessRole::ReadPlace
                || access.place != place
                || access.byte_offset != field_byte_offset
                || access.byte_count != byte_count
            {
                return false;
            }
            let clean_edge_instruction =
                |instruction: &selected_instructions::SelectedInstruction| {
                    instruction.provenance.edges == [edge]
                        && instruction.implicit_uses.is_empty()
                        && instruction.implicit_defs.is_empty()
                        && instruction.clobbers.is_empty()
                };
            let [pointer_operand, result_operand] = load.operands.as_slice() else {
                return false;
            };
            if !clean_edge_instruction(load)
                || pointer_operand.operand != 0
                || pointer_operand.access != RegisterOperandAccess::Use
                || result_operand.operand != 1
                || result_operand.access != RegisterOperandAccess::Def
            {
                return false;
            }
            let instructions = &function.blocks[block_index].instructions;
            let Some(mut position) = instructions.iter().position(|row| row.id == load.id) else {
                return false;
            };
            let mut pointer = pointer_operand.virtual_register;
            let mut offset = displacement;
            loop {
                let definition = last_definition_before(instructions, position, pointer);
                if let SelectedCaseDispatchSource::Borrowed { pointer: root, .. } = source
                    && pointer == root
                {
                    return definition.is_none() && offset == field_byte_offset;
                }
                let Some(definition) = definition else {
                    return false;
                };
                let producer = &instructions[definition];
                if !clean_edge_instruction(producer) {
                    return false;
                }
                match producer.kind {
                    SelectedInstructionKind::AddressOffset { byte_offset } => {
                        let [base, result] = producer.operands.as_slice() else {
                            return false;
                        };
                        if base.operand != 0
                            || base.access != RegisterOperandAccess::Use
                            || result.operand != 1
                            || result.access != RegisterOperandAccess::Def
                            || result.virtual_register != pointer
                            || base.virtual_register == pointer
                        {
                            return false;
                        }
                        let Some(combined) = offset.checked_add(byte_offset) else {
                            return false;
                        };
                        offset = combined;
                        pointer = base.virtual_register;
                        position = definition;
                    }
                    SelectedInstructionKind::FrameAddress {
                        slot,
                        byte_offset: 0,
                    } => {
                        let SelectedCaseDispatchSource::Local { slot: root } = source else {
                            return false;
                        };
                        let [result] = producer.operands.as_slice() else {
                            return false;
                        };
                        return slot == FrameStorageSlotId::Local(root)
                            && result.operand == 0
                            && result.access == RegisterOperandAccess::Def
                            && result.virtual_register == pointer
                            && offset == field_byte_offset;
                    }
                    _ => return false,
                }
            }
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            LocalStorageSlotId, SelectedCasePayloadTransport, SelectedInstruction,
            SelectedLocalStorageSlot, SelectedStructuralTransport, SelectedValueTransport,
            VirtualRegister, VirtualRegisterId,
        };

        use super::{
            RuntimeSpillDerivation, RuntimeSpillError, RuntimeSpillSpanPolicy,
            ValidatedRuntimeSpill, admission, derive_runtime_spill,
        };
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::settlement_gaps::{gap_tables, push_position, remap};

        fn reload_for_use(
            admitted: &RuntimeSpillDerivation<'_>,
            slot: LocalStorageSlotId,
            register: VirtualRegisterId,
            share: bool,
            open: &mut Option<VirtualRegisterId>,
            virtual_registers: &mut Vec<VirtualRegister>,
            instructions: &mut Vec<SelectedInstruction>,
            next_instruction: &mut u32,
            next_register: &mut u32,
        ) -> Result<VirtualRegisterId, RuntimeSpillError> {
            if share && let Some(existing) = *open {
                return Ok(existing);
            }
            let reload =
                admission::reload(admitted, slot, register, next_instruction, next_register)?;
            let reloaded = reload.reload_register.id;
            let (registers, sequence) = reload.into_streams();
            virtual_registers.extend(registers);
            instructions.extend(sequence);
            if share {
                *open = Some(reloaded);
            }
            Ok(reloaded)
        }

        pub fn spill_selected_runtime_value(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            register: VirtualRegisterId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
            spill_selected_runtime_value_with_span_policy(
                source,
                function_index,
                register,
                environment,
                budget,
                RuntimeSpillSpanPolicy::UnitWriteBounded,
            )
        }

        pub fn spill_selected_runtime_value_with_span_policy(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            register: VirtualRegisterId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
            span_policy: RuntimeSpillSpanPolicy,
        ) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
            spill_selected_runtime_value_with_crossing_status(
                source,
                function_index,
                register,
                environment,
                budget,
                span_policy,
            )
            .map(|(spill, _)| spill)
        }

        pub fn spill_selected_runtime_value_with_crossing_status(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            register: VirtualRegisterId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
            span_policy: RuntimeSpillSpanPolicy,
        ) -> Result<(ValidatedRuntimeSpill, bool), RuntimeSpillError> {
            let admitted = derive_runtime_spill(
                source,
                function_index,
                register,
                environment,
                span_policy,
                budget,
            )?;
            let crossed_unit_write = admitted.crossed_unit_write;
            let transformed = (|| -> Result<_, RuntimeSpillError> {
                let mut transformed = source.selected_plan().clone();
                transformed
                    .functions
                    .mutate_preserving_register_prefix(
                        function_index,
                        |local_storage_slots, virtual_registers, boundary_settlements, blocks| -> Result<(), RuntimeSpillError> {
                        if admitted.assignment.declares_private {
                            local_storage_slots.push(SelectedLocalStorageSlot {
                                id: admitted.assignment.private,
                                byte_size: 8,
                                alignment: 8,
                            });
                        }
                        let mut next_instruction = admitted.first_instruction;
                        let mut next_register = admitted.first_register;
                        for (block_index, block) in admitted.function.blocks.iter().enumerate() {
                            if !admitted.use_blocks.contains(&block_index)
                                && !admitted
                                    .definitions
                                    .iter()
                                    .any(|definition| definition.block_index == block_index)
                            {
                                continue;
                            }
                            let shared = admitted.shared_reload[block_index];
                            let mut open_reload = None;
                            let mut use_reloads = std::collections::BTreeMap::new();
                            let has_settlements = boundary_settlements
                                .iter()
                                .any(|settlement| settlement.block == block.id);
                            let mut instructions = Vec::with_capacity(block.instructions.len() + 8);
                            let (mut boundaries, mut positions) = gap_tables(has_settlements, block);
                            let mut has_block_start_store = false;
                            for (definition_index, definition) in
                                admitted
                                    .definitions
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, definition)| {
                                        definition.block_index == block_index
                                            && matches!(
                                                definition.position,
                                                super::StoragePosition::BlockStart
                                            )
                                    })
                            {
                                has_block_start_store = true;
                                let store = admission::store(
                                    &admitted,
                                    admitted.assignment.store_slot(definition_index),
                                    definition.register,
                                    &mut next_instruction,
                                    &mut next_register,
                                )?;
                                let (registers, sequence) = store.into_streams();
                                virtual_registers.extend(registers);
                                instructions.extend(sequence);
                            }
                            let mut window_has_interior = true;
                            let active_window = if has_settlements || block.instructions.is_empty() {
                                if block.instructions.is_empty() {
                                    None
                                } else {
                                    Some((0usize, block.instructions.len() - 1))
                                }
                            } else {
                                let mut window = admitted
                                    .function
                                    .blocks
                                    .block_register_instruction_span_detail(block_index, register)
                                    .map(|(s, e, interior)| {
                                        window_has_interior = interior;
                                        (s, e)
                                    });
                                for definition in &admitted.definitions {
                                    if definition.block_index != block_index {
                                        continue;
                                    }
                                    if let super::StoragePosition::AfterInstruction(anchor)
                                    | super::StoragePosition::AfterUseDef {
                                        instruction: anchor,
                                        ..
                                    } = definition.position
                                        && let Some(idx) = admitted
                                            .function
                                            .blocks
                                            .instruction_index_in_block(block_index, anchor)
                                    {
                                        window = Some(match window {
                                            Some((s, e)) => {
                                                if idx != s && idx != e {
                                                    window_has_interior = true;
                                                }
                                                (s.min(idx), e.max(idx))
                                            }
                                            None => {
                                                window_has_interior = false;
                                                (idx, idx)
                                            }
                                        });
                                    }
                                }
                                window
                            };
                            let unchanged_prefix = if has_block_start_store {
                                0
                            } else {
                                active_window.map_or(block.instructions.len(), |(start, _)| start)
                            };
                            if let Some((active_start, active_end)) = active_window {
                                if active_start > 0 {
                                    instructions.extend_from_slice(&block.instructions[..active_start]);
                                }
                                let has_retained_uses = !admitted.retained_uses.is_empty();
                                let block_dedicated_ties = &admitted.dedicated_tie_uses[block_index];
                                let has_dedicated_ties = !block_dedicated_ties.is_empty();
                                let block_span_closes = &admitted.span_closes[block_index];
                                let has_span_closes = !block_span_closes.is_empty();
                                let has_after_defs = admitted.definitions.iter().any(|definition| {
                                    definition.block_index == block_index
                                        && !matches!(definition.position, super::StoragePosition::BlockStart)
                                });
                                let mut rewrite_instruction =
                                    |instruction_index: usize,
                                     original: &selected_instructions::SelectedInstruction,
                                     open_reload: &mut Option<VirtualRegisterId>,
                                     virtual_registers: &mut Vec<VirtualRegister>,
                                     instructions: &mut Vec<selected_instructions::SelectedInstruction>,
                                     next_instruction: &mut u32,
                                     next_register: &mut u32|
                                     -> Result<(), RuntimeSpillError> {
                                        push_position(&mut boundaries, has_settlements, instructions.len())
                                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                                        let mut rewritten = original.clone();
                                        for operand in &mut rewritten.operands {
                                            if operand.virtual_register != register
                                                || (has_retained_uses
                                                    && admitted
                                                        .retained_uses
                                                        .contains(&(original.id, operand.operand)))
                                                || !matches!(
                                                    operand.access,
                                                    RegisterOperandAccess::Use
                                                        | RegisterOperandAccess::UseDef
                                                )
                                            {
                                                continue;
                                            }
                                            let dedicated = has_dedicated_ties
                                                && block_dedicated_ties
                                                    .contains(&(instruction_index, operand.operand));
                                            let reloaded = reload_for_use(
                                                &admitted,
                                                admitted
                                                    .assignment
                                                    .load_slot(block_index, instruction_index),
                                                register,
                                                shared && operand.fixed_view.is_none() && !dedicated,
                                                open_reload,
                                                virtual_registers,
                                                instructions,
                                                next_instruction,
                                                next_register,
                                            )?;
                                            use_reloads.entry(rewritten.id).or_insert(reloaded);
                                            operand.virtual_register = reloaded;
                                        }
                                        push_position(&mut positions, has_settlements, instructions.len())
                                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                                        instructions.push(rewritten);
                                        if has_span_closes
                                            && open_reload.is_some()
                                            && block_span_closes.binary_search(&instruction_index).is_ok()
                                        {
                                            *open_reload = None;
                                        }
                                        if has_after_defs {
                                            for (definition_index, definition) in
                                                admitted
                                                    .definitions
                                                    .iter()
                                                    .enumerate()
                                                    .filter(|(_, definition)| {
                                                        definition.block_index == block_index
                                                            && match definition.position {
                                                                super::StoragePosition::AfterInstruction(
                                                                    anchor,
                                                                )
                                                                | super::StoragePosition::AfterUseDef {
                                                                    instruction: anchor,
                                                                    ..
                                                                } => anchor == original.id,
                                                                super::StoragePosition::BlockStart => false,
                                                            }
                                                    })
                                            {
                                                let stored = match definition.position {
                                                    super::StoragePosition::AfterUseDef {
                                                        operand, ..
                                                    } => instructions
                                                        .last()
                                                        .and_then(|emitted| emitted.operand(operand))
                                                        .map(|operand| operand.virtual_register)
                                                        .ok_or(RuntimeSpillError::SourceMismatch)?,
                                                    _ => definition.register,
                                                };
                                                let store = admission::store(
                                                    &admitted,
                                                    admitted.assignment.store_slot(definition_index),
                                                    stored,
                                                    next_instruction,
                                                    next_register,
                                                )?;
                                                let (registers, sequence) = store.into_streams();
                                                virtual_registers.extend(registers);
                                                instructions.extend(sequence);
                                            }
                                        }
                                        Ok(())
                                    };
                                if !has_settlements && !window_has_interior {
                                    rewrite_instruction(
                                        active_start,
                                        &block.instructions[active_start],
                                        &mut open_reload,
                                        virtual_registers,
                                        &mut instructions,
                                        &mut next_instruction,
                                        &mut next_register,
                                    )?;
                                    if active_end > active_start {
                                        if active_start + 1 < active_end {
                                            instructions.extend_from_slice(
                                                &block.instructions[active_start + 1..active_end],
                                            );
                                            if has_span_closes
                                                && open_reload.is_some()
                                                && block_span_closes[block_span_closes
                                                    .partition_point(|&idx| idx <= active_start)..]
                                                    .first()
                                                    .is_some_and(|&idx| idx < active_end)
                                            {
                                                open_reload = None;
                                            }
                                        }
                                        rewrite_instruction(
                                            active_end,
                                            &block.instructions[active_end],
                                            &mut open_reload,
                                            virtual_registers,
                                            &mut instructions,
                                            &mut next_instruction,
                                            &mut next_register,
                                        )?;
                                    }
                                } else {
                                    for (offset, original) in block.instructions[active_start..=active_end]
                                        .iter()
                                        .enumerate()
                                    {
                                        rewrite_instruction(
                                            active_start + offset,
                                            original,
                                            &mut open_reload,
                                            virtual_registers,
                                            &mut instructions,
                                            &mut next_instruction,
                                            &mut next_register,
                                        )?;
                                    }
                                }
                                if active_end + 1 < block.instructions.len() {
                                    instructions
                                        .extend_from_slice(&block.instructions[active_end + 1..]);
                                }
                            } else if !block.instructions.is_empty() {
                                instructions.extend_from_slice(&block.instructions);
                            }
                            let pre_terminator_len = instructions.len();
                            let mut unshared = None;
                            let mut end_of_block_reload = |register: VirtualRegisterId| {
                                reload_for_use(
                                    &admitted,
                                    admitted.assignment.end_of_block_slot(block_index),
                                    register,
                                    false,
                                    &mut unshared,
                                    virtual_registers,
                                    &mut instructions,
                                    &mut next_instruction,
                                    &mut next_register,
                                )
                            };
                            let mut terminator = block.terminator.clone();
                            for operand in &mut super::control_mut(&mut terminator).operands {
                                if operand.virtual_register != register
                                    || operand.access != RegisterOperandAccess::Use
                                {
                                    continue;
                                }
                                operand.virtual_register = end_of_block_reload(register)?;
                            }
                            for successor in super::control_successors_mut(&mut terminator)
                                .into_iter()
                                .flatten()
                            {
                                for binding in &mut successor.bindings {
                                    if !matches!(
                                        binding.transport,
                                        SelectedValueTransport::Registers { argument, .. } if argument == register)
                                    {
                                        continue;
                                    }
                                    let reloaded = end_of_block_reload(register)?;
                                    let SelectedValueTransport::Registers { argument, .. } =
                                        &mut binding.transport
                                    else {
                                        unreachable!()
                                    };
                                    *argument = reloaded;
                                }
                                for binding in &mut successor.structural_bindings {
                                    if let SelectedStructuralTransport::ScalarReference { base, .. }
                                    | SelectedStructuralTransport::Address { base, .. } =
                                        &mut binding.transport
                                        && *base
                                            == selected_instructions::SelectedAddressBase::Register(
                                                register,
                                            )
                                    {
                                        let reloaded = end_of_block_reload(register)?;
                                        *base = selected_instructions::SelectedAddressBase::Register(
                                            reloaded,
                                        );
                                        continue;
                                    }
                                    let Some(byte_size) =
                                        super::stored_transport_size(binding.transport)
                                    else {
                                        continue;
                                    };
                                    let SelectedStructuralTransport::WholeValue {
                                        argument,
                                        source_byte_offset,
                                        ..
                                    } = &mut binding.transport
                                    else {
                                        continue;
                                    };
                                    if *argument != register {
                                        continue;
                                    }
                                    let Some(pending) = admitted.structural_uses.iter().find(|pending| {
                                        pending.block == block_index
                                            && Some(pending.edge) == successor.psi_edge
                                            && pending.place == binding.semantic.argument.place
                                            && pending.source_byte_offset == *source_byte_offset
                                            && pending.byte_size == byte_size
                                    }) else {
                                        return Err(RuntimeSpillError::SourceMismatch);
                                    };
                                    let reloaded = if pending.dedicated_reload {
                                        end_of_block_reload(register)?
                                    } else {
                                        pending
                                            .loads
                                            .iter()
                                            .try_fold(None, |found: Option<VirtualRegisterId>, load| {
                                                let named = use_reloads
                                                    .get(load)
                                                    .copied()
                                                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                                                match found {
                                                    None => Ok(Some(named)),
                                                    Some(existing) if existing == named => Ok(found),
                                                    _ => Err(RuntimeSpillError::SourceMismatch),
                                                }
                                            })?
                                            .ok_or(RuntimeSpillError::SourceMismatch)?
                                    };
                                    *argument = reloaded;
                                }
                                if let Some(case) = &mut successor.structural_case {
                                    for payload in &mut case.payloads {
                                        if !matches!(
                                            payload.transport,
                                            SelectedCasePayloadTransport::Registers { argument, .. }
                                                if argument == register)
                                        {
                                            continue;
                                        }
                                        let reloaded = end_of_block_reload(register)?;
                                        let SelectedCasePayloadTransport::Registers { argument, .. } =
                                            &mut payload.transport
                                        else {
                                            unreachable!()
                                        };
                                        *argument = reloaded;
                                    }
                                }
                            }
                            push_position(&mut boundaries, has_settlements, instructions.len())
                                .ok_or(RuntimeSpillError::IdentityOverflow)?;
                            push_position(&mut positions, has_settlements, instructions.len())
                                .ok_or(RuntimeSpillError::IdentityOverflow)?;
                            if has_settlements {
                                remap(boundary_settlements, &boundaries, &positions, block.id)
                                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                            }
                            let unchanged_suffix = if instructions.len() == pre_terminator_len {
                                active_window
                                    .map_or(block.instructions.len(), |(_, end)| {
                                        block.instructions.len() - (end + 1)
                                    })
                                    .min(block.instructions.len() - unchanged_prefix)
                            } else {
                                0
                            };
                            blocks.replace_block_contents(
                                block_index,
                                instructions,
                                terminator,
                                unchanged_prefix,
                                unchanged_suffix,
                            );
                        }
                        Ok(())
                    },
                )?;
                Ok(transformed)
            })()?;
            let sealed =
                super::validation::seal_runtime_spill_with_span_policy(source, transformed)?;
            Ok((sealed, crossed_unit_write))
        }
    }
    pub(crate) mod slot {
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterClassId, RegisterOperandAccess};
        use selected_instructions::{
            LocalStorageSlotId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
            SelectedInstructionKind, VirtualRegister, VirtualRegisterId,
        };
        use semantic_vocabulary::{IeeeFloatFormat, ScalarType};

        use super::{BitsConversion, RuntimeSpillError, StorageDefinition, StoragePosition};
        pub(crate) mod reaching {
            use std::collections::VecDeque;

            pub(crate) struct DefinitionEntries {
                words: usize,
                entries: Vec<u64>,
            }
            impl DefinitionEntries {
                pub(crate) fn compute(
                    count: usize,
                    last: &[Option<usize>],
                    successors: &[Vec<usize>],
                ) -> Self {
                    let words = count.div_ceil(64);
                    let mut result = Self {
                        words,
                        entries: vec![0; words * last.len()],
                    };
                    let mut pending: VecDeque<_> = last
                        .iter()
                        .enumerate()
                        .filter_map(|(block, value)| value.map(|_| block))
                        .collect();
                    let mut queued: Vec<_> = last.iter().map(Option::is_some).collect();
                    let mut state = result.scratch();
                    while let Some(block) = pending.pop_front() {
                        queued[block] = false;
                        if let Some(definition) = last[block] {
                            replace(&mut state, definition);
                        } else {
                            result.copy_into(block, &mut state);
                        }
                        for &destination in &successors[block] {
                            let row =
                                &mut result.entries[destination * words..(destination + 1) * words];
                            let mut changed = false;
                            for (word, incoming) in row.iter_mut().zip(&state) {
                                let merged = *word | *incoming;
                                changed |= merged != *word;
                                *word = merged;
                            }
                            if changed && last[destination].is_none() && !queued[destination] {
                                queued[destination] = true;
                                pending.push_back(destination);
                            }
                        }
                    }
                    result
                }
                pub(crate) fn scratch(&self) -> Vec<u64> {
                    vec![0; self.words]
                }
                pub(crate) fn copy_into(&self, block: usize, state: &mut [u64]) {
                    state.copy_from_slice(
                        &self.entries[block * self.words..(block + 1) * self.words],
                    );
                }
            }
            pub(crate) fn replace(state: &mut [u64], definition: usize) {
                state.fill(0);
                state[definition / 64] = 1u64 << (definition % 64);
            }
            pub(crate) fn members(state: &[u64]) -> impl Iterator<Item = usize> + '_ {
                state
                    .iter()
                    .copied()
                    .enumerate()
                    .flat_map(|(word, mut bits)| {
                        std::iter::from_fn(move || {
                            if bits == 0 {
                                return None;
                            }
                            let bit = bits.trailing_zeros() as usize;
                            bits &= bits - 1;
                            Some(word * 64 + bit)
                        })
                    })
            }
            #[cfg(test)]
            mod tests {
                use super::*;
                #[test]
                fn cyclic_joins_preserve_definition_bits_across_word_boundaries() {
                    let entries = DefinitionEntries::compute(
                        130,
                        &[Some(0), Some(65), Some(129), None],
                        &[vec![3], vec![3], vec![3], vec![0, 1, 2]],
                    );
                    let mut state = entries.scratch();
                    for block in 0..4 {
                        entries.copy_into(block, &mut state);
                        assert_eq!(members(&state).collect::<Vec<_>>(), vec![0, 65, 129]);
                    }
                    let entries = DefinitionEntries::compute(
                        65,
                        &[Some(63), None, Some(64), None],
                        &[vec![1], vec![2], vec![3], vec![]],
                    );
                    for (block, expected) in [vec![], vec![63], vec![63], vec![64]]
                        .into_iter()
                        .enumerate()
                    {
                        entries.copy_into(block, &mut state[..2]);
                        assert_eq!(members(&state[..2]).collect::<Vec<_>>(), expected);
                    }
                }
            }
        }

        #[derive(Debug, Default)]
        pub(crate) struct BlockUsePositions {
            pub unpinned: Vec<usize>,
            pub pinned: Vec<usize>,
            pub dedicated: Vec<usize>,
            pub end_of_block: bool,
        }

        impl BlockUsePositions {
            pub(crate) fn record_use(
                &mut self,
                instruction_index: usize,
                fixed_view: bool,
                flexible: &mut bool,
            ) {
                let recorded = if fixed_view {
                    &mut self.pinned
                } else {
                    *flexible = true;
                    &mut self.unpinned
                };
                if recorded.last() != Some(&instruction_index) {
                    recorded.push(instruction_index);
                }
            }
        }

        pub(crate) fn instruction_block(
            function: &SelectedFunction,
            instruction: SelectedInstructionId,
        ) -> Option<usize> {
            function.blocks.instruction_block(instruction).or_else(|| {
                function
                    .blocks
                    .iter()
                    .position(|block| super::control(&block.terminator).0.id == instruction)
            })
        }

        #[derive(Debug)]
        pub(crate) struct Assignment {
            pub stores: Vec<LocalStorageSlotId>,
            pub loads: Vec<BlockLoads>,
            pub declares_private: bool,
            pub private: LocalStorageSlotId,
        }

        #[derive(Debug, Default)]
        pub(crate) struct BlockLoads {
            pub instructions: std::collections::BTreeMap<usize, LocalStorageSlotId>,
            pub end_of_block: Option<LocalStorageSlotId>,
        }

        impl Assignment {
            pub(crate) fn store_slot(&self, definition: usize) -> LocalStorageSlotId {
                self.stores[definition]
            }

            pub(crate) fn load_slot(&self, block: usize, index: usize) -> LocalStorageSlotId {
                self.loads
                    .get(block)
                    .and_then(|loads| loads.instructions.get(&index).copied())
                    .unwrap_or(self.private)
            }

            pub(crate) fn end_of_block_slot(&self, block: usize) -> LocalStorageSlotId {
                self.loads
                    .get(block)
                    .and_then(|loads| loads.end_of_block)
                    .unwrap_or(self.private)
            }
        }

        const EXISTING: u8 = 1;
        const NEW: u8 = 2;

        #[derive(Debug, Clone, Copy)]
        enum Event {
            ExistingStore,
            ExistingLoad,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        enum StorePosition {
            Boundary,
            Instruction(usize),
        }

        struct CandidateAccesses {
            accesses: Vec<Vec<(usize, Event)>>,
            terminal_loads: Vec<bool>,
            last_existing_store: Vec<Option<usize>>,
            store_blocks: Vec<usize>,
            load_blocks: Vec<usize>,
            is_entry_insensitive: bool,
        }

        struct WindowSummary {
            boundary_store: Vec<bool>,
            anchored_stores: Vec<Vec<usize>>,
            last_store: Vec<Option<StorePosition>>,
            has_loads: Vec<bool>,
            store_blocks: Vec<usize>,
            load_blocks: Vec<usize>,
            active_blocks: Vec<usize>,
            is_entry_insensitive: bool,
        }

        struct ShareableScratch {
            exits: Vec<Option<u8>>,
            entries: Vec<u8>,
            queued: Vec<bool>,
            pending: std::collections::VecDeque<usize>,
        }

        impl ShareableScratch {
            fn new(block_count: usize) -> Self {
                Self {
                    exits: vec![None; block_count],
                    entries: vec![0; block_count],
                    queued: vec![false; block_count],
                    pending: std::collections::VecDeque::with_capacity(block_count),
                }
            }
        }

        pub(crate) fn assign(
            function: &SelectedFunction,
            victim: VirtualRegisterId,
            definitions: &[StorageDefinition],
            uses: &[BlockUsePositions],
            shared_reload: &[bool],
            span_closes: &[Vec<usize>],
            reuse_slots: bool,
        ) -> Assignment {
            let private = LocalStorageSlotId::Spill { register: victim };
            let block_count = function.blocks.len();
            let mut candidate_slots: Vec<LocalStorageSlotId> = Vec::new();
            if reuse_slots {
                for slot in &function.local_storage_slots {
                    if slot.byte_size == 8
                        && slot.alignment == 8
                        && matches!(slot.id, LocalStorageSlotId::Spill { register } if register != victim)
                        && !candidate_slots.contains(&slot.id)
                    {
                        candidate_slots.push(slot.id);
                    }
                }
            }
            if candidate_slots.is_empty() {
                let mut any_emitted_load = false;
                if definitions.is_empty() {
                    for (block_index, block) in function.blocks.iter().enumerate() {
                        let positions = &uses[block_index];
                        if positions.end_of_block {
                            any_emitted_load = true;
                            break;
                        }
                        if positions.unpinned.is_empty()
                            && positions.pinned.is_empty()
                            && positions.dedicated.is_empty()
                        {
                            continue;
                        }
                        for &index in positions
                            .unpinned
                            .iter()
                            .chain(&positions.pinned)
                            .chain(&positions.dedicated)
                        {
                            if index < block.instructions.len()
                                && loads_before(
                                    uses,
                                    shared_reload,
                                    span_closes,
                                    block_index,
                                    index,
                                )
                            {
                                any_emitted_load = true;
                                break;
                            }
                        }
                        if any_emitted_load {
                            break;
                        }
                    }
                }
                return Assignment {
                    stores: vec![private; definitions.len()],
                    loads: Vec::new(),
                    declares_private: !definitions.is_empty() || any_emitted_load,
                    private,
                };
            }
            let mut emitted: Vec<Vec<usize>> = vec![Vec::new(); block_count];
            for (block_index, block) in function.blocks.iter().enumerate() {
                let positions = &uses[block_index];
                if positions.unpinned.is_empty()
                    && positions.pinned.is_empty()
                    && positions.dedicated.is_empty()
                {
                    continue;
                }
                let row = &mut emitted[block_index];
                for &index in positions
                    .unpinned
                    .iter()
                    .chain(&positions.pinned)
                    .chain(&positions.dedicated)
                {
                    if index < block.instructions.len()
                        && loads_before(uses, shared_reload, span_closes, block_index, index)
                    {
                        row.push(index);
                    }
                }
                row.sort_unstable();
                row.dedup();
            }
            let mut anchored: Vec<std::collections::BTreeMap<usize, Vec<usize>>> = (0..block_count)
                .map(|_| std::collections::BTreeMap::new())
                .collect();
            let mut boundary: Vec<Vec<usize>> = vec![Vec::new(); block_count];
            for (definition, storage) in definitions.iter().enumerate() {
                match storage.position {
                    StoragePosition::BlockStart => boundary[storage.block_index].push(definition),
                    StoragePosition::AfterInstruction(anchor)
                    | StoragePosition::AfterUseDef {
                        instruction: anchor,
                        ..
                    } => {
                        if let Some(index) = function
                            .blocks
                            .instruction_index_in_block(storage.block_index, anchor)
                        {
                            anchored[storage.block_index]
                                .entry(index)
                                .or_default()
                                .push(definition);
                        }
                    }
                }
            }
            let max_block_id = function
                .blocks
                .iter()
                .map(|block| block.id.0 as usize)
                .max()
                .unwrap_or(0);
            let mut block_positions = vec![u32::MAX; max_block_id + 1];
            for (block_index, block) in function.blocks.iter().enumerate() {
                block_positions[block.id.0 as usize] = block_index as u32;
            }
            let successors: Vec<Vec<usize>> = function
                .blocks
                .iter()
                .map(|block| {
                    super::control(&block.terminator)
                        .1
                        .into_iter()
                        .flatten()
                        .filter_map(|successor| {
                            block_positions
                                .get(successor.block.0 as usize)
                                .copied()
                                .filter(|&pos| pos != u32::MAX)
                                .map(|pos| pos as usize)
                        })
                        .collect()
                })
                .collect();
            let last_definitions: Vec<_> = (0..block_count)
                .map(|block| {
                    anchored[block]
                        .values()
                        .rev()
                        .find_map(|rows| rows.last().copied())
                        .or_else(|| boundary[block].last().copied())
                })
                .collect();
            let entries = reaching::DefinitionEntries::compute(
                definitions.len(),
                &last_definitions,
                &successors,
            );
            fn union(parent: &mut [usize], left: usize, right: usize) {
                let left = find(parent, left);
                let right = find(parent, right);
                parent[left] = right;
            }
            fn merge(parent: &mut [usize], state: &[u64]) -> Option<usize> {
                let mut members = reaching::members(state);
                let first = members.next()?;
                for other in members {
                    union(parent, first, other);
                }
                Some(first)
            }
            let mut parent: Vec<usize> = (0..definitions.len()).collect();
            let mut window_of_position: Vec<(usize, usize, Option<usize>)> = Vec::new();
            let mut window_of_end: Vec<Option<usize>> = vec![None; block_count];
            let mut state = entries.scratch();
            let mut sampled: Vec<usize> = Vec::new();
            let mut event_positions: Vec<usize> = Vec::new();
            for (block_index, block) in function.blocks.iter().enumerate() {
                let positions = &uses[block_index];
                if positions.unpinned.is_empty()
                    && positions.pinned.is_empty()
                    && positions.dedicated.is_empty()
                    && !positions.end_of_block
                {
                    continue;
                }
                sampled.clear();
                sampled.extend(
                    positions
                        .unpinned
                        .iter()
                        .chain(&positions.pinned)
                        .chain(&positions.dedicated)
                        .copied(),
                );
                sampled.sort_unstable();
                sampled.dedup();
                entries.copy_into(block_index, &mut state);
                for &definition in &boundary[block_index] {
                    reaching::replace(&mut state, definition);
                }
                event_positions.clear();
                event_positions.extend(
                    sampled
                        .iter()
                        .chain(anchored[block_index].keys())
                        .copied()
                        .filter(|index| *index < block.instructions.len()),
                );
                event_positions.sort_unstable();
                event_positions.dedup();
                for &index in &event_positions {
                    if sampled.binary_search(&index).is_ok() {
                        window_of_position.push((block_index, index, merge(&mut parent, &state)));
                    }
                    if let Some(definitions_here) = anchored[block_index].get(&index) {
                        for &definition in definitions_here {
                            reaching::replace(&mut state, definition);
                        }
                    }
                }
                if positions.end_of_block {
                    window_of_end[block_index] = merge(&mut parent, &state);
                }
            }
            for (_, _, mapped) in &mut window_of_position {
                *mapped = mapped.map(|definition| find(&mut parent, definition));
            }
            for mapped in &mut window_of_end {
                *mapped = mapped.map(|definition| find(&mut parent, definition));
            }
            let mut windows: Vec<std::collections::BTreeSet<usize>> = Vec::new();
            let mut window_index: Vec<usize> = vec![usize::MAX; definitions.len()];
            for definition in 0..definitions.len() {
                let root = find(&mut parent, definition);
                let window = if window_index[root] != usize::MAX {
                    window_index[root]
                } else {
                    windows.push(std::collections::BTreeSet::new());
                    let w = windows.len() - 1;
                    window_index[root] = w;
                    w
                };
                windows[window].insert(definition);
            }
            let windowless = windows.len();
            windows.push(std::collections::BTreeSet::new());
            let mut assignment = Assignment {
                stores: vec![private; definitions.len()],
                loads: (0..block_count).map(|_| BlockLoads::default()).collect(),
                declares_private: false,
                private,
            };
            let mut window_slot: Vec<LocalStorageSlotId> = vec![private; windows.len()];
            let mut classified_slots: Option<RawCandidateSlots> = None;
            let mut scratch = ShareableScratch::new(block_count);
            for (window, window_definitions) in windows.iter().enumerate() {
                let mut load_marks: Vec<Vec<usize>> = vec![Vec::new(); block_count];
                let mut end_marks = vec![false; block_count];
                for &(block_index, index, mapped) in &window_of_position {
                    let member = mapped
                        .and_then(|root| window_index.get(root).copied())
                        .filter(|&w| w != usize::MAX)
                        .unwrap_or(windowless);
                    if member == window && emitted[block_index].binary_search(&index).is_ok() {
                        load_marks[block_index].push(index);
                    }
                }
                for marks in &mut load_marks {
                    if marks.len() > 1 {
                        marks.sort_unstable();
                        marks.dedup();
                    }
                }
                for (block_index, &mapped) in window_of_end.iter().enumerate() {
                    if !uses[block_index].end_of_block {
                        continue;
                    }
                    let member = mapped
                        .and_then(|root| window_index.get(root).copied())
                        .filter(|&w| w != usize::MAX)
                        .unwrap_or(windowless);
                    if member == window {
                        end_marks[block_index] = true;
                    }
                }
                let has_loads = load_marks.iter().any(|marks| !marks.is_empty())
                    || end_marks.iter().any(|&marked| marked);
                if window_definitions.is_empty() && !has_loads {
                    window_slot[window] = private;
                    continue;
                }
                let mut chosen = None;
                if !candidate_slots.is_empty() {
                    let classified = classified_slots
                        .get_or_insert_with(|| RawCandidateSlots::scan(function, &candidate_slots));
                    let summary = summarize_window(
                        function,
                        &boundary,
                        &anchored,
                        window_definitions,
                        &load_marks,
                        &end_marks,
                    );
                    for (slot_idx, &slot) in candidate_slots.iter().enumerate() {
                        let Some(accesses) = classified.get(slot_idx) else {
                            continue;
                        };
                        if shareable(
                            accesses,
                            &summary,
                            &successors,
                            &load_marks,
                            &end_marks,
                            &mut scratch,
                        ) {
                            chosen = Some(slot);
                            break;
                        }
                    }
                }
                let chosen = match chosen {
                    Some(slot) => slot,
                    None => {
                        assignment.declares_private = true;
                        private
                    }
                };
                window_slot[window] = chosen;
            }
            for (definition, store) in assignment.stores.iter_mut().enumerate() {
                let root = find(&mut parent, definition);
                let window = window_index
                    .get(root)
                    .copied()
                    .filter(|&w| w != usize::MAX)
                    .unwrap_or(windowless);
                *store = window_slot[window];
            }
            for &(block_index, index, mapped) in &window_of_position {
                let window = mapped
                    .and_then(|root| window_index.get(root).copied())
                    .filter(|&w| w != usize::MAX)
                    .unwrap_or(windowless);
                assignment.loads[block_index]
                    .instructions
                    .insert(index, window_slot[window]);
            }
            for (block_index, &mapped) in window_of_end.iter().enumerate() {
                if uses[block_index].end_of_block {
                    let window = mapped
                        .and_then(|root| window_index.get(root).copied())
                        .filter(|&w| w != usize::MAX)
                        .unwrap_or(windowless);
                    assignment.loads[block_index].end_of_block = Some(window_slot[window]);
                }
            }
            assignment
        }

        fn find(parent: &mut [usize], member: usize) -> usize {
            let mut root = member;
            while parent[root] != root {
                root = parent[root];
            }
            let mut next = member;
            while parent[next] != next {
                let up = parent[next];
                parent[next] = root;
                next = up;
            }
            root
        }

        fn summarize_window(
            function: &SelectedFunction,
            boundary: &[Vec<usize>],
            anchored: &[std::collections::BTreeMap<usize, Vec<usize>>],
            window_definitions: &std::collections::BTreeSet<usize>,
            load_marks: &[Vec<usize>],
            end_marks: &[bool],
        ) -> WindowSummary {
            let block_count = function.blocks.len();
            let mut boundary_store = vec![false; block_count];
            let mut anchored_stores = vec![Vec::new(); block_count];
            let mut last_store = vec![None; block_count];
            let mut has_loads = vec![false; block_count];
            let mut store_blocks = Vec::new();
            let mut load_blocks = Vec::new();
            let mut active_blocks = Vec::new();
            let mut is_entry_insensitive = true;
            for (block_index, block) in function.blocks.iter().enumerate() {
                let has_boundary = boundary[block_index]
                    .iter()
                    .any(|definition| window_definitions.contains(definition));
                boundary_store[block_index] = has_boundary;
                let stores: Vec<usize> = anchored[block_index]
                    .iter()
                    .filter_map(|(&index, definitions)| {
                        (index < block.instructions.len()
                            && definitions
                                .iter()
                                .any(|definition| window_definitions.contains(definition)))
                        .then_some(index)
                    })
                    .collect();
                let block_last_store = stores
                    .last()
                    .copied()
                    .map(StorePosition::Instruction)
                    .or_else(|| has_boundary.then_some(StorePosition::Boundary));
                last_store[block_index] = block_last_store;
                if block_last_store.is_some() {
                    store_blocks.push(block_index);
                }
                let block_has_loads = !load_marks[block_index].is_empty() || end_marks[block_index];
                has_loads[block_index] = block_has_loads;
                if block_has_loads {
                    load_blocks.push(block_index);
                    if !has_boundary
                        && !stores.first().is_some_and(|&s_idx| {
                            load_marks[block_index]
                                .first()
                                .is_none_or(|&l_idx| s_idx < l_idx)
                        })
                    {
                        is_entry_insensitive = false;
                    }
                }
                if block_last_store.is_some() || block_has_loads {
                    active_blocks.push(block_index);
                }
                anchored_stores[block_index] = stores;
            }
            WindowSummary {
                boundary_store,
                anchored_stores,
                last_store,
                has_loads,
                store_blocks,
                load_blocks,
                active_blocks,
                is_entry_insensitive,
            }
        }

        struct RawCandidateSlots {
            block_count: usize,
            invalid: Vec<bool>,
            accesses: Vec<Vec<(usize, usize, Event)>>,
            terminal_loads: Vec<Vec<usize>>,
            finalized: Vec<Option<CandidateAccesses>>,
        }

        impl RawCandidateSlots {
            fn scan(function: &SelectedFunction, candidates: &[LocalStorageSlotId]) -> Self {
                let count = candidates.len();
                let block_count = function.blocks.len();
                if count == 0 {
                    return Self {
                        block_count,
                        invalid: Vec::new(),
                        accesses: Vec::new(),
                        terminal_loads: Vec::new(),
                        finalized: Vec::new(),
                    };
                }
                let max_spill_reg = candidates
                    .iter()
                    .filter_map(|slot| match slot {
                        LocalStorageSlotId::Spill { register } => Some(register.0 as usize),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                let mut slot_lookup = vec![u32::MAX; max_spill_reg + 1];
                for (idx, &slot) in candidates.iter().enumerate() {
                    if let LocalStorageSlotId::Spill { register } = slot {
                        slot_lookup[register.0 as usize] = idx as u32;
                    }
                }
                let lookup_slot = |slot: LocalStorageSlotId| -> Option<usize> {
                    match slot {
                        LocalStorageSlotId::Spill { register } => slot_lookup
                            .get(register.0 as usize)
                            .copied()
                            .filter(|&idx| idx != u32::MAX)
                            .map(|idx| idx as usize),
                        _ => None,
                    }
                };

                let mut invalid = vec![false; count];
                let mut accesses: Vec<Vec<(usize, usize, Event)>> = vec![Vec::new(); count];
                let mut terminal_loads: Vec<Vec<usize>> = vec![Vec::new(); count];
                const NO_OWNER: u32 = u32::MAX;
                const MULTI_OWNER: u32 = u32::MAX - 1;
                let mut address_owner: Vec<u32> = Vec::new();
                let mut owner_bits: Vec<u64> = Vec::new();
                let mut first_owner_word = usize::MAX;

                let invalidate = |slot: LocalStorageSlotId, invalid: &mut [bool]| {
                    if let Some(idx) = lookup_slot(slot) {
                        invalid[idx] = true;
                    }
                };

                for block_index in 0..block_count {
                    let Some(summary) = function.blocks.block_slot_summary(block_index) else {
                        continue;
                    };
                    for &event in &summary.events {
                        match event {
                            selected_instructions::BlockSlotEvent::Store {
                                slot: target,
                                index,
                            } => {
                                if let Some(slot_idx) = lookup_slot(target)
                                    && !invalid[slot_idx]
                                {
                                    accesses[slot_idx].push((
                                        block_index,
                                        index,
                                        Event::ExistingStore,
                                    ));
                                }
                            }
                            selected_instructions::BlockSlotEvent::FrameAddress {
                                slot: target,
                                register,
                            } => {
                                let reg_idx = register.0 as usize;
                                if reg_idx >= address_owner.len() {
                                    address_owner.resize(reg_idx + 1, NO_OWNER);
                                }
                                let entry = &mut address_owner[reg_idx];
                                if let Some(slot_idx) = lookup_slot(target)
                                    && !invalid[slot_idx]
                                {
                                    if *entry == NO_OWNER {
                                        *entry = slot_idx as u32;
                                        let word = reg_idx >> 6;
                                        if word >= owner_bits.len() {
                                            owner_bits.resize(word + 1, 0);
                                        }
                                        owner_bits[word] |= 1u64 << (reg_idx & 63);
                                        first_owner_word = first_owner_word.min(word);
                                    } else if *entry != slot_idx as u32 {
                                        if *entry < MULTI_OWNER {
                                            invalid[*entry as usize] = true;
                                        }
                                        invalid[slot_idx] = true;
                                        *entry = MULTI_OWNER;
                                    }
                                } else {
                                    if *entry < MULTI_OWNER {
                                        invalid[*entry as usize] = true;
                                    }
                                    *entry = MULTI_OWNER;
                                }
                            }
                            selected_instructions::BlockSlotEvent::Invalidate { slot: target } => {
                                invalidate(target, &mut invalid);
                            }
                        }
                    }
                }

                if first_owner_word != usize::MAX {
                    let active_owner_bits = &owner_bits[first_owner_word..];
                    for block_index in 0..block_count {
                        let Some(summary) = function.blocks.block_slot_summary(block_index) else {
                            continue;
                        };
                        if let Some(non_load_slice) =
                            summary.non_load_register_bits().get(first_owner_word..)
                        {
                            for (offset, (&non_load_word, &owner_word)) in
                                non_load_slice.iter().zip(active_owner_bits).enumerate()
                            {
                                let mut hit = non_load_word & owner_word;
                                while hit != 0 {
                                    let bit = hit.trailing_zeros() as usize;
                                    hit &= hit - 1;
                                    let reg_idx = ((first_owner_word + offset) << 6) | bit;
                                    let owner = address_owner[reg_idx];
                                    if owner < MULTI_OWNER {
                                        invalid[owner as usize] = true;
                                    }
                                }
                            }
                        }
                        let has_owner_load = summary
                            .loads_register_bits()
                            .get(first_owner_word..)
                            .is_some_and(|load_slice| {
                                load_slice
                                    .iter()
                                    .zip(active_owner_bits)
                                    .any(|(&load_w, &owner_w)| (load_w & owner_w) != 0)
                            });
                        if !has_owner_load {
                            continue;
                        }
                        for &(reg, index) in &summary.loads {
                            let Some(&owner) = address_owner.get(reg.0 as usize) else {
                                continue;
                            };
                            if owner >= MULTI_OWNER {
                                continue;
                            };
                            let slot_idx = owner as usize;
                            if invalid[slot_idx] {
                                continue;
                            }
                            if index == usize::MAX {
                                terminal_loads[slot_idx].push(block_index);
                            } else {
                                accesses[slot_idx].push((block_index, index, Event::ExistingLoad));
                            }
                        }
                    }
                }

                Self {
                    block_count,
                    invalid,
                    accesses,
                    terminal_loads,
                    finalized: (0..count).map(|_| None).collect(),
                }
            }

            fn get(&mut self, slot_idx: usize) -> Option<&CandidateAccesses> {
                if self.invalid[slot_idx] {
                    return None;
                }
                if self.finalized[slot_idx].is_none() {
                    let raw_accesses = std::mem::take(&mut self.accesses[slot_idx]);
                    let raw_terminal_loads = std::mem::take(&mut self.terminal_loads[slot_idx]);
                    let mut slot_accesses = vec![Vec::new(); self.block_count];
                    for (block_index, index, event) in raw_accesses {
                        slot_accesses[block_index].push((index, event));
                    }
                    let mut slot_terminal_loads = vec![false; self.block_count];
                    for block_index in raw_terminal_loads {
                        slot_terminal_loads[block_index] = true;
                    }
                    for block_accesses in &mut slot_accesses {
                        if block_accesses.len() > 1 {
                            block_accesses.sort_by_key(|&(idx, _)| idx);
                            block_accesses.dedup_by(|a, b| {
                                if a.0 == b.0 {
                                    b.1 = a.1;
                                    true
                                } else {
                                    false
                                }
                            });
                        }
                    }
                    let mut last_existing_store = Vec::with_capacity(self.block_count);
                    let mut store_blocks = Vec::new();
                    let mut load_blocks = Vec::new();
                    let mut is_entry_insensitive = true;
                    for (block_index, (block_accesses, &term_load)) in
                        slot_accesses.iter().zip(&slot_terminal_loads).enumerate()
                    {
                        let last_store = block_accesses.iter().rev().find_map(|&(index, event)| {
                            matches!(event, Event::ExistingStore).then_some(index)
                        });
                        last_existing_store.push(last_store);
                        if last_store.is_some() {
                            store_blocks.push(block_index);
                        }
                        let block_has_loads = term_load
                            || block_accesses
                                .iter()
                                .any(|&(_, event)| matches!(event, Event::ExistingLoad));
                        if block_has_loads {
                            load_blocks.push(block_index);
                            if !block_accesses
                                .first()
                                .is_some_and(|&(_, event)| matches!(event, Event::ExistingStore))
                            {
                                is_entry_insensitive = false;
                            }
                        }
                    }
                    self.finalized[slot_idx] = Some(CandidateAccesses {
                        accesses: slot_accesses,
                        terminal_loads: slot_terminal_loads,
                        last_existing_store,
                        store_blocks,
                        load_blocks,
                        is_entry_insensitive,
                    });
                }
                self.finalized[slot_idx].as_ref()
            }
        }

        fn shareable(
            candidate: &CandidateAccesses,
            window: &WindowSummary,
            successors: &[Vec<usize>],
            load_marks: &[Vec<usize>],
            end_marks: &[bool],
            scratch: &mut ShareableScratch,
        ) -> bool {
            let check_block = |block_index: usize, initial_state: u8| -> bool {
                let mut state = initial_state;
                if window.boundary_store[block_index] {
                    state = NEW;
                }
                let loads = &load_marks[block_index];
                let cand = &candidate.accesses[block_index];
                let stores = &window.anchored_stores[block_index];
                let mut i_load = 0;
                let mut i_cand = 0;
                let mut i_store = 0;
                loop {
                    let next_load = loads.get(i_load).copied().unwrap_or(usize::MAX);
                    let next_store = stores.get(i_store).copied().unwrap_or(usize::MAX);
                    let next_victim = next_load.min(next_store);
                    if state == EXISTING && i_cand < cand.len() && cand[i_cand].0 < next_victim {
                        if next_victim == usize::MAX {
                            i_cand = cand.len();
                        } else {
                            i_cand += cand[i_cand..].partition_point(|&(idx, _)| idx < next_victim);
                        }
                    }
                    let next_cand = cand.get(i_cand).map(|&(idx, _)| idx).unwrap_or(usize::MAX);
                    let index = next_victim.min(next_cand);
                    if index == usize::MAX {
                        break;
                    }
                    if next_load == index {
                        if state != NEW {
                            return false;
                        }
                        i_load += 1;
                    }
                    if next_cand == index {
                        match cand[i_cand].1 {
                            Event::ExistingStore => state = EXISTING,
                            Event::ExistingLoad => {
                                if state & NEW != 0 {
                                    return false;
                                }
                            }
                        }
                        i_cand += 1;
                    }
                    if next_store == index {
                        state = NEW;
                        i_store += 1;
                    }
                }
                if end_marks[block_index] && state != NEW {
                    return false;
                }
                if candidate.terminal_loads[block_index] && state & NEW != 0 {
                    return false;
                }
                true
            };

            if window.is_entry_insensitive && candidate.is_entry_insensitive {
                for &block_index in &window.active_blocks {
                    if candidate.accesses[block_index].is_empty()
                        && !candidate.terminal_loads[block_index]
                    {
                        continue;
                    }
                    if !check_block(block_index, 0) {
                        return false;
                    }
                }
                return true;
            }

            scratch.pending.clear();
            scratch.entries.fill(0);
            scratch.exits.fill(None);
            for &block_index in &window.store_blocks {
                scratch.exits[block_index] = Some(NEW);
                scratch.queued[block_index] = true;
                scratch.pending.push_back(block_index);
            }
            for &block_index in &candidate.store_blocks {
                let e_idx = candidate.last_existing_store[block_index].unwrap();
                let exit = match window.last_store[block_index] {
                    Some(StorePosition::Instruction(w_idx)) => {
                        if e_idx > w_idx {
                            Some(EXISTING)
                        } else {
                            Some(NEW)
                        }
                    }
                    Some(StorePosition::Boundary) | None => Some(EXISTING),
                };
                scratch.exits[block_index] = exit;
                if !scratch.queued[block_index] {
                    scratch.queued[block_index] = true;
                    scratch.pending.push_back(block_index);
                }
            }
            while let Some(block) = scratch.pending.pop_front() {
                scratch.queued[block] = false;
                let exit = scratch.exits[block].unwrap_or(scratch.entries[block]);
                for &destination in &successors[block] {
                    let merged = scratch.entries[destination] | exit;
                    if merged == scratch.entries[destination] {
                        continue;
                    }
                    scratch.entries[destination] = merged;
                    if scratch.exits[destination].is_none() && !scratch.queued[destination] {
                        scratch.queued[destination] = true;
                        scratch.pending.push_back(destination);
                    }
                }
            }
            for &block_index in &window.load_blocks {
                if !check_block(block_index, scratch.entries[block_index]) {
                    return false;
                }
            }
            for &block_index in &candidate.load_blocks {
                if window.has_loads[block_index] {
                    continue;
                }
                if !check_block(block_index, scratch.entries[block_index]) {
                    return false;
                }
            }
            true
        }

        fn loads_before(
            uses: &[BlockUsePositions],
            shared_reload: &[bool],
            span_closes: &[Vec<usize>],
            block_index: usize,
            index: usize,
        ) -> bool {
            let positions = &uses[block_index];
            if positions.pinned.contains(&index) || positions.dedicated.contains(&index) {
                return true;
            }
            if !positions.unpinned.contains(&index) {
                return false;
            }
            if !shared_reload[block_index] {
                return true;
            }
            let Some(&previous) = positions
                .unpinned
                .iter()
                .rev()
                .find(|&point| *point < index)
            else {
                return true;
            };
            let closes = &span_closes[block_index];
            closes[closes.partition_point(|&c| c < previous)..]
                .first()
                .is_some_and(|&c| c < index)
        }

        pub(crate) fn bits_transport<'source>(
            environment: &'source ValidatedTargetRegisterEnvironment,
            victim: &VirtualRegister,
            carrier_class: RegisterClassId,
        ) -> Result<BitsConversion<'source>, RuntimeSpillError> {
            let keys = environment.selected_keys();
            let (to_key, from_key, to_kind, from_kind) = match victim.scalar_type {
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => (
                    keys.float32_to_bits,
                    keys.bits_to_float32,
                    SelectedInstructionKind::Float32ToBits,
                    SelectedInstructionKind::BitsToFloat32,
                ),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => (
                    keys.float64_to_bits,
                    keys.bits_to_float64,
                    SelectedInstructionKind::Float64ToBits,
                    SelectedInstructionKind::BitsToFloat64,
                ),
                _ => return Err(RuntimeSpillError::UnsupportedValue),
            };
            let to_bits = environment
                .constraint(to_key.ok_or(RuntimeSpillError::UnsupportedValue)?)
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            let from_bits = environment
                .constraint(from_key.ok_or(RuntimeSpillError::UnsupportedValue)?)
                .ok_or(RuntimeSpillError::UnsupportedValue)?;
            for (row, classes) in [
                (to_bits, [victim.class, carrier_class]),
                (from_bits, [carrier_class, victim.class]),
            ] {
                if row.operands.len() != 2
                    || !row.implicit_defs.is_empty()
                    || !row.clobbers.is_empty()
                {
                    return Err(RuntimeSpillError::UnsupportedValue);
                }
                for (ordinal, (operand, class)) in row.operands.iter().zip(classes).enumerate() {
                    if usize::from(operand.operand) != ordinal
                        || operand.access
                            != if ordinal == 0 {
                                RegisterOperandAccess::Use
                            } else {
                                RegisterOperandAccess::Def
                            }
                        || operand.class != class
                        || operand.fixed_view.is_some()
                        || operand.tied_to.is_some()
                        || operand.early_clobber
                    {
                        return Err(RuntimeSpillError::UnsupportedValue);
                    }
                }
            }
            Ok(BitsConversion {
                to_bits,
                from_bits,
                to_kind,
                from_kind,
            })
        }

        pub(crate) fn entry_definitions(
            entry: usize,
            victim: &VirtualRegister,
        ) -> Vec<StorageDefinition> {
            vec![StorageDefinition {
                block_index: entry,
                position: StoragePosition::BlockStart,
                register: victim.id,
            }]
        }

        pub(crate) fn physical_definition(
            function: &SelectedFunction,
            argument: VirtualRegisterId,
        ) -> Result<(usize, &SelectedInstruction), RuntimeSpillError> {
            let mut definitions = function
                .blocks
                .iter()
                .enumerate()
                .filter_map(|(owner, block)| {
                    let (start_idx, end_idx) = function
                        .blocks
                        .block_register_instruction_span(owner, argument)?;
                    Some((owner, &block.instructions[start_idx..=end_idx]))
                })
                .flat_map(|(owner, instructions)| {
                    instructions.iter().flat_map(move |candidate| {
                        candidate
                            .operands
                            .iter()
                            .filter(move |operand| {
                                operand.virtual_register == argument
                                    && operand.access != RegisterOperandAccess::Use
                            })
                            .map(move |_| (owner, candidate))
                    })
                });
            let Some(definition) = definitions.next() else {
                return Err(RuntimeSpillError::UnsupportedValue);
            };
            if definitions.next().is_some() {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            Ok(definition)
        }
    }
    pub(crate) mod snapshot_copy {
        use super::RuntimeSpillError;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{SelectedBlock, SelectedInstructionKind, VirtualRegisterId};
        use semantic_vocabulary::IntegerValue;

        fn definition(
            block: &SelectedBlock,
            before: usize,
            register: VirtualRegisterId,
            steps: &mut usize,
            ceiling: u64,
        ) -> Result<Option<usize>, RuntimeSpillError> {
            for index in (0..before).rev() {
                *steps = steps
                    .checked_add(1)
                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
                if u64::try_from(*steps).map_err(|_| RuntimeSpillError::IdentityOverflow)? > ceiling
                {
                    return Err(RuntimeSpillError::WorkBudgetExceeded);
                }
                if block.instructions[index].operands.iter().any(|operand| {
                    operand.virtual_register == register
                        && matches!(
                            operand.access,
                            RegisterOperandAccess::Def | RegisterOperandAccess::UseDef
                        )
                }) {
                    return Ok(Some(index));
                }
            }
            Ok(None)
        }

        pub(crate) fn source_use(
            block: &SelectedBlock,
            copy: usize,
            register: VirtualRegisterId,
            source_offset: u32,
            byte_size: u32,
            steps: &mut usize,
            ceiling: u64,
        ) -> Result<Option<usize>, RuntimeSpillError> {
            let instruction = &block.instructions[copy];
            let [source, _, count, _, _] = instruction.operands.as_slice() else {
                return Err(RuntimeSpillError::UnsupportedUse);
            };
            if source.access != RegisterOperandAccess::Use
                || count.access != RegisterOperandAccess::Use
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            let use_index = if source.virtual_register == register {
                if source_offset != 0 {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                copy
            } else {
                let Some(index) = definition(block, copy, source.virtual_register, steps, ceiling)?
                else {
                    return Ok(None);
                };
                let address = &block.instructions[index];
                let [input, output] = address.operands.as_slice() else {
                    return Ok(None);
                };
                if input.virtual_register != register {
                    return Ok(None);
                }
                if address.kind
                    != (SelectedInstructionKind::AddressOffset {
                        byte_offset: source_offset,
                    })
                    || source_offset == 0
                    || input.access != RegisterOperandAccess::Use
                    || output.virtual_register != source.virtual_register
                    || output.access != RegisterOperandAccess::Def
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                index
            };
            let Some(index) = definition(block, copy, count.virtual_register, steps, ceiling)?
            else {
                return Err(RuntimeSpillError::UnsupportedUse);
            };
            let materialize = &block.instructions[index];
            if materialize.kind
                != (SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(u128::from(byte_size)),
                })
                || !matches!(materialize.operands.as_slice(), [output]
                if output.virtual_register == count.virtual_register && output.access == RegisterOperandAccess::Def)
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
            Ok(Some(use_index))
        }

        #[cfg(test)]
        pub(crate) mod tests {
            use super::*;
            use register_environment::ValidatedTargetRegisterEnvironment;
            use selected_instructions::{
                SelectedBlockId, SelectedBlockOrigin, SelectedInstruction, SelectedInstructionId,
                SelectedInstructionProvenance, SelectedOperand, SelectedTerminator,
            };
            use semantic_vocabulary::{BlockId, EdgeId};

            fn instruction(
                environment: &ValidatedTargetRegisterEnvironment,
                id: u32,
                kind: SelectedInstructionKind,
                registers: &[u32],
            ) -> SelectedInstruction {
                let keys = environment.selected_keys();
                let key = match kind {
                    SelectedInstructionKind::MaterializeI64 { .. } => keys.materialize_i64,
                    SelectedInstructionKind::AddressOffset { .. } => keys.address_offset.unwrap(),
                    SelectedInstructionKind::CopyBytes => keys.copy_bytes.unwrap(),
                    SelectedInstructionKind::ReturnUnit => keys.return_unit,
                    _ => panic!("unsupported test instruction"),
                };
                let row = environment.constraints().constraint(key).unwrap();
                assert_eq!(row.operands.len(), registers.len());
                SelectedInstruction {
                    id: SelectedInstructionId(id),
                    kind,
                    constraint: key,
                    operands: row
                        .operands
                        .iter()
                        .zip(registers)
                        .map(|(operand, register)| SelectedOperand {
                            operand: operand.operand,
                            virtual_register: VirtualRegisterId(*register),
                            access: operand.access,
                            class: operand.class,
                            fixed_view: operand.fixed_view,
                            tied_to: operand.tied_to,
                            early_clobber: operand.early_clobber,
                        })
                        .collect(),
                    implicit_uses: row.implicit_uses.clone(),
                    implicit_defs: row.implicit_defs.clone(),
                    clobbers: row.clobbers.clone(),
                    provenance: SelectedInstructionProvenance::default(),
                }
            }

            fn block(
                environment: &ValidatedTargetRegisterEnvironment,
                offset: u32,
            ) -> SelectedBlock {
                let mut instructions = vec![];
                if offset != 0 {
                    instructions.push(instruction(
                        environment,
                        0,
                        SelectedInstructionKind::AddressOffset {
                            byte_offset: offset,
                        },
                        &[0, 5],
                    ));
                }
                instructions.push(instruction(
                    environment,
                    1,
                    SelectedInstructionKind::MaterializeI64 {
                        value: IntegerValue::Unsigned(8192),
                    },
                    &[2],
                ));
                instructions.push(instruction(
                    environment,
                    2,
                    SelectedInstructionKind::CopyBytes,
                    &[if offset == 0 { 0 } else { 5 }, 1, 2, 3, 4],
                ));
                SelectedBlock {
                    id: SelectedBlockId(0),
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions,
                    terminator: SelectedTerminator::Return {
                        instruction: instruction(
                            environment,
                            99,
                            SelectedInstructionKind::ReturnUnit,
                            &[],
                        ),
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }
            }

            fn resolve(
                block: &SelectedBlock,
                offset: u32,
            ) -> Result<Option<usize>, RuntimeSpillError> {
                source_use(
                    block,
                    block.instructions.len() - 1,
                    VirtualRegisterId(0),
                    offset,
                    8192,
                    &mut 0,
                    100,
                )
            }

            #[test]
            fn bulk_snapshot_exact_zero_and_nonzero_offsets_name_the_actual_pointer_use() {
                let environment = register_environment::baseline_target_register_environment(
                    target::NativeTarget::linux_arm64(),
                )
                .unwrap();
                let direct = block(&environment, 0);
                assert_eq!(resolve(&direct, 0), Ok(Some(1)));
                let displaced = block(&environment, 17);
                assert_eq!(resolve(&displaced, 17), Ok(Some(0)));
            }

            #[test]
            fn bulk_snapshot_wrong_redefined_and_unknown_counts_refuse() {
                let environment = register_environment::baseline_target_register_environment(
                    target::NativeTarget::linux_arm64(),
                )
                .unwrap();
                let mut wrong = block(&environment, 0);
                wrong.instructions[0].kind = SelectedInstructionKind::MaterializeI64 {
                    value: IntegerValue::Unsigned(8191),
                };
                assert_eq!(resolve(&wrong, 0), Err(RuntimeSpillError::UnsupportedUse));
                let mut redefined = block(&environment, 0);
                redefined.instructions.insert(
                    1,
                    instruction(
                        &environment,
                        3,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(8191),
                        },
                        &[2],
                    ),
                );
                assert_eq!(
                    resolve(&redefined, 0),
                    Err(RuntimeSpillError::UnsupportedUse)
                );
                let mut unknown = block(&environment, 0);
                unknown.instructions.remove(0);
                assert_eq!(resolve(&unknown, 0), Err(RuntimeSpillError::UnsupportedUse));
            }

            #[test]
            fn bulk_snapshot_wrong_or_redefined_address_never_names_the_binding() {
                let environment = register_environment::baseline_target_register_environment(
                    target::NativeTarget::linux_arm64(),
                )
                .unwrap();
                let mut wrong_offset = block(&environment, 17);
                wrong_offset.instructions[0].kind =
                    SelectedInstructionKind::AddressOffset { byte_offset: 18 };
                assert_eq!(
                    resolve(&wrong_offset, 17),
                    Err(RuntimeSpillError::UnsupportedUse)
                );
                let mut unrelated = block(&environment, 17);
                unrelated.instructions[0].operands[0].virtual_register = VirtualRegisterId(6);
                assert_eq!(resolve(&unrelated, 17), Ok(None));
                let mut redefined = block(&environment, 17);
                redefined.instructions.insert(
                    2,
                    instruction(
                        &environment,
                        3,
                        SelectedInstructionKind::MaterializeI64 {
                            value: IntegerValue::Unsigned(17),
                        },
                        &[5],
                    ),
                );
                assert_eq!(resolve(&redefined, 17), Ok(None));
                let mut unknown = block(&environment, 17);
                unknown.instructions.remove(0);
                assert_eq!(resolve(&unknown, 17), Ok(None));
            }

            #[test]
            fn bulk_snapshot_definition_walk_keeps_the_existing_work_budget() {
                let environment = register_environment::baseline_target_register_environment(
                    target::NativeTarget::linux_arm64(),
                )
                .unwrap();
                let block = block(&environment, 17);
                let mut steps = 0;
                assert_eq!(
                    source_use(&block, 2, VirtualRegisterId(0), 17, 8192, &mut steps, 0),
                    Err(RuntimeSpillError::WorkBudgetExceeded)
                );
                assert_eq!(steps, 1);
            }
        }
    }
    pub(crate) mod unit_effects {
        use register_model::RegisterInstructionConstraint;
        use selected_instructions::SelectedFunction;

        use super::{
            RuntimeSpillError, StorageDefinition, StoragePosition, slot::BlockUsePositions,
        };

        pub(crate) fn preserve_live_units(
            function_index: usize,
            function: &SelectedFunction,
            definitions: &[StorageDefinition],
            uses: &[BlockUsePositions],
            address: &RegisterInstructionConstraint,
            load: &RegisterInstructionConstraint,
            store: &RegisterInstructionConstraint,
        ) -> Result<(), RuntimeSpillError> {
            let reload_clobbers: Vec<_> = address
                .clobbers
                .iter()
                .chain(&load.clobbers)
                .copied()
                .collect();
            let all_clobbers: Vec<_> = reload_clobbers
                .iter()
                .chain(&store.clobbers)
                .copied()
                .collect();
            if all_clobbers.is_empty()
                || !(0..function.blocks.len()).any(|block_index| {
                    function
                        .blocks
                        .block_implicit_units_and_fixed_views(block_index)
                        .is_some_and(|(units, _)| {
                            units.iter().any(|unit| all_clobbers.contains(unit))
                        })
                })
            {
                return Ok(());
            }
            let mut needed_blocks = vec![false; function.blocks.len()];
            for definition in definitions {
                if let Some(slot) = needed_blocks.get_mut(definition.block_index) {
                    *slot = true;
                }
            }
            for (block_index, positions) in uses.iter().enumerate() {
                if (!positions.unpinned.is_empty()
                    || !positions.pinned.is_empty()
                    || !positions.dedicated.is_empty()
                    || positions.end_of_block)
                    && let Some(slot) = needed_blocks.get_mut(block_index)
                {
                    *slot = true;
                }
            }
            let liveness = crate::analyses::liveness::compute::compute_function_units_filtered(
                function_index,
                function,
                &all_clobbers,
                &needed_blocks,
            )
            .map_err(|_| RuntimeSpillError::SourceMismatch)?;
            for definition in definitions {
                let block = liveness
                    .blocks
                    .get(definition.block_index)
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                if block.instructions.is_empty() {
                    match definition.position {
                        StoragePosition::BlockStart => {}
                        StoragePosition::AfterInstruction(instruction)
                        | StoragePosition::AfterUseDef { instruction, .. } => {
                            let exists = function
                                .blocks
                                .instruction_index_in_block(definition.block_index, instruction)
                                .is_some()
                                || super::control(
                                    &function
                                        .blocks
                                        .get(definition.block_index)
                                        .ok_or(RuntimeSpillError::SourceMismatch)?
                                        .terminator,
                                )
                                .0
                                .id == instruction;
                            if !exists {
                                return Err(RuntimeSpillError::SourceMismatch);
                            }
                        }
                    }
                    continue;
                }
                let live = match definition.position {
                    StoragePosition::BlockStart => &block.unit_live_in,
                    StoragePosition::AfterInstruction(instruction)
                    | StoragePosition::AfterUseDef { instruction, .. } => {
                        &block
                            .instructions
                            .iter()
                            .find(|row| row.instruction == instruction)
                            .ok_or(RuntimeSpillError::SourceMismatch)?
                            .unit_live_out
                    }
                };
                if store.clobbers.iter().any(|unit| live.contains(unit)) {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
            }
            for (block_index, positions) in uses.iter().enumerate() {
                let block = liveness
                    .blocks
                    .get(block_index)
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                if block.instructions.is_empty() {
                    let max_pos = function
                        .blocks
                        .get(block_index)
                        .ok_or(RuntimeSpillError::SourceMismatch)?
                        .instructions
                        .len();
                    if positions
                        .unpinned
                        .iter()
                        .chain(&positions.pinned)
                        .chain(&positions.dedicated)
                        .any(|&pos| pos > max_pos)
                    {
                        return Err(RuntimeSpillError::SourceMismatch);
                    }
                    continue;
                }
                for position in positions
                    .unpinned
                    .iter()
                    .chain(&positions.pinned)
                    .chain(&positions.dedicated)
                {
                    let instruction = block
                        .instructions
                        .get(*position)
                        .ok_or(RuntimeSpillError::SourceMismatch)?;
                    if reload_clobbers
                        .iter()
                        .any(|unit| instruction.unit_live_in.contains(unit))
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                }
                if positions.end_of_block {
                    let terminator = block
                        .instructions
                        .last()
                        .ok_or(RuntimeSpillError::SourceMismatch)?;
                    if reload_clobbers.iter().any(|unit| {
                        terminator.unit_live_in.contains(unit)
                            || terminator.unit_live_out.contains(unit)
                    }) {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                }
            }
            Ok(())
        }
    }
    pub(crate) mod validation {
        use selected_instructions::SelectedInstructionPlan;

        use super::{RuntimeSpillError, RuntimeSpillReceipt, ValidatedRuntimeSpill};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::sealed;
        pub(crate) fn seal_runtime_spill_with_span_policy(
            source: &impl ValidatedSelectedAnalysis,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
            Ok(sealed!(
                ValidatedRuntimeSpill,
                RuntimeSpillReceipt,
                source,
                proposed
            ))
        }
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use register_environment::ValidatedTargetRegisterEnvironment;
    use register_model::{RegisterClassId, RegisterInstructionConstraint, RegisterUnitId};
    use selected_instructions::{
        SelectedFunction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
        SelectedInstructionPlanIdentity, SelectedStructuralTransport, VirtualRegister,
        VirtualRegisterId,
    };
    use semantic_vocabulary::{EdgeId, PlaceId, ScalarType, ValueId, WorkScheduleIdentity};

    pub use rewrite::{
        spill_selected_runtime_value, spill_selected_runtime_value_with_crossing_status,
        spill_selected_runtime_value_with_span_policy,
    };
    use std::collections::BTreeSet;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuntimeSpillSpanPolicy {
        UnitWriteBounded,
        UnitWriteCrossing,
    }

    validated_transformed_receipt!(ValidatedRuntimeSpill, RuntimeSpillReceipt);

    rewrite_receipt!(RuntimeSpillReceipt);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuntimeSpillError {
        SourceMismatch,
        UnsupportedValue,
        UnsupportedControlFlow,
        UnsupportedUse,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    failed_display!(RuntimeSpillError, "invalid runtime-value spill");

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum VictimLineage {
        Scalar(ValueId),
        Structural { place: PlaceId, byte_offset: u32 },
    }

    pub(crate) struct StorageDefinition {
        pub block_index: usize,
        pub position: StoragePosition,
        pub register: VirtualRegisterId,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum StoragePosition {
        AfterInstruction(SelectedInstructionId),
        AfterUseDef {
            instruction: SelectedInstructionId,
            operand: u16,
        },
        BlockStart,
    }

    pub(crate) struct StructuralArgumentUse {
        pub block: usize,
        pub edge: EdgeId,
        pub place: PlaceId,
        pub source_byte_offset: u32,
        pub byte_size: u32,
        pub source_read_roles: Vec<selected_instructions::SelectedMemoryAccessRole>,
        pub loads: Vec<SelectedInstructionId>,
        pub dedicated_reload: bool,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct BitsConversion<'source> {
        pub to_bits: &'source RegisterInstructionConstraint,
        pub from_bits: &'source RegisterInstructionConstraint,
        pub to_kind: SelectedInstructionKind,
        pub from_kind: SelectedInstructionKind,
    }

    pub(crate) fn stored_transport_size(transport: SelectedStructuralTransport) -> Option<u32> {
        match transport {
            SelectedStructuralTransport::Unused
            | SelectedStructuralTransport::Address { .. }
            | SelectedStructuralTransport::ScalarReference {
                source: selected_instructions::SelectedScalarReferenceSource::Canonical { .. },
                ..
            } => None,
            SelectedStructuralTransport::ScalarReference {
                source: selected_instructions::SelectedScalarReferenceSource::Descriptor,
                ..
            } => Some(16),
            SelectedStructuralTransport::WholeValue { byte_size, .. } => Some(byte_size),
        }
    }

    pub(crate) fn next_chunk(byte_size: u32, offset: u32) -> Option<u8> {
        [8u32, 4, 2, 1]
            .into_iter()
            .find(|width| *width <= byte_size - offset)
            .map(|width| width as u8)
    }

    pub(crate) fn surviving_home_exists(
        environment: &ValidatedTargetRegisterEnvironment,
        class: RegisterClassId,
        implicit_use_units: &BTreeSet<RegisterUnitId>,
        pinned_units: &BTreeSet<RegisterUnitId>,
        written_units: &BTreeSet<RegisterUnitId>,
    ) -> bool {
        let reserved = environment.reservations().reserved_units();
        let physical = environment.physical().model();
        physical
            .classes
            .iter()
            .find(|row| row.id == class)
            .is_some_and(|row| {
                row.views.iter().any(|view_id| {
                    physical
                        .views
                        .get(usize::from(view_id.0))
                        .is_some_and(|view| {
                            view.id == *view_id
                                && view.allocatable
                                && view.units.iter().chain(&view.write_units).all(|unit| {
                                    !implicit_use_units.contains(unit)
                                        && !pinned_units.contains(unit)
                                        && !written_units.contains(unit)
                                        && reserved.binary_search(unit).is_err()
                                })
                        })
                })
            })
    }

    pub(crate) fn control(
        terminator: &selected_instructions::SelectedTerminator,
    ) -> (
        &selected_instructions::SelectedInstruction,
        [Option<&selected_instructions::SelectedSuccessor>; 2],
    ) {
        use selected_instructions::SelectedTerminator;
        match terminator {
            selected_instructions::terminator_no_successor_pattern!(instruction, ..) => {
                (instruction, [None, None])
            }
            SelectedTerminator::Jump {
                instruction,
                successor,
            } => (instruction, [Some(successor), None]),
            SelectedTerminator::ConditionalBranch {
                instruction,
                when_nonzero,
                when_zero,
            } => (instruction, [Some(when_nonzero), Some(when_zero)]),
            selected_instructions::conditional_branch_less_pair_pattern!(
                instruction,
                when_less,
                when_not_less,
                ..
            ) => (instruction, [Some(when_less), Some(when_not_less)]),
        }
    }

    pub(crate) fn next_instruction_id(function: &SelectedFunction) -> Option<u32> {
        function
            .blocks
            .maximum_instruction_id()
            .map_or(0, |instruction| instruction.0)
            .checked_add(1)
    }

    pub(crate) fn next_register_id(function: &SelectedFunction) -> Option<u32> {
        u32::try_from(function.virtual_registers.len()).ok()
    }

    pub(crate) fn control_mut(
        terminator: &mut selected_instructions::SelectedTerminator,
    ) -> &mut selected_instructions::SelectedInstruction {
        selected_instructions::terminator_instruction_mut(terminator)
    }

    pub(crate) fn control_successors_mut(
        terminator: &mut selected_instructions::SelectedTerminator,
    ) -> [Option<&mut selected_instructions::SelectedSuccessor>; 2] {
        use selected_instructions::SelectedTerminator;
        match terminator {
            selected_instructions::terminator_no_successor_pattern!(..) => [None, None],
            SelectedTerminator::Jump { successor, .. } => [Some(successor), None],
            SelectedTerminator::ConditionalBranch {
                when_nonzero,
                when_zero,
                ..
            } => [Some(when_nonzero), Some(when_zero)],
            selected_instructions::conditional_branch_less_pair_pattern!(
                when_less,
                when_not_less,
                ..
            ) => [Some(when_less), Some(when_not_less)],
        }
    }

    pub(crate) fn require_dominated_uses(
        function: &selected_instructions::SelectedFunction,
        anchor: usize,
        use_blocks: &[usize],
    ) -> Result<(), RuntimeSpillError> {
        let entry_id = function.entry_block;
        let entry = if function
            .blocks
            .get(entry_id.0 as usize)
            .is_some_and(|b| b.id == entry_id)
        {
            entry_id.0 as usize
        } else {
            function
                .block_index(entry_id)
                .ok_or(RuntimeSpillError::SourceMismatch)?
        };
        let only_anchor_uses = use_blocks.iter().all(|&b| b == anchor);
        if only_anchor_uses && anchor == entry {
            return Ok(());
        }
        let mut reached = vec![false; function.blocks.len()];
        let mut pending = Vec::with_capacity(function.blocks.len());
        for bypass_anchor in [false, true] {
            if bypass_anchor && only_anchor_uses {
                break;
            }
            reached.fill(false);
            pending.clear();
            pending.push(entry);
            while let Some(block_index) = pending.pop() {
                if reached[block_index] || (bypass_anchor && block_index == anchor) {
                    continue;
                }
                reached[block_index] = true;
                if !bypass_anchor && only_anchor_uses && block_index == anchor {
                    break;
                }
                for successor in control(&function.blocks[block_index].terminator)
                    .1
                    .into_iter()
                    .flatten()
                {
                    let succ_id = successor.block;
                    let destination = if function
                        .blocks
                        .get(succ_id.0 as usize)
                        .is_some_and(|b| b.id == succ_id)
                    {
                        succ_id.0 as usize
                    } else {
                        function
                            .block_index(succ_id)
                            .ok_or(RuntimeSpillError::SourceMismatch)?
                    };
                    if !reached[destination] && !(bypass_anchor && destination == anchor) {
                        pending.push(destination);
                    }
                }
            }
            if (!bypass_anchor && !reached[anchor])
                || use_blocks
                    .iter()
                    .any(|block_index| reached[*block_index] == bypass_anchor)
            {
                return Err(RuntimeSpillError::UnsupportedUse);
            }
        }
        Ok(())
    }

    pub(crate) struct RuntimeSpillDerivation<'source> {
        pub function: &'source SelectedFunction,
        pub use_blocks: Vec<usize>,
        pub shared_reload: Vec<bool>,
        pub dedicated_tie_uses: Vec<BTreeSet<(usize, u16)>>,
        pub span_closes: Vec<Vec<usize>>,
        pub crossed_unit_write: bool,
        pub retained_uses: BTreeSet<(SelectedInstructionId, u16)>,
        pub victim: &'source VirtualRegister,
        pub lineage: VictimLineage,
        pub address_scalar_type: ScalarType,
        pub definitions: Vec<StorageDefinition>,
        pub assignment: slot::Assignment,
        pub structural_uses: Vec<StructuralArgumentUse>,
        pub first_instruction: u32,
        pub first_register: u32,
        pub address: &'source RegisterInstructionConstraint,
        pub load: &'source RegisterInstructionConstraint,
        pub store: &'source RegisterInstructionConstraint,
        pub carrier_class: RegisterClassId,
        pub bits_conversion: Option<BitsConversion<'source>>,
    }

    pub(crate) fn derive_runtime_spill<'source>(
        source: &'source impl crate::ValidatedSelectedAnalysis,
        function_index: usize,
        register: VirtualRegisterId,
        environment: &'source ValidatedTargetRegisterEnvironment,
        span_policy: RuntimeSpillSpanPolicy,
        budget: optimization_core::OptimizationWorkBudget,
    ) -> Result<RuntimeSpillDerivation<'source>, RuntimeSpillError> {
        use optimization_unit::ValueDefinitionSite;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            LocalStorageSlotId, SelectedBlockOrigin, SelectedCasePayloadTransport,
            SelectedInstructionKind, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
            SelectedSuccessorRole, SelectedValueTransport, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{EdgeId, IntegerSign, IntegerType, OperationId};
        use slot::{bits_transport, entry_definitions};
        let (plan, function) =
            crate::rewrites::plan_function(source, function_index, environment, || {
                RuntimeSpillError::SourceMismatch
            })?;
        let victim = function
            .virtual_register(register)
            .ok_or(RuntimeSpillError::UnsupportedValue)?;
        let unsigned = IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| RuntimeSpillError::UnsupportedValue)?;
        let entry_boundary = function.is_entry_register(victim);
        let (lineage, definition, block_index) = match victim.origin {
            VirtualRegisterOrigin::InstructionResult {
                instruction,
                source_value,
            } => {
                let block_index = slot::instruction_block(function, instruction)
                    .ok_or(RuntimeSpillError::UnsupportedValue)?;
                (
                    VictimLineage::Scalar(source_value),
                    Some(instruction),
                    block_index,
                )
            }
            VirtualRegisterOrigin::BlockParameter {
                source_value,
                block,
                parameter_index,
            } => {
                let block_index = function
                    .block_index(block)
                    .ok_or(RuntimeSpillError::UnsupportedValue)?;
                if block == function.entry_block
                    || !matches!(victim.definition_site,
                    Some(ValueDefinitionSite::BlockParameter { block: semantic_block, position })
                        if semantic_block == function.blocks[block_index].source_block()
                            && position as usize == parameter_index)
                {
                    return Err(RuntimeSpillError::UnsupportedValue);
                }
                (VictimLineage::Scalar(source_value), None, block_index)
            }
            VirtualRegisterOrigin::EntryParameter {
                source_value,
                parameter_index,
            } => {
                let block_index = function
                    .block_index(function.entry_block)
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                if !matches!(victim.definition_site,
                    Some(ValueDefinitionSite::FunctionParameter(position))
                        if position as usize == parameter_index)
                {
                    return Err(RuntimeSpillError::UnsupportedValue);
                }
                (VictimLineage::Scalar(source_value), None, block_index)
            }
            VirtualRegisterOrigin::StructuralParameter {
                place,
                parameter_index,
            } => {
                let block_index = function
                    .block_index(function.entry_block)
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                if victim.definition_site.is_some()
                    || !function
                        .structural
                        .as_ref()
                        .and_then(|contract| contract.parameters.get(parameter_index))
                        .is_some_and(|parameter| {
                            parameter.semantic.place == place && parameter.target.place == place
                        })
                {
                    return Err(RuntimeSpillError::UnsupportedValue);
                }
                (
                    VictimLineage::Structural {
                        place,
                        byte_offset: 0,
                    },
                    None,
                    block_index,
                )
            }
            VirtualRegisterOrigin::AbiTransport { place, .. } if entry_boundary => {
                let block_index = function
                    .block_index(function.entry_block)
                    .ok_or(RuntimeSpillError::SourceMismatch)?;
                (
                    VictimLineage::Structural {
                        place,
                        byte_offset: 0,
                    },
                    None,
                    block_index,
                )
            }
            VirtualRegisterOrigin::StructuralObservation {
                instruction,
                place,
                byte_offset,
            }
            | VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            } => {
                let block_index = slot::instruction_block(function, instruction)
                    .ok_or(RuntimeSpillError::UnsupportedValue)?;
                (
                    VictimLineage::Structural { place, byte_offset },
                    Some(instruction),
                    block_index,
                )
            }
            _ => return Err(RuntimeSpillError::UnsupportedValue),
        };
        let scalar_payload = match victim.scalar_type {
            ScalarType::Callable(_) | ScalarType::Boolean | ScalarType::IeeeFloat(_) => true,
            ScalarType::Integer(integer) => matches!(integer.bits(), 8 | 16 | 32 | 64),
        };
        if !scalar_payload
            || (victim.entry_fixed_view.is_some() && !entry_boundary)
            || !(matches!(
                victim.definition_site,
                Some(ValueDefinitionSite::FunctionParameter(_))
                    | Some(ValueDefinitionSite::BlockParameter { .. })
                    | Some(ValueDefinitionSite::Node { .. })
            ) || (victim.definition_site.is_none()
                && matches!(
                    victim.origin,
                    VirtualRegisterOrigin::StructuralParameter { .. }
                        | VirtualRegisterOrigin::AbiTransport { .. }
                        | VirtualRegisterOrigin::StructuralObservation { .. }
                )))
        {
            return Err(RuntimeSpillError::UnsupportedValue);
        }
        let private_slot = LocalStorageSlotId::Spill { register };
        if function
            .local_storage_slots
            .iter()
            .any(|storage| storage.id == private_slot)
        {
            return Err(RuntimeSpillError::UnsupportedUse);
        }
        let mut definitions = if let Some(instruction) = definition {
            vec![StorageDefinition {
                block_index,
                position: StoragePosition::AfterInstruction(instruction),
                register,
            }]
        } else if entry_boundary {
            entry_definitions(block_index, victim)
        } else {
            let VictimLineage::Scalar(source_value) = lineage else {
                return Err(RuntimeSpillError::UnsupportedValue);
            };
            parameter_definitions(function, block_index, victim, source_value)?
        };
        let mut redefinitions: Vec<BTreeSet<usize>> =
            function.blocks.iter().map(|_| BTreeSet::new()).collect();
        let mut usedef_writes: Vec<BTreeSet<usize>> =
            function.blocks.iter().map(|_| BTreeSet::new()).collect();
        let mut dedicated_tie_uses: Vec<BTreeSet<(usize, u16)>> =
            function.blocks.iter().map(|_| BTreeSet::new()).collect();
        let mut defined = definition.is_none();
        let mut uses = 0usize;
        let mut use_blocks = Vec::new();
        let mut flexible_uses = vec![false; function.blocks.len()];
        let mut use_positions: Vec<slot::BlockUsePositions> = function
            .blocks
            .iter()
            .map(|_| slot::BlockUsePositions::default())
            .collect();
        let mut structural_uses: Vec<StructuralArgumentUse> = Vec::new();
        let is_frame_address = definition.is_some_and(|def_id| {
            function
                .blocks
                .instruction_index_in_block(block_index, def_id)
                .and_then(|idx| function.blocks[block_index].instructions.get(idx))
                .is_some_and(|inst| {
                    matches!(inst.kind, SelectedInstructionKind::FrameAddress { .. })
                })
        });
        let establishing: BTreeSet<(OperationId, PlaceId)> = if is_frame_address {
            definition
                .into_iter()
                .flat_map(|definition| {
                    function.memory_accesses.iter().filter_map(move |access| {
                        match (access.origin, access.role) {
                            (
                                SelectedMemoryAccessOrigin::Operation(operation),
                                SelectedMemoryAccessRole::AddressLocal { .. },
                            ) if access.instruction == definition => {
                                Some((operation, access.place))
                            }
                            _ => None,
                        }
                    })
                })
                .collect()
        } else {
            BTreeSet::new()
        };
        let established_stores: BTreeSet<SelectedInstructionId> = if establishing.is_empty() {
            BTreeSet::new()
        } else {
            function
                .memory_accesses
                .iter()
                .filter_map(|access| match (access.origin, access.role) {
                    (
                        SelectedMemoryAccessOrigin::Operation(operation),
                        SelectedMemoryAccessRole::WritePlace,
                    ) if establishing.contains(&(operation, access.place)) => {
                        Some(access.instruction)
                    }
                    _ => None,
                })
                .collect()
        };
        let mut retained_uses = BTreeSet::new();
        for (current_block_index, block) in function.blocks.iter().enumerate() {
            if !function
                .blocks
                .block_references_register(current_block_index, register)
            {
                continue;
            }
            let previous_uses = uses;
            let (terminal, successors) = control(&block.terminator);
            for operand in &terminal.operands {
                if operand.virtual_register != register {
                    continue;
                }
                match operand.access {
                    RegisterOperandAccess::Use
                        if operand.tied_to.is_none()
                            && !operand.early_clobber
                            && operand.class == victim.class =>
                    {
                        if terminal
                            .operands
                            .iter()
                            .any(|other| other.tied_to == Some(operand.operand))
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        if operand.fixed_view.is_none() {
                            flexible_uses[current_block_index] = true;
                        }
                        use_positions[current_block_index].end_of_block = true;
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                    }
                    _ => return Err(RuntimeSpillError::UnsupportedUse),
                }
            }
            for successor in successors.into_iter().flatten() {
                for binding in &successor.bindings {
                    let SelectedValueTransport::Registers {
                        argument,
                        parameter,
                    } = binding.transport
                    else {
                        continue;
                    };
                    if parameter == register
                        && (definition.is_some()
                            || successor.block != function.blocks[block_index].id)
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    if argument != register {
                        continue;
                    }
                    if lineage != VictimLineage::Scalar(binding.semantic.argument)
                        || binding.semantic.scalar_type != victim.scalar_type
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    flexible_uses[current_block_index] = true;
                    use_positions[current_block_index].end_of_block = true;
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                }
                for binding in &successor.structural_bindings {
                    if matches!(binding.transport,
                        SelectedStructuralTransport::ScalarReference {
                            base: selected_instructions::SelectedAddressBase::Register(argument),
                            ..
                        }
                        | SelectedStructuralTransport::Address {
                            base: selected_instructions::SelectedAddressBase::Register(argument),
                            ..
                        } if argument == register)
                    {
                        flexible_uses[current_block_index] = true;
                        use_positions[current_block_index].end_of_block = true;
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeSpillError::IdentityOverflow)?;
                        continue;
                    }
                    let Some(byte_size) = stored_transport_size(binding.transport) else {
                        continue;
                    };
                    let SelectedStructuralTransport::WholeValue {
                        argument,
                        source_byte_offset,
                        ..
                    } = binding.transport
                    else {
                        continue;
                    };
                    if argument != register {
                        continue;
                    }
                    if successor.role != SelectedSuccessorRole::EdgeTransferContinuation
                        || !matches!(block.origin, SelectedBlockOrigin::EdgeTransfer { edge, target }
                            if Some(edge) == successor.psi_edge && target == successor.source_target)
                    {
                        return Err(RuntimeSpillError::UnsupportedControlFlow);
                    }
                    if definition.is_none()
                        && !entry_boundary
                        && successor.block == function.blocks[block_index].id
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    structural_uses.push(StructuralArgumentUse {
                        block: current_block_index,
                        edge: successor
                            .psi_edge
                            .ok_or(RuntimeSpillError::UnsupportedControlFlow)?,
                        place: binding.semantic.argument.place,
                        source_byte_offset,
                        byte_size,
                        source_read_roles: binding.source_read_roles().collect(),
                        loads: Vec::new(),
                        dedicated_reload: false,
                    });
                }
                if let Some(case) = &successor.structural_case {
                    for payload in &case.payloads {
                        match payload.transport {
                            SelectedCasePayloadTransport::Registers {
                                argument,
                                parameter,
                            } => {
                                if parameter == register
                                    && (definition.is_some()
                                        || successor.block != function.blocks[block_index].id)
                                {
                                    return Err(RuntimeSpillError::UnsupportedUse);
                                }
                                if argument != register {
                                    continue;
                                }
                                if payload.semantic.parameter.scalar_type != victim.scalar_type {
                                    return Err(RuntimeSpillError::UnsupportedUse);
                                }
                                if let VictimLineage::Structural { place, byte_offset } = lineage
                                    && (case.source.place() != Some(place)
                                        || byte_offset != payload.semantic.field_byte_offset)
                                {
                                    return Err(RuntimeSpillError::UnsupportedUse);
                                }
                                flexible_uses[current_block_index] = true;
                                use_positions[current_block_index].end_of_block = true;
                                uses = uses
                                    .checked_add(1)
                                    .ok_or(RuntimeSpillError::IdentityOverflow)?;
                            }
                            SelectedCasePayloadTransport::Unmaterialized { parameter }
                                if parameter == register =>
                            {
                                return Err(RuntimeSpillError::UnsupportedUse);
                            }
                            _ => {}
                        }
                    }
                }
            }
            if let Some((start_idx, end_idx, has_interior)) = function
                .blocks
                .block_register_instruction_span_detail(current_block_index, register)
            {
                let mut visit_instruction =
                    |instruction_index: usize,
                     instruction: &selected_instructions::SelectedInstruction|
                     -> Result<(), RuntimeSpillError> {
                        for operand in &instruction.operands {
                            if operand.virtual_register != register {
                                continue;
                            }
                            match operand.access {
                                RegisterOperandAccess::Def
                                    if !defined
                                        && current_block_index == block_index
                                        && Some(instruction.id) == definition =>
                                {
                                    defined = true;
                                }
                                RegisterOperandAccess::Def
                                    if (defined || current_block_index != block_index)
                                        && Some(instruction.id) != definition
                                        && operand.class == victim.class =>
                                {
                                    if redefinitions[current_block_index].insert(instruction_index)
                                    {
                                        definitions.push(StorageDefinition {
                                            block_index: current_block_index,
                                            position: StoragePosition::AfterInstruction(
                                                instruction.id,
                                            ),
                                            register,
                                        });
                                    } else if usedef_writes[current_block_index]
                                        .contains(&instruction_index)
                                    {
                                        return Err(RuntimeSpillError::UnsupportedUse);
                                    }
                                }
                                RegisterOperandAccess::Use
                                    if (defined || current_block_index != block_index)
                                        && Some(instruction.id) != definition
                                        && operand.tied_to.is_none()
                                        && !operand.early_clobber
                                        && operand.class == victim.class =>
                                {
                                    if established_stores.contains(&instruction.id) {
                                        if operand.operand != 0
                                            || !matches!(
                                                instruction.kind,
                                                SelectedInstructionKind::Store { .. }
                                                    | SelectedInstructionKind::ZeroBytes
                                            )
                                        {
                                            return Err(RuntimeSpillError::UnsupportedUse);
                                        }
                                        retained_uses.insert((instruction.id, operand.operand));
                                        continue;
                                    }
                                    let mut tied_writes = instruction
                                        .operands
                                        .iter()
                                        .filter(|other| other.tied_to == Some(operand.operand));
                                    let tied = tied_writes.next();
                                    if let Some(other) = tied
                                        && (other.access != RegisterOperandAccess::Def
                                            || other.virtual_register != register
                                            || tied_writes.next().is_some())
                                    {
                                        return Err(RuntimeSpillError::UnsupportedUse);
                                    }
                                    if let Some(other) = tied
                                        && other.early_clobber
                                        && operand.fixed_view.is_none()
                                        && instruction.operands.iter().any(|co| {
                                            co.operand != operand.operand
                                                && co.virtual_register == register
                                                && matches!(
                                                    co.access,
                                                    RegisterOperandAccess::Use
                                                        | RegisterOperandAccess::UseDef
                                                )
                                                && co.fixed_view.is_none()
                                        })
                                    {
                                        dedicated_tie_uses[current_block_index]
                                            .insert((instruction_index, operand.operand));
                                        let dedicated =
                                            &mut use_positions[current_block_index].dedicated;
                                        if dedicated.last() != Some(&instruction_index) {
                                            dedicated.push(instruction_index);
                                        }
                                    }
                                    use_positions[current_block_index].record_use(
                                        instruction_index,
                                        operand.fixed_view.is_some(),
                                        &mut flexible_uses[current_block_index],
                                    );
                                    uses = uses
                                        .checked_add(1)
                                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                                }
                                RegisterOperandAccess::UseDef
                                    if (defined || current_block_index != block_index)
                                        && Some(instruction.id) != definition
                                        && operand.tied_to.is_none()
                                        && !operand.early_clobber
                                        && operand.class == victim.class =>
                                {
                                    if established_stores.contains(&instruction.id)
                                        || instruction
                                            .operands
                                            .iter()
                                            .any(|other| other.tied_to == Some(operand.operand))
                                        || !redefinitions[current_block_index]
                                            .insert(instruction_index)
                                    {
                                        return Err(RuntimeSpillError::UnsupportedUse);
                                    }
                                    usedef_writes[current_block_index].insert(instruction_index);
                                    definitions.push(StorageDefinition {
                                        block_index: current_block_index,
                                        position: StoragePosition::AfterUseDef {
                                            instruction: instruction.id,
                                            operand: operand.operand,
                                        },
                                        register,
                                    });
                                    use_positions[current_block_index].record_use(
                                        instruction_index,
                                        operand.fixed_view.is_some(),
                                        &mut flexible_uses[current_block_index],
                                    );
                                    uses = uses
                                        .checked_add(1)
                                        .ok_or(RuntimeSpillError::IdentityOverflow)?;
                                }
                                _ => return Err(RuntimeSpillError::UnsupportedUse),
                            }
                        }
                        Ok(())
                    };
                if !has_interior {
                    visit_instruction(start_idx, &block.instructions[start_idx])?;
                    if end_idx > start_idx {
                        visit_instruction(end_idx, &block.instructions[end_idx])?;
                    }
                } else {
                    for (offset, instruction) in
                        block.instructions[start_idx..=end_idx].iter().enumerate()
                    {
                        visit_instruction(start_idx + offset, instruction)?;
                    }
                }
            }
            if uses != previous_uses {
                use_blocks.push(current_block_index);
            }
        }
        if !defined || uses == 0 {
            return Err(RuntimeSpillError::UnsupportedValue);
        }
        let keys = environment.selected_keys();
        let address = environment
            .constraint(
                keys.frame_address
                    .ok_or(RuntimeSpillError::ConstraintMismatch)?,
            )
            .ok_or(RuntimeSpillError::ConstraintMismatch)?;
        let load = environment
            .constraint(keys.load64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
            .ok_or(RuntimeSpillError::ConstraintMismatch)?;
        let store = environment
            .constraint(keys.store64.ok_or(RuntimeSpillError::ConstraintMismatch)?)
            .ok_or(RuntimeSpillError::ConstraintMismatch)?;
        if !load.implicit_uses.is_empty() {
            return Err(RuntimeSpillError::ConstraintMismatch);
        }
        for (row, accesses) in [
            (address, &[RegisterOperandAccess::Def][..]),
            (
                load,
                &[RegisterOperandAccess::Use, RegisterOperandAccess::Def][..],
            ),
            (
                store,
                if store.operands.len() == 2 {
                    &[RegisterOperandAccess::Use, RegisterOperandAccess::Def][..]
                } else {
                    &[RegisterOperandAccess::Use][..]
                },
            ),
        ] {
            if row.operands.len() != accesses.len() || !row.implicit_defs.is_empty() {
                return Err(RuntimeSpillError::ConstraintMismatch);
            }
            for (ordinal, (operand, access)) in row.operands.iter().zip(accesses).enumerate() {
                if usize::from(operand.operand) != ordinal
                    || operand.access != *access
                    || operand.fixed_view.is_some()
                    || operand.tied_to.is_some()
                    || operand.early_clobber != (row.key == store.key && ordinal == 1)
                {
                    return Err(RuntimeSpillError::ConstraintMismatch);
                }
            }
        }
        let carrier_class = address.operands[0].class;
        if [address, load, store]
            .iter()
            .flat_map(|row| row.operands.iter())
            .any(|operand| operand.class != carrier_class)
        {
            return Err(RuntimeSpillError::ConstraintMismatch);
        }
        let access_rows = [address, load, store];
        if access_rows.iter().any(|writer| {
            writer.clobbers.iter().any(|unit| {
                access_rows
                    .iter()
                    .any(|reader| reader.implicit_uses.contains(unit))
            })
        }) {
            return Err(RuntimeSpillError::ConstraintMismatch);
        }
        let bits_conversion = if victim.class == carrier_class {
            None
        } else {
            Some(bits_transport(environment, victim, carrier_class)?)
        };
        let function_scan =
            crate::rewrites::function_scan(function).ok_or(RuntimeSpillError::IdentityOverflow)?;
        let mut steps = crate::rewrites::rewrite_scan(plan, function)
            .and_then(|total| {
                total.checked_add(uses.checked_mul(if bits_conversion.is_some() {
                    6
                } else {
                    4
                })?)
            })
            .and_then(|total| {
                total.checked_add(
                    structural_uses
                        .len()
                        .checked_mul(if bits_conversion.is_some() { 6 } else { 4 })?,
                )
            })
            .and_then(|total| {
                total.checked_add(definitions.len().checked_mul(
                    (if bits_conversion.is_some() { 3 } else { 1 }) + store.operands.len() - 1,
                )?)
            })
            .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
            .and_then(|total| total.checked_add(function_scan))
            .and_then(|total| {
                total.checked_add(if access_rows.iter().any(|row| !row.clobbers.is_empty()) {
                    function_scan
                } else {
                    0
                })
            })
            .and_then(|total| total.checked_add(function.memory_accesses.len().checked_mul(3)?))
            .ok_or(RuntimeSpillError::IdentityOverflow)?;
        let charged =
            |steps: usize| u64::try_from(steps).map_err(|_| RuntimeSpillError::IdentityOverflow);
        let ceiling = crate::rewrites::required_rewrite_budget(plan, function, budget)
            .ok_or(RuntimeSpillError::IdentityOverflow)?
            .validation_steps();
        if charged(steps)? > ceiling {
            return Err(RuntimeSpillError::WorkBudgetExceeded);
        }
        require_dominated_uses(function, block_index, &use_blocks)?;
        let first_instruction =
            next_instruction_id(function).ok_or(RuntimeSpillError::IdentityOverflow)?;
        let first_register =
            next_register_id(function).ok_or(RuntimeSpillError::IdentityOverflow)?;
        let any_flexible = flexible_uses.iter().any(|&flexible| flexible);
        let mut implicit_use_units = BTreeSet::new();
        let mut pinned_units = BTreeSet::new();
        let mut base_excluded_mask = 0u128;
        let mut can_use_unit_mask = false;
        if any_flexible {
            let model = environment.physical().model();
            can_use_unit_mask = model.views.iter().all(|v| {
                v.units
                    .iter()
                    .chain(&v.write_units)
                    .all(|u| (u.0 as usize) < 128)
            });
            let mut units_mask = 0u128;
            let mut views_mask = 0u128;
            let mut seen_units = Vec::new();
            let mut seen_views = Vec::new();
            let mut mark_unit = |unit: RegisterUnitId| {
                let idx = usize::from(unit.0);
                if idx < 128 {
                    units_mask |= 1u128 << idx;
                } else {
                    if idx >= seen_units.len() {
                        seen_units.resize(idx + 1, false);
                    }
                    seen_units[idx] = true;
                }
            };
            let mut mark_view = |view: register_model::RegisterViewId| {
                let idx = usize::from(view.0);
                if idx < 128 {
                    views_mask |= 1u128 << idx;
                } else {
                    if idx >= seen_views.len() {
                        seen_views.resize(idx + 1, false);
                    }
                    seen_views[idx] = true;
                }
            };
            for register in &function.virtual_registers {
                if let Some(view) = register.entry_fixed_view {
                    mark_view(view);
                } else if register.id.0 as usize >= 256 {
                    for tail in &function.virtual_registers[register.id.0 as usize..] {
                        if let Some(view) = tail.entry_fixed_view {
                            mark_view(view);
                        }
                    }
                    break;
                }
            }
            let mut block_units_mask = 0u128;
            let mut block_views_mask = 0u128;
            for block_index in 0..function.blocks.len() {
                if let Some((implicit_mask, _, _, fixed_mask, has_high)) =
                    function.blocks.block_unit_masks(block_index)
                    && !has_high
                {
                    block_units_mask |= implicit_mask as u128;
                    block_views_mask |= fixed_mask as u128;
                } else if let Some((units, views)) = function
                    .blocks
                    .block_implicit_units_and_fixed_views(block_index)
                {
                    for &unit in units {
                        mark_unit(unit);
                    }
                    for &view in views {
                        mark_view(view);
                    }
                }
            }
            for &unit in address
                .implicit_uses
                .iter()
                .chain(&load.implicit_uses)
                .chain(&store.implicit_uses)
            {
                mark_unit(unit);
            }
            if let Some(conversion) = &bits_conversion {
                for &unit in conversion
                    .to_bits
                    .implicit_uses
                    .iter()
                    .chain(&conversion.from_bits.implicit_uses)
                {
                    mark_unit(unit);
                }
            }
            units_mask |= block_units_mask;
            views_mask |= block_views_mask;
            can_use_unit_mask &= seen_units.is_empty();
            if can_use_unit_mask {
                let mut pinned_mask = 0u128;
                let mut vm = views_mask;
                while vm != 0 {
                    let bit = vm.trailing_zeros() as usize;
                    vm &= vm - 1;
                    if let Some(view) = model.views.get(bit) {
                        for &u in view.units.iter().chain(&view.write_units) {
                            pinned_mask |= 1u128 << u.0;
                        }
                    }
                }
                for (idx, &seen) in seen_views.iter().enumerate() {
                    if seen && let Some(view) = model.views.get(idx) {
                        for &u in view.units.iter().chain(&view.write_units) {
                            pinned_mask |= 1u128 << u.0;
                        }
                    }
                }
                let mut reserved_mask = 0u128;
                for &u in environment.reservations().reserved_units() {
                    if (u.0 as usize) < 128 {
                        reserved_mask |= 1u128 << u.0;
                    }
                }
                base_excluded_mask = units_mask | pinned_mask | reserved_mask;
            } else {
                let mut um = units_mask;
                while um != 0 {
                    let bit = um.trailing_zeros() as u16;
                    um &= um - 1;
                    implicit_use_units.insert(RegisterUnitId(bit));
                }
                for (idx, &seen) in seen_units.iter().enumerate() {
                    if seen && let Ok(id) = u16::try_from(idx) {
                        implicit_use_units.insert(RegisterUnitId(id));
                    }
                }
                let mut pinned_mask = 0u128;
                let mut vm = views_mask;
                while vm != 0 {
                    let bit = vm.trailing_zeros() as usize;
                    vm &= vm - 1;
                    if let Some(view) = model.views.get(bit) {
                        for &u in view.units.iter().chain(&view.write_units) {
                            if (u.0 as usize) < 128 {
                                pinned_mask |= 1u128 << u.0;
                            } else {
                                pinned_units.insert(u);
                            }
                        }
                    }
                }
                while pinned_mask != 0 {
                    let bit = pinned_mask.trailing_zeros() as u16;
                    pinned_mask &= pinned_mask - 1;
                    pinned_units.insert(RegisterUnitId(bit));
                }
                for (idx, &seen) in seen_views.iter().enumerate() {
                    if seen && let Some(view) = model.views.get(idx) {
                        for &u in view.units.iter().chain(&view.write_units) {
                            pinned_units.insert(u);
                        }
                    }
                }
            }
        }
        let surviving_home_exists_mask = |excluded_mask: u128| -> bool {
            let physical = environment.physical().model();
            physical
                .classes
                .iter()
                .find(|row| row.id == victim.class)
                .is_some_and(|row| {
                    row.views.iter().any(|view_id| {
                        physical
                            .views
                            .get(usize::from(view_id.0))
                            .is_some_and(|view| {
                                view.id == *view_id
                                    && view.allocatable
                                    && view
                                        .units
                                        .iter()
                                        .chain(&view.write_units)
                                        .all(|unit| (excluded_mask & (1u128 << unit.0)) == 0)
                            })
                    })
                })
        };
        let empty_writes = BTreeSet::new();
        let base_surviving_home = any_flexible
            && if can_use_unit_mask {
                surviving_home_exists_mask(base_excluded_mask)
            } else {
                surviving_home_exists(
                    environment,
                    victim.class,
                    &implicit_use_units,
                    &pinned_units,
                    &empty_writes,
                )
            };
        let shared_reload: Vec<bool> = flexible_uses
            .iter()
            .map(|&flexible| flexible && base_surviving_home)
            .collect();
        let mut crossed_unit_writes: Vec<BTreeSet<usize>> =
            function.blocks.iter().map(|_| BTreeSet::new()).collect();
        if span_policy == RuntimeSpillSpanPolicy::UnitWriteCrossing {
            for &block_index in &use_blocks {
                if !shared_reload[block_index] {
                    continue;
                }
                let block = &function.blocks[block_index];
                let has_structural = structural_uses
                    .iter()
                    .any(|pending| pending.block == block_index);
                let first_unpinned = use_positions[block_index].unpinned.first().copied();
                let last_unpinned = use_positions[block_index]
                    .unpinned
                    .iter()
                    .next_back()
                    .copied();
                let mut open = false;
                let mut written_mask = 0u128;
                let mut written = BTreeSet::new();
                if let Some(start_idx) = first_unpinned {
                    for (offset, instruction) in block.instructions[start_idx..].iter().enumerate()
                    {
                        let index = start_idx + offset;
                        if !has_structural && last_unpinned.is_none_or(|last| index >= last) {
                            break;
                        }
                        if use_positions[block_index]
                            .unpinned
                            .binary_search(&index)
                            .is_ok()
                        {
                            open = true;
                        }
                        if redefinitions[block_index].contains(&index) {
                            open = false;
                            written_mask = 0;
                            written.clear();
                            continue;
                        }
                        if instruction.clobbers.is_empty() && instruction.implicit_defs.is_empty()
                            || !open
                        {
                            continue;
                        }
                        if can_use_unit_mask
                            && instruction
                                .clobbers
                                .iter()
                                .chain(&instruction.implicit_defs)
                                .all(|u| (u.0 as usize) < 128)
                        {
                            let mut trial_mask = written_mask;
                            for &u in instruction
                                .clobbers
                                .iter()
                                .chain(&instruction.implicit_defs)
                            {
                                trial_mask |= 1u128 << u.0;
                            }
                            if surviving_home_exists_mask(base_excluded_mask | trial_mask) {
                                written_mask = trial_mask;
                                crossed_unit_writes[block_index].insert(index);
                            } else {
                                open = false;
                                written_mask = 0;
                            }
                        } else {
                            let mut trial = written.clone();
                            trial.extend(instruction.clobbers.iter().copied());
                            trial.extend(instruction.implicit_defs.iter().copied());
                            if surviving_home_exists(
                                environment,
                                victim.class,
                                &implicit_use_units,
                                &pinned_units,
                                &trial,
                            ) {
                                written = trial;
                                crossed_unit_writes[block_index].insert(index);
                            } else {
                                open = false;
                                written.clear();
                            }
                        }
                    }
                }
            }
        }
        let crossed_unit_write = crossed_unit_writes.iter().any(|set| !set.is_empty());
        let mut span_closes: Vec<Vec<usize>> = vec![Vec::new(); function.blocks.len()];
        for &block_index in &use_blocks {
            if !shared_reload[block_index]
                && redefinitions[block_index].is_empty()
                && !structural_uses
                    .iter()
                    .any(|pending| pending.block == block_index)
            {
                continue;
            }
            if let Some(writers) = function.blocks.block_unit_writer_indices(block_index) {
                let crossed = &crossed_unit_writes[block_index];
                if redefinitions[block_index].is_empty() && crossed.is_empty() {
                    span_closes[block_index] = writers.to_vec();
                } else {
                    let mut closes: Vec<usize> =
                        redefinitions[block_index].iter().copied().collect();
                    if crossed.is_empty() {
                        closes.extend_from_slice(writers);
                    } else {
                        for &index in writers {
                            if !crossed.contains(&index) {
                                closes.push(index);
                            }
                        }
                    }
                    closes.sort_unstable();
                    closes.dedup();
                    span_closes[block_index] = closes;
                }
            } else {
                let block = &function.blocks[block_index];
                span_closes[block_index] = block
                    .instructions
                    .iter()
                    .enumerate()
                    .filter_map(|(index, instruction)| {
                        (redefinitions[block_index].contains(&index)
                            || ((!instruction.clobbers.is_empty()
                                || !instruction.implicit_defs.is_empty())
                                && !crossed_unit_writes[block_index].contains(&index)))
                        .then_some(index)
                    })
                    .collect();
            }
        }
        if !structural_uses.is_empty() {
            let mut chunk_loads: std::collections::BTreeMap<
                (EdgeId, PlaceId),
                Vec<&selected_instructions::SelectedMemoryAccess>,
            > = std::collections::BTreeMap::new();
            for access in &function.memory_accesses {
                if let SelectedMemoryAccessOrigin::Edge(edge) = access.origin
                    && matches!(
                        access.role,
                        SelectedMemoryAccessRole::ReadPlace
                            | SelectedMemoryAccessRole::ReadIndexedPrimitive { .. }
                    )
                {
                    chunk_loads
                        .entry((edge, access.place))
                        .or_default()
                        .push(access);
                }
            }
            for accesses in chunk_loads.values_mut() {
                accesses.sort_by_key(|access| access.byte_offset);
            }
            steps = structural_uses
                .iter()
                .try_fold(function_scan, |total, pending| {
                    total.checked_add(
                        chunk_loads
                            .get(&(pending.edge, pending.place))
                            .map_or(0, Vec::len),
                    )
                })
                .and_then(|walks| steps.checked_add(walks))
                .ok_or(RuntimeSpillError::IdentityOverflow)?;
            if charged(steps)? > ceiling {
                return Err(RuntimeSpillError::WorkBudgetExceeded);
            }
            for pending in &mut structural_uses {
                if pending.byte_size == 0 {
                    pending.dedicated_reload = true;
                    use_positions[pending.block].end_of_block = true;
                    continue;
                }
                let Some(accesses) = chunk_loads.get(&(pending.edge, pending.place)) else {
                    return Err(RuntimeSpillError::UnsupportedUse);
                };
                let block = &function.blocks[pending.block];
                let mut positions = Vec::with_capacity(accesses.len());
                let mut read_instructions = Vec::with_capacity(accesses.len());
                let mut pinned = false;
                let mut offset = 0u32;
                for access in accesses.iter().filter(|access| {
                    access.byte_offset >= pending.source_byte_offset
                        && access.byte_offset - pending.source_byte_offset < pending.byte_size
                        && pending.source_read_roles.contains(&access.role)
                }) {
                    let Some(index) = function
                        .blocks
                        .instruction_index_in_block(pending.block, access.instruction)
                    else {
                        continue;
                    };
                    let instruction = &block.instructions[index];
                    if instruction.kind == SelectedInstructionKind::CopyBytes {
                        let Some(use_index) = snapshot_copy::source_use(
                            block,
                            index,
                            register,
                            pending.source_byte_offset,
                            pending.byte_size,
                            &mut steps,
                            ceiling,
                        )?
                        else {
                            continue;
                        };
                        let source_use = &block.instructions[use_index];
                        if read_instructions.contains(&access.instruction) {
                            continue;
                        }
                        if offset != 0
                            || access.byte_offset != pending.source_byte_offset
                            || access.byte_count != pending.byte_size
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        pinned |= source_use.operands[0].fixed_view.is_some();
                        positions.push(use_index);
                        pending.loads.push(source_use.id);
                        read_instructions.push(access.instruction);
                        offset = pending.byte_size;
                        continue;
                    }
                    if !instruction
                        .operands
                        .first()
                        .is_some_and(|operand| operand.virtual_register == register)
                    {
                        continue;
                    }
                    if read_instructions.contains(&access.instruction) {
                        continue;
                    }
                    let Some(width) = next_chunk(pending.byte_size, offset) else {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    };
                    let source_offset = pending
                        .source_byte_offset
                        .checked_add(offset)
                        .ok_or(RuntimeSpillError::UnsupportedUse)?;
                    if access.byte_offset != source_offset || access.byte_count != u32::from(width)
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    let Some(index) = function
                        .blocks
                        .instruction_index_in_block(pending.block, access.instruction)
                    else {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    };
                    let instruction = &block.instructions[index];
                    let expected_kind = match width {
                        8 => SelectedInstructionKind::Load64 {
                            byte_offset: source_offset,
                        },
                        4 => SelectedInstructionKind::Load32 {
                            byte_offset: source_offset,
                        },
                        2 => SelectedInstructionKind::Load16 {
                            byte_offset: source_offset,
                        },
                        _ => SelectedInstructionKind::Load8 {
                            byte_offset: source_offset,
                        },
                    };
                    let Some(address) = instruction.operands.first() else {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    };
                    if instruction.kind != expected_kind
                        || address.virtual_register != register
                        || address.access != RegisterOperandAccess::Use
                    {
                        return Err(RuntimeSpillError::UnsupportedUse);
                    }
                    pinned |= address.fixed_view.is_some();
                    positions.push(index);
                    pending.loads.push(access.instruction);
                    read_instructions.push(access.instruction);
                    offset += u32::from(width);
                }
                if offset != pending.byte_size {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                let first = *positions.iter().min().unwrap();
                pending.dedicated_reload = span_closes[pending.block]
                    .last()
                    .is_some_and(|&last| last >= first)
                    || (pending.loads.len() > 1 && (!shared_reload[pending.block] || pinned));
                if pending.dedicated_reload {
                    use_positions[pending.block].end_of_block = true;
                }
            }
        }
        unit_effects::preserve_live_units(
            function_index,
            function,
            &definitions,
            &use_positions,
            address,
            load,
            store,
        )?;
        let reuse_slots = function
            .local_storage_slots
            .len()
            .checked_mul(function_scan)
            .and_then(|slot_scan| steps.checked_add(slot_scan))
            .map(charged)
            .transpose()?
            .is_some_and(|total| total <= ceiling);
        let assignment = slot::assign(
            function,
            register,
            &definitions,
            &use_positions,
            &shared_reload,
            &span_closes,
            reuse_slots,
        );
        Ok(RuntimeSpillDerivation {
            function,
            use_blocks,
            shared_reload,
            dedicated_tie_uses,
            span_closes,
            crossed_unit_write,
            retained_uses,
            victim,
            lineage,
            address_scalar_type: ScalarType::Integer(unsigned),
            definitions,
            assignment,
            structural_uses,
            first_instruction,
            first_register,
            address,
            load,
            store,
            carrier_class,
            bits_conversion,
        })
    }

    pub(crate) fn parameter_definitions(
        function: &SelectedFunction,
        destination: usize,
        victim: &VirtualRegister,
        source_value: ValueId,
    ) -> Result<Vec<StorageDefinition>, RuntimeSpillError> {
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            SelectedBlockOrigin, SelectedCasePayloadTransport, SelectedSuccessorRole,
            SelectedTerminator, SelectedValueTransport, VirtualRegisterOrigin,
        };
        use slot::physical_definition;
        let mut definitions = Vec::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            for successor in control(&block.terminator).1.into_iter().flatten() {
                if successor.block != function.blocks[destination].id {
                    continue;
                }
                if successor.role != SelectedSuccessorRole::EdgeTransferContinuation
                    || !matches!(block.terminator, SelectedTerminator::Jump { .. })
                    || !matches!(block.origin, SelectedBlockOrigin::EdgeTransfer { edge, target }
                        if Some(edge) == successor.psi_edge && target == successor.source_target)
                {
                    return Err(RuntimeSpillError::UnsupportedControlFlow);
                }
                if let Some(case) = &successor.structural_case {
                    let mut payloads = case
                        .payloads
                        .iter()
                        .filter(|payload| payload.semantic.parameter.value == source_value);
                    if let Some(payload) = payloads.next() {
                        if payloads.next().is_some()
                            || successor
                                .bindings
                                .iter()
                                .any(|binding| binding.semantic.parameter == source_value)
                            || payload.semantic.parameter.scalar_type != victim.scalar_type
                            || Some(payload.semantic.parameter.definition_site)
                                != victim.definition_site
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        let SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                            ..
                        } = payload.transport
                        else {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        };
                        if parameter != victim.id {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        let value = function
                            .virtual_register(argument)
                            .ok_or(RuntimeSpillError::UnsupportedValue)?;
                        if value.scalar_type != victim.scalar_type
                            || value.class != victim.class
                            || value.definition_site.is_some()
                        {
                            return Err(RuntimeSpillError::UnsupportedValue);
                        }
                        let VirtualRegisterOrigin::StructuralObservation {
                            instruction,
                            place,
                            byte_offset,
                        } = value.origin
                        else {
                            return Err(RuntimeSpillError::UnsupportedValue);
                        };
                        if case.source.place() != Some(place)
                            || byte_offset != payload.semantic.field_byte_offset
                        {
                            return Err(RuntimeSpillError::UnsupportedValue);
                        }
                        let (owner, load) = physical_definition(function, argument)?;
                        let (byte_count, displacement) =
                            match (payload.semantic.parameter.scalar_type, load.kind) {
                                (
                                    ScalarType::Integer(integer),
                                    SelectedInstructionKind::Load32 { byte_offset },
                                ) if integer.bits() == 32 => (4, byte_offset),
                                (
                                    ScalarType::Integer(integer),
                                    SelectedInstructionKind::Load64 { byte_offset },
                                ) if integer.bits() == 64 => (8, byte_offset),
                                _ => return Err(RuntimeSpillError::UnsupportedValue),
                            };
                        if owner != block_index
                            || load.id != instruction
                            || !case_payload_address::case_payload_address_corresponds(
                                function,
                                block_index,
                                load,
                                case.source,
                                successor
                                    .psi_edge
                                    .ok_or(RuntimeSpillError::UnsupportedControlFlow)?,
                                byte_offset,
                                byte_count,
                                displacement,
                            )
                            || !load.operands.iter().any(|operand| {
                                operand.virtual_register == argument
                                    && operand.access == RegisterOperandAccess::Def
                            })
                        {
                            return Err(RuntimeSpillError::UnsupportedUse);
                        }
                        definitions.push(StorageDefinition {
                            block_index,
                            position: StoragePosition::AfterInstruction(instruction),
                            register: argument,
                        });
                        continue;
                    }
                }
                let mut bindings = successor.bindings.iter().filter(|binding| {
                    matches!(binding.transport,
                    SelectedValueTransport::Registers { parameter, .. } if parameter == victim.id)
                });
                let binding = bindings.next().ok_or(RuntimeSpillError::UnsupportedUse)?;
                if bindings.next().is_some()
                    || binding.semantic.parameter != source_value
                    || binding.semantic.scalar_type != victim.scalar_type
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                let SelectedValueTransport::Registers { argument, .. } = binding.transport else {
                    return Err(RuntimeSpillError::UnsupportedUse);
                };
                let value = function
                    .virtual_register(argument)
                    .ok_or(RuntimeSpillError::UnsupportedValue)?;
                if value.scalar_type != victim.scalar_type || value.class != victim.class {
                    return Err(RuntimeSpillError::UnsupportedValue);
                }
                let VirtualRegisterOrigin::InstructionResult {
                    instruction,
                    source_value: argument_value,
                } = value.origin
                else {
                    return Err(RuntimeSpillError::UnsupportedValue);
                };
                if argument_value != binding.semantic.argument {
                    return Err(RuntimeSpillError::UnsupportedValue);
                }
                let (owner, definition) = physical_definition(function, argument)?;
                if owner != block_index
                    || definition.id != instruction
                    || !definition.operands.iter().any(|operand| {
                        operand.virtual_register == argument
                            && operand.access == RegisterOperandAccess::Def
                    })
                {
                    return Err(RuntimeSpillError::UnsupportedUse);
                }
                definitions.push(StorageDefinition {
                    block_index,
                    position: StoragePosition::AfterInstruction(instruction),
                    register: argument,
                });
            }
        }
        if definitions.is_empty() {
            return Err(RuntimeSpillError::UnsupportedUse);
        }
        Ok(definitions)
    }
}

pub(crate) mod runtime_split {
    pub(crate) mod admission {
        use optimization_core::OptimizationWorkBudget;
        use optimization_unit::ValueDefinitionSite;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::{RegisterInstructionConstraint, RegisterOperandAccess};
        use selected_instructions::{
            SelectedCaseDispatchSource, SelectedCasePayloadTransport, SelectedFunction,
            SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
            SelectedInstructionProvenance, SelectedOperand, SelectedStructuralTransport,
            SelectedValueTransport, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::{ScalarType, ValueId};

        use super::RuntimeSplitError;
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::runtime_spill::{
            control, next_instruction_id, next_register_id, require_dominated_uses,
        };
        use crate::rewrites::selected_operands::copy_shaped;

        pub(crate) struct Admission {
            pub block_index: usize,
            pub definition_index: usize,
            pub use_blocks: Vec<usize>,
            pub register: VirtualRegister,
            pub copy: SelectedInstruction,
        }

        pub(crate) fn admit(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            register: VirtualRegisterId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<Admission, RuntimeSplitError> {
            let (plan, function) =
                crate::rewrites::plan_function(source, function_index, environment, || {
                    RuntimeSplitError::SourceMismatch
                })?;
            let victim = function
                .virtual_register(register)
                .ok_or(RuntimeSplitError::UnsupportedValue)?;
            let VirtualRegisterOrigin::InstructionResult { source_value, .. } = victim.origin
            else {
                return Err(RuntimeSplitError::UnsupportedValue);
            };
            let scalar_payload = match victim.scalar_type {
                ScalarType::Callable(_) | ScalarType::Boolean => true,
                ScalarType::IeeeFloat(_) => false,
                ScalarType::Integer(integer) => {
                    !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64)
                }
            };
            if !scalar_payload
                || victim.entry_fixed_view.is_some()
                || !matches!(
                    victim.definition_site,
                    Some(ValueDefinitionSite::FunctionParameter(_))
                        | Some(ValueDefinitionSite::BlockParameter { .. })
                        | Some(ValueDefinitionSite::Node { .. })
                )
            {
                return Err(RuntimeSplitError::UnsupportedValue);
            }
            let copy_row = copy_row(environment, victim)?;
            if !function.blocks.has_fixed_definition(register) {
                return Err(RuntimeSplitError::UnsupportedValue);
            }
            let (block_index, definition_index) = function
                .blocks
                .iter()
                .enumerate()
                .find_map(|(block_index, block)| {
                    let (start_idx, end_idx) = function
                        .blocks
                        .block_register_instruction_span(block_index, register)?;
                    block.instructions[start_idx..=end_idx]
                        .iter()
                        .position(|candidate| {
                            candidate.operands.iter().any(|operand| {
                                operand.virtual_register == register
                                    && operand.access != RegisterOperandAccess::Use
                            })
                        })
                        .map(|offset| (block_index, start_idx + offset))
                })
                .ok_or(RuntimeSplitError::UnsupportedValue)?;
            let defining = &function.blocks[block_index].instructions[definition_index];
            let mut naming = defining
                .operands
                .iter()
                .filter(|operand| operand.virtual_register == register);
            let Some(written) = naming.next() else {
                return Err(RuntimeSplitError::UnsupportedValue);
            };
            if naming.next().is_some()
                || written.access != RegisterOperandAccess::Def
                || written.fixed_view.is_none()
                || written.tied_to.is_some()
                || written.early_clobber
                || written.class != victim.class
            {
                return Err(RuntimeSplitError::UnsupportedValue);
            }
            let mut uses = 0usize;
            let mut use_blocks = Vec::new();
            for (current_block_index, block) in function.blocks.iter().enumerate() {
                if !function
                    .blocks
                    .block_references_register(current_block_index, register)
                {
                    continue;
                }
                let previous_uses = uses;
                if let Some((start_idx, end_idx)) = function
                    .blocks
                    .block_register_instruction_span(current_block_index, register)
                {
                    for (offset, instruction) in
                        block.instructions[start_idx..=end_idx].iter().enumerate()
                    {
                        let instruction_index = start_idx + offset;
                        if current_block_index == block_index
                            && instruction_index == definition_index
                        {
                            continue;
                        }
                        let after_definition = current_block_index != block_index
                            || instruction_index > definition_index;
                        for operand in &instruction.operands {
                            if operand.virtual_register != register {
                                continue;
                            }
                            if !after_definition || !rebindable_use(operand, victim) {
                                return Err(RuntimeSplitError::UnsupportedUse);
                            }
                            uses = uses
                                .checked_add(1)
                                .ok_or(RuntimeSplitError::IdentityOverflow)?;
                        }
                    }
                }
                let (terminal, successors) = control(&block.terminator);
                for operand in &terminal.operands {
                    if operand.virtual_register != register {
                        continue;
                    }
                    if !rebindable_use(operand, victim) {
                        return Err(RuntimeSplitError::UnsupportedUse);
                    }
                    uses = uses
                        .checked_add(1)
                        .ok_or(RuntimeSplitError::IdentityOverflow)?;
                }
                for successor in successors.into_iter().flatten() {
                    for binding in &successor.bindings {
                        let SelectedValueTransport::Registers {
                            argument,
                            parameter,
                        } = binding.transport
                        else {
                            continue;
                        };
                        if parameter == register {
                            return Err(RuntimeSplitError::UnsupportedUse);
                        }
                        if argument != register {
                            continue;
                        }
                        if binding.semantic.argument != source_value
                            || binding.semantic.scalar_type != victim.scalar_type
                        {
                            return Err(RuntimeSplitError::UnsupportedUse);
                        }
                        uses = uses
                            .checked_add(1)
                            .ok_or(RuntimeSplitError::IdentityOverflow)?;
                    }
                    if names_structurally(successor, register) {
                        return Err(RuntimeSplitError::UnsupportedUse);
                    }
                }
                if uses != previous_uses {
                    use_blocks.push(current_block_index);
                }
            }
            if uses == 0 {
                return Err(RuntimeSplitError::UnsupportedValue);
            }
            charge(plan, function, uses, budget)?;
            require_dominated_uses(function, block_index, &use_blocks).map_err(
                |error| match error {
                    crate::RuntimeSpillError::SourceMismatch => RuntimeSplitError::SourceMismatch,
                    _ => RuntimeSplitError::UnsupportedUse,
                },
            )?;
            let instruction = SelectedInstructionId(next_instruction(function)?);
            let fresh = VirtualRegisterId(next_register(function)?);
            Ok(Admission {
                block_index,
                definition_index,
                use_blocks,
                register: VirtualRegister {
                    id: fresh,
                    scalar_type: victim.scalar_type,
                    class: victim.class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction,
                        source_value,
                    },
                    definition_site: victim.definition_site,
                    entry_fixed_view: None,
                },
                copy: copy_instruction(copy_row, instruction, register, fresh, source_value),
            })
        }

        pub(crate) fn copy_row<'environment>(
            environment: &'environment ValidatedTargetRegisterEnvironment,
            victim: &VirtualRegister,
        ) -> Result<&'environment RegisterInstructionConstraint, RuntimeSplitError> {
            crate::rewrites::integer_copy_row(environment, victim.class, victim.class, || {
                RuntimeSplitError::ConstraintMismatch
            })
        }

        pub(crate) fn copy_instruction(
            row: &RegisterInstructionConstraint,
            id: SelectedInstructionId,
            victim: VirtualRegisterId,
            fresh: VirtualRegisterId,
            source_value: ValueId,
        ) -> SelectedInstruction {
            copy_shaped(
                id,
                SelectedInstructionKind::CopyI64,
                row,
                victim,
                fresh,
                SelectedInstructionProvenance {
                    values: vec![source_value],
                    ..Default::default()
                },
            )
        }

        pub(crate) fn rebindable_use(operand: &SelectedOperand, victim: &VirtualRegister) -> bool {
            operand.access == RegisterOperandAccess::Use
                && operand.tied_to.is_none()
                && !operand.early_clobber
                && operand.class == victim.class
        }

        pub(crate) fn names_structurally(
            successor: &selected_instructions::SelectedSuccessor,
            register: VirtualRegisterId,
        ) -> bool {
            let mut named = false;
            visit_structural_registers(successor, |candidate| named |= candidate == register);
            named
        }

        pub(crate) fn visit_structural_registers(
            successor: &selected_instructions::SelectedSuccessor,
            mut visit: impl FnMut(VirtualRegisterId),
        ) {
            for binding in &successor.structural_bindings {
                match binding.transport {
                    SelectedStructuralTransport::WholeValue { argument, .. }
                    | SelectedStructuralTransport::ScalarReference {
                        base: selected_instructions::SelectedAddressBase::Register(argument),
                        ..
                    }
                    | SelectedStructuralTransport::Address {
                        base: selected_instructions::SelectedAddressBase::Register(argument),
                        ..
                    } => visit(argument),
                    _ => {}
                }
            }
            if let Some(case) = &successor.structural_case {
                if let SelectedCaseDispatchSource::Borrowed { pointer, .. } = case.source {
                    visit(pointer);
                }
                for payload in &case.payloads {
                    match payload.transport {
                        SelectedCasePayloadTransport::Registers {
                            argument,
                            parameter,
                        } => {
                            visit(argument);
                            visit(parameter);
                        }
                        SelectedCasePayloadTransport::Unmaterialized { parameter } => {
                            visit(parameter)
                        }
                        SelectedCasePayloadTransport::Unused => {}
                    }
                }
            }
        }

        pub(crate) fn charge(
            plan: &selected_instructions::SelectedInstructionPlan,
            function: &SelectedFunction,
            uses: usize,
            budget: OptimizationWorkBudget,
        ) -> Result<(), RuntimeSplitError> {
            let steps = crate::rewrites::rewrite_scan(plan, function)
                .and_then(|total| total.checked_add(uses.checked_mul(2)?))
                .and_then(|total| total.checked_add(function.blocks.len().checked_mul(2)?))
                .ok_or(RuntimeSplitError::IdentityOverflow)?;
            crate::rewrites::charge_steps(
                steps,
                crate::rewrites::required_rewrite_budget(plan, function, budget)
                    .ok_or(RuntimeSplitError::IdentityOverflow)?,
                || RuntimeSplitError::IdentityOverflow,
                || RuntimeSplitError::WorkBudgetExceeded,
            )?;
            Ok(())
        }

        pub(crate) fn next_instruction(
            function: &SelectedFunction,
        ) -> Result<u32, RuntimeSplitError> {
            next_instruction_id(function).ok_or(RuntimeSplitError::IdentityOverflow)
        }

        pub(crate) fn next_register(function: &SelectedFunction) -> Result<u32, RuntimeSplitError> {
            next_register_id(function).ok_or(RuntimeSplitError::IdentityOverflow)
        }
    }
    pub(crate) mod before_use {
        use super::{RuntimeSplitError, ValidatedRuntimeSplit, admission};
        use crate::ValidatedSelectedAnalysis;
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use register_model::RegisterOperandAccess;
        use selected_instructions::{
            SelectedBlockOrigin, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
        };
        use semantic_vocabulary::ScalarType;

        impl ValidatedRuntimeSplit {
            pub fn before_fixed_use(
                source: &impl ValidatedSelectedAnalysis,
                function_index: usize,
                register: VirtualRegisterId,
                environment: &ValidatedTargetRegisterEnvironment,
                budget: OptimizationWorkBudget,
            ) -> Result<Self, RuntimeSplitError> {
                let (plan, function) =
                    crate::rewrites::plan_function(source, function_index, environment, || {
                        RuntimeSplitError::SourceMismatch
                    })?;
                let victim = function
                    .virtual_register(register)
                    .ok_or(RuntimeSplitError::UnsupportedValue)?;
                if !matches!(victim.scalar_type, ScalarType::Integer(integer) if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64))
                {
                    return Err(RuntimeSplitError::UnsupportedValue);
                }
                let source_value = match victim.origin {
                    VirtualRegisterOrigin::InstructionResult { source_value, .. }
                    | VirtualRegisterOrigin::BlockParameter { source_value, .. }
                    | VirtualRegisterOrigin::EntryParameter { source_value, .. } => source_value,
                    _ => return Err(RuntimeSplitError::UnsupportedValue),
                };
                if victim.definition_site.is_none() {
                    return Err(RuntimeSplitError::UnsupportedValue);
                }
                let (block_index, instruction_index, operand_index) = function
                    .blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, block)| {
                        matches!(
                            block.origin,
                            SelectedBlockOrigin::Source(_) | SelectedBlockOrigin::Operation { .. }
                        )
                    })
                    .find_map(|(block_index, block)| {
                        block.instructions.iter().enumerate().find_map(
                            |(instruction_index, instruction)| {
                                instruction
                                    .operands
                                    .iter()
                                    .position(|operand| {
                                        operand.virtual_register == register
                                            && operand.fixed_view.is_some()
                                            && admission::rebindable_use(operand, victim)
                                    })
                                    .map(|operand| (block_index, instruction_index, operand))
                            },
                        )
                    })
                    .ok_or(RuntimeSplitError::UnsupportedUse)?;
                let consumer = &function.blocks[block_index].instructions[instruction_index];
                let chosen = &consumer.operands[operand_index];
                if chosen.access != RegisterOperandAccess::Use
                    || consumer
                        .operands
                        .iter()
                        .any(|operand| operand.tied_to == Some(chosen.operand))
                {
                    return Err(RuntimeSplitError::UnsupportedUse);
                }
                let row = admission::copy_row(environment, victim)?;
                admission::charge(plan, function, 1, budget)?;
                let instruction = selected_instructions::SelectedInstructionId(
                    admission::next_instruction(function)?,
                );
                let fresh = VirtualRegisterId(admission::next_register(function)?);
                let copy =
                    admission::copy_instruction(row, instruction, register, fresh, source_value);
                let result = VirtualRegister {
                    id: fresh,
                    scalar_type: victim.scalar_type,
                    class: victim.class,
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction,
                        source_value,
                    },
                    definition_site: victim.definition_site,
                    entry_fixed_view: None,
                };
                let mut transformed = plan.clone();
                let body = &mut transformed.functions[function_index];
                let block_id = body.blocks[block_index].id;
                body.blocks[block_index].instructions[instruction_index].operands[operand_index]
                    .virtual_register = fresh;
                body.blocks[block_index]
                    .instructions
                    .insert(instruction_index, copy);
                body.virtual_registers.push(result);
                let insertion = u32::try_from(instruction_index)
                    .map_err(|_| RuntimeSplitError::IdentityOverflow)?;
                for settlement in &mut body.boundary_settlements {
                    if settlement.block == block_id && settlement.instruction_index >= insertion {
                        settlement.instruction_index = settlement
                            .instruction_index
                            .checked_add(1)
                            .ok_or(RuntimeSplitError::IdentityOverflow)?;
                    }
                }
                super::validation::seal_runtime_split(source, transformed)
            }
        }
    }
    pub(crate) mod rewrite {
        use optimization_core::OptimizationWorkBudget;
        use register_environment::ValidatedTargetRegisterEnvironment;
        use selected_instructions::{SelectedValueTransport, VirtualRegisterId};

        use super::{RuntimeSplitError, ValidatedRuntimeSplit, admission, shifted_position};
        use crate::ValidatedSelectedAnalysis;
        use crate::rewrites::runtime_spill::{control_mut, control_successors_mut};

        pub fn split_selected_runtime_value(
            source: &impl ValidatedSelectedAnalysis,
            function_index: usize,
            register: VirtualRegisterId,
            environment: &ValidatedTargetRegisterEnvironment,
            budget: OptimizationWorkBudget,
        ) -> Result<ValidatedRuntimeSplit, RuntimeSplitError> {
            let admitted = admission::admit(source, function_index, register, environment, budget)?;
            let fresh = admitted.register.id;
            let definition_index = u32::try_from(admitted.definition_index)
                .map_err(|_| RuntimeSplitError::IdentityOverflow)?;
            let mut transformed = source.selected_plan().clone();
            transformed.functions.mutate_preserving_register_prefix(
                function_index,
                |_local_storage_slots,
                 virtual_registers,
                 boundary_settlements,
                 blocks|
                 -> Result<(), RuntimeSplitError> {
                    for &block_index in &admitted.use_blocks {
                        let span = blocks.block_register_instruction_span(block_index, register);
                        let block = &mut blocks[block_index];
                        if let Some((start_idx, end_idx)) = span {
                            let first_idx = if block_index == admitted.block_index {
                                start_idx.max(admitted.definition_index + 1)
                            } else {
                                start_idx
                            };
                            if first_idx <= end_idx && end_idx < block.instructions.len() {
                                for instruction in &mut block.instructions[first_idx..=end_idx] {
                                    for operand in &mut instruction.operands {
                                        if operand.virtual_register == register {
                                            operand.virtual_register = fresh;
                                        }
                                    }
                                }
                            }
                        }
                        for operand in &mut control_mut(&mut block.terminator).operands {
                            if operand.virtual_register == register {
                                operand.virtual_register = fresh;
                            }
                        }
                        for successor in control_successors_mut(&mut block.terminator)
                            .into_iter()
                            .flatten()
                        {
                            for binding in &mut successor.bindings {
                                if let SelectedValueTransport::Registers { argument, .. } =
                                    &mut binding.transport
                                    && *argument == register
                                {
                                    *argument = fresh;
                                }
                            }
                        }
                    }
                    let block_id = blocks[admitted.block_index].id;
                    blocks[admitted.block_index]
                        .instructions
                        .insert(admitted.definition_index + 1, admitted.copy);
                    virtual_registers.push(admitted.register);
                    for settlement in &mut *boundary_settlements {
                        if settlement.block == block_id {
                            settlement.instruction_index =
                                shifted_position(settlement.instruction_index, definition_index)
                                    .ok_or(RuntimeSplitError::IdentityOverflow)?;
                        }
                    }
                    Ok(())
                },
            )?;
            super::validation::seal_runtime_split(source, transformed)
        }
    }
    pub(crate) mod validation {
        use selected_instructions::SelectedInstructionPlan;

        use super::{RuntimeSplitError, RuntimeSplitReceipt, ValidatedRuntimeSplit};
        use crate::ValidatedSelectedAnalysis;

        pub(crate) fn seal_runtime_split(
            source: &impl ValidatedSelectedAnalysis,
            proposed: SelectedInstructionPlan,
        ) -> Result<ValidatedRuntimeSplit, RuntimeSplitError> {
            Ok(sealed!(
                ValidatedRuntimeSplit,
                RuntimeSplitReceipt,
                source,
                proposed
            ))
        }
        use crate::rewrites::sealed;
    }

    use std::sync::Arc;

    use optimization_core::OptimizationUnitIdentity;
    use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
    use semantic_vocabulary::WorkScheduleIdentity;

    pub(crate) use admission::visit_structural_registers;
    pub use rewrite::split_selected_runtime_value;

    validated_transformed_receipt!(ValidatedRuntimeSplit, RuntimeSplitReceipt);

    rewrite_receipt!(RuntimeSplitReceipt);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RuntimeSplitError {
        SourceMismatch,
        UnsupportedValue,
        UnsupportedUse,
        ConstraintMismatch,
        WorkBudgetExceeded,
        IdentityOverflow,
        ReplayMismatch,
    }

    impl RuntimeSplitError {
        pub const fn is_inadmissible(self) -> bool {
            matches!(self, Self::UnsupportedValue | Self::UnsupportedUse)
        }
    }

    failed_display!(RuntimeSplitError, "invalid runtime live-range split");

    pub(crate) const fn shifted_position(position: u32, definition_index: u32) -> Option<u32> {
        if position > definition_index {
            position.checked_add(1)
        } else {
            Some(position)
        }
    }
}

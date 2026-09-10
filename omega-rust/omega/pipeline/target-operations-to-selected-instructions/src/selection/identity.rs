use super::shared::*;

mod contracts;
mod ordinary;
mod primitives;
mod projected_structural;
mod register;

use primitives::{encode_constraint_key, encode_machine_register};

pub(super) fn receipt(
    plan: &SelectedInstructionPlan,
    legalized: &ValidatedLegalizedOperations,
) -> SelectedInstructionValidationReceipt {
    let function_count = plan.functions.len() + 2 * plan.projected_structural_call_returns.len();
    let block_count = plan
        .functions
        .iter()
        .map(|function| function.blocks.len())
        .sum::<usize>()
        + 2 * plan.projected_structural_call_returns.len();
    let virtual_register_count = plan
        .functions
        .iter()
        .map(|function| function.virtual_registers.len())
        .sum();
    let instruction_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| block.instructions.len() + 1)
        .sum::<usize>();
    SelectedInstructionValidationReceipt {
        identity: selected_instruction_plan_identity(plan),
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        optimization_unit: legalized.receipt().optimization_unit(),
        fuel_schedule: legalized.receipt().fuel_schedule(),
        function_count,
        block_count,
        virtual_register_count,
        instruction_count,
        projected_structural_call_return_count: plan.projected_structural_call_returns.len(),
    }
}

pub fn selected_instruction_plan_identity(
    plan: &SelectedInstructionPlan,
) -> SelectedInstructionPlanIdentity {
    let domain = b"omega.terminal-selected-instructions.v36\0".as_slice();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(&mut bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_id(
            &mut bytes,
            function.attachment.map(|attachment| attachment.get()),
        );
        encode_ids(
            &mut bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|operation| operation.get()),
        );
        encode_ids(
            &mut bytes,
            function.provenance.edges.iter().map(|edge| edge.get()),
        );
        contracts::encode(&mut bytes, function);
        bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
        encode_len(&mut bytes, function.virtual_registers.len());
        for register in &function.virtual_registers {
            register::encode(&mut bytes, register);
        }
        encode_len(&mut bytes, function.blocks.len());
        for block in &function.blocks {
            bytes.extend_from_slice(&block.id.0.to_le_bytes());
            match block.origin {
                selected_instructions::SelectedBlockOrigin::Source(source) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                }
                selected_instructions::SelectedBlockOrigin::CaseDispatch {
                    source,
                    case_ordinal,
                } => {
                    bytes.push(2);
                    bytes.extend_from_slice(&source.get().to_le_bytes());
                    bytes.extend_from_slice(&case_ordinal.to_le_bytes());
                }
                selected_instructions::SelectedBlockOrigin::EdgeTransfer { edge, target } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&edge.get().to_le_bytes());
                    bytes.extend_from_slice(&target.get().to_le_bytes());
                }
            }
            encode_len(&mut bytes, block.instructions.len());
            for instruction in &block.instructions {
                encode_instruction(&mut bytes, instruction);
            }
            ordinary::encode_terminator(&mut bytes, &block.terminator);
        }
    }
    if !plan.projected_structural_call_returns.is_empty() {
        projected_structural::encode(&mut bytes, &plan.projected_structural_call_returns);
    }
    SelectedInstructionPlanIdentity::from_canonical_bytes(&bytes)
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    bytes.extend_from_slice(&instruction.id.0.to_le_bytes());
    bytes.push(match instruction.kind {
        SelectedInstructionKind::LoadPacked { .. } => 46,
        SelectedInstructionKind::StorePacked { .. } => 47,
        SelectedInstructionKind::CallAggregate { .. } => 35,
        SelectedInstructionKind::ReturnAggregate { .. } => 36,
        SelectedInstructionKind::HostedExitProcessI32 => 31,
        SelectedInstructionKind::HostedReadByte { .. } => 32,
        SelectedInstructionKind::HostedWriteByteI32 { .. } => 23,
        SelectedInstructionKind::Store { .. } => 24,
        SelectedInstructionKind::AddressOffset { .. } => 25,
        SelectedInstructionKind::Load64 { .. } => 16,
        SelectedInstructionKind::Load8 { .. } => 33,
        SelectedInstructionKind::Load16 { .. } => 34,
        SelectedInstructionKind::Load32 { .. } => 30,
        SelectedInstructionKind::Load8Indexed => 21,
        SelectedInstructionKind::ByteViewAddress => 22,
        SelectedInstructionKind::Store64 { .. } => 17,
        SelectedInstructionKind::FrameAddress { .. } => 18,
        SelectedInstructionKind::CallUnit { .. } => 19,

        SelectedInstructionKind::Jump => 14,
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::ConditionalBranchNonZero => 2,
        SelectedInstructionKind::ReturnScalar => 3,
        SelectedInstructionKind::CopyI64 => 4,
        SelectedInstructionKind::Float32ToBits => 26,
        SelectedInstructionKind::Float64ToBits => 27,
        SelectedInstructionKind::BitsToFloat32 => 28,
        SelectedInstructionKind::BitsToFloat64 => 29,
        SelectedInstructionKind::ZeroExtendU8 => 15,
        SelectedInstructionKind::ZeroExtendU32 => 20,
        SelectedInstructionKind::ZeroExtendU16 => 37,
        SelectedInstructionKind::SignExtendI8 => 38,
        SelectedInstructionKind::SignExtendI16 => 39,
        SelectedInstructionKind::SignExtendI32 => 40,
        SelectedInstructionKind::MaterializeBooleanEqual => 41,
        SelectedInstructionKind::MaterializeBooleanU64LessThan => 42,
        SelectedInstructionKind::MaterializeBooleanI64LessThan => 43,
        SelectedInstructionKind::MaterializeBooleanU64LessOrEqual => 44,
        SelectedInstructionKind::MaterializeBooleanI64LessOrEqual => 45,
        SelectedInstructionKind::ExactAddI64 { .. } => 5,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 6,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 7,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
        SelectedInstructionKind::CompareI64 => 10,
        SelectedInstructionKind::ConditionalBranchU64LessThan => 11,
        SelectedInstructionKind::CallScalar { .. } => 12,
        SelectedInstructionKind::ConditionalBranchI64LessThan => 13,
    });
    match instruction.kind {
        SelectedInstructionKind::LoadPacked { byte_offset, width }
        | SelectedInstructionKind::StorePacked { byte_offset, width } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.push(width.byte_size());
        }
        SelectedInstructionKind::ReturnAggregate { fragment_count } => bytes.push(fragment_count),
        SelectedInstructionKind::CallAggregate { callee } => {
            bytes.extend_from_slice(&callee.get().to_le_bytes())
        }
        SelectedInstructionKind::HostedWriteByteI32 { slot }
        | SelectedInstructionKind::HostedReadByte { slot } => {
            contracts::frame_slot(
                bytes,
                selected_instructions::FrameStorageSlotId::Local(slot),
            );
        }
        SelectedInstructionKind::Store {
            byte_offset,
            byte_size,
        } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.push(byte_size);
        }
        SelectedInstructionKind::AddressOffset { byte_offset } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::Load64 { byte_offset }
        | SelectedInstructionKind::Load8 { byte_offset }
        | SelectedInstructionKind::Load16 { byte_offset }
        | SelectedInstructionKind::Load32 { byte_offset } => {
            bytes.extend_from_slice(&byte_offset.to_le_bytes())
        }
        SelectedInstructionKind::Store64 { slot, byte_offset }
        | SelectedInstructionKind::FrameAddress { slot, byte_offset } => {
            contracts::frame_slot(bytes, slot);
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
        }
        SelectedInstructionKind::MaterializeI64 { value } => match value {
            semantic_vocabulary::IntegerValue::Signed(value) => {
                bytes.push(0);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            semantic_vocabulary::IntegerValue::Unsigned(value) => {
                bytes.push(1);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        },
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactSubtractI64 {
            obligation,
            accepted_fact,
        } => {
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::ExactAddI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate,
            obligation,
            accepted_fact,
        } => {
            match immediate {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        SelectedInstructionKind::CompareI64Zero
        | SelectedInstructionKind::CompareI64
        | SelectedInstructionKind::CopyI64
        | SelectedInstructionKind::Float32ToBits
        | SelectedInstructionKind::Float64ToBits
        | SelectedInstructionKind::BitsToFloat32
        | SelectedInstructionKind::BitsToFloat64
        | SelectedInstructionKind::ZeroExtendU8
        | SelectedInstructionKind::ZeroExtendU32
        | SelectedInstructionKind::ZeroExtendU16
        | SelectedInstructionKind::SignExtendI8
        | SelectedInstructionKind::SignExtendI16
        | SelectedInstructionKind::SignExtendI32
        | SelectedInstructionKind::MaterializeBooleanEqual
        | SelectedInstructionKind::MaterializeBooleanU64LessThan
        | SelectedInstructionKind::MaterializeBooleanI64LessThan
        | SelectedInstructionKind::MaterializeBooleanU64LessOrEqual
        | SelectedInstructionKind::MaterializeBooleanI64LessOrEqual
        | SelectedInstructionKind::Load8Indexed
        | SelectedInstructionKind::ByteViewAddress
        | SelectedInstructionKind::ConditionalBranchNonZero
        | SelectedInstructionKind::ConditionalBranchU64LessThan
        | SelectedInstructionKind::ConditionalBranchI64LessThan
        | SelectedInstructionKind::ReturnScalar
        | SelectedInstructionKind::HostedExitProcessI32
        | SelectedInstructionKind::ReturnUnit
        | SelectedInstructionKind::Jump => {}
        SelectedInstructionKind::CallScalar { callee }
        | SelectedInstructionKind::CallUnit { callee } => {
            bytes.extend_from_slice(&callee.get().to_le_bytes());
        }
    }
    encode_constraint_key(bytes, instruction.constraint);
    encode_len(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        bytes.extend_from_slice(&operand.operand.to_le_bytes());
        bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
        bytes.push(match operand.access {
            RegisterOperandAccess::Use => 0,
            RegisterOperandAccess::Def => 1,
            RegisterOperandAccess::UseDef => 2,
        });
        bytes.extend_from_slice(&operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_ids(
        bytes,
        instruction
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .values
            .iter()
            .map(|value| value.get()),
    );
    encode_ids(
        bytes,
        instruction.provenance.edges.iter().map(|edge| edge.get()),
    );
    encode_ids(
        bytes,
        instruction
            .provenance
            .obligations
            .iter()
            .map(|obligation| obligation.get()),
    );
    encode_fuel(bytes, &instruction.provenance.fuel);
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.push(match successor.role {
        selected_instructions::SelectedSuccessorRole::Semantic => 0,
        selected_instructions::SelectedSuccessorRole::EdgeTransferContinuation => 1,
        selected_instructions::SelectedSuccessorRole::CaseDispatchContinuation => 2,
    });
    bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.block.0.to_le_bytes());
    bytes.extend_from_slice(&successor.source_target.get().to_le_bytes());
    encode_len(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        bytes.extend_from_slice(&binding.semantic.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.semantic.argument.get().to_le_bytes());
        encode_scalar_type(bytes, binding.semantic.scalar_type);
        match binding.transport {
            selected_instructions::SelectedValueTransport::Unused => bytes.push(0),
            selected_instructions::SelectedValueTransport::Registers {
                argument,
                parameter,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&argument.0.to_le_bytes());
                bytes.extend_from_slice(&parameter.0.to_le_bytes());
            }
        }
    }
    match &successor.structural_case {
        None => bytes.push(0),
        Some(case) => {
            bytes.push(1);
            case.encode_identity(bytes);
        }
    }
    encode_len(bytes, successor.structural_bindings.len());
    for binding in &successor.structural_bindings {
        bytes.extend_from_slice(&binding.semantic.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.semantic.argument.place.get().to_le_bytes());
        encode_len(bytes, binding.semantic.argument.path.len());
        for segment in &binding.semantic.argument.path {
            match segment {
                terminal_psi::StructuralPathSegment::Field(field) => {
                    bytes.push(0);
                    encode_len(bytes, field.len());
                    bytes.extend_from_slice(field.as_bytes());
                }
                terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&index.to_le_bytes());
                }
            }
        }
        bytes.push(match binding.semantic.argument.access {
            terminal_psi::StructuralAccess::Owned => 0,
            terminal_psi::StructuralAccess::SharedBorrow => 1,
            terminal_psi::StructuralAccess::MutableBorrow => 2,
            terminal_psi::StructuralAccess::WriteOnlyBorrow => 3,
        });
        match binding.transport {
            selected_instructions::SelectedStructuralTransport::Unused => bytes.push(0),
            selected_instructions::SelectedStructuralTransport::WholeValue {
                argument,
                destination,
                byte_size,
                alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&argument.0.to_le_bytes());
                destination.encode_identity(bytes);
                bytes.extend_from_slice(&byte_size.to_le_bytes());
                bytes.extend_from_slice(&alignment.to_le_bytes());
            }
            selected_instructions::SelectedStructuralTransport::Descriptor {
                argument,
                destination,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&argument.0.to_le_bytes());
                destination.encode_identity(bytes);
            }
        }
    }
    encode_fuel(bytes, &successor.fuel);
}

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    bytes.extend_from_slice(&(fuel.len() as u64).to_le_bytes());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(operation) => {
                bytes.push(0);
                bytes.extend_from_slice(&operation.get().to_le_bytes());
            }
            PsiProvenance::Edge(edge) => {
                bytes.push(1);
                bytes.extend_from_slice(&edge.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: target::NativeTarget) {
    bytes.push(match target.architecture {
        target::Architecture::X86_64 => 0,
        target::Architecture::Aarch64 => 1,
    });
    bytes.push(match target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    });
    bytes.extend_from_slice(&(target.pointer_size as u64).to_le_bytes());
    bytes.extend_from_slice(&(target.pointer_alignment as u64).to_le_bytes());
}

fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => 0,
                semantic_vocabulary::IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(2);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

fn encode_option_id(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u64).to_le_bytes());
}

fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    encode_len(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}

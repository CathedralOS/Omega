use omega_abstract_operations::ValueBinding;
use omega_optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use omega_register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintKey, RegisterOperandAccess,
    RegisterUnitId, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedOperand, SelectedSuccessor, SelectedTerminator,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerCarrier, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, ScalarType, StructuralTypeId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FixedViewCopy,
    FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyIdentity, FixedViewCopyPlan,
    FixedViewCopyPolicy, LiveRangeIdentity, LiveRangePoint, LivenessPosition,
    VirtualFixedConstraintSite, fixed_view_copy_identity,
};

const MAGIC: &[u8; 8] = b"OMGFCV\0\0";
const VERSION: u32 = 4;

impl FixedViewCopyPlan {
    /// Canonical self-authenticating artifact. Decoding returns plain content;
    /// independent fixed-view-copy validation is still required for custody.
    pub fn encode(&self) -> Vec<u8> {
        let identity = fixed_view_copy_identity(self);
        let mut encoded = Vec::new();
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encode_content(&mut encoded, self);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, FixedViewCopyDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(MAGIC.len())? != MAGIC {
            return Err(FixedViewCopyDecodeError::WrongMagic);
        }
        let version = cursor.u32()?;
        if version != VERSION {
            return Err(FixedViewCopyDecodeError::UnsupportedVersion(version));
        }
        let identity = FixedViewCopyIdentity::from_bytes(cursor.array()?);
        let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let source_ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let source_legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let policy = match cursor.byte()? {
            0 => FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1,
            1 => FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            tag => return Err(FixedViewCopyDecodeError::UnknownPolicy(tag)),
        };
        let budget = omega_optimization_core::OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| FixedViewCopyDecodeError::InvalidBudget)?;
        let usage = omega_optimization_core::OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| FixedViewCopyDecodeError::InvalidUsage)?;
        let copy_count = cursor.length()?;
        let mut copies = Vec::with_capacity(copy_count.min(cursor.remaining()));
        for _ in 0..copy_count {
            copies.push(decode_copy(&mut cursor)?);
        }
        let expected_transformed = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let transformed = decode_selected_plan(&mut cursor)?;
        if cursor.remaining() != 0 {
            return Err(FixedViewCopyDecodeError::TrailingBytes);
        }
        if selected_instruction_plan_identity(&transformed) != expected_transformed {
            return Err(FixedViewCopyDecodeError::TransformedIdentityMismatch);
        }
        let plan = Self {
            source_selected,
            source_ranges,
            source_legality,
            register_environment,
            allocator_availability,
            policy,
            budget,
            usage,
            copies,
            transformed,
        };
        if fixed_view_copy_identity(&plan) != identity {
            return Err(FixedViewCopyDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

fn encode_content(bytes: &mut Vec<u8>, plan: &FixedViewCopyPlan) {
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.source_ranges.bytes());
    bytes.extend_from_slice(&plan.source_legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.push(match plan.policy {
        FixedViewCopyPolicy::LeafLocalBeforeFixedUseV1 => 0,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1 => 1,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(bytes, plan.copies.len());
    for copy in &plan.copies {
        encode_copy(bytes, copy);
    }
    bytes.extend_from_slice(&selected_instruction_plan_identity(&plan.transformed).bytes());
    encode_selected_plan(bytes, &plan.transformed);
}

fn encode_copy(bytes: &mut Vec<u8>, copy: &FixedViewCopy) {
    bytes.extend_from_slice(&copy.function.to_le_bytes());
    bytes.extend_from_slice(&copy.machine.get().to_le_bytes());
    bytes.extend_from_slice(&copy.source_virtual_register.0.to_le_bytes());
    bytes.extend_from_slice(&copy.source_value.get().to_le_bytes());
    encode_definition_site(bytes, copy.source_definition_site);
    bytes.extend_from_slice(&copy.from_view.0.to_le_bytes());
    bytes.extend_from_slice(&copy.to_view.0.to_le_bytes());
    bytes.extend_from_slice(&copy.insertion_block.0.to_le_bytes());
    bytes.extend_from_slice(&copy.before_instruction.0.to_le_bytes());
    length(bytes, copy.destinations.len());
    for destination in &copy.destinations {
        encode_fixed_site(bytes, destination.site);
        bytes.extend_from_slice(&destination.block.0.to_le_bytes());
        bytes.extend_from_slice(&destination.view.0.to_le_bytes());
    }
    bytes.extend_from_slice(&copy.copy_instruction.0.to_le_bytes());
    bytes.extend_from_slice(&copy.result_virtual_register.0.to_le_bytes());
    encode_constraint_key(bytes, copy.copy_constraint);
}

fn decode_copy(cursor: &mut Cursor<'_>) -> Result<FixedViewCopy, FixedViewCopyDecodeError> {
    let function = cursor.u32()?;
    let machine = decode_id(cursor, MachineId::new)?;
    let source_virtual_register = VirtualRegisterId(cursor.u32()?);
    let source_value = decode_id(cursor, ValueId::new)?;
    let source_definition_site = decode_definition_site(cursor)?;
    let from_view = RegisterViewId(cursor.u16()?);
    let to_view = RegisterViewId(cursor.u16()?);
    let insertion_block = SelectedBlockId(cursor.u32()?);
    let before_instruction = SelectedInstructionId(cursor.u32()?);
    let destination_count = cursor.length()?;
    let mut destinations = Vec::with_capacity(destination_count.min(cursor.remaining()));
    for _ in 0..destination_count {
        destinations.push(FixedViewCopyDestination {
            site: decode_fixed_site(cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            view: RegisterViewId(cursor.u16()?),
        });
    }
    Ok(FixedViewCopy {
        function,
        machine,
        source_virtual_register,
        source_value,
        source_definition_site,
        from_view,
        to_view,
        insertion_block,
        before_instruction,
        destinations,
        copy_instruction: SelectedInstructionId(cursor.u32()?),
        result_virtual_register: VirtualRegisterId(cursor.u32()?),
        copy_constraint: decode_constraint_key(cursor)?,
    })
}

fn encode_selected_plan(bytes: &mut Vec<u8>, plan: &SelectedInstructionPlan) {
    bytes.extend_from_slice(plan.psi.program_fingerprint.as_bytes());
    bytes.extend_from_slice(&plan.psi.vocabulary_marker.get().to_le_bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    encode_target(bytes, plan.target);
    bytes.extend_from_slice(&plan.entry.get().to_le_bytes());
    length(bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        encode_option_u64(bytes, function.attachment.map(|value| value.get()));
        encode_ids(
            bytes,
            function
                .provenance
                .operations
                .iter()
                .map(|value| value.get()),
        );
        encode_ids(
            bytes,
            function.provenance.edges.iter().map(|value| value.get()),
        );
        bytes.extend_from_slice(&function.entry_block.0.to_le_bytes());
        length(bytes, function.virtual_registers.len());
        for register in &function.virtual_registers {
            encode_register(bytes, register);
        }
        length(bytes, function.blocks.len());
        for block in &function.blocks {
            encode_block(bytes, block);
        }
    }
}

fn decode_selected_plan(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionPlan, FixedViewCopyDecodeError> {
    let fingerprint = SemanticFingerprint::from_bytes(cursor.array()?);
    let marker_raw = cursor.u16()?;
    let marker = VocabularyMarker::new(marker_raw)
        .ok_or(FixedViewCopyDecodeError::InvalidVocabulary(marker_raw))?;
    let fuel_raw = cursor.u32()?;
    let fuel_schedule = FuelScheduleIdentity::new(fuel_raw)
        .ok_or(FixedViewCopyDecodeError::InvalidFuelSchedule(fuel_raw))?;
    let target = decode_target(cursor)?;
    let entry = decode_id(cursor, MachineId::new)?;
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = decode_id(cursor, MachineId::new)?;
        let attachment = match decode_option_u64(cursor)? {
            None => None,
            Some(raw) => Some(
                StructuralTypeId::new(raw)
                    .ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
            ),
        };
        let operations = decode_ids(cursor, OperationId::new)?;
        let edges = decode_ids(cursor, EdgeId::new)?;
        let entry_block = SelectedBlockId(cursor.u32()?);
        let register_count = cursor.length()?;
        let mut virtual_registers = Vec::with_capacity(register_count.min(cursor.remaining()));
        for _ in 0..register_count {
            virtual_registers.push(decode_register(cursor)?);
        }
        let block_count = cursor.length()?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            blocks.push(decode_block(cursor)?);
        }
        let provenance = omega_target_operations::TerminalPsiProvenance { operations, edges };
        functions.push(SelectedFunction {
            machine,
            attachment,
            provenance,
            entry_block,
            virtual_registers,
            blocks,
        });
    }
    Ok(SelectedInstructionPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: marker,
            program_fingerprint: fingerprint,
        },
        fuel_schedule,
        target,
        entry,
        functions,
        structural_unit_functions: Vec::new(),
    })
}

fn encode_register(bytes: &mut Vec<u8>, register: &VirtualRegister) {
    bytes.extend_from_slice(&register.id.0.to_le_bytes());
    encode_scalar(bytes, register.scalar_type);
    bytes.extend_from_slice(&register.class.0.to_le_bytes());
    match register.origin {
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            length(bytes, parameter_index);
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
        VirtualRegisterOrigin::LegalizationTemporary {
            instruction,
            temporary,
            source_value,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&temporary.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
    }
    encode_definition_site(bytes, register.definition_site);
    encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
}

fn decode_register(cursor: &mut Cursor<'_>) -> Result<VirtualRegister, FixedViewCopyDecodeError> {
    let id = VirtualRegisterId(cursor.u32()?);
    let scalar_type = decode_scalar(cursor)?;
    let class = RegisterClassId(cursor.u16()?);
    let origin = match cursor.byte()? {
        0 => VirtualRegisterOrigin::EntryParameter {
            source_value: decode_id(cursor, ValueId::new)?,
            parameter_index: cursor.length()?,
        },
        1 => VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
        2 => VirtualRegisterOrigin::LegalizationTemporary {
            instruction: SelectedInstructionId(cursor.u32()?),
            temporary: omega_legalized_operations::LegalizedTemporaryId(cursor.u32()?),
            source_value: decode_id(cursor, ValueId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownRegisterOrigin(tag)),
    };
    Ok(VirtualRegister {
        id,
        scalar_type,
        class,
        origin,
        definition_site: decode_definition_site(cursor)?,
        entry_fixed_view: decode_option_u16(cursor)?.map(RegisterViewId),
    })
}

fn encode_block(bytes: &mut Vec<u8>, block: &SelectedBlock) {
    bytes.extend_from_slice(&block.id.0.to_le_bytes());
    bytes.extend_from_slice(&block.source_block.get().to_le_bytes());
    length(bytes, block.instructions.len());
    for instruction in &block.instructions {
        encode_instruction(bytes, instruction);
    }
    match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            instruction,
            when_nonzero,
            when_zero,
        } => {
            bytes.push(0);
            encode_instruction(bytes, instruction);
            encode_successor(bytes, when_nonzero);
            encode_successor(bytes, when_zero);
        }
        SelectedTerminator::Return {
            instruction,
            psi_return_edge,
        } => {
            bytes.push(1);
            encode_instruction(bytes, instruction);
            bytes.extend_from_slice(&psi_return_edge.get().to_le_bytes());
        }
    }
}

fn decode_block(cursor: &mut Cursor<'_>) -> Result<SelectedBlock, FixedViewCopyDecodeError> {
    let id = SelectedBlockId(cursor.u32()?);
    let source_block = decode_id(cursor, BlockId::new)?;
    let instruction_count = cursor.length()?;
    let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
    for _ in 0..instruction_count {
        instructions.push(decode_instruction(cursor)?);
    }
    let terminator = match cursor.byte()? {
        0 => SelectedTerminator::ConditionalBranch {
            instruction: decode_instruction(cursor)?,
            when_nonzero: decode_successor(cursor)?,
            when_zero: decode_successor(cursor)?,
        },
        1 => SelectedTerminator::Return {
            instruction: decode_instruction(cursor)?,
            psi_return_edge: decode_id(cursor, EdgeId::new)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownTerminator(tag)),
    };
    Ok(SelectedBlock {
        id,
        source_block,
        instructions,
        terminator,
    })
}

fn encode_instruction(bytes: &mut Vec<u8>, instruction: &SelectedInstruction) {
    bytes.extend_from_slice(&instruction.id.0.to_le_bytes());
    encode_kind(bytes, instruction.kind);
    encode_constraint_key(bytes, instruction.constraint);
    length(bytes, instruction.operands.len());
    for operand in &instruction.operands {
        bytes.extend_from_slice(&operand.operand.to_le_bytes());
        bytes.extend_from_slice(&operand.virtual_register.0.to_le_bytes());
        bytes.push(access_tag(operand.access));
        bytes.extend_from_slice(&operand.class.0.to_le_bytes());
        encode_option_u16(bytes, operand.fixed_view.map(|view| view.0));
        encode_option_u16(bytes, operand.tied_to);
        bytes.push(u8::from(operand.early_clobber));
    }
    encode_u16s(bytes, instruction.implicit_uses.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.implicit_defs.iter().map(|unit| unit.0));
    encode_u16s(bytes, instruction.clobbers.iter().map(|unit| unit.0));
    encode_provenance(bytes, &instruction.provenance);
}

fn decode_instruction(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstruction, FixedViewCopyDecodeError> {
    let id = SelectedInstructionId(cursor.u32()?);
    let kind = decode_kind(cursor)?;
    let constraint = decode_constraint_key(cursor)?;
    let operand_count = cursor.length()?;
    let mut operands = Vec::with_capacity(operand_count.min(cursor.remaining()));
    for _ in 0..operand_count {
        operands.push(SelectedOperand {
            operand: cursor.u16()?,
            virtual_register: VirtualRegisterId(cursor.u32()?),
            access: decode_access(cursor)?,
            class: RegisterClassId(cursor.u16()?),
            fixed_view: decode_option_u16(cursor)?.map(RegisterViewId),
            tied_to: decode_option_u16(cursor)?,
            early_clobber: decode_bool(cursor)?,
        });
    }
    Ok(SelectedInstruction {
        id,
        kind,
        constraint,
        operands,
        implicit_uses: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        implicit_defs: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        clobbers: decode_u16s(cursor)?
            .into_iter()
            .map(RegisterUnitId)
            .collect(),
        provenance: decode_provenance(cursor)?,
    })
}

fn encode_kind(bytes: &mut Vec<u8>, kind: SelectedInstructionKind) {
    let tag = match kind {
        SelectedInstructionKind::CompareI64Zero => 0,
        SelectedInstructionKind::MaterializeI64 { .. } => 1,
        SelectedInstructionKind::ConditionalBranchNonZero => 2,
        SelectedInstructionKind::ReturnI64 => 3,
        SelectedInstructionKind::CopyI64 => 4,
        SelectedInstructionKind::ExactAddI64 { .. } => 5,
        SelectedInstructionKind::ExactAddI64Immediate { .. } => 6,
        SelectedInstructionKind::ExactSubtractI64 { .. } => 7,
        SelectedInstructionKind::ExactSubtractI64Immediate { .. } => 8,
        SelectedInstructionKind::ReturnUnit => 9,
    };
    bytes.push(tag);
    match kind {
        SelectedInstructionKind::MaterializeI64 { value } => encode_integer(bytes, value),
        SelectedInstructionKind::ExactAddI64 {
            obligation,
            accepted_fact,
        }
        | SelectedInstructionKind::ExactSubtractI64 {
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
            encode_integer(bytes, immediate);
            bytes.extend_from_slice(&obligation.get().to_le_bytes());
            bytes.extend_from_slice(&accepted_fact.bytes());
        }
        _ => {}
    }
}

fn decode_kind(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionKind, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => SelectedInstructionKind::CompareI64Zero,
        1 => SelectedInstructionKind::MaterializeI64 {
            value: decode_integer(cursor)?,
        },
        2 => SelectedInstructionKind::ConditionalBranchNonZero,
        3 => SelectedInstructionKind::ReturnI64,
        4 => SelectedInstructionKind::CopyI64,
        5 => SelectedInstructionKind::ExactAddI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        6 => SelectedInstructionKind::ExactAddI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        7 => SelectedInstructionKind::ExactSubtractI64 {
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        8 => SelectedInstructionKind::ExactSubtractI64Immediate {
            immediate: decode_integer(cursor)?,
            obligation: decode_id(cursor, ObligationId::new)?,
            accepted_fact: omega_optimization_core::AcceptedObligationFactIdentity::from_bytes(
                cursor.array()?,
            ),
        },
        9 => SelectedInstructionKind::ReturnUnit,
        tag => {
            return Err(FixedViewCopyDecodeError::UnknownInstructionKind(tag));
        }
    })
}

fn encode_successor(bytes: &mut Vec<u8>, successor: &SelectedSuccessor) {
    bytes.extend_from_slice(&successor.psi_edge.get().to_le_bytes());
    bytes.extend_from_slice(&successor.block.0.to_le_bytes());
    bytes.extend_from_slice(&successor.source_target.get().to_le_bytes());
    length(bytes, successor.bindings.len());
    for binding in &successor.bindings {
        bytes.extend_from_slice(&binding.parameter.get().to_le_bytes());
        bytes.extend_from_slice(&binding.argument.get().to_le_bytes());
        encode_scalar(bytes, binding.scalar_type);
    }
    encode_fuel(bytes, &successor.fuel);
}

fn decode_successor(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedSuccessor, FixedViewCopyDecodeError> {
    let psi_edge = decode_id(cursor, EdgeId::new)?;
    let block = SelectedBlockId(cursor.u32()?);
    let source_target = decode_id(cursor, BlockId::new)?;
    let count = cursor.length()?;
    let mut bindings = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        bindings.push(ValueBinding {
            parameter: decode_id(cursor, ValueId::new)?,
            argument: decode_id(cursor, ValueId::new)?,
            scalar_type: decode_scalar(cursor)?,
        });
    }
    Ok(SelectedSuccessor {
        psi_edge,
        block,
        source_target,
        bindings,
        fuel: decode_fuel(cursor)?,
    })
}

fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    encode_ids(bytes, provenance.operations.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.values.iter().map(|value| value.get()));
    encode_ids(bytes, provenance.edges.iter().map(|value| value.get()));
    encode_ids(
        bytes,
        provenance.obligations.iter().map(|value| value.get()),
    );
    encode_fuel(bytes, &provenance.fuel);
}

fn decode_provenance(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedInstructionProvenance, FixedViewCopyDecodeError> {
    Ok(SelectedInstructionProvenance {
        operations: decode_ids(cursor, OperationId::new)?,
        values: decode_ids(cursor, ValueId::new)?,
        edges: decode_ids(cursor, EdgeId::new)?,
        obligations: decode_ids(cursor, ObligationId::new)?,
        fuel: decode_fuel(cursor)?,
    })
}

fn encode_fuel(bytes: &mut Vec<u8>, fuel: &[FuelSettlement]) {
    length(bytes, fuel.len());
    for settlement in fuel {
        match settlement.site {
            PsiProvenance::Operation(id) => {
                bytes.push(0);
                bytes.extend_from_slice(&id.get().to_le_bytes());
            }
            PsiProvenance::Edge(id) => {
                bytes.push(1);
                bytes.extend_from_slice(&id.get().to_le_bytes());
            }
        }
        bytes.extend_from_slice(&settlement.units.to_le_bytes());
    }
}
fn decode_fuel(cursor: &mut Cursor<'_>) -> Result<Vec<FuelSettlement>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut fuel = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let tag = cursor.byte()?;
        let raw = cursor.u64()?;
        let site = match tag {
            0 => PsiProvenance::Operation(
                OperationId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
            ),
            1 => PsiProvenance::Edge(
                EdgeId::new(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))?,
            ),
            tag => return Err(FixedViewCopyDecodeError::UnknownFuelSite(tag)),
        };
        fuel.push(FuelSettlement {
            site,
            units: cursor.u64()?,
        });
    }
    Ok(fuel)
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}
fn decode_definition_site(
    cursor: &mut Cursor<'_>,
) -> Result<ValueDefinitionSite, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => ValueDefinitionSite::FunctionParameter(cursor.u32()?),
        1 => ValueDefinitionSite::BlockParameter {
            block: decode_id(cursor, BlockId::new)?,
            position: cursor.u32()?,
        },
        2 => ValueDefinitionSite::Node {
            block: decode_id(cursor, BlockId::new)?,
            node: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownDefinitionSite(tag)),
    })
}
fn encode_fixed_site(bytes: &mut Vec<u8>, site: VirtualFixedConstraintSite) {
    match site {
        VirtualFixedConstraintSite::Entry => bytes.push(0),
        VirtualFixedConstraintSite::Operand {
            position,
            point,
            instruction,
            operand,
            access,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&position.0.to_le_bytes());
            bytes.extend_from_slice(&point.0.to_le_bytes());
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&operand.to_le_bytes());
            bytes.push(access_tag(access));
        }
    }
}
fn decode_fixed_site(
    cursor: &mut Cursor<'_>,
) -> Result<VirtualFixedConstraintSite, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        0 => VirtualFixedConstraintSite::Entry,
        1 => VirtualFixedConstraintSite::Operand {
            position: LivenessPosition(cursor.u32()?),
            point: LiveRangePoint(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            operand: cursor.u16()?,
            access: decode_access(cursor)?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownFixedSite(tag)),
    })
}

fn encode_target(bytes: &mut Vec<u8>, target: omega_target::NativeTarget) {
    bytes.push(match target.architecture {
        omega_target::Architecture::X86_64 => 0,
        omega_target::Architecture::Aarch64 => 1,
    });
    bytes.push(match target.object_format {
        omega_target::ObjectFormat::Elf => 0,
        omega_target::ObjectFormat::MachO => 1,
        omega_target::ObjectFormat::Coff => 2,
    });
    length(bytes, target.pointer_size);
    length(bytes, target.pointer_alignment);
}
fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<omega_target::NativeTarget, FixedViewCopyDecodeError> {
    let architecture = match cursor.byte()? {
        0 => omega_target::Architecture::X86_64,
        1 => omega_target::Architecture::Aarch64,
        tag => return Err(FixedViewCopyDecodeError::UnknownArchitecture(tag)),
    };
    let object_format = match cursor.byte()? {
        0 => omega_target::ObjectFormat::Elf,
        1 => omega_target::ObjectFormat::MachO,
        2 => omega_target::ObjectFormat::Coff,
        tag => return Err(FixedViewCopyDecodeError::UnknownObjectFormat(tag)),
    };
    Ok(omega_target::NativeTarget {
        architecture,
        object_format,
        pointer_size: cursor.length()?,
        pointer_alignment: cursor.length()?,
    })
}
fn encode_scalar(bytes: &mut Vec<u8>, scalar: ScalarType) {
    match scalar {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) => {
            bytes.push(1);
            bytes.push(match integer.carrier() {
                IntegerCarrier::Fixed => 0,
                IntegerCarrier::Address => 1,
            });
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
    }
}
fn decode_scalar(cursor: &mut Cursor<'_>) -> Result<ScalarType, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(ScalarType::Boolean),
        1 => {
            let carrier = cursor.byte()?;
            let sign = match cursor.byte()? {
                0 => IntegerSign::Signed,
                1 => IntegerSign::Unsigned,
                tag => return Err(FixedViewCopyDecodeError::UnknownIntegerSign(tag)),
            };
            let bits = cursor.u16()?;
            let integer = match carrier {
                0 => IntegerType::new(sign, bits),
                1 if sign == IntegerSign::Unsigned => IntegerType::address(bits),
                tag => return Err(FixedViewCopyDecodeError::UnknownIntegerCarrier(tag)),
            }
            .map_err(|_| FixedViewCopyDecodeError::InvalidIntegerType)?;
            Ok(ScalarType::Integer(integer))
        }
        tag => Err(FixedViewCopyDecodeError::UnknownScalarType(tag)),
    }
}
fn encode_integer(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}
fn decode_integer(cursor: &mut Cursor<'_>) -> Result<IntegerValue, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(i128::from_le_bytes(cursor.array()?))),
        1 => Ok(IntegerValue::Unsigned(u128::from_le_bytes(cursor.array()?))),
        tag => Err(FixedViewCopyDecodeError::UnknownIntegerValue(tag)),
    }
}
fn encode_constraint_key(bytes: &mut Vec<u8>, key: RegisterConstraintKey) {
    bytes.push(match key.family {
        RegisterConstraintFamily::Call => 0,
        RegisterConstraintFamily::Return => 1,
        RegisterConstraintFamily::SystemCall => 2,
        RegisterConstraintFamily::InlineAssembly => 3,
        RegisterConstraintFamily::Instruction => 4,
    });
    bytes.extend_from_slice(&key.variant.to_le_bytes());
}
fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, FixedViewCopyDecodeError> {
    let family = match cursor.byte()? {
        0 => RegisterConstraintFamily::Call,
        1 => RegisterConstraintFamily::Return,
        2 => RegisterConstraintFamily::SystemCall,
        3 => RegisterConstraintFamily::InlineAssembly,
        4 => RegisterConstraintFamily::Instruction,
        tag => {
            return Err(FixedViewCopyDecodeError::UnknownConstraintFamily(tag));
        }
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}
fn access_tag(access: RegisterOperandAccess) -> u8 {
    match access {
        RegisterOperandAccess::Use => 0,
        RegisterOperandAccess::Def => 1,
        RegisterOperandAccess::UseDef => 2,
    }
}
fn decode_access(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterOperandAccess, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(RegisterOperandAccess::Use),
        1 => Ok(RegisterOperandAccess::Def),
        2 => Ok(RegisterOperandAccess::UseDef),
        tag => Err(FixedViewCopyDecodeError::UnknownOperandAccess(tag)),
    }
}
fn decode_bool(cursor: &mut Cursor<'_>) -> Result<bool, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(FixedViewCopyDecodeError::UnknownBoolean(tag)),
    }
}
fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}
fn decode_option_u64(cursor: &mut Cursor<'_>) -> Result<Option<u64>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u64()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}
fn encode_option_u16(bytes: &mut Vec<u8>, value: Option<u16>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}
fn decode_option_u16(cursor: &mut Cursor<'_>) -> Result<Option<u16>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u16()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}
fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    length(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
fn decode_ids<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<Vec<T>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(decode_id(cursor, constructor)?);
    }
    Ok(values)
}
fn encode_u16s(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u16>) {
    length(bytes, values.len());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
fn decode_u16s(cursor: &mut Cursor<'_>) -> Result<Vec<u16>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut values = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        values.push(cursor.u16()?);
    }
    Ok(values)
}
fn decode_id<T>(
    cursor: &mut Cursor<'_>,
    constructor: fn(u64) -> Option<T>,
) -> Result<T, FixedViewCopyDecodeError> {
    let raw = cursor.u64()?;
    constructor(raw).ok_or(FixedViewCopyDecodeError::InvalidSemanticId(raw))
}
fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("fixed-view-copy artifact length fits u64")
            .to_le_bytes(),
    );
}

struct Cursor<'a> {
    encoded: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    const fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], FixedViewCopyDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(FixedViewCopyDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(FixedViewCopyDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], FixedViewCopyDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FixedViewCopyDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, FixedViewCopyDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, FixedViewCopyDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, FixedViewCopyDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, FixedViewCopyDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn length(&mut self) -> Result<usize, FixedViewCopyDecodeError> {
        usize::try_from(self.u64()?).map_err(|_| FixedViewCopyDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;

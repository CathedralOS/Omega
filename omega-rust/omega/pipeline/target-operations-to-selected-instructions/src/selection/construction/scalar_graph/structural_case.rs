//! Observe the source sum tag; payload observations belong to selected edges.
use super::*;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarTerminator, LegalizedStructuralCaseSuccessor,
};
use selected_instructions::{
    FrameStorageSlotId, LocalStorageSlotId, SelectedCasePayloadBinding,
    SelectedCasePayloadTransport, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole, SelectedStructuralCaseEdge,
};
use semantic_vocabulary::{IntegerType, PlaceId};

fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::SourceCustodyMismatch
}

pub(super) fn build(
    source: &LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    order: &[usize],
    builder: &mut Builder<'_>,
) -> Result<SelectedTerminator, SelectedInstructionError> {
    let LegalizedScalarTerminator::StructuralCase {
        defining_operation,
        result,
        layout,
        cases,
        ..
    } = &block.terminator
    else {
        return Err(invalid());
    };
    if cases.len() < 2
        || cases.iter().enumerate().any(|(ordinal, case)| usize::try_from(case.case_tag) != Ok(ordinal))
        || layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return Err(invalid());
    }
    let slot = LocalStorageSlotId::Structural {
        operation: *defining_operation,
        place: result.place,
    };
    if builder
        .transport
        .local_slots
        .iter()
        .filter(|candidate| {
            candidate.id == slot
                && candidate.byte_size == u32::from(layout.shape.byte_size)
                && candidate.alignment == layout.shape.alignment
        })
        .count()
        != 1
    {
        return Err(invalid());
    }
    let pointer = register(builder, result.place, 0, 64, false)?;
    memory(
        builder,
        block.id,
        result.place,
        0,
        u32::from(layout.shape.byte_size),
        SelectedMemoryAccessRole::AddressLocal { slot },
    )?;
    builder.emit(
        SelectedInstructionKind::FrameAddress {
            slot: FrameStorageSlotId::Local(slot),
            byte_offset: 0,
        },
        builder.constraints.keys.frame_address.ok_or_else(invalid)?,
        &[pointer],
        Default::default(),
    )?;
    let tag = register(builder, result.place, 0, 32, true)?;
    memory(
        builder,
        block.id,
        result.place,
        0,
        4,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load32 { byte_offset: 0 },
        builder.constraints.keys.load32.ok_or_else(invalid)?,
        &[pointer, tag],
        Default::default(),
    )?;
    let extra_base = source.blocks.len() + builder.case_blocks.len();
    let mut first = None;
    for ordinal in 0..cases.len() - 1 {
        let start = builder.instructions.len();
        if ordinal == 0 {
            builder.emit(SelectedInstructionKind::CompareI64Zero,
                builder.constraints.keys.compare_i64_zero, &[tag], Default::default())?;
        } else {
            let expected = register(builder, result.place, 0, 64, false)?;
            builder.emit(SelectedInstructionKind::MaterializeI64 { value: semantic_vocabulary::IntegerValue::Unsigned(ordinal as u128) },
                builder.constraints.keys.materialize_i64, &[expected], Default::default())?;
            builder.emit(SelectedInstructionKind::CompareI64,
                builder.constraints.keys.compare_i64, &[tag, expected], Default::default())?;
        }
        let when_zero = successor(source, order, builder, slot, &cases[ordinal])?;
        let when_nonzero = if ordinal + 2 == cases.len() {
            successor(source, order, builder, slot, &cases[ordinal + 1])?
        } else {
            SelectedSuccessor {
                role: selected_instructions::SelectedSuccessorRole::CaseDispatchContinuation,
                psi_edge: cases[ordinal + 1].edge,
                source_target: block.id,
                block: SelectedBlockId((extra_base + ordinal).try_into().map_err(|_| invalid())?),
                bindings: Vec::new(), structural_bindings: Vec::new(), fuel: Vec::new(), structural_case: None,
            }
        };
        builder.emit(SelectedInstructionKind::ConditionalBranchNonZero,
            builder.constraints.keys.conditional_branch, &[], Default::default())?;
        let terminator = SelectedTerminator::ConditionalBranch {
            instruction: builder.instructions.last().cloned().ok_or_else(invalid)?,
            when_zero, when_nonzero,
        };
        if ordinal == 0 {
            builder.case_body_end = Some(builder.instructions.len() - 1);
            first = Some(terminator);
        } else {
            builder.case_blocks.push(SelectedBlock {
                id: SelectedBlockId((extra_base + ordinal - 1).try_into().map_err(|_| invalid())?),
                origin: selected_instructions::SelectedBlockOrigin::CaseDispatch {
                    source: block.id, case_ordinal: ordinal.try_into().map_err(|_| invalid())?,
                },
                instructions: builder.instructions[start..builder.instructions.len() - 1].to_vec(),
                terminator,
            });
        }
    }
    first.ok_or_else(invalid)
}

pub(super) fn register(
    builder: &mut Builder<'_>,
    place: PlaceId,
    byte_offset: u32,
    bits: u16,
    observation: bool,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let id = VirtualRegisterId(builder.registers.len().try_into().map_err(|_| invalid())?);
    let instruction = SelectedInstructionId(
        builder
            .instructions
            .len()
            .try_into()
            .map_err(|_| invalid())?,
    );
    builder.registers.push(VirtualRegister {
        id,
        scalar_type: ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| invalid())?,
        ),
        class: builder.class,
        origin: if observation {
            VirtualRegisterOrigin::StructuralObservation {
                instruction,
                place,
                byte_offset,
            }
        } else {
            VirtualRegisterOrigin::AbiTransport {
                instruction,
                place,
                byte_offset,
            }
        },
        definition_site: None,
        entry_fixed_view: None,
    });
    Ok(id)
}

pub(super) fn memory(
    builder: &mut Builder<'_>,
    block: semantic_vocabulary::BlockId,
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    role: SelectedMemoryAccessRole,
) -> Result<(), SelectedInstructionError> {
    builder.transport.memory.push(SelectedMemoryAccess {
        instruction: SelectedInstructionId(
            builder
                .instructions
                .len()
                .try_into()
                .map_err(|_| invalid())?,
        ),
        origin: SelectedMemoryAccessOrigin::Block(block),
        place,
        byte_offset,
        byte_count,
        role,
    });
    Ok(())
}

fn successor(
    source: &LegalizedScalarFunction,
    order: &[usize],
    builder: &Builder<'_>,
    slot: LocalStorageSlotId,
    case: &LegalizedStructuralCaseSuccessor,
) -> Result<SelectedSuccessor, SelectedInstructionError> {
    let block = order
        .iter()
        .position(|position| source.blocks[*position].id == case.target)
        .ok_or_else(invalid)?;
    let mut payloads = Vec::with_capacity(case.payloads.len());
    for payload in &case.payloads {
        let transport = if builder.required_values.contains(&payload.parameter.value) {
            let (_, parameter, site, scalar_type) = builder
                .resolve(payload.parameter.value)
                .ok_or_else(invalid)?;
            if site != payload.parameter.definition_site
                || scalar_type != payload.parameter.scalar_type
            {
                return Err(invalid());
            }
            SelectedCasePayloadTransport::Unmaterialized { parameter }
        } else {
            SelectedCasePayloadTransport::Unused
        };
        payloads.push(SelectedCasePayloadBinding {
            semantic: *payload,
            transport,
        });
    }
    Ok(SelectedSuccessor {
        role: selected_instructions::SelectedSuccessorRole::Semantic,
        psi_edge: case.edge,
        block: SelectedBlockId(block.try_into().map_err(|_| invalid())?),
        source_target: case.target,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        fuel: case.fuel.clone(),
        structural_case: Some(SelectedStructuralCaseEdge {
            slot,
            case: case.case,
            case_tag: case.case_tag,
            payloads,
            trivial_affine_discards: case.trivial_affine_discards.clone(),
        }),
    })
}

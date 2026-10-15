//! Test the source sum tag once per case but the last; payload observations
//! belong to selected edges.
use super::{
    Builder, IntegerSign, LegalizedScalarFunction, ScalarType, SelectedBlock, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionKind, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};
use crate::SelectedInstructionError;
use legalized_operations::{
    LegalizedScalarBlock, LegalizedScalarTerminator, LegalizedStructuralCaseSuccessor,
};
use selected_instructions::{
    LocalStorageSlotId, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedStructuralCaseEdge,
};
use selected_instructions::{SelectedSuccessor, SelectedTerminator};
use semantic_vocabulary::{IntegerType, PlaceId};

#[track_caller]
fn invalid() -> SelectedInstructionError {
    SelectedInstructionError::custody()
}

pub(super) fn build(
    source: &LegalizedScalarFunction,
    block: &LegalizedScalarBlock,
    order: &[usize],
    builder: &mut Builder<'_>,
) -> Result<SelectedTerminator, SelectedInstructionError> {
    let LegalizedScalarTerminator::StructuralCase {
        source: subject,
        path,
        root_shape,
        byte_offset,
        layout,
        cases,
        ..
    } = &block.terminator
    else {
        return Err(invalid());
    };
    // A sum of n cases needs n - 1 tag tests: each test selects its own case
    // and the last case is the final test's other leg. The sole case of a
    // one-case sum is that leg with no test before it.
    let Some((last, tested)) = cases.split_last() else {
        return Err(invalid());
    };
    if cases
        .iter()
        .enumerate()
        .any(|(ordinal, case)| usize::try_from(case.case_tag) != Ok(ordinal))
        || layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return Err(invalid());
    }
    let mut dispatch_source = match subject {
        legalized_operations::LegalizedStructuralCaseSource::OperationResult {
            operation,
            result,
        } => selected_instructions::SelectedCaseDispatchSource::Local {
            slot: LocalStorageSlotId::Structural {
                operation: *operation,
                place: result.place,
            },
        },
        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
            block,
            declaration,
        } => selected_instructions::SelectedCaseDispatchSource::Local {
            slot: LocalStorageSlotId::StructuralBlockParameter {
                block: *block,
                place: declaration.place,
            },
        },
        // Entry uses the same private-home demand for every owned ABI arrival.
        // Incoming stack coordinates and indirect pointers do not establish
        // the semantic value home's alignment or storage identity.
        legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration }
            if crate::selection::aggregate_result_input::owned_parameter_uses_local_home(
                source,
                declaration,
            )
            .ok_or(invalid())? =>
        {
            selected_instructions::SelectedCaseDispatchSource::Local {
                slot: LocalStorageSlotId::StructuralParameter {
                    place: declaration.place,
                },
            }
        }
        // Borrow the entry-retained address for this observation. For an owned
        // parameter it names the ABI value copy; for a loan it names the
        // caller's referent. Neither changes the subject's semantic custody.
        legalized_operations::LegalizedStructuralCaseSource::Parameter { declaration }
        | legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter { declaration } => {
            let pointer = builder
                .transport
                .pointers
                .iter()
                .find(|(stored, _)| *stored == declaration.place)
                .map(|(_, pointer)| *pointer)
                .ok_or_else(|| invalid())?;
            selected_instructions::SelectedCaseDispatchSource::Borrowed {
                place: declaration.place,
                pointer,
                byte_size: root_shape.byte_size,
            }
        }
    };
    match dispatch_source {
        selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
            if builder
                .transport
                .local_slots
                .iter()
                .filter(|candidate| {
                    candidate.id == slot
                        && candidate.byte_size == root_shape.byte_size
                        && candidate.alignment == root_shape.alignment
                })
                .count()
                != 1
            {
                return Err(invalid());
            }
        }
        selected_instructions::SelectedCaseDispatchSource::Borrowed { pointer, .. } => {
            if builder
                .registers
                .get(pointer.0 as usize)
                .is_none_or(|register| {
                    register.scalar_type
                        != ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 pointer type"),
                        )
                })
            {
                return Err(invalid());
            }
        }
    }
    let declarations = &source
        .structural
        .as_ref()
        .ok_or(invalid())?
        .structural_types;
    let root =
        crate::selection::reference_input::root_type(source, subject.place()).ok_or(invalid())?;
    let geometry = crate::structural_inputs::structural_reference_input::reference_source_geometry(
        root,
        path,
        declarations,
    )
    .ok_or(invalid())?;
    let region_shape = crate::structural_inputs::structural_reference_input::shape(
        geometry.region_root,
        declarations,
    )
    .ok_or(invalid())?;
    if !geometry.elements.is_empty()
        || !geometry.readable
        || geometry.byte_offset != *byte_offset
        || byte_offset
            .checked_add(layout.shape.byte_size)
            .is_none_or(|end| end > region_shape.byte_size)
    {
        return Err(invalid());
    }
    if !geometry.pointer_slots.is_empty() {
        // Keep the original descriptor slot and its extent intact. Resolve the
        // address words once, before any branch or edge cleanup, even when a
        // sole case needs no tag test. The final pointer addresses backing,
        // whose extent can exceed the descriptor's eight-byte home.
        let mut pointer = match dispatch_source {
            selected_instructions::SelectedCaseDispatchSource::Local { slot } => {
                frame_address_pointer(
                    builder,
                    block.id,
                    subject.place(),
                    slot,
                    root_shape.byte_size,
                )?
            }
            selected_instructions::SelectedCaseDispatchSource::Borrowed { pointer, .. } => pointer,
        };
        for slot in &geometry.pointer_slots {
            if slot.scalar_descriptor || !slot.indices.is_empty() {
                return Err(invalid());
            }
            pointer = super::structural::reference::load_pointer_word(
                builder,
                subject.place(),
                pointer,
                slot.byte_offset,
                SelectedMemoryAccessOrigin::Block(block.id),
                Default::default(),
            )?;
        }
        dispatch_source = selected_instructions::SelectedCaseDispatchSource::Borrowed {
            place: subject.place(),
            pointer,
            byte_size: region_shape.byte_size,
        };
    }
    // Payload loads and cleanup belong to the selected case edge, so the
    // unconditional transfer carries the same edge a tested leg would.
    if tested.is_empty() {
        let successor = successor(source, order, builder, dispatch_source, last)?;
        builder.emit(
            SelectedInstructionKind::Jump,
            builder.constraints.keys.jump,
            &[],
            Default::default(),
        )?;
        return Ok(SelectedTerminator::Jump {
            instruction: builder
                .instructions
                .last()
                .cloned()
                .ok_or_else(|| invalid())?,
            successor,
        });
    }
    let pointer = match dispatch_source {
        selected_instructions::SelectedCaseDispatchSource::Local { slot } => frame_address_pointer(
            builder,
            block.id,
            subject.place(),
            slot,
            root_shape.byte_size,
        )?,
        selected_instructions::SelectedCaseDispatchSource::Borrowed { pointer, .. } => pointer,
    };
    let (pointer, displacement) = super::structural::displaced_memory_pointer(
        builder,
        subject.place(),
        pointer,
        *byte_offset,
        4,
        Default::default(),
    )?;
    let tag = register(builder, subject.place(), *byte_offset, 32, true)?;
    memory(
        builder,
        block.id,
        subject.place(),
        *byte_offset,
        4,
        SelectedMemoryAccessRole::ReadPlace,
    )?;
    builder.emit(
        SelectedInstructionKind::Load32 {
            byte_offset: displacement,
        },
        builder.constraints.keys.load32.ok_or_else(|| invalid())?,
        &[pointer, tag],
        Default::default(),
    )?;
    let extra_base = source.blocks.len() + builder.case_blocks.len();
    let mut first = None;
    for ordinal in 0..tested.len() {
        let start = builder.instructions.len();
        if ordinal == 0 {
            builder.emit(
                SelectedInstructionKind::CompareI64Zero,
                builder.constraints.keys.compare_i64_zero,
                &[tag],
                Default::default(),
            )?;
        } else {
            let expected = register(builder, subject.place(), 0, 64, false)?;
            builder.emit_materialize_i64(
                semantic_vocabulary::IntegerValue::Unsigned(ordinal as u128),
                expected,
                Default::default(),
            )?;
            builder.emit(
                SelectedInstructionKind::CompareI64,
                builder.constraints.keys.compare_i64,
                &[tag, expected],
                Default::default(),
            )?;
        }
        let when_zero = successor(source, order, builder, dispatch_source, &tested[ordinal])?;
        let when_nonzero = if ordinal + 1 == tested.len() {
            successor(source, order, builder, dispatch_source, last)?
        } else {
            SelectedSuccessor {
                role: selected_instructions::SelectedSuccessorRole::CaseDispatchContinuation,
                psi_edge: cases[ordinal + 1].edge,
                source_target: block.id,
                block: SelectedBlockId((extra_base + ordinal).try_into().map_err(|_| invalid())?),
                bindings: Vec::new(),
                structural_bindings: Vec::new(),
                fuel: Vec::new(),
                structural_case: None,
            }
        };
        builder.emit(
            SelectedInstructionKind::ConditionalBranchNonZero,
            builder.constraints.keys.conditional_branch,
            &[],
            Default::default(),
        )?;
        let terminator = SelectedTerminator::ConditionalBranch {
            instruction: builder
                .instructions
                .last()
                .cloned()
                .ok_or_else(|| invalid())?,
            when_zero,
            when_nonzero,
        };
        if ordinal == 0 {
            builder.block_body_end = Some(builder.instructions.len() - 1);
            first = Some(terminator);
        } else {
            builder.case_blocks.push(SelectedBlock {
                id: SelectedBlockId(
                    (extra_base + ordinal - 1)
                        .try_into()
                        .map_err(|_| invalid())?,
                ),
                origin: selected_instructions::SelectedBlockOrigin::CaseDispatch {
                    source: block.id,
                    case_ordinal: ordinal.try_into().map_err(|_| invalid())?,
                },
                instructions: builder.instructions[start..builder.instructions.len() - 1].to_vec(),
                terminator,
            });
        }
    }
    first.ok_or_else(|| invalid())
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

/// The address an owned aggregate home hands its consumers: a 64-bit
/// pointer register carrying the local slot's frame address, beside the
/// address-local memory record the transport keeps.
#[track_caller]
pub(super) fn frame_address_pointer(
    builder: &mut Builder<'_>,
    block: semantic_vocabulary::BlockId,
    place: PlaceId,
    slot: LocalStorageSlotId,
    byte_size: u32,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let pointer = register(builder, place, 0, 64, false)?;
    memory(
        builder,
        block,
        place,
        0,
        byte_size,
        SelectedMemoryAccessRole::AddressLocal { slot },
    )?;
    builder.emit_local_frame_address(slot, pointer, Default::default())?;
    Ok(pointer)
}

fn successor(
    source: &LegalizedScalarFunction,
    order: &[usize],
    builder: &Builder<'_>,
    dispatch_source: selected_instructions::SelectedCaseDispatchSource,
    case: &LegalizedStructuralCaseSuccessor,
) -> Result<SelectedSuccessor, SelectedInstructionError> {
    let block = order
        .iter()
        .position(|position| source.blocks[*position].id == case.target)
        .ok_or_else(|| invalid())?;
    let mut payloads = Vec::with_capacity(case.payloads.len());
    for payload in &case.payloads {
        let transport = if builder.required_values.contains(&payload.parameter.value) {
            let (_, parameter, site, scalar_type) = builder
                .resolve(payload.parameter.value)
                .ok_or_else(|| invalid())?;
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
            source: dispatch_source,
            case: case.case,
            case_tag: case.case_tag,
            payloads,
            cleanup_actions: case.cleanup_actions.clone(),
        }),
    })
}

//! Project resolved instructions and their fixups directly into framed function coordinates.
use machine_code::fragment_origins::{
    FragmentFunctionOrigin, FragmentInstructionOrigin, FragmentOrigins,
};

#[cfg(test)]
use machine_code::FunctionFragmentSuccessorProvenance;
use machine_code::{
    FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentEmissionPlan,
    FunctionFragmentInstructionSpan, FunctionFragmentInternalMachineFixup,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupState, ResolvedMachineLayout, ResolvedMachineProgram,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
    SelectedFormInternalMachineFixupState, SelectedFormNormalizedForeignCallFixupState,
};
#[cfg(test)]
use selected_instructions::{
    SelectedFunction, SelectedInstruction, SelectedInstructionKind, SelectedSuccessor,
    SelectedTerminator,
};
use std::sync::Arc;

pub(crate) use machine_code::fragment_origins::ResolvedFragmentEmissionError;

use crate::FunctionTextLayoutError;
use crate::frame_application::{FrameWriter, FunctionFragmentFrameApplicationError as FrameError};
use crate::relative_branches::FunctionFrameBranchOrigins;

impl From<ResolvedFragmentEmissionError> for FunctionTextLayoutError {
    fn from(error: ResolvedFragmentEmissionError) -> Self {
        Self::Fragment(error)
    }
}

impl From<FrameError> for FunctionTextLayoutError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}

pub(super) struct FramedFunctions {
    pub application: machine_code::FunctionFragmentFrameApplication,
    pub unframed: Option<FunctionFragmentEmissionPlan>,
    pub branches: Vec<FunctionFrameBranchOrigins>,
}

pub(super) fn emit_functions(
    program: &ResolvedMachineProgram,
    retain_original: bool,
    check_branches: bool,
) -> Result<FramedFunctions, FunctionTextLayoutError> {
    let selected = &program.fragment_origins;
    let layout = &program.layout;
    if selected.target != layout.target
        || layout.machine.selected != selected.selected
        || selected.functions.len() != layout.functions.len()
        || program.context.publication.record().selected != selected.selected
        || program.context.machine.plan.selected != selected.selected
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch.into());
    }
    let protocol = &program.protocol;
    if selected.target != protocol.target {
        return Err(FrameError::RootMismatch.into());
    }
    if selected.functions.len() != protocol.functions.len()
        || selected.functions.len() != program.control_alternatives.len()
    {
        return Err(FrameError::FunctionRosterMismatch.into());
    }
    let mut fragments = FunctionFragmentEmissionPlan {
        psi: selected.psi,
        work_schedule: selected.work_schedule,
        selected: Arc::clone(&selected.selected),
        target: selected.target,
        entry: selected.entry,
        functions: Vec::with_capacity(selected.functions.len()),
    };
    let mut unframed = retain_original.then(|| fragments.clone());
    let mut branches = Vec::new();
    let mut frames = Vec::with_capacity(selected.functions.len());
    for (function, controls) in selected
        .functions
        .iter()
        .zip(&program.control_alternatives[..])
    {
        let resolved = layout.function(function.machine).ok_or(
            ResolvedFragmentEmissionError::MissingFunction(function.machine),
        )?;
        let row = crate::frame_application::unique_protocol(protocol, function.machine)?;
        let prologue = row
            .prologue
            .bytes(&protocol.bytes)
            .ok_or(FrameError::InvalidProtocolSpan(function.machine))?;
        let epilogue = row
            .epilogue
            .bytes(&protocol.bytes)
            .ok_or(FrameError::InvalidProtocolSpan(function.machine))?;
        let writer = FrameWriter::new(function.machine, resolved.byte_count, prologue, epilogue)?;
        let (mut fragment, mut frame) = emit_function(function, resolved, writer)?;
        if check_branches {
            branches.push(crate::frame_application::branch_origins(
                &fragment, controls,
            ));
        }
        if let Some(original) = &mut unframed {
            let writer = FrameWriter::new(function.machine, resolved.byte_count, &[], &[])?;
            original
                .functions
                .push(emit_function(function, resolved, writer)?.0);
        }
        crate::frame_application::patch_branches(
            &mut fragment,
            &mut frame.epilogues,
            selected.target.architecture,
            controls,
        )?;
        fragments.functions.push(fragment);
        frames.push(frame);
    }
    if protocol.functions.iter().any(|row| {
        !selected
            .functions
            .iter()
            .any(|function| function.machine == row.machine)
    }) {
        return Err(FrameError::FunctionRosterMismatch.into());
    }
    Ok(FramedFunctions {
        application: machine_code::FunctionFragmentFrameApplication {
            fragments,
            functions: frames,
        },
        unframed,
        branches,
    })
}

pub(super) fn emit_function(
    selected: &FragmentFunctionOrigin,
    resolved: &ResolvedSelectedFunctionLayout,
    mut writer: FrameWriter<'_>,
) -> Result<(FunctionFragment, machine_code::FunctionAppliedFrameProtocol), FunctionTextLayoutError>
{
    let mut blocks = Vec::with_capacity(resolved.blocks.len());
    for block in &resolved.blocks {
        blocks.push(emit_block(selected, block, &mut writer)?);
    }
    if writer.source_offset != resolved.byte_count {
        return Err(ResolvedFragmentEmissionError::RootMismatch.into());
    }
    Ok((
        FunctionFragment {
            machine: selected.machine,
            attachment: selected.attachment,
            provenance: selected.provenance.clone(),
            byte_count: writer.bytes.len() as u64,
            bytes: writer.bytes,
            blocks,
        },
        writer.frame,
    ))
}

fn emit_block(
    selected: &FragmentFunctionOrigin,
    resolved: &ResolvedSelectedBlockLayout,
    writer: &mut FrameWriter<'_>,
) -> Result<FunctionFragmentBlockSpan, FunctionTextLayoutError> {
    if writer.source_offset != resolved.offset {
        return Err(ResolvedFragmentEmissionError::RootMismatch.into());
    }
    let block_start = writer.bytes.len() as u64;
    let selected_block = selected
        .block(resolved.block)
        .ok_or(ResolvedFragmentEmissionError::MissingBlock(resolved.block))?;
    let mut instructions = Vec::with_capacity(resolved.instructions.len());
    let mut selected_instructions = selected_block.instructions.iter();
    for (index, row) in resolved.instructions.iter().enumerate() {
        let instruction = selected_instructions.next().ok_or(
            ResolvedFragmentEmissionError::MissingInstruction(row.instruction),
        )?;
        if writer.source_offset != row.offset {
            return Err(ResolvedFragmentEmissionError::RootMismatch.into());
        }
        let control = instruction.control.as_ref().map_err(Clone::clone)?;
        let offset = writer.write(
            resolved.block,
            row.instruction,
            control,
            &row.bytes,
            index + 1 == resolved.instructions.len(),
        )?;
        instructions.push(emit_instruction(instruction, row, offset)?);
    }
    if selected_instructions.next().is_some()
        || writer.source_offset.checked_sub(resolved.offset) != Some(resolved.byte_count)
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch.into());
    }
    Ok(FunctionFragmentBlockSpan {
        block: resolved.block,
        offset: block_start,
        byte_count: writer.bytes.len() as u64 - block_start,
        instructions,
    })
}

fn emit_instruction(
    instruction: &FragmentInstructionOrigin,
    row: &ResolvedSelectedFormRow,
    offset: u64,
) -> Result<FunctionFragmentInstructionSpan, ResolvedFragmentEmissionError> {
    if instruction.id != row.instruction {
        return Err(ResolvedFragmentEmissionError::MissingInstruction(
            row.instruction,
        ));
    }
    let control = instruction.control.clone()?;
    let internal_machine_fixup = translate_fixup(row, instruction, offset)?;
    let import_free = instruction.import_free.clone()?;
    let normalized_foreign_call_fixup =
        translate_foreign_fixup(row, instruction, import_free, offset)?;
    let image_address_fixup = translate_byte_literal_fixup(row, instruction, offset)?;
    Ok(FunctionFragmentInstructionSpan {
        instruction: row.instruction,
        alternative: row.alternative,
        offset,
        bytes: row.bytes.clone(),
        branch: row.branch.clone(),
        internal_machine_fixup: internal_machine_fixup.map(Box::new),
        normalized_foreign_call_fixup: normalized_foreign_call_fixup.map(Box::new),
        image_address_fixup: image_address_fixup.map(Box::new),
        provenance: instruction.provenance.clone(),
        control,
    })
}

fn translate_byte_literal_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &FragmentInstructionOrigin,
    offset: u64,
) -> Result<Option<machine_code::FunctionFragmentImageAddressFixup>, ResolvedFragmentEmissionError>
{
    let reference = instruction.image_reference;
    match (row.image_address_fixup(), reference) {
        (None, None) => Ok(None),
        (Some(fixup), Some(reference)) if fixup.reference == reference && row.branch.is_none() => {
            Ok(Some(machine_code::FunctionFragmentImageAddressFixup {
                reference,
                fields: fixup
                    .fields
                    .shifted(offset)
                    .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?,
            }))
        }
        _ => Err(ResolvedFragmentEmissionError::RootMismatch),
    }
}

fn translate_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &FragmentInstructionOrigin,
    offset: u64,
) -> Result<Option<FunctionFragmentInternalMachineFixup>, ResolvedFragmentEmissionError> {
    let Some(fixup) = row.internal_machine_fixup() else {
        if instruction.callee.is_some() {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    };
    let callee = instruction
        .callee
        .ok_or(ResolvedFragmentEmissionError::RootMismatch)?;
    if fixup.state != SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.callee != callee
        || row.branch.is_some()
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let translate = |field: u16| {
        offset
            .checked_add(u64::from(field))
            .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)
    };
    Ok(Some(FunctionFragmentInternalMachineFixup {
        kind: fixup.kind,
        state: FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1,
        callee,
        opcode_function_offset: translate(fixup.opcode_row_offset)?,
        patch_function_offset: translate(fixup.patch_row_offset)?,
        reference_function_offset: translate(fixup.reference_row_offset)?,
        patch_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
    }))
}

fn translate_foreign_fixup(
    row: &ResolvedSelectedFormRow,
    instruction: &FragmentInstructionOrigin,
    import_free: bool,
    offset: u64,
) -> Result<Option<FunctionFragmentNormalizedForeignCallFixup>, ResolvedFragmentEmissionError> {
    if import_free {
        if row.normalized_foreign_call_fixup.is_some()
            || row.internal_machine_fixup.is_some()
            || row.branch.is_some()
        {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    }
    let Some(fixup) = row.normalized_foreign_call_fixup.clone() else {
        if instruction.foreign.is_some() {
            return Err(ResolvedFragmentEmissionError::RootMismatch);
        }
        return Ok(None);
    };
    let Some((boundary, ordinal)) = instruction.foreign else {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    };
    if fixup.state != SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.boundary != boundary
        || fixup.ordinal != ordinal
        || row.branch.is_some()
        || row.internal_machine_fixup.is_some()
    {
        return Err(ResolvedFragmentEmissionError::RootMismatch);
    }
    let translate = |field: u16| {
        offset
            .checked_add(u64::from(field))
            .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)
    };
    Ok(Some(FunctionFragmentNormalizedForeignCallFixup {
        kind: fixup.kind,
        state: FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1,
        boundary,
        ordinal,
        opcode_function_offset: translate(fixup.opcode_row_offset)?,
        patch_function_offset: translate(fixup.patch_row_offset)?,
        reference_function_offset: translate(fixup.reference_row_offset)?,
        patch_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
        callbacks: fixup
            .callbacks
            .iter()
            .map(|callback| {
                Ok(machine_code::FunctionFragmentCallbackMaterialization {
                    destination: callback.destination.clone(),
                    code_function_offset: translate(callback.code_row_offset)?,
                    code_byte_count: u64::from(callback.code_byte_count),
                    encoding: match callback.encoding {
                        machine_code::SelectedFormCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_row_offset,
                        } => machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_function_offset: translate(patch_row_offset)?,
                        },
                        machine_code::SelectedFormCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_row_offset,
                            page_offset_patch_row_offset,
                        } => machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_function_offset: translate(page_patch_row_offset)?,
                            page_offset_patch_function_offset: translate(
                                page_offset_patch_row_offset,
                            )?,
                        },
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    }))
}

pub(crate) fn validate_resolved_function_fragments(
    program: &ResolvedMachineProgram,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    let selected = &program.fragment_origins;
    let layout = &program.layout;
    require(
        fragments.psi == selected.psi
            && fragments.work_schedule == selected.work_schedule
            && fragments.selected == layout.machine.selected
            && layout.machine.selected == selected.selected
            && fragments.target == selected.target
            && layout.target == selected.target
            && fragments.entry == selected.entry,
    )?;
    check_ordinary_fragments(selected, layout, fragments)
}

fn require(condition: bool) -> Result<(), ResolvedFragmentEmissionError> {
    if condition {
        Ok(())
    } else {
        Err(ResolvedFragmentEmissionError::ArtifactMismatch)
    }
}

fn byte_span(
    bytes: &[u8],
    offset: u64,
    content: &[u8],
) -> Result<(), ResolvedFragmentEmissionError> {
    let start =
        usize::try_from(offset).map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?;
    let end = start
        .checked_add(content.len())
        .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?;
    require(bytes.get(start..end) == Some(content))
}

fn check_ordinary_fragments(
    selected: &FragmentOrigins,
    layout: &ResolvedMachineLayout,
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        selected.functions.len() == layout.functions.len()
            && fragments.functions.len() == selected.functions.len(),
    )?;
    for ((source, resolved), claimed) in selected
        .functions
        .iter()
        .zip(&layout.functions)
        .zip(&fragments.functions)
    {
        require(
            resolved.machine == source.machine
                && claimed.machine == source.machine
                && claimed.attachment == source.attachment
                && claimed.provenance == source.provenance
                && claimed.byte_count == resolved.byte_count
                && u64::try_from(claimed.bytes.len()).ok() == Some(claimed.byte_count)
                && claimed.blocks.len() == resolved.blocks.len()
                && resolved.blocks.len() == source.blocks.len(),
        )?;
        let mut offset = 0_u64;
        for (block, actual) in resolved.blocks.iter().zip(&claimed.blocks) {
            let selected_block = source
                .block(block.block)
                .ok_or(ResolvedFragmentEmissionError::MissingBlock(block.block))?;
            require(
                actual.block == block.block
                    && actual.offset == block.offset
                    && block.offset == offset
                    && actual.byte_count == block.byte_count
                    && actual.instructions.len() == block.instructions.len()
                    && block.instructions.len() == selected_block.instructions.len(),
            )?;
            for ((row, span), selected_instruction) in block
                .instructions
                .iter()
                .zip(&actual.instructions)
                .zip(selected_block.instructions.iter())
            {
                require(row.offset == offset && row.instruction == selected_instruction.id)?;
                check_instruction_span(selected_instruction, row, span)?;
                byte_span(&claimed.bytes, offset, &span.bytes)?;
                offset = offset
                    .checked_add(
                        u64::try_from(span.bytes.len())
                            .map_err(|_| ResolvedFragmentEmissionError::OffsetOverflow)?,
                    )
                    .ok_or(ResolvedFragmentEmissionError::OffsetOverflow)?;
            }
            require(offset.checked_sub(block.offset) == Some(block.byte_count))?;
        }
        require(offset == claimed.byte_count)?;
    }
    Ok(())
}

fn check_foreign_call_fixup(
    selected: &FragmentInstructionOrigin,
    row: &ResolvedSelectedFormRow,
    span: &FunctionFragmentInstructionSpan,
) -> Result<(), ResolvedFragmentEmissionError> {
    let import_free = *selected.import_free.as_ref().map_err(Clone::clone)?;
    let (source, placed) = match (
        &row.normalized_foreign_call_fixup,
        &span.normalized_foreign_call_fixup,
    ) {
        (None, None) if import_free => {
            return require(row.branch.is_none() && row.internal_machine_fixup.is_none());
        }
        (None, None) => return require(selected.foreign.is_none()),
        (Some(source), Some(placed)) if !import_free => (source, placed),
        _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    };
    // Replay in the row's coordinate system. The emitter translates in the opposite direction.
    let matches_offset = |relative: u16, absolute: u64| {
        absolute.checked_sub(row.offset) == Some(u64::from(relative))
    };
    require(
        selected.foreign == Some((source.boundary, source.ordinal))
            && row.branch.is_none()
            && row.internal_machine_fixup.is_none()
            && placed.boundary == source.boundary
            && placed.ordinal == source.ordinal
            && placed.kind == source.kind
            && source.state == SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1
            && placed.state
                == FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1
            && matches_offset(source.opcode_row_offset, placed.opcode_function_offset)
            && matches_offset(source.patch_row_offset, placed.patch_function_offset)
            && matches_offset(
                source.reference_row_offset,
                placed.reference_function_offset,
            )
            && placed.patch_byte_width == source.patch_byte_width
            && placed.addend == source.addend
            && placed.callbacks.len() == source.callbacks.len(),
    )?;
    for (source, placed) in source.callbacks.iter().zip(&placed.callbacks) {
        use machine_code::{
            FunctionFragmentCallbackMaterializationEncoding as Placed,
            SelectedFormCallbackMaterializationEncoding as Row,
        };
        require(
            source.destination == placed.destination
                && matches_offset(source.code_row_offset, placed.code_function_offset)
                && u64::from(source.code_byte_count) == placed.code_byte_count,
        )?;
        require(match (source.encoding, placed.encoding) {
            (
                Row::X86_64Relative32 { patch_row_offset },
                Placed::X86_64Relative32 {
                    patch_function_offset,
                },
            ) => matches_offset(patch_row_offset, patch_function_offset),
            (
                Row::Aarch64PageAddress {
                    page_patch_row_offset,
                    page_offset_patch_row_offset,
                },
                Placed::Aarch64PageAddress {
                    page_patch_function_offset,
                    page_offset_patch_function_offset,
                },
            ) => {
                matches_offset(page_patch_row_offset, page_patch_function_offset)
                    && matches_offset(
                        page_offset_patch_row_offset,
                        page_offset_patch_function_offset,
                    )
            }
            _ => false,
        })?;
    }
    Ok(())
}

fn check_instruction_span(
    selected: &FragmentInstructionOrigin,
    row: &ResolvedSelectedFormRow,
    span: &FunctionFragmentInstructionSpan,
) -> Result<(), ResolvedFragmentEmissionError> {
    require(
        span.instruction == row.instruction
            && span.alternative == row.alternative
            && span.offset == row.offset
            && span.bytes == row.bytes
            && span.provenance == selected.provenance,
    )?;
    require(selected.control.as_ref().map_err(Clone::clone)? == &span.control)?;
    check_foreign_call_fixup(selected, row, span)?;
    let reference = selected.image_reference;
    match (
        row.image_address_fixup(),
        span.image_address_fixup(),
        reference,
    ) {
        (None, None, None) => {}
        (Some(source), Some(actual), Some(reference)) => require(
            source.reference == reference
                && actual.reference == reference
                && source.fields.shifted(row.offset) == Some(actual.fields),
        )?,
        _ => return Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
    require(row.branch == span.branch)?;
    match (
        &row.internal_machine_fixup,
        &span.internal_machine_fixup,
        selected.callee,
    ) {
        (Some(source), Some(actual), Some(callee)) => require(
            row.branch.is_none()
                && source.callee == callee
                && actual.callee == callee
                && source.state == SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1
                && actual.state == FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
                && source.kind == actual.kind
                && Some(actual.opcode_function_offset)
                    == row.offset.checked_add(u64::from(source.opcode_row_offset))
                && Some(actual.patch_function_offset)
                    == row.offset.checked_add(u64::from(source.patch_row_offset))
                && Some(actual.reference_function_offset)
                    == row.offset.checked_add(u64::from(source.reference_row_offset))
                && actual.patch_byte_width == source.patch_byte_width
                && actual.addend == source.addend,
        ),
        (None, None, None) =>
        {
            Ok(())
        }
        _ => Err(ResolvedFragmentEmissionError::ArtifactMismatch),
    }
}

#[cfg(test)]
mod tests {
    use encoding_admission::fragment_origins::check_successor_provenance;
    fn translate_fixup(
        row: &ResolvedSelectedFormRow,
        instruction: &SelectedInstruction,
    ) -> Result<
        Option<machine_code::FunctionFragmentInternalMachineFixup>,
        ResolvedFragmentEmissionError,
    > {
        super::translate_fixup(
            row,
            &encoding_admission::fragment_origins::prepare_fixup_origin(instruction),
            row.offset,
        )
    }
    fn translate_foreign_fixup(
        row: &ResolvedSelectedFormRow,
        instruction: &SelectedInstruction,
        import_free: bool,
    ) -> Result<
        Option<machine_code::FunctionFragmentNormalizedForeignCallFixup>,
        ResolvedFragmentEmissionError,
    > {
        super::translate_foreign_fixup(
            row,
            &encoding_admission::fragment_origins::prepare_fixup_origin(instruction),
            import_free,
            row.offset,
        )
    }

    use super::{
        FunctionFragmentInternalMachineFixupState, FunctionFragmentSuccessorProvenance,
        ResolvedFragmentEmissionError, ResolvedSelectedBlockLayout, ResolvedSelectedFormRow,
        ResolvedSelectedFunctionLayout, SelectedFormInternalMachineFixupState, SelectedFunction,
        SelectedInstruction, SelectedInstructionKind, SelectedSuccessor, SelectedTerminator,
    };
    use machine_code::{
        FunctionFragmentControlProvenance, SelectedFormInternalMachineFixup,
        SelectedFormInternalMachineFixupKind,
    };
    use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlock, SelectedBlockId,
        SelectedBlockOrigin, SelectedInstructionId, SelectedInstructionProvenance,
        SelectedSuccessorRole,
    };
    use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};

    fn selected_call(callee: MachineId) -> SelectedInstruction {
        SelectedInstruction {
            id: SelectedInstructionId(17),
            kind: SelectedInstructionKind::CallScalar {
                target: selected_instructions::SelectedCallTarget::Machine(callee),
            },
            constraint: RegisterConstraintKey {
                family: RegisterConstraintFamily::Call,
                variant: 0,
            },
            operands: Vec::new(),
            implicit_uses: Vec::new(),
            implicit_defs: Vec::new(),
            clobbers: Vec::new(),
            provenance: SelectedInstructionProvenance::default(),
        }
    }

    #[test]
    fn replay_checks_foreign_call_import_identity_and_callback_offsets() {
        use machine_code::{
            CallbackAddressDestination, SelectedFormCallbackMaterialization,
            SelectedFormCallbackMaterializationEncoding, SelectedFormNormalizedForeignCallFixup,
            SelectedFormNormalizedForeignCallFixupKind,
            SelectedFormNormalizedForeignCallFixupState,
        };
        let boundary = semantic_vocabulary::BoundaryMachineId::new(3).unwrap();
        let mut instruction = selected_call(MachineId::new(29).unwrap());
        instruction.kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary,
            ordinal: 4,
        };
        let origin = encoding_admission::fragment_origins::prepare_fixup_origin(&instruction);
        let row = ResolvedSelectedFormRow {
            instruction: instruction.id,
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            },
            offset: 37,
            bytes: vec![0x48, 0x8d, 0x0d, 0, 0, 0, 0, 0xe8, 0, 0, 0, 0],
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: Some(Box::new(SelectedFormNormalizedForeignCallFixup {
                kind: SelectedFormNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
                state: SelectedFormNormalizedForeignCallFixupState::UnresolvedImportFieldV1,
                boundary,
                ordinal: 4,
                opcode_row_offset: 7,
                patch_row_offset: 8,
                reference_row_offset: 12,
                patch_byte_width: 4,
                addend: 0,
                callbacks: vec![SelectedFormCallbackMaterialization {
                    destination: CallbackAddressDestination::Register(calling_conventions::MachineRegister::X86Rcx),
                    code_row_offset: 0,
                    code_byte_count: 7,
                    encoding: SelectedFormCallbackMaterializationEncoding::X86_64Relative32 { patch_row_offset: 3 },
                }],
            })),
            image_address_fixup: None,
        };
        let mut span = super::emit_instruction(&origin, &row, row.offset).unwrap();
        super::check_instruction_span(&origin, &row, &span).unwrap();
        let original = span.normalized_foreign_call_fixup.clone();
        span.normalized_foreign_call_fixup.as_mut().unwrap().ordinal += 1;
        assert!(
            super::check_instruction_span(&origin, &row, &span).is_err(),
            "changed import"
        );
        span.normalized_foreign_call_fixup = original;
        span.normalized_foreign_call_fixup
            .as_mut()
            .unwrap()
            .callbacks[0]
            .code_function_offset += 1;
        assert!(
            super::check_instruction_span(&origin, &row, &span).is_err(),
            "changed callback"
        );
        span.normalized_foreign_call_fixup = None;
        assert!(
            super::check_instruction_span(&origin, &row, &span).is_err(),
            "missing import"
        );
    }

    #[test]
    fn system_call_projection_has_no_import_fixup_but_imports_still_require_one() {
        let mut instruction = selected_call(MachineId::new(29).unwrap());
        instruction.kind = SelectedInstructionKind::NormalizedForeignCall {
            boundary: semantic_vocabulary::BoundaryMachineId::new(3).unwrap(),
            ordinal: 0,
        };
        instruction.constraint = isa_x86_64::x86_64_linux_system_call_keys()[0];
        let row = ResolvedSelectedFormRow {
            instruction: instruction.id,
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::NormalizedForeignCall,
                variant: 0,
            },
            offset: 37,
            bytes: Vec::new(),
            branch: None,
            internal_machine_fixup: None,
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        };
        assert_eq!(
            translate_foreign_fixup(&row, &instruction, true).unwrap(),
            None
        );
        instruction.constraint.family = RegisterConstraintFamily::Call;
        assert!(translate_foreign_fixup(&row, &instruction, false).is_err());
    }

    fn row(bytes: Vec<u8>, fixup: SelectedFormInternalMachineFixup) -> ResolvedSelectedFormRow {
        ResolvedSelectedFormRow {
            instruction: SelectedInstructionId(17),
            alternative: MachineAlternativeKey {
                family: MachineAlternativeFamily::CallScalar,
                variant: 0,
            },
            offset: 37,
            bytes,
            branch: None,
            internal_machine_fixup: Some(Box::new(fixup)),
            normalized_foreign_call_fixup: None,
            image_address_fixup: None,
        }
    }

    #[test]
    fn row_relative_call_fixups_become_function_relative_without_resolution() {
        let callee = MachineId::new(29).unwrap();
        for (kind, bytes, opcode, patch, reference) in [
            (
                SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                vec![0xe8, 0, 0, 0, 0],
                0,
                1,
                5,
            ),
            (
                SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                0x9400_0000_u32.to_le_bytes().to_vec(),
                0,
                0,
                0,
            ),
        ] {
            let selected = selected_call(callee);
            let translated = translate_fixup(
                &row(
                    bytes,
                    SelectedFormInternalMachineFixup {
                        kind,
                        state: SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1,
                        callee,
                        opcode_row_offset: opcode,
                        patch_row_offset: patch,
                        reference_row_offset: reference,
                        patch_byte_width: 4,
                        addend: 0,
                    },
                ),
                &selected,
            )
            .unwrap()
            .unwrap();
            assert_eq!(translated.callee, callee);
            assert_eq!(translated.opcode_function_offset, 37 + u64::from(opcode));
            assert_eq!(translated.patch_function_offset, 37 + u64::from(patch));
            assert_eq!(
                translated.reference_function_offset,
                37 + u64::from(reference)
            );
            assert_eq!(
                translated.state,
                FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
            );
        }
    }

    #[test]
    fn call_fixup_translation_rejects_detached_callee_and_missing_custody() {
        let callee = MachineId::new(29).unwrap();
        let selected = selected_call(callee);
        let detached = row(
            vec![0xe8, 0, 0, 0, 0],
            SelectedFormInternalMachineFixup {
                kind: SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
                state: SelectedFormInternalMachineFixupState::UnresolvedZeroFieldV1,
                callee: MachineId::new(30).unwrap(),
                opcode_row_offset: 0,
                patch_row_offset: 1,
                reference_row_offset: 5,
                patch_byte_width: 4,
                addend: 0,
            },
        );
        assert!(translate_fixup(&detached, &selected).is_err());
        let mut missing = detached;
        missing.internal_machine_fixup = None;
        assert!(translate_fixup(&missing, &selected).is_err());
    }

    fn emit_ordinary_function(
        selected: &SelectedFunction,
        layout: &ResolvedSelectedFunctionLayout,
    ) -> Result<machine_code::FunctionFragment, crate::FunctionTextLayoutError> {
        let current = post_allocation_machine::PhysicalFunction {
            contracts: std::sync::Arc::clone(&selected.contracts),
            entry_block: selected.entry_block,
            blocks: selected.blocks.clone(),
        };
        super::emit_function(
            &encoding_admission::fragment_origins::prepare_function_origin(&current),
            layout,
            super::FrameWriter::new(layout.machine, layout.byte_count, &[], &[])?,
        )
        .map(|(fragment, _)| fragment)
    }

    fn fixture(instruction_count: u32) -> (SelectedFunction, ResolvedSelectedFunctionLayout) {
        let mut selected = Vec::new();
        let mut rows = Vec::new();
        for ordinal in 0..=instruction_count {
            let terminal = ordinal == instruction_count;
            let id = SelectedInstructionId(u32::MAX - ordinal * 3);
            selected.push(SelectedInstruction {
                id,
                kind: if terminal {
                    SelectedInstructionKind::ReturnUnit
                } else {
                    SelectedInstructionKind::CopyI64
                },
                constraint: RegisterConstraintKey {
                    family: if terminal {
                        RegisterConstraintFamily::Return
                    } else {
                        RegisterConstraintFamily::Instruction
                    },
                    variant: 0,
                },
                operands: Vec::new(),
                implicit_uses: Vec::new(),
                implicit_defs: Vec::new(),
                clobbers: Vec::new(),
                provenance: SelectedInstructionProvenance {
                    operations: vec![OperationId::new(u64::from(ordinal) + 1).unwrap()],
                    ..Default::default()
                },
            });
            rows.push(ResolvedSelectedFormRow {
                instruction: id,
                alternative: MachineAlternativeKey {
                    family: if terminal {
                        MachineAlternativeFamily::ReturnUnit
                    } else {
                        MachineAlternativeFamily::CopyI64
                    },
                    variant: 0,
                },
                offset: u64::from(ordinal) * 4,
                bytes: ordinal.to_le_bytes().to_vec(),
                branch: None,
                internal_machine_fixup: None,
                normalized_foreign_call_fixup: None,
                image_address_fixup: None,
            });
        }
        let machine = MachineId::new(1).unwrap();
        let block = SelectedBlockId(0);
        let terminal = selected.pop().unwrap();
        let byte_count = u64::from(instruction_count + 1) * 4;
        (
            SelectedFunction {
                contracts: std::sync::Arc::new(selected_instructions::SelectedFunctionContracts {
                    structural_block_parameters: Vec::new(),
                    machine,
                    attachment: None,
                    provenance: Default::default(),
                    structural: None,
                    local_storage_slots: Vec::new(),
                    outgoing_arguments: Vec::new(),
                    calls: Vec::new(),
                    normalized_foreign_calls: Vec::new(),
                    memory_accesses: Vec::new(),
                    boundary_settlements: Vec::new(),
                    virtual_registers: Vec::new(),
                }),
                entry_block: block,
                blocks: (vec![SelectedBlock {
                    id: block,
                    origin: SelectedBlockOrigin::Source(BlockId::new(1).unwrap()),
                    instructions: selected,
                    terminator: SelectedTerminator::Return {
                        instruction: terminal,
                        psi_return_edge: EdgeId::new(1).unwrap(),
                    },
                }])
                .into(),
            },
            ResolvedSelectedFunctionLayout {
                machine,
                byte_count,
                blocks: vec![ResolvedSelectedBlockLayout {
                    block,
                    offset: 0,
                    byte_count,
                    instructions: rows,
                }],
            },
        )
    }

    #[test]
    fn ordered_projection_preserves_bytes_identity_and_provenance() {
        for instruction_count in [0, 3, 257] {
            let (selected, layout) = fixture(instruction_count);
            let fragment = emit_ordinary_function(&selected, &layout).unwrap();
            assert_eq!(fragment.byte_count, layout.byte_count);
            assert_eq!(fragment.blocks.len(), 1);
            assert_eq!(
                fragment.blocks[0].instructions.len(),
                instruction_count as usize + 1
            );
            let expected_bytes: Vec<_> =
                (0..=instruction_count).flat_map(u32::to_le_bytes).collect();
            assert_eq!(fragment.bytes, expected_bytes);
            for (ordinal, span) in fragment.blocks[0].instructions.iter().enumerate() {
                let row = &layout.blocks[0].instructions[ordinal];
                assert_eq!(span.instruction, row.instruction);
                assert_eq!(span.alternative, row.alternative);
                assert_eq!(span.offset, row.offset);
                assert_eq!(span.bytes, row.bytes);
                assert_eq!(
                    span.provenance.operations,
                    vec![OperationId::new(ordinal as u64 + 1).unwrap()]
                );
                assert_eq!(
                    span.control,
                    if ordinal == instruction_count as usize {
                        FunctionFragmentControlProvenance::Return {
                            psi_return_edge: EdgeId::new(1).unwrap(),
                        }
                    } else {
                        FunctionFragmentControlProvenance::None
                    }
                );
            }
        }
    }

    #[test]
    fn ordered_projection_rejects_missing_instruction() {
        let (selected, mut layout) = fixture(3);
        let missing = SelectedInstructionId(17);
        layout.blocks[0].instructions[1].instruction = missing;
        assert!(
            matches!(emit_ordinary_function(&selected, &layout), Err(crate::FunctionTextLayoutError::Fragment(crate::FunctionFragmentEmissionError::MissingInstruction(id))) if id == missing)
        );
    }

    #[test]
    fn ordered_projection_preserves_zero_byte_rows() {
        let (selected, mut layout) = fixture(3);
        layout.blocks[0].instructions[0].bytes.clear();
        for row in &mut layout.blocks[0].instructions[1..] {
            row.offset -= 4;
        }
        layout.blocks[0].byte_count -= 4;
        layout.byte_count -= 4;
        let fragment = emit_ordinary_function(&selected, &layout).unwrap();
        assert_eq!(fragment.blocks[0].instructions.len(), 4);
        assert!(fragment.blocks[0].instructions[0].bytes.is_empty());
        assert_eq!(
            fragment.blocks[0].instructions[0].provenance,
            selected.blocks[0].instructions[0].provenance
        );
        assert_eq!(
            fragment.bytes,
            (1_u32..=3).flat_map(u32::to_le_bytes).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "manual fragment-emission scaling measurement"]
    fn ordered_projection_scaling() {
        for instruction_count in [4096, 8192, 16384] {
            let (selected, layout) = fixture(instruction_count);
            let expected = emit_ordinary_function(&selected, &layout).unwrap();
            let mut timings = Vec::new();
            for _ in 0..7 {
                let started = std::time::Instant::now();
                let actual =
                    std::hint::black_box(emit_ordinary_function(&selected, &layout).unwrap());
                timings.push(started.elapsed());
                assert_eq!(actual, expected);
            }
            timings.sort_unstable();
            eprintln!(
                "fragment rows={} median_ns={}",
                instruction_count + 1,
                timings[3].as_nanos()
            );
        }
    }

    #[test]
    fn continuation_retains_exact_successor_role_and_destination() {
        let source = SelectedSuccessor {
            structural_case: None,
            role: SelectedSuccessorRole::EdgeTransferContinuation,
            structural_bindings: Vec::new(),
            psi_edge: EdgeId::new(1),
            block: SelectedBlockId(2),
            source_target: BlockId::new(3).unwrap(),
            bindings: Vec::new(),
            work: Vec::new(),
        };
        let mut actual = FunctionFragmentSuccessorProvenance {
            role: source.role,
            psi_edge: source.psi_edge,
            block: source.block,
            source_target: source.source_target,
            bindings: Vec::new(),
            work: Vec::new(),
        };
        check_successor_provenance(&source, &actual).unwrap();
        actual.role = SelectedSuccessorRole::Semantic;
        assert!(check_successor_provenance(&source, &actual).is_err());
        actual.role = source.role;
        actual.block = SelectedBlockId(4);
        assert!(check_successor_provenance(&source, &actual).is_err());
    }
}

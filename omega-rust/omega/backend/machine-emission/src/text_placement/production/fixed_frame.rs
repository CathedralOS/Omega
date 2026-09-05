use std::collections::{BTreeMap, BTreeSet};

use machine_code::{
    FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState,
};
use machine_code::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState, PlacedFunctionFragment,
    PlacedInternalMachineCallResolution, RelocationFreeTextSectionPlacement,
    TextSectionPlacementPolicy, TextSectionRelocationRequirements,
};
use optimization_core::TerminalRelocationFreeTextSectionIdentity;
use semantic_vocabulary::{MachineId, OperationId};
use target::Architecture;

use super::super::TextPlacementError;
use super::{
    super::conversion::usize_to_u64,
    relocation_free::{alignment, block_spans},
};

pub(in crate::text_placement) fn place(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, TextPlacementError> {
    if !fragments.structural_unit_functions.is_empty() {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let section_alignment = match fragments.target.architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 1,
    };
    let mut offsets = BTreeMap::new();
    let mut next_offset = 0_u64;
    for function in &fragments.functions {
        if offsets.insert(function.machine, next_offset).is_some() {
            return Err(TextPlacementError::DuplicateFunction(function.machine));
        }
        alignment::validate(
            fragments.target.architecture,
            next_offset,
            function.byte_count,
        )?;
        if usize_to_u64(function.bytes.len())? != function.byte_count {
            return Err(TextPlacementError::SourceShapeMismatch);
        }
        next_offset = next_offset
            .checked_add(function.byte_count)
            .ok_or(TextPlacementError::OffsetOverflow)?;
    }

    let semantic_entry_offset = *offsets
        .get(&fragments.entry)
        .ok_or(TextPlacementError::MissingSemanticEntry(fragments.entry))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(next_offset).map_err(|_| TextPlacementError::OffsetOverflow)?,
    );
    let mut functions = Vec::with_capacity(fragments.functions.len());
    let mut resolved = Vec::new();
    let mut seen_entry = BTreeSet::new();

    for (source_function_index, function) in fragments.functions.iter().enumerate() {
        if !seen_entry.insert(function.machine) {
            return Err(TextPlacementError::DuplicateFunction(function.machine));
        }
        let section_offset = offsets[&function.machine];
        let mut function_bytes = function.bytes.clone();
        for block in &function.blocks {
            for instruction in &block.instructions {
                let Some(fixup) = instruction.internal_machine_fixup else {
                    continue;
                };
                let FunctionFragmentControlProvenance::DirectInternalCall { callee } =
                    instruction.control
                else {
                    return Err(TextPlacementError::SourceShapeMismatch);
                };
                if callee != fixup.callee {
                    return Err(TextPlacementError::SourceShapeMismatch);
                }
                let operation = exact_operation(&instruction.provenance.operations)?;
                let callee_section_offset = *offsets
                    .get(&callee)
                    .ok_or(TextPlacementError::MissingInternalMachineTarget(callee))?;
                resolved.push(resolve(
                    fragments.target.architecture,
                    function.machine,
                    block.block,
                    instruction.instruction,
                    operation,
                    instruction.offset,
                    &instruction.bytes,
                    section_offset,
                    callee_section_offset,
                    fixup,
                    &mut function_bytes,
                )?);
            }
        }
        let blocks = block_spans::place(fragments.target.architecture, function, section_offset)?;
        bytes.extend_from_slice(&function_bytes);
        functions.push(PlacedFunctionFragment {
            source_function_index: usize_to_u64(source_function_index)?,
            machine: function.machine,
            section_offset,
            byte_count: function.byte_count,
            blocks,
        });
    }

    let source_fixups = fragments
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.internal_machine_fixup.is_some())
        .count();
    if resolved.len() != source_fixups || usize_to_u64(bytes.len())? != next_offset {
        return Err(TextPlacementError::UnresolvedInternalMachineFixups);
    }

    let mut section = RelocationFreeTextSectionPlacement {
        identity: TerminalRelocationFreeTextSectionIdentity::from_canonical_bytes(b"pending"),
        source_fragments: fragments.identity,
        psi: fragments.psi,
        fuel_schedule: fragments.fuel_schedule,
        selected: fragments.selected,
        target: fragments.target,
        semantic_entry: fragments.entry,
        semantic_entry_offset,
        policy: TextSectionPlacementPolicy::DenseValidatedFragmentOrderNoPaddingV1,
        section_alignment,
        byte_count: next_offset,
        bytes,
        functions,
        resolved_internal_machine_calls: resolved,
        relocation_requirements:
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1,
    };
    section.identity = section.recomputed_identity();
    Ok(section)
}

fn exact_operation(operations: &[OperationId]) -> Result<OperationId, TextPlacementError> {
    match operations {
        [operation] => Ok(*operation),
        _ => Err(TextPlacementError::SourceShapeMismatch),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    architecture: Architecture,
    caller: MachineId,
    block: selected_instructions::SelectedBlockId,
    instruction: selected_instructions::SelectedInstructionId,
    operation: OperationId,
    instruction_offset: u64,
    instruction_bytes: &[u8],
    function_section_offset: u64,
    callee_section_offset: u64,
    fixup: FunctionFragmentInternalMachineFixup,
    function_bytes: &mut [u8],
) -> Result<PlacedInternalMachineCallResolution, TextPlacementError> {
    if fixup.state != FunctionFragmentInternalMachineFixupState::UnresolvedZeroFieldV1
        || fixup.opcode_function_offset != instruction_offset
        || fixup.addend != 0
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let call_section_offset = function_section_offset
        .checked_add(instruction_offset)
        .ok_or(TextPlacementError::OffsetOverflow)?;
    let (kind, displacement, patch_bytes) = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            FunctionFragmentInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1,
        ) => {
            if instruction_bytes != [0xe8, 0, 0, 0, 0]
                || fixup.patch_function_offset != instruction_offset + 1
                || fixup.reference_function_offset != instruction_offset + 5
                || fixup.patch_byte_width != 4
            {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            let reference = function_section_offset
                .checked_add(fixup.reference_function_offset)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            let displacement = i32::try_from(
                i128::from(callee_section_offset) - i128::from(reference),
            )
            .map_err(|_| TextPlacementError::InternalCallOutOfRange)?;
            (
                InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
                displacement,
                displacement.to_le_bytes(),
            )
        }
        (
            Architecture::Aarch64,
            FunctionFragmentInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
        ) => {
            if instruction_bytes != 0x9400_0000_u32.to_le_bytes()
                || fixup.patch_function_offset != instruction_offset
                || fixup.reference_function_offset != instruction_offset
                || fixup.patch_byte_width != 4
            {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            let byte_displacement = i128::from(callee_section_offset)
                - i128::from(call_section_offset);
            if byte_displacement % 4 != 0 {
                return Err(TextPlacementError::InternalCallOutOfRange);
            }
            let word_displacement = byte_displacement / 4;
            if !(-(1_i128 << 25)..(1_i128 << 25)).contains(&word_displacement) {
                return Err(TextPlacementError::InternalCallOutOfRange);
            }
            let immediate = u32::try_from(word_displacement.rem_euclid(1_i128 << 26))
                .map_err(|_| TextPlacementError::InternalCallOutOfRange)?;
            let encoded = 0x9400_0000_u32 | immediate;
            (
                InternalMachineCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1,
                i32::try_from(byte_displacement)
                    .map_err(|_| TextPlacementError::InternalCallOutOfRange)?,
                encoded.to_le_bytes(),
            )
        }
        _ => return Err(TextPlacementError::SourceShapeMismatch),
    };

    let patch_start = usize::try_from(fixup.patch_function_offset)
        .map_err(|_| TextPlacementError::OffsetOverflow)?;
    let patch_end = patch_start
        .checked_add(usize::from(fixup.patch_byte_width))
        .ok_or(TextPlacementError::OffsetOverflow)?;
    let instruction_start =
        usize::try_from(instruction_offset).map_err(|_| TextPlacementError::OffsetOverflow)?;
    let instruction_end = instruction_start
        .checked_add(instruction_bytes.len())
        .ok_or(TextPlacementError::OffsetOverflow)?;
    if function_bytes.get(instruction_start..instruction_end) != Some(instruction_bytes) {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    function_bytes
        .get_mut(patch_start..patch_end)
        .ok_or(TextPlacementError::SourceShapeMismatch)?
        .copy_from_slice(&patch_bytes);

    Ok(PlacedInternalMachineCallResolution {
        kind,
        state: InternalMachineCallResolutionState::ResolvedInSectionV1,
        caller,
        block,
        instruction,
        operation,
        callee: fixup.callee,
        call_function_offset: instruction_offset,
        call_section_offset,
        call_byte_count: usize_to_u64(instruction_bytes.len())?,
        opcode_function_offset: fixup.opcode_function_offset,
        opcode_section_offset: function_section_offset
            .checked_add(fixup.opcode_function_offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
        field_function_offset: fixup.patch_function_offset,
        field_section_offset: function_section_offset
            .checked_add(fixup.patch_function_offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
        next_instruction_function_offset: fixup.reference_function_offset,
        next_instruction_section_offset: function_section_offset
            .checked_add(fixup.reference_function_offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
        callee_section_offset,
        field_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
        displacement,
    })
}

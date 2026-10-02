use std::collections::{BTreeMap, BTreeSet};

use machine_code::{
    FunctionFragmentControlProvenance, FunctionFragmentEmissionPlan,
    FunctionFragmentInternalMachineFixup, FunctionFragmentInternalMachineFixupKind,
    FunctionFragmentInternalMachineFixupState, FunctionFragmentNormalizedForeignCallFixup,
    FunctionFragmentNormalizedForeignCallFixupKind,
    FunctionFragmentNormalizedForeignCallFixupState,
};
use machine_code::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
    NormalizedForeignCallResolutionKind, NormalizedForeignCallResolutionState,
    PlacedFunctionFragment, PlacedInternalMachineCallResolution,
    PlacedNormalizedForeignCallResolution, RelocationFreeTextSectionPlacement,
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

pub(in crate::text_placement::placement) fn place(
    fragments: &FunctionFragmentEmissionPlan,
) -> Result<RelocationFreeTextSectionPlacement, TextPlacementError> {
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
    let mut unresolved_foreign = Vec::new();
    let mut unresolved_literals = Vec::new();
    let mut seen_entry = BTreeSet::new();

    for (source_function_index, function) in fragments.functions.iter().enumerate() {
        if !seen_entry.insert(function.machine) {
            return Err(TextPlacementError::DuplicateFunction(function.machine));
        }
        let section_offset = offsets[&function.machine];
        let mut function_bytes = function.bytes.clone();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(fixup) = instruction.internal_machine_fixup {
                    let FunctionFragmentControlProvenance::DirectInternalCall { callee, owner } =
                        instruction.control
                    else {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    };
                    if callee != fixup.callee {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                    match owner {
                        target_operations::CallSiteOwner::Operation(operation)
                            if exact_operation(&instruction.provenance.operations)?
                                == operation => {}
                        target_operations::CallSiteOwner::CleanupAction { edge, .. }
                            if instruction.provenance.operations.is_empty()
                                && instruction.provenance.edges.as_slice() == [edge]
                                && instruction.provenance.fuel.is_empty() => {}
                        _ => return Err(TextPlacementError::SourceShapeMismatch),
                    }
                    let callee_section_offset = *offsets
                        .get(&callee)
                        .ok_or(TextPlacementError::MissingInternalMachineTarget(callee))?;
                    resolved.push(resolve(
                        fragments.target.architecture,
                        function.machine,
                        block.block,
                        instruction.instruction,
                        owner,
                        instruction.offset,
                        &instruction.bytes,
                        section_offset,
                        callee_section_offset,
                        fixup,
                        &mut function_bytes,
                    )?);
                }
                if let Some(fixup) = instruction.normalized_foreign_call_fixup.clone() {
                    let FunctionFragmentControlProvenance::NormalizedForeignCall {
                        boundary,
                        ordinal,
                    } = instruction.control
                    else {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    };
                    if boundary != fixup.boundary || ordinal != fixup.ordinal {
                        return Err(TextPlacementError::SourceShapeMismatch);
                    }
                    let operation = exact_operation(&instruction.provenance.operations)?;
                    unresolved_foreign.push(resolve_foreign(
                        fragments.target.architecture,
                        function.machine,
                        block.block,
                        instruction.instruction,
                        operation,
                        instruction.offset,
                        &instruction.bytes,
                        section_offset,
                        fixup,
                        &function_bytes,
                    )?);
                }
                if let Some(fixup) = instruction.byte_literal_address_fixup {
                    unresolved_literals.push(place_byte_literal_address(
                        function.machine,
                        block.block,
                        instruction,
                        section_offset,
                        fixup,
                        &function_bytes,
                    )?);
                }
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
    let source_foreign = fragments
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.normalized_foreign_call_fixup.is_some())
        .count();
    let source_literals = fragments
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter(|instruction| instruction.byte_literal_address_fixup.is_some())
        .count();
    if resolved.len() != source_fixups
        || unresolved_foreign.len() != source_foreign
        || unresolved_literals.len() != source_literals
        || usize_to_u64(bytes.len())? != next_offset
    {
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
        relocation_requirements: if !unresolved_literals.is_empty() {
            TextSectionRelocationRequirements::UnresolvedObjectSymbolFieldsV1
        } else if unresolved_foreign.is_empty() {
            TextSectionRelocationRequirements::ProvenNoneForFullyResolvedInternalControlV1
        } else {
            TextSectionRelocationRequirements::UnresolvedNormalizedForeignImportFieldsV1
        },
        unresolved_normalized_foreign_calls: unresolved_foreign,
        unresolved_byte_literal_addresses: unresolved_literals,
    };
    section.identity = section.recomputed_identity();
    Ok(section)
}

/// Keep a literal address field unresolved: record its section coordinates
/// and require the placed bytes to still hold the canonical zero field.
fn place_byte_literal_address(
    machine: semantic_vocabulary::MachineId,
    block: selected_instructions::SelectedBlockId,
    instruction: &machine_code::FunctionFragmentInstructionSpan,
    section_offset: u64,
    fixup: machine_code::FunctionFragmentByteLiteralAddressFixup,
    function_bytes: &[u8],
) -> Result<machine_code::PlacedByteLiteralAddress, TextPlacementError> {
    let instruction_end = instruction
        .offset
        .checked_add(usize_to_u64(instruction.bytes.len())?)
        .ok_or(TextPlacementError::OffsetOverflow)?;
    // The four field bytes, when they lie inside this instruction's span.
    let field = |offset: u64| {
        if offset < instruction.offset || offset.checked_add(4)? > instruction_end {
            return None;
        }
        let start = usize::try_from(offset).ok()?;
        function_bytes.get(start..start.checked_add(4)?)
    };
    let canonical = match fixup.fields {
        machine_code::ByteLiteralAddressFields::X86_64Relative32 { patch_offset } => {
            field(patch_offset) == Some(&[0, 0, 0, 0][..])
        }
        machine_code::ByteLiteralAddressFields::Aarch64PageAddress {
            page_patch_offset,
            page_offset_patch_offset,
        } => {
            // `adrp` with zero immlo/immhi, then `add` with a zero imm12.
            let word = |offset| {
                field(offset)
                    .map(|field| u32::from_le_bytes([field[0], field[1], field[2], field[3]]))
            };
            word(page_patch_offset).is_some_and(|word| word & 0x9fff_ffe0 == 0x9000_0000)
                && word(page_offset_patch_offset)
                    .is_some_and(|word| word & 0xffff_fc00 == 0x9100_0000)
        }
    };
    if !canonical || instruction.branch.is_some() {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    Ok(machine_code::PlacedByteLiteralAddress {
        machine,
        block,
        instruction: instruction.instruction,
        literal: fixup.literal,
        instruction_function_offset: instruction.offset,
        instruction_section_offset: section_offset
            .checked_add(instruction.offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
        instruction_byte_count: usize_to_u64(instruction.bytes.len())?,
        fields: fixup
            .fields
            .shifted(section_offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
    })
}

fn exact_operation(operations: &[OperationId]) -> Result<OperationId, TextPlacementError> {
    // Selection replay can prefix zero-payload constructors to this call.
    // The call itself remains the final operation; this placement phase only
    // resolves its fixup, and does not grant authority to the preceding charges.
    operations
        .last()
        .copied()
        .ok_or(TextPlacementError::SourceShapeMismatch)
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    architecture: Architecture,
    caller: MachineId,
    block: selected_instructions::SelectedBlockId,
    instruction: selected_instructions::SelectedInstructionId,
    owner: target_operations::CallSiteOwner,
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
                return Err(TextPlacementError::SourceShapeMismatch)
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
                return Err(TextPlacementError::SourceShapeMismatch)
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
        owner,
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

/// The foreign call keeps its canonical zero placeholder bytes in the section;
/// the import field is object-construction custody, never an in-section patch.
#[allow(clippy::too_many_arguments)]
fn resolve_foreign(
    architecture: Architecture,
    caller: MachineId,
    block: selected_instructions::SelectedBlockId,
    instruction: selected_instructions::SelectedInstructionId,
    operation: OperationId,
    instruction_offset: u64,
    instruction_bytes: &[u8],
    function_section_offset: u64,
    fixup: FunctionFragmentNormalizedForeignCallFixup,
    function_bytes: &[u8],
) -> Result<PlacedNormalizedForeignCallResolution, TextPlacementError> {
    if fixup.state != FunctionFragmentNormalizedForeignCallFixupState::UnresolvedImportFieldV1
        || fixup.opcode_function_offset < instruction_offset
        || fixup.addend != 0
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let call_section_offset = function_section_offset
        .checked_add(instruction_offset)
        .ok_or(TextPlacementError::OffsetOverflow)?;
    // Every callback materialization group precedes the call word in roster
    // order; `cursor` tracks the running offset inside the row's bytes while
    // each group is checked against its destination's exact byte shape.
    // `patch_offsets` are the group's byte offsets whose fields stay
    // unresolved for object construction.
    let mut cursor = 0_u64;
    let mut check_group =
        |callback: &machine_code::FunctionFragmentCallbackMaterialization,
         expected: &[u8],
         patch_offsets: &[u64]|
         -> Result<(), TextPlacementError> {
            let group_start = instruction_offset
                .checked_add(cursor)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            let group_end = cursor
                .checked_add(expected.len() as u64)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            let bytes_match = usize::try_from(group_end)
                .ok()
                .and_then(|end| instruction_bytes.get(cursor as usize..end));
            if callback.code_function_offset != group_start
                || callback.code_byte_count != expected.len() as u64
                || bytes_match != Some(expected)
            {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            let absolute: Vec<u64> = match callback.encoding {
            machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                patch_function_offset,
            } => vec![patch_function_offset],
            machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                page_patch_function_offset,
                page_offset_patch_function_offset,
            } => vec![page_patch_function_offset, page_offset_patch_function_offset],
        };
            if absolute.len() != patch_offsets.len()
                || !absolute
                    .iter()
                    .zip(patch_offsets)
                    .all(|(absolute, relative)| *absolute == group_start + relative)
            {
                return Err(TextPlacementError::SourceShapeMismatch);
            }
            cursor = group_end;
            Ok(())
        };
    let kind = match (architecture, fixup.kind) {
        (
            Architecture::X86_64,
            FunctionFragmentNormalizedForeignCallFixupKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1,
        ) => {
            for callback in &fixup.callbacks {
                let machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 { .. } =
                    callback.encoding
                else {
                    return Err(TextPlacementError::SourceShapeMismatch)
                };
                let bytes: Vec<u8> = match callback.destination {
                    machine_code::CallbackAddressDestination::Register(register) => {
                        if register.codec_index() >= 16 {
                            return Err(TextPlacementError::SourceShapeMismatch)
                        }
                        let index = register.codec_index();
                        if index == 4 {
                            return Err(TextPlacementError::SourceShapeMismatch)
                        }
                        let rex = 0x48_u8 | u8::from(index >= 8);
                        let modrm = 0x05_u8 | ((index & 7) << 3);
                        vec![rex, 0x8d, modrm, 0, 0, 0, 0]
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        byte_offset,
                    } => {
                        let root = root_register.codec_index();
                        if root >= 16 || root == 4 || root == 11 {
                            return Err(TextPlacementError::SourceShapeMismatch)
                        }
                        let off = byte_offset.to_le_bytes();
                        let rex_store = 0x4c_u8 | u8::from(root >= 8);
                        vec![
                            0x4c, 0x8d, 0x1d, 0, 0, 0, 0, rex_store, 0x89,
                            0x98 | (root & 7), off[0], off[1], off[2], off[3],
                        ]
                    }
                    machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                        return Err(TextPlacementError::SourceShapeMismatch)
                    }
                };
                check_group(callback, &bytes, &[3])?;
            }
            let call_offset = instruction_offset
                .checked_add(cursor)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            if instruction_bytes.get(cursor as usize..)
                != Some(&[0xe8_u8, 0, 0, 0, 0][..])
                || fixup.opcode_function_offset != call_offset
                || fixup.patch_function_offset != call_offset + 1
                || fixup.reference_function_offset != call_offset + 5
                || fixup.patch_byte_width != 4
            {
                return Err(TextPlacementError::SourceShapeMismatch)
            }
            NormalizedForeignCallResolutionKind::X86Relative32FromNextInstructionToNormalizedForeignImportV1
        }
        (
            Architecture::Aarch64,
            FunctionFragmentNormalizedForeignCallFixupKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1,
        ) => {
            for callback in &fixup.callbacks {
                let machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress { .. } =
                    callback.encoding
                else {
                    return Err(TextPlacementError::SourceShapeMismatch)
                };
                let bytes: Vec<u8> = match callback.destination {
                    machine_code::CallbackAddressDestination::Register(register) => {
                        let calling_conventions::MachineRegister::Aarch64X(index) = register
                        else {
                            return Err(TextPlacementError::SourceShapeMismatch)
                        };
                        if index > 30 {
                            return Err(TextPlacementError::SourceShapeMismatch)
                        }
                        let adrp = (0x9000_0000_u32 | u32::from(index)).to_le_bytes();
                        let add =
                            (0x9100_0000_u32 | (u32::from(index) << 5) | u32::from(index))
                                .to_le_bytes();
                        vec![
                            adrp[0], adrp[1], adrp[2], adrp[3], add[0], add[1], add[2], add[3],
                        ]
                    }
                    machine_code::CallbackAddressDestination::RecordField {
                        root_register,
                        byte_offset,
                    } => {
                        let calling_conventions::MachineRegister::Aarch64X(root) = root_register
                        else {
                            return Err(TextPlacementError::SourceShapeMismatch)
                        };
                        if root > 30 || byte_offset % 8 != 0 || byte_offset / 8 >= 4096 {
                            return Err(TextPlacementError::SourceShapeMismatch)
                        }
                        let scratch = 16_u32;
                        let adrp = (0x9000_0000_u32 | scratch).to_le_bytes();
                        let add = (0x9100_0000_u32 | (scratch << 5) | scratch).to_le_bytes();
                        let store = (0xF900_0000_u32
                            | ((byte_offset / 8) << 10)
                            | (u32::from(root) << 5)
                            | scratch)
                            .to_le_bytes();
                        vec![
                            adrp[0], adrp[1], adrp[2], adrp[3], add[0], add[1], add[2], add[3],
                            store[0], store[1], store[2], store[3],
                        ]
                    }
                    machine_code::CallbackAddressDestination::OutgoingStack { .. } => {
                        return Err(TextPlacementError::SourceShapeMismatch)
                    }
                };
                check_group(callback, &bytes, &[0, 4])?;
            }
            let call_offset = instruction_offset
                .checked_add(cursor)
                .ok_or(TextPlacementError::OffsetOverflow)?;
            if instruction_bytes.get(cursor as usize..)
                != Some(&0x9400_0000_u32.to_le_bytes()[..])
                || fixup.opcode_function_offset != call_offset
                || fixup.patch_function_offset != call_offset
                || fixup.reference_function_offset != call_offset
                || fixup.patch_byte_width != 4
            {
                return Err(TextPlacementError::SourceShapeMismatch)
            }
            NormalizedForeignCallResolutionKind::Aarch64BranchLinkImmediate26FromInstructionToNormalizedForeignImportV1
        }
        _ => return Err(TextPlacementError::SourceShapeMismatch),
    };

    let instruction_start =
        usize::try_from(instruction_offset).map_err(|_| TextPlacementError::OffsetOverflow)?;
    let instruction_end = instruction_start
        .checked_add(instruction_bytes.len())
        .ok_or(TextPlacementError::OffsetOverflow)?;
    if function_bytes.get(instruction_start..instruction_end) != Some(instruction_bytes) {
        return Err(TextPlacementError::SourceShapeMismatch);
    }

    Ok(PlacedNormalizedForeignCallResolution {
        kind,
        state: NormalizedForeignCallResolutionState::UnresolvedImportFieldV1,
        caller,
        block,
        instruction,
        operation,
        boundary: fixup.boundary,
        ordinal: fixup.ordinal,
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
        field_byte_width: fixup.patch_byte_width,
        addend: fixup.addend,
        callback_materializations: fixup
            .callbacks
            .iter()
            .map(|callback| {
                Ok(machine_code::PlacedCallbackAddressMaterialization {
                    destination: callback.destination,
                    code_function_offset: callback.code_function_offset,
                    code_section_offset: function_section_offset
                        .checked_add(callback.code_function_offset)
                        .ok_or(TextPlacementError::OffsetOverflow)?,
                    code_byte_count: callback.code_byte_count,
                    encoding: match callback.encoding {
                        machine_code::FunctionFragmentCallbackMaterializationEncoding::X86_64Relative32 {
                            patch_function_offset,
                        } => machine_code::PlacedCallbackAddressEncoding::X86_64Relative32 {
                            relocation_function_offset: patch_function_offset,
                            relocation_section_offset: function_section_offset
                                .checked_add(patch_function_offset)
                                .ok_or(TextPlacementError::OffsetOverflow)?,
                        },
                        machine_code::FunctionFragmentCallbackMaterializationEncoding::Aarch64PageAddress {
                            page_patch_function_offset,
                            page_offset_patch_function_offset,
                        } => machine_code::PlacedCallbackAddressEncoding::Aarch64PageAddress {
                            page_relocation_function_offset: page_patch_function_offset,
                            page_relocation_section_offset: function_section_offset
                                .checked_add(page_patch_function_offset)
                                .ok_or(TextPlacementError::OffsetOverflow)?,
                            page_offset_relocation_function_offset:
                                page_offset_patch_function_offset,
                            page_offset_relocation_section_offset: function_section_offset
                                .checked_add(page_offset_patch_function_offset)
                                .ok_or(TextPlacementError::OffsetOverflow)?,
                        },
                    },
                })
            })
            .collect::<Result<Vec<_>, TextPlacementError>>()?,
    })
}

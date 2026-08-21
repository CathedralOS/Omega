//! Receiver-free physical operand images for the generated entry wrapper.
//!
//! This module binds each exact logical `Extent` value to its retained
//! Microsoft x64 indirect placement and produces the bytes for the required
//! caller-owned copy. The enclosing carrier keeps the installed authority
//! alive. It does not allocate or write stack storage, populate pointer
//! registers, emit a wrapper or call, or claim native execution.

use super::{
    ProgramEntrySourceExtentFieldRole, ProgramStorageEntryDiagnostic,
    ProgramStorageEntryExtentLogicalValue, ProgramStorageEntryRootRole,
    ProgramStorageEntryWholeRootLogicalValueCarrier,
};
use omega_calling_conventions::{
    IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement, ValueShape,
};
use std::ops::Range;

const EXTENT_BYTE_SIZE: u16 = 16;
const EXTENT_ALIGNMENT: u16 = 8;
const CALLER_COPY_ALIGNMENT: u32 = 16;

/// One exact logical `Extent` image and its address-free indirect ABI
/// destination. The bytes are geometry only; authority remains in the
/// enclosing logical-value carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryExtentOperandImage {
    role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    call_parameter_index: usize,
    bytes: [u8; 16],
    placement: ValuePlacement,
    pointer: IndirectPointerLocation,
    caller_copy_stack_byte_offset: u32,
    byte_size: u16,
    alignment: u16,
}

impl ProgramStorageEntryExtentOperandImage {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn visible_parameter_index(&self) -> usize {
        self.visible_parameter_index
    }

    pub const fn call_parameter_index(&self) -> usize {
        self.call_parameter_index
    }

    pub const fn bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    pub const fn placement(&self) -> &ValuePlacement {
        &self.placement
    }

    pub const fn pointer(&self) -> IndirectPointerLocation {
        self.pointer
    }

    pub const fn caller_copy_stack_byte_offset(&self) -> u32 {
        self.caller_copy_stack_byte_offset
    }

    pub const fn caller_copy_byte_range(&self) -> Range<u32> {
        self.caller_copy_stack_byte_offset
            ..self.caller_copy_stack_byte_offset + self.byte_size as u32
    }

    pub const fn byte_size(&self) -> u16 {
        self.byte_size
    }

    pub const fn alignment(&self) -> u16 {
        self.alignment
    }
}

/// The two receiver-free operand images, inseparable from their logical values
/// and the whole installed root authorities beneath them.
#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootOperandCarrier {
    logical_values: ProgramStorageEntryWholeRootLogicalValueCarrier,
    operands: [ProgramStorageEntryExtentOperandImage; 2],
}

impl ProgramStorageEntryWholeRootOperandCarrier {
    pub const fn logical_values(&self) -> &ProgramStorageEntryWholeRootLogicalValueCarrier {
        &self.logical_values
    }

    pub const fn operands(&self) -> &[ProgramStorageEntryExtentOperandImage; 2] {
        &self.operands
    }

    pub fn into_logical_values(self) -> ProgramStorageEntryWholeRootLogicalValueCarrier {
        self.logical_values
    }
}

/// Bind receiver-free logical root values to exact indirect Microsoft x64
/// operand images. Rejection returns the intact authority-bearing input.
pub fn bind_program_storage_entry_whole_root_operands(
    logical_values: ProgramStorageEntryWholeRootLogicalValueCarrier,
) -> Result<ProgramStorageEntryWholeRootOperandCarrier, ProgramStorageEntryWholeRootOperandError> {
    match validate_operands(&logical_values) {
        Ok(operands) => Ok(ProgramStorageEntryWholeRootOperandCarrier {
            logical_values,
            operands,
        }),
        Err(diagnostic) => Err(ProgramStorageEntryWholeRootOperandError {
            logical_values,
            diagnostic,
        }),
    }
}

fn validate_operands(
    carrier: &ProgramStorageEntryWholeRootLogicalValueCarrier,
) -> Result<[ProgramStorageEntryExtentOperandImage; 2], ProgramStorageEntryDiagnostic> {
    if carrier.arguments().target() != omega_target::NativeTarget::uefi_x64() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage operand images require the exact UEFI x86-64 target".into(),
        ));
    }

    let expected = [
        (
            ProgramStorageEntryRootRole::Image,
            IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            32,
        ),
        (
            ProgramStorageEntryRootRole::InitialStorage,
            IndirectPointerLocation::Register(MachineRegister::X86Rdx),
            48,
        ),
    ];
    let mut operands = Vec::with_capacity(2);
    for (index, ((expected_role, expected_pointer, expected_copy_offset), (value, argument))) in
        expected
            .into_iter()
            .zip(carrier.values().iter().zip(carrier.arguments().arguments()))
            .enumerate()
    {
        if value.role() != expected_role
            || argument.role() != expected_role
            || value.visible_parameter_index() != index
            || argument.visible_parameter_index() != index
            || value.call_parameter_index() != index
            || argument.call_parameter_index() != index
            || value.layout() != argument.extent_value_layout()
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage {expected_role:?} operand identity drifted from its logical value and continuation argument"
            )));
        }
        let (pointer, copy_offset, byte_size, alignment) = validate_indirect_placement(
            argument.placement(),
            expected_pointer,
            expected_copy_offset,
        )?;
        let bytes = encode_extent(value)?;
        operands.push(ProgramStorageEntryExtentOperandImage {
            role: expected_role,
            visible_parameter_index: index,
            call_parameter_index: index,
            bytes,
            placement: argument.placement().clone(),
            pointer,
            caller_copy_stack_byte_offset: copy_offset,
            byte_size,
            alignment,
        });
    }
    let operands: [ProgramStorageEntryExtentOperandImage; 2] =
        operands.try_into().map_err(|_| {
            ProgramStorageEntryDiagnostic(
                "program-storage operand carrier lost its exact two root rows".into(),
            )
        })?;
    validate_copy_ranges(&operands)?;
    Ok(operands)
}

fn validate_indirect_placement(
    placement: &ValuePlacement,
    expected_pointer: IndirectPointerLocation,
    expected_copy_offset: u32,
) -> Result<(IndirectPointerLocation, u32, u16, u16), ProgramStorageEntryDiagnostic> {
    let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset: Some(copy_offset),
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage Extent operand requires one indirect placement with a caller-owned copy"
                .into(),
        ));
    };
    if placement.shape != ValueShape::integer(EXTENT_BYTE_SIZE, EXTENT_ALIGNMENT)
        || *pointer != expected_pointer
        || *copy_offset != expected_copy_offset
        || *byte_size != EXTENT_BYTE_SIZE
        || *alignment != EXTENT_ALIGNMENT
        || copy_offset % CALLER_COPY_ALIGNMENT != 0
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage Extent operand drifted from its exact Microsoft x64 indirect placement"
                .into(),
        ));
    }
    copy_offset
        .checked_add(u32::from(*byte_size))
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "program-storage Extent caller-copy range wraps the stack-offset domain".into(),
            )
        })?;
    Ok((*pointer, *copy_offset, *byte_size, *alignment))
}

fn encode_extent(
    value: &ProgramStorageEntryExtentLogicalValue,
) -> Result<[u8; 16], ProgramStorageEntryDiagnostic> {
    let [base, length] = value.layout().fields();
    let scalar = ValueShape::integer(8, 8);
    if value.layout().shape() != ValueShape::integer(EXTENT_BYTE_SIZE, EXTENT_ALIGNMENT)
        || base.role() != ProgramEntrySourceExtentFieldRole::Base
        || base.byte_offset() != 0
        || base.shape() != scalar
        || length.role() != ProgramEntrySourceExtentFieldRole::Length
        || length.byte_offset() != 8
        || length.shape() != scalar
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage Extent operand cannot encode a drifted field graph".into(),
        ));
    }
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&value.base().to_le_bytes());
    bytes[8..].copy_from_slice(&value.length().to_le_bytes());
    Ok(bytes)
}

fn validate_copy_ranges(
    operands: &[ProgramStorageEntryExtentOperandImage; 2],
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let [image, initial_storage] = operands;
    let image_range = image.caller_copy_byte_range();
    let storage_range = initial_storage.caller_copy_byte_range();
    if image_range != (32..48) || storage_range != (48..64) || image_range.end > storage_range.start
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage Extent caller-owned copies overlap or drift from their exact Microsoft x64 ranges"
                .into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct ProgramStorageEntryWholeRootOperandError {
    logical_values: ProgramStorageEntryWholeRootLogicalValueCarrier,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryWholeRootOperandError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_logical_values(self) -> ProgramStorageEntryWholeRootLogicalValueCarrier {
        self.logical_values
    }
}

impl std::fmt::Display for ProgramStorageEntryWholeRootOperandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryWholeRootOperandError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(pointer: IndirectPointerLocation, copy_offset: Option<u32>) -> ValuePlacement {
        ValuePlacement {
            shape: ValueShape::integer(16, 8),
            locations: vec![ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset: copy_offset,
                byte_size: 16,
                alignment: 8,
            }],
        }
    }

    #[test]
    fn exact_microsoft_x64_indirect_extent_placement_is_admitted() {
        let placement = placement(
            IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            Some(32),
        );
        assert_eq!(
            validate_indirect_placement(
                &placement,
                IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                32,
            )
            .unwrap(),
            (
                IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                32,
                16,
                8,
            )
        );
    }

    #[test]
    fn missing_copy_or_redirected_pointer_rejects() {
        let missing = placement(
            IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            None,
        );
        assert!(
            validate_indirect_placement(
                &missing,
                IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                32,
            )
            .is_err()
        );
        let redirected = placement(
            IndirectPointerLocation::Register(MachineRegister::X86Rdx),
            Some(32),
        );
        assert!(
            validate_indirect_placement(
                &redirected,
                IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                32,
            )
            .is_err()
        );
    }

    #[test]
    fn copy_offset_size_and_alignment_drift_reject() {
        let mut drifted = placement(
            IndirectPointerLocation::Register(MachineRegister::X86Rcx),
            Some(40),
        );
        assert!(
            validate_indirect_placement(
                &drifted,
                IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                32,
            )
            .is_err()
        );
        let ValueLocation::Indirect {
            byte_size,
            alignment,
            ..
        } = &mut drifted.locations[0]
        else {
            unreachable!()
        };
        *byte_size = 8;
        *alignment = 4;
        assert!(
            validate_indirect_placement(
                &drifted,
                IndirectPointerLocation::Register(MachineRegister::X86Rcx),
                32,
            )
            .is_err()
        );
    }
}

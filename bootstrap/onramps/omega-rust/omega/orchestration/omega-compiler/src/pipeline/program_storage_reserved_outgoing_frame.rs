//! Sealed planning authority for the receiver-free wrapper's outgoing frame.
//!
//! This carrier proves only that the retained caller-frame recipe authorizes
//! four exact word writes. It does not claim that RSP has changed or that any
//! machine instruction, wrapper, or call exists.

use super::{
    ProgramEntrySourceExtentFieldRole, ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole,
    ProgramStorageEntryWrapperCallerFramePlan, ProgramStorageEntryWrapperCallerFrameStep,
};
use std::ops::Range;

const FRAME_BYTE_COUNT: u32 = 72;
const SHADOW_BYTE_COUNT: u32 = 32;
const WORD_BYTE_COUNT: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryOutgoingStackWord {
    role: ProgramStorageEntryRootRole,
    visible_parameter_index: usize,
    call_parameter_index: usize,
    field: ProgramEntrySourceExtentFieldRole,
    operand_byte_offset: u16,
    stack_byte_offset: u32,
    bytes: [u8; 8],
    value: u64,
}

impl ProgramStorageEntryOutgoingStackWord {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }
    pub const fn visible_parameter_index(&self) -> usize {
        self.visible_parameter_index
    }
    pub const fn call_parameter_index(&self) -> usize {
        self.call_parameter_index
    }
    pub const fn field(&self) -> ProgramEntrySourceExtentFieldRole {
        self.field
    }
    pub const fn operand_byte_offset(&self) -> u16 {
        self.operand_byte_offset
    }
    pub const fn stack_byte_offset(&self) -> u32 {
        self.stack_byte_offset
    }
    pub const fn bytes(&self) -> &[u8; 8] {
        &self.bytes
    }
    pub const fn value(&self) -> u64 {
        self.value
    }
}

#[derive(Debug)]
pub struct ProgramStorageEntryReservedOutgoingStackFramePlan {
    caller_frame: ProgramStorageEntryWrapperCallerFramePlan,
    words: [ProgramStorageEntryOutgoingStackWord; 4],
}

impl ProgramStorageEntryReservedOutgoingStackFramePlan {
    pub const fn caller_frame(&self) -> &ProgramStorageEntryWrapperCallerFramePlan {
        &self.caller_frame
    }

    pub const fn frame_byte_count(&self) -> u32 {
        FRAME_BYTE_COUNT
    }
    pub const fn shadow_byte_range(&self) -> Range<u32> {
        0..SHADOW_BYTE_COUNT
    }
    pub const fn image_writable_byte_range(&self) -> Range<u32> {
        32..48
    }
    pub const fn initial_storage_writable_byte_range(&self) -> Range<u32> {
        48..64
    }
    pub const fn words(&self) -> &[ProgramStorageEntryOutgoingStackWord; 4] {
        &self.words
    }

    pub fn into_caller_frame(self) -> ProgramStorageEntryWrapperCallerFramePlan {
        self.caller_frame
    }
}

pub fn reserve_program_storage_entry_outgoing_stack_frame(
    caller_frame: ProgramStorageEntryWrapperCallerFramePlan,
) -> Result<
    ProgramStorageEntryReservedOutgoingStackFramePlan,
    ProgramStorageEntryReservedOutgoingStackFrameError,
> {
    match derive_words(&caller_frame) {
        Ok(words) => Ok(ProgramStorageEntryReservedOutgoingStackFramePlan {
            caller_frame,
            words,
        }),
        Err(diagnostic) => Err(ProgramStorageEntryReservedOutgoingStackFrameError {
            caller_frame,
            diagnostic,
        }),
    }
}

fn derive_words(
    caller_frame: &ProgramStorageEntryWrapperCallerFramePlan,
) -> Result<[ProgramStorageEntryOutgoingStackWord; 4], ProgramStorageEntryDiagnostic> {
    if caller_frame.shadow_byte_count() != SHADOW_BYTE_COUNT
        || caller_frame.outgoing_reservation_byte_count() != FRAME_BYTE_COUNT
        || caller_frame.outgoing_release_byte_count() != FRAME_BYTE_COUNT
        || caller_frame.pre_call_stack_alignment() != 16
    {
        return Err(ProgramStorageEntryDiagnostic(
            "reserved outgoing frame lost its exact balanced 72-byte Microsoft x64 geometry".into(),
        ));
    }
    let expected = [
        (
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Base,
            0,
            32,
        ),
        (
            ProgramStorageEntryRootRole::Image,
            0,
            ProgramEntrySourceExtentFieldRole::Length,
            8,
            40,
        ),
        (
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Base,
            0,
            48,
        ),
        (
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            ProgramEntrySourceExtentFieldRole::Length,
            8,
            56,
        ),
    ];
    let mut words = Vec::with_capacity(4);
    for (index, expected) in expected.into_iter().enumerate() {
        let Some(ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord {
            role,
            visible_parameter_index,
            call_parameter_index,
            field,
            operand_byte_offset,
            stack_byte_offset,
            bytes,
        }) = caller_frame.steps().get(index)
        else {
            return Err(ProgramStorageEntryDiagnostic(
                "reserved outgoing frame lost one of its four ordered Extent words".into(),
            ));
        };
        let (
            expected_role,
            expected_index,
            expected_field,
            expected_operand_offset,
            expected_stack_offset,
        ) = expected;
        let end = stack_byte_offset.checked_add(WORD_BYTE_COUNT);
        if *role != expected_role
            || *visible_parameter_index != expected_index
            || *call_parameter_index != expected_index
            || *field != expected_field
            || *operand_byte_offset != expected_operand_offset
            || *stack_byte_offset != expected_stack_offset
            || *stack_byte_offset < SHADOW_BYTE_COUNT
            || *stack_byte_offset % WORD_BYTE_COUNT != 0
            || end.is_none_or(|end| end > 64)
        {
            return Err(ProgramStorageEntryDiagnostic(
                "reserved outgoing frame word identity, order, or writable range drifted".into(),
            ));
        }
        let value = u64::from_le_bytes(*bytes);
        if value.to_le_bytes() != *bytes {
            return Err(ProgramStorageEntryDiagnostic(
                "reserved outgoing frame word lost its exact little-endian image".into(),
            ));
        }
        words.push(ProgramStorageEntryOutgoingStackWord {
            role: *role,
            visible_parameter_index: *visible_parameter_index,
            call_parameter_index: *call_parameter_index,
            field: *field,
            operand_byte_offset: *operand_byte_offset,
            stack_byte_offset: *stack_byte_offset,
            bytes: *bytes,
            value,
        });
    }
    for (step, (expected_role, expected_index, expected_register, expected_offset)) in
        caller_frame.steps()[4..].iter().zip([
            (
                ProgramStorageEntryRootRole::Image,
                0,
                omega_calling_conventions::MachineRegister::X86Rcx,
                32,
            ),
            (
                ProgramStorageEntryRootRole::InitialStorage,
                1,
                omega_calling_conventions::MachineRegister::X86Rdx,
                48,
            ),
        ])
    {
        let ProgramStorageEntryWrapperCallerFrameStep::BindCallerCopyAddress {
            role,
            visible_parameter_index,
            call_parameter_index,
            register,
            caller_copy_stack_byte_offset,
            caller_copy_byte_count,
            caller_copy_alignment,
        } = step
        else {
            return Err(ProgramStorageEntryDiagnostic(
                "reserved outgoing frame lost an exact caller-copy address binding".into(),
            ));
        };
        if *role != expected_role
            || *visible_parameter_index != expected_index
            || *call_parameter_index != expected_index
            || *register != expected_register
            || *caller_copy_stack_byte_offset != expected_offset
            || *caller_copy_byte_count != 16
            || *caller_copy_alignment != 8
        {
            return Err(ProgramStorageEntryDiagnostic(
                "reserved outgoing frame caller-copy address binding drifted".into(),
            ));
        }
    }
    words.try_into().map_err(|_| {
        ProgramStorageEntryDiagnostic(
            "reserved outgoing frame did not retain exactly four words".into(),
        )
    })
}

#[derive(Debug)]
pub struct ProgramStorageEntryReservedOutgoingStackFrameError {
    caller_frame: ProgramStorageEntryWrapperCallerFramePlan,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryReservedOutgoingStackFrameError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }
    pub fn into_caller_frame(self) -> ProgramStorageEntryWrapperCallerFramePlan {
        self.caller_frame
    }
}

impl std::fmt::Display for ProgramStorageEntryReservedOutgoingStackFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryReservedOutgoingStackFrameError {}

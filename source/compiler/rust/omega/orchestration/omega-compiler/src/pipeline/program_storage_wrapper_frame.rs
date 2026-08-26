//! Planning-only caller frame for a future generated program-storage wrapper.
//!
//! This module derives the exact balanced Microsoft x64 outgoing reservation,
//! caller-copy writes, and pointer-address bindings from the sealed operand
//! carrier. Its steps are immutable recipe rows. They do not mutate the stack,
//! write bytes, change registers, insert a wrapper, emit a call or relocation,
//! or prove native execution.

use super::{
    ProgramEntrySourceExtentFieldRole, ProgramLocalStorageCustody, ProgramLocalStorageCustodyError,
    ProgramStorageEntryDiagnostic, ProgramStorageEntryRootRole,
    ProgramStorageEntryWholeRootOperandCarrier,
};
use omega_calling_conventions::{IndirectPointerLocation, MachineRegister};

const SHADOW_BYTE_COUNT: u32 = 32;
const OUTGOING_RESERVATION_BYTE_COUNT: u32 = 72;
const PRE_CALL_STACK_ALIGNMENT: u32 = 16;
const INCOMING_STACK_ALIGNMENT_REMAINDER: u32 = 8;
const EXTENT_WORD_BYTE_COUNT: u16 = 8;

/// One ordered recipe row in the future wrapper's caller-frame preparation.
/// Rows describe intended storage/register destinations without realizing
/// either destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramStorageEntryWrapperCallerFrameStep {
    WriteExtentWord {
        role: ProgramStorageEntryRootRole,
        visible_parameter_index: usize,
        call_parameter_index: usize,
        field: ProgramEntrySourceExtentFieldRole,
        operand_byte_offset: u16,
        stack_byte_offset: u32,
        bytes: [u8; 8],
    },
    BindCallerCopyAddress {
        role: ProgramStorageEntryRootRole,
        visible_parameter_index: usize,
        call_parameter_index: usize,
        register: MachineRegister,
        caller_copy_stack_byte_offset: u32,
        caller_copy_byte_count: u16,
        caller_copy_alignment: u16,
    },
}

/// Exact address-free pre/post-call frame recipe for the currently admitted
/// receiver-free UEFI/Microsoft x64 continuation ABI.
#[derive(Debug)]
pub struct ProgramStorageEntryWrapperCallerFramePlan {
    operands: ProgramStorageEntryWholeRootOperandCarrier,
    shadow_byte_count: u32,
    outgoing_reservation_byte_count: u32,
    outgoing_release_byte_count: u32,
    pre_call_stack_alignment: u32,
    steps: [ProgramStorageEntryWrapperCallerFrameStep; 6],
}

impl ProgramStorageEntryWrapperCallerFramePlan {
    pub const fn operands(&self) -> &ProgramStorageEntryWholeRootOperandCarrier {
        &self.operands
    }

    pub const fn shadow_byte_count(&self) -> u32 {
        self.shadow_byte_count
    }

    pub const fn outgoing_reservation_byte_count(&self) -> u32 {
        self.outgoing_reservation_byte_count
    }

    pub const fn outgoing_release_byte_count(&self) -> u32 {
        self.outgoing_release_byte_count
    }

    pub const fn pre_call_stack_alignment(&self) -> u32 {
        self.pre_call_stack_alignment
    }

    pub const fn steps(&self) -> &[ProgramStorageEntryWrapperCallerFrameStep; 6] {
        &self.steps
    }

    pub fn into_operands(self) -> ProgramStorageEntryWholeRootOperandCarrier {
        self.operands
    }
}

/// Derive the exact balanced caller-frame recipe. Rejection returns the intact
/// operand carrier, including all authority retained below it.
pub fn plan_program_storage_entry_wrapper_caller_frame(
    operands: ProgramStorageEntryWholeRootOperandCarrier,
) -> Result<ProgramStorageEntryWrapperCallerFramePlan, ProgramStorageEntryWrapperCallerFrameError> {
    match derive_and_validate_recipe(&operands) {
        Ok(recipe) => Ok(ProgramStorageEntryWrapperCallerFramePlan {
            operands,
            shadow_byte_count: recipe.shadow_byte_count,
            outgoing_reservation_byte_count: recipe.outgoing_reservation_byte_count,
            outgoing_release_byte_count: recipe.outgoing_release_byte_count,
            pre_call_stack_alignment: recipe.pre_call_stack_alignment,
            steps: recipe.steps,
        }),
        Err(diagnostic) => Err(ProgramStorageEntryWrapperCallerFrameError {
            operands,
            diagnostic,
        }),
    }
}

pub fn plan_program_local_storage_entry_wrapper_caller_frame<'root, 'code>(
    custody: ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryWholeRootOperandCarrier>,
) -> Result<
    ProgramLocalStorageCustody<'root, 'code, ProgramStorageEntryWrapperCallerFramePlan>,
    ProgramLocalStorageCustodyError<'root, 'code, ProgramStorageEntryWholeRootOperandCarrier>,
> {
    let (operands, registry) = custody.into_parts();
    match plan_program_storage_entry_wrapper_caller_frame(operands) {
        Ok(frame) => Ok(ProgramLocalStorageCustody::new(frame, registry)),
        Err(error) => {
            let diagnostic = error.diagnostic().clone();
            Err(ProgramLocalStorageCustodyError::new(
                ProgramLocalStorageCustody::new(error.into_operands(), registry),
                diagnostic,
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallerFrameRecipe {
    shadow_byte_count: u32,
    outgoing_reservation_byte_count: u32,
    outgoing_release_byte_count: u32,
    pre_call_stack_alignment: u32,
    steps: [ProgramStorageEntryWrapperCallerFrameStep; 6],
}

fn derive_and_validate_recipe(
    carrier: &ProgramStorageEntryWholeRootOperandCarrier,
) -> Result<CallerFrameRecipe, ProgramStorageEntryDiagnostic> {
    if carrier.logical_values().arguments().target() != omega_target::NativeTarget::uefi_x64() {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper caller frame requires the exact UEFI x86-64 target".into(),
        ));
    }
    let [image, initial_storage] = carrier.operands();
    let expected_operands = [
        (
            image,
            ProgramStorageEntryRootRole::Image,
            MachineRegister::X86Rcx,
            32,
        ),
        (
            initial_storage,
            ProgramStorageEntryRootRole::InitialStorage,
            MachineRegister::X86Rdx,
            48,
        ),
    ];
    for (index, (operand, role, register, stack_offset)) in
        expected_operands.into_iter().enumerate()
    {
        if operand.role() != role
            || operand.visible_parameter_index() != index
            || operand.call_parameter_index() != index
            || operand.pointer() != IndirectPointerLocation::Register(register)
            || operand.caller_copy_stack_byte_offset() != stack_offset
            || operand.byte_size() != 16
            || operand.alignment() != 8
        {
            return Err(ProgramStorageEntryDiagnostic(format!(
                "program-storage {role:?} caller-frame input drifted from its exact operand identity or placement"
            )));
        }
    }

    let steps = expected_steps(image.bytes(), initial_storage.bytes());
    let recipe = CallerFrameRecipe {
        shadow_byte_count: SHADOW_BYTE_COUNT,
        outgoing_reservation_byte_count: OUTGOING_RESERVATION_BYTE_COUNT,
        outgoing_release_byte_count: OUTGOING_RESERVATION_BYTE_COUNT,
        pre_call_stack_alignment: PRE_CALL_STACK_ALIGNMENT,
        steps: steps.clone(),
    };
    validate_recipe(&recipe, &steps)?;
    Ok(recipe)
}

fn expected_steps(
    image: &[u8; 16],
    initial_storage: &[u8; 16],
) -> [ProgramStorageEntryWrapperCallerFrameStep; 6] {
    let word = |bytes: &[u8; 16], start: usize| {
        bytes[start..start + 8]
            .try_into()
            .expect("Extent operand image contains two exact u64 words")
    };
    [
        ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord {
            role: ProgramStorageEntryRootRole::Image,
            visible_parameter_index: 0,
            call_parameter_index: 0,
            field: ProgramEntrySourceExtentFieldRole::Base,
            operand_byte_offset: 0,
            stack_byte_offset: 32,
            bytes: word(image, 0),
        },
        ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord {
            role: ProgramStorageEntryRootRole::Image,
            visible_parameter_index: 0,
            call_parameter_index: 0,
            field: ProgramEntrySourceExtentFieldRole::Length,
            operand_byte_offset: 8,
            stack_byte_offset: 40,
            bytes: word(image, 8),
        },
        ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord {
            role: ProgramStorageEntryRootRole::InitialStorage,
            visible_parameter_index: 1,
            call_parameter_index: 1,
            field: ProgramEntrySourceExtentFieldRole::Base,
            operand_byte_offset: 0,
            stack_byte_offset: 48,
            bytes: word(initial_storage, 0),
        },
        ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord {
            role: ProgramStorageEntryRootRole::InitialStorage,
            visible_parameter_index: 1,
            call_parameter_index: 1,
            field: ProgramEntrySourceExtentFieldRole::Length,
            operand_byte_offset: 8,
            stack_byte_offset: 56,
            bytes: word(initial_storage, 8),
        },
        ProgramStorageEntryWrapperCallerFrameStep::BindCallerCopyAddress {
            role: ProgramStorageEntryRootRole::Image,
            visible_parameter_index: 0,
            call_parameter_index: 0,
            register: MachineRegister::X86Rcx,
            caller_copy_stack_byte_offset: 32,
            caller_copy_byte_count: 16,
            caller_copy_alignment: 8,
        },
        ProgramStorageEntryWrapperCallerFrameStep::BindCallerCopyAddress {
            role: ProgramStorageEntryRootRole::InitialStorage,
            visible_parameter_index: 1,
            call_parameter_index: 1,
            register: MachineRegister::X86Rdx,
            caller_copy_stack_byte_offset: 48,
            caller_copy_byte_count: 16,
            caller_copy_alignment: 8,
        },
    ]
}

fn validate_recipe(
    recipe: &CallerFrameRecipe,
    expected_steps: &[ProgramStorageEntryWrapperCallerFrameStep; 6],
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let covered_stack_byte_count = 64;
    let expected_reservation = (covered_stack_byte_count + INCOMING_STACK_ALIGNMENT_REMAINDER)
        .next_multiple_of(PRE_CALL_STACK_ALIGNMENT)
        - INCOMING_STACK_ALIGNMENT_REMAINDER;
    if recipe.shadow_byte_count != SHADOW_BYTE_COUNT
        || recipe.outgoing_reservation_byte_count != expected_reservation
        || recipe.outgoing_release_byte_count != recipe.outgoing_reservation_byte_count
        || recipe.pre_call_stack_alignment != PRE_CALL_STACK_ALIGNMENT
    {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper caller frame lost its exact balanced Microsoft x64 reservation"
                .into(),
        ));
    }
    if recipe.steps != *expected_steps {
        return Err(ProgramStorageEntryDiagnostic(
            "program-storage wrapper caller frame drifted from the exact operand bytes, identities, ranges, or action order"
                .into(),
        ));
    }

    for (index, expected_offset) in [32, 40, 48, 56].into_iter().enumerate() {
        let ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord {
            stack_byte_offset,
            operand_byte_offset,
            ..
        } = &recipe.steps[index]
        else {
            return Err(ProgramStorageEntryDiagnostic(
                "program-storage wrapper caller frame reordered its four Extent writes".into(),
            ));
        };
        if *stack_byte_offset != expected_offset
            || *operand_byte_offset != if index % 2 == 0 { 0 } else { 8 }
            || stack_byte_offset + u32::from(EXTENT_WORD_BYTE_COUNT)
                > recipe.outgoing_reservation_byte_count
        {
            return Err(ProgramStorageEntryDiagnostic(
                "program-storage wrapper caller-frame write range or field order drifted".into(),
            ));
        }
    }

    let expected_bindings = [
        (
            ProgramStorageEntryRootRole::Image,
            0,
            MachineRegister::X86Rcx,
            32,
        ),
        (
            ProgramStorageEntryRootRole::InitialStorage,
            1,
            MachineRegister::X86Rdx,
            48,
        ),
    ];
    for (step, (expected_role, expected_index, expected_register, expected_offset)) in
        recipe.steps[4..].iter().zip(expected_bindings)
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
                "program-storage wrapper caller frame reordered its address bindings".into(),
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
                "program-storage wrapper caller-copy address binding drifted".into(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct ProgramStorageEntryWrapperCallerFrameError {
    operands: ProgramStorageEntryWholeRootOperandCarrier,
    diagnostic: ProgramStorageEntryDiagnostic,
}

impl ProgramStorageEntryWrapperCallerFrameError {
    pub const fn diagnostic(&self) -> &ProgramStorageEntryDiagnostic {
        &self.diagnostic
    }

    pub fn into_operands(self) -> ProgramStorageEntryWholeRootOperandCarrier {
        self.operands
    }
}

impl std::fmt::Display for ProgramStorageEntryWrapperCallerFrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.diagnostic, formatter)
    }
}

impl std::error::Error for ProgramStorageEntryWrapperCallerFrameError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe() -> CallerFrameRecipe {
        CallerFrameRecipe {
            shadow_byte_count: 32,
            outgoing_reservation_byte_count: 72,
            outgoing_release_byte_count: 72,
            pre_call_stack_alignment: 16,
            steps: expected_steps(&[1; 16], &[2; 16]),
        }
    }

    #[test]
    fn exact_balanced_recipe_is_admitted() {
        validate_recipe(&recipe(), &expected_steps(&[1; 16], &[2; 16]))
            .expect("exact caller-frame recipe");
    }

    #[test]
    fn reservation_or_release_drift_rejects() {
        let mut drifted = recipe();
        drifted.outgoing_reservation_byte_count = 64;
        assert!(validate_recipe(&drifted, &expected_steps(&[1; 16], &[2; 16])).is_err());
        drifted.outgoing_reservation_byte_count = 72;
        drifted.outgoing_release_byte_count = 64;
        assert!(validate_recipe(&drifted, &expected_steps(&[1; 16], &[2; 16])).is_err());
    }

    #[test]
    fn write_or_address_action_order_drift_rejects() {
        let mut drifted = recipe();
        drifted.steps.swap(0, 1);
        assert!(validate_recipe(&drifted, &expected_steps(&[1; 16], &[2; 16])).is_err());
        let mut drifted = recipe();
        drifted.steps.swap(4, 5);
        assert!(validate_recipe(&drifted, &expected_steps(&[1; 16], &[2; 16])).is_err());
    }

    #[test]
    fn write_range_or_pointer_register_drift_rejects() {
        let mut drifted = recipe();
        let ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord {
            stack_byte_offset, ..
        } = &mut drifted.steps[2]
        else {
            unreachable!()
        };
        *stack_byte_offset = 56;
        assert!(validate_recipe(&drifted, &expected_steps(&[1; 16], &[2; 16])).is_err());

        let mut drifted = recipe();
        let ProgramStorageEntryWrapperCallerFrameStep::BindCallerCopyAddress { register, .. } =
            &mut drifted.steps[4]
        else {
            unreachable!()
        };
        *register = MachineRegister::X86Rdx;
        assert!(validate_recipe(&drifted, &expected_steps(&[1; 16], &[2; 16])).is_err());
    }

    #[test]
    fn copied_operand_byte_drift_rejects() {
        let mut drifted = recipe();
        let ProgramStorageEntryWrapperCallerFrameStep::WriteExtentWord { bytes, .. } =
            &mut drifted.steps[0]
        else {
            unreachable!()
        };
        bytes[0] ^= 1;
        assert!(validate_recipe(&drifted, &expected_steps(&[1; 16], &[2; 16])).is_err());
    }
}

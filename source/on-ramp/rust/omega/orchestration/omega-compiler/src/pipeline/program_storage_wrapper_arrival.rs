//! Final-image evidence that the generated receiver-free wrapper consumes the
//! exact physical program-storage arrival placements.
//!
//! This is a compile-time join only. It binds the already checked
//! `BoundaryEntryPlan` placements retained by the bridge to the generated
//! wrapper's exact launch-value copy rows and their final, relocation-free
//! bytes. It does not bind installed authority, prove firmware invocation, or
//! claim native execution.

use super::{
    ProgramStorageEntryDiagnostic, ProgramStorageEntryNativeBridgePlan, ProgramStorageEntryRootRole,
};
use omega_calling_conventions::{
    IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement,
};
use omega_control_flow::MachineFunctionIdentity;
use omega_image::EmittedImageOutput;
use omega_machine_bytes::CompilerInstructionValidationKind;
use omega_object_file::RelocationOrigin;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryEmittedArrivalCopyEvidence {
    source_byte_offset: u32,
    caller_copy_stack_byte_offset: u32,
    selected_instruction_index: u32,
    section_byte_range: Range<usize>,
    final_bytes: [u8; 15],
}

impl ProgramStorageEntryEmittedArrivalCopyEvidence {
    pub const fn source_byte_offset(&self) -> u32 {
        self.source_byte_offset
    }

    pub const fn caller_copy_stack_byte_offset(&self) -> u32 {
        self.caller_copy_stack_byte_offset
    }

    pub const fn selected_instruction_index(&self) -> u32 {
        self.selected_instruction_index
    }

    pub const fn section_byte_range(&self) -> &Range<usize> {
        &self.section_byte_range
    }

    pub const fn final_bytes(&self) -> &[u8; 15] {
        &self.final_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryEmittedArrivalRootEvidence {
    role: ProgramStorageEntryRootRole,
    arrival_parameter_index: usize,
    physical_arrival_placement: ValuePlacement,
    copies: [ProgramStorageEntryEmittedArrivalCopyEvidence; 2],
}

impl ProgramStorageEntryEmittedArrivalRootEvidence {
    pub const fn role(&self) -> ProgramStorageEntryRootRole {
        self.role
    }

    pub const fn arrival_parameter_index(&self) -> usize {
        self.arrival_parameter_index
    }

    pub const fn physical_arrival_placement(&self) -> &ValuePlacement {
        &self.physical_arrival_placement
    }

    pub const fn copies(&self) -> &[ProgramStorageEntryEmittedArrivalCopyEvidence; 2] {
        &self.copies
    }
}

/// Exact placed wrapper rows that consume the two physical arrival values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramStorageEntryEmittedArrivalEvidence {
    target: omega_target::NativeTarget,
    wrapper_identity: MachineFunctionIdentity,
    boundary_contract_fingerprint: u64,
    roots: [ProgramStorageEntryEmittedArrivalRootEvidence; 2],
}

impl ProgramStorageEntryEmittedArrivalEvidence {
    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub const fn wrapper_identity(&self) -> MachineFunctionIdentity {
        self.wrapper_identity
    }

    pub const fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }

    pub const fn roots(&self) -> &[ProgramStorageEntryEmittedArrivalRootEvidence; 2] {
        &self.roots
    }
}

pub(super) fn bind_final_program_storage_entry_wrapper_arrival_evidence(
    bridge: &ProgramStorageEntryNativeBridgePlan,
    backend: &omega_backend_plan::BackendPlan,
    image: &EmittedImageOutput,
) -> Result<ProgramStorageEntryEmittedArrivalEvidence, ProgramStorageEntryDiagnostic> {
    let template = bridge.wrapper_body_template().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage arrival evidence requires the receiver-free wrapper template"
                .into(),
        )
    })?;
    let inbound = bridge.continuation_inbound().ok_or_else(|| {
        ProgramStorageEntryDiagnostic(
            "final program-storage arrival evidence requires the exact source inbound ABI".into(),
        )
    })?;
    if backend.target != omega_target::NativeTarget::uefi_x64()
        || template.target() != backend.target
        || inbound.target() != backend.target
        || bridge.entry_function_identity() != template.wrapper_identity()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage arrival evidence requires one exact UEFI x64 wrapper identity"
                .into(),
        ));
    }

    let mut wrappers = backend
        .encoded_machine
        .code
        .functions
        .iter()
        .filter_map(|(_, function)| {
            (function.identity == template.wrapper_identity()).then_some(function)
        });
    let Some(wrapper) = wrappers.next() else {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage arrival evidence has no encoded wrapper function".into(),
        ));
    };
    if wrappers.next().is_some()
        || wrapper.symbol.as_ref() != bridge.entry_symbol()
        || wrapper.byte_offset != bridge.entry_text_offset()
        || wrapper.byte_count != bridge.entry_text_size()
    {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage arrival wrapper identity or interval is ambiguous".into(),
        ));
    }
    let instructions = backend
        .encoded_machine
        .code
        .instructions
        .span(wrapper.instructions)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "final program-storage arrival wrapper has an invalid instruction span".into(),
            )
        })?;
    if instructions.len() != 11 {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage arrival wrapper lost its exact instruction inventory".into(),
        ));
    }
    let (wrapper_symbol_handle, wrapper_symbol) =
        omega_object_file::object_function_symbol(&backend.object, template.wrapper_identity())
            .ok_or_else(|| {
                ProgramStorageEntryDiagnostic(
                    "final program-storage arrival wrapper has no object linkage".into(),
                )
            })?;
    if wrapper_symbol_handle != backend.object.layout.entry_symbol
        || wrapper_symbol.name != bridge.entry_symbol()
        || wrapper_symbol.offset != wrapper.byte_offset
        || wrapper_symbol.size != wrapper.byte_count
    {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage arrival wrapper drifted from the object entry".into(),
        ));
    }

    let wrapper_end = wrapper
        .byte_offset
        .checked_add(wrapper.byte_count)
        .ok_or_else(|| {
            ProgramStorageEntryDiagnostic(
                "final program-storage arrival wrapper interval overflows".into(),
            )
        })?;
    let transfer_roots = bridge.wrapper_transfer().roots();
    let inbound_roots = inbound.arguments();
    let expected = [
        (
            ProgramStorageEntryRootRole::Image,
            MachineRegister::X86Rcx,
            32,
            [2usize, 3usize],
        ),
        (
            ProgramStorageEntryRootRole::InitialStorage,
            MachineRegister::X86Rdx,
            48,
            [4usize, 5usize],
        ),
    ];
    let mut roots = Vec::with_capacity(2);
    for (index, (role, register, caller_copy_offset, row_indices)) in
        expected.into_iter().enumerate()
    {
        let transfer = &transfer_roots[index];
        let inbound_argument = &inbound_roots[index];
        validate_physical_placement(
            role,
            index,
            register,
            caller_copy_offset,
            transfer,
            inbound_argument,
        )?;
        let mut copies = Vec::with_capacity(2);
        for (field_index, row_index) in row_indices.into_iter().enumerate() {
            copies.push(validate_final_copy(
                role,
                register,
                (field_index * 8) as u32,
                caller_copy_offset + (field_index * 8) as u32,
                &instructions[row_index],
                wrapper.byte_offset..wrapper_end,
                wrapper_symbol_handle,
                backend,
                image,
            )?);
        }
        roots.push(ProgramStorageEntryEmittedArrivalRootEvidence {
            role,
            arrival_parameter_index: index,
            physical_arrival_placement: transfer.physical_arrival_placement().clone(),
            copies: copies.try_into().map_err(|_| {
                ProgramStorageEntryDiagnostic(
                    "final program-storage arrival root lost its two field copies".into(),
                )
            })?,
        });
    }

    let roots: [ProgramStorageEntryEmittedArrivalRootEvidence; 2] =
        roots.try_into().map_err(|_| {
            ProgramStorageEntryDiagnostic(
                "final program-storage arrival evidence lost its two root rows".into(),
            )
        })?;
    let copies = roots
        .iter()
        .flat_map(|root| root.copies())
        .collect::<Vec<_>>();
    if copies.windows(2).any(|pair| {
        pair[0].selected_instruction_index == pair[1].selected_instruction_index
            || pair[0].section_byte_range.end > pair[1].section_byte_range.start
    }) {
        return Err(ProgramStorageEntryDiagnostic(
            "final program-storage arrival copy identities or byte intervals overlap".into(),
        ));
    }

    Ok(ProgramStorageEntryEmittedArrivalEvidence {
        target: backend.target,
        wrapper_identity: template.wrapper_identity(),
        boundary_contract_fingerprint: bridge.binding().boundary_contract_fingerprint(),
        roots,
    })
}

fn validate_physical_placement(
    role: ProgramStorageEntryRootRole,
    index: usize,
    register: MachineRegister,
    caller_copy_offset: u32,
    transfer: &super::ProgramStorageEntryWrapperRootTransferPlan,
    inbound: &super::ProgramStorageEntryContinuationInboundArgument,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let placement = transfer.physical_arrival_placement();
    let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset,
            byte_size,
            alignment,
        },
    ] = placement.locations.as_slice()
    else {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "final program-storage {role:?} arrival is not one exact indirect placement"
        )));
    };
    if transfer.role() != role
        || transfer.arrival_parameter_index() != index
        || transfer.source_parameter_index() != index
        || inbound.role() != role
        || inbound.visible_parameter_index() != index
        || inbound.call_parameter_index() != index
        || inbound.placement() != placement
        || !exact_indirect_placement_fields(
            placement,
            *pointer,
            *copy_stack_byte_offset,
            *byte_size,
            *alignment,
            register,
            caller_copy_offset,
        )
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "final program-storage {role:?} physical arrival placement drifted from its exact wrapper transfer"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn exact_indirect_placement_fields(
    placement: &ValuePlacement,
    pointer: IndirectPointerLocation,
    copy_stack_byte_offset: Option<u32>,
    byte_size: u16,
    alignment: u16,
    expected_register: MachineRegister,
    expected_copy_offset: u32,
) -> bool {
    placement.shape == omega_calling_conventions::ValueShape::integer(16, 8)
        && pointer == IndirectPointerLocation::Register(expected_register)
        && copy_stack_byte_offset == Some(expected_copy_offset)
        && byte_size == 16
        && alignment == 8
}

#[allow(clippy::too_many_arguments)]
fn validate_final_copy(
    role: ProgramStorageEntryRootRole,
    source_register: MachineRegister,
    source_byte_offset: u32,
    stack_byte_offset: u32,
    instruction: &omega_machine_bytes::EncodedMachineInstruction,
    wrapper_range: Range<usize>,
    wrapper_symbol_handle: psi_arena::Handle<omega_object_file::SymbolPlan>,
    backend: &omega_backend_plan::BackendPlan,
    image: &EmittedImageOutput,
) -> Result<ProgramStorageEntryEmittedArrivalCopyEvidence, ProgramStorageEntryDiagnostic> {
    let expected_kind = CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
        source_register,
        source_byte_offset,
        stack_byte_offset,
    };
    if instruction.compiler_validation_kind.as_ref() != Some(&expected_kind)
        || instruction.bytes.is_empty()
        || !instruction.bytes.start().is_valid()
    {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "final program-storage {role:?} launch-value copy metadata drifted"
        )));
    }
    let start = instruction.bytes.start().arena_index() as usize - 1;
    let end = start.checked_add(instruction.bytes.len()).ok_or_else(|| {
        ProgramStorageEntryDiagnostic(format!(
            "final program-storage {role:?} launch-value copy interval overflows"
        ))
    })?;
    let range = start..end;
    if range.start < wrapper_range.start || range.end > wrapper_range.end {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "final program-storage {role:?} launch-value copy escapes the wrapper interval"
        )));
    }
    if backend.relocations.records().any(|(_, relocation)| {
        relocation.origin
            == (RelocationOrigin::Instruction {
                function_symbol_handle: wrapper_symbol_handle,
                selected_instruction_index: instruction.selected_instruction_index,
            })
    }) {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "final program-storage {role:?} launch-value copy unexpectedly owns a relocation"
        )));
    }
    let expected = omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
        source_register,
        source_byte_offset,
        stack_byte_offset,
    )
    .map_err(|diagnostic| ProgramStorageEntryDiagnostic(diagnostic.message))?;
    validate_copy_bytes(role, &expected, backend, image, range.clone())?;
    Ok(ProgramStorageEntryEmittedArrivalCopyEvidence {
        source_byte_offset,
        caller_copy_stack_byte_offset: stack_byte_offset,
        selected_instruction_index: instruction.selected_instruction_index,
        section_byte_range: range,
        final_bytes: expected,
    })
}

fn validate_copy_bytes(
    role: ProgramStorageEntryRootRole,
    expected: &[u8; 15],
    backend: &omega_backend_plan::BackendPlan,
    image: &EmittedImageOutput,
    range: Range<usize>,
) -> Result<(), ProgramStorageEntryDiagnostic> {
    let encoded = backend
        .encoded_machine
        .code
        .bytes
        .storage_slice()
        .get(range.clone());
    let final_bytes = image.final_text_bytes.get(range);
    if !exact_copy_slices(expected, encoded, final_bytes) {
        return Err(ProgramStorageEntryDiagnostic(format!(
            "final program-storage {role:?} launch-value copy bytes drifted"
        )));
    }
    Ok(())
}

fn exact_copy_slices(
    expected: &[u8; 15],
    encoded: Option<&[u8]>,
    final_bytes: Option<&[u8]>,
) -> bool {
    encoded == Some(expected.as_slice()) && final_bytes == Some(expected.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_placement_requires_exact_indirect_pointer_and_copy_slot() {
        let placement = |register, copy_stack_byte_offset| ValuePlacement {
            shape: omega_calling_conventions::ValueShape::integer(16, 8),
            locations: vec![ValueLocation::Indirect {
                pointer: IndirectPointerLocation::Register(register),
                copy_stack_byte_offset,
                byte_size: 16,
                alignment: 8,
            }],
        };
        let exact = placement(MachineRegister::X86Rcx, Some(32));
        let [
            ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            },
        ] = exact.locations.as_slice()
        else {
            panic!("test placement")
        };
        assert!(exact_indirect_placement_fields(
            &exact,
            *pointer,
            *copy_stack_byte_offset,
            *byte_size,
            *alignment,
            MachineRegister::X86Rcx,
            32,
        ));
        for drifted in [
            placement(MachineRegister::X86Rdx, Some(32)),
            placement(MachineRegister::X86Rcx, Some(48)),
            placement(MachineRegister::X86Rcx, None),
        ] {
            let [
                ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                },
            ] = drifted.locations.as_slice()
            else {
                panic!("test placement")
            };
            assert!(!exact_indirect_placement_fields(
                &drifted,
                *pointer,
                *copy_stack_byte_offset,
                *byte_size,
                *alignment,
                MachineRegister::X86Rcx,
                32,
            ));
        }
    }

    #[test]
    fn canonical_copy_bytes_reject_opcode_source_and_destination_tamper() {
        let exact = omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
            MachineRegister::X86Rcx,
            0,
            32,
        )
        .unwrap();
        let mut opcode = exact;
        opcode[0] ^= 1;
        assert!(exact_copy_slices(&exact, Some(&exact), Some(&exact)));
        assert!(!exact_copy_slices(&exact, Some(&opcode), Some(&exact)));
        assert!(!exact_copy_slices(&exact, Some(&exact), Some(&opcode)));
        assert!(!exact_copy_slices(&exact, None, Some(&exact)));
        assert_ne!(
            omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                MachineRegister::X86Rdx,
                0,
                32
            )
            .unwrap(),
            exact
        );
        assert_ne!(
            omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                MachineRegister::X86Rcx,
                8,
                32
            )
            .unwrap(),
            exact
        );
        assert_ne!(
            omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                MachineRegister::X86Rcx,
                0,
                40
            )
            .unwrap(),
            exact
        );
    }
}

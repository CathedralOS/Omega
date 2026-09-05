//! Target-specific structural call footprints and section-dependent fixups.
//!
//! These are replayable data, not admitted templates. ISA encoders and
//! independent decoders retain the authority to establish their contents.

use omega_calling_conventions::MachineRegister;
use omega_register_model::RegisterUnitId;
use omega_selected_instructions::{
    MachineCleanupEffect, MachineTrapBehavior, StructuralUnitCallBarrier, StructuralUnitCallEffect,
};
use psi_core::MachineId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitInternalControlFixupKind {
    Relative32FromNextInstructionToInternalMachineV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitInternalControlFixupState {
    UnresolvedZeroFieldV1,
}

/// One section-layout-dependent internal-control field. This is not an object
/// relocation: the selected callee is an in-roster [`MachineId`], but its
/// section coordinate is deliberately unavailable at selected-form encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitInternalControlFixup {
    pub kind: X86_64StructuralUnitInternalControlFixupKind,
    pub state: X86_64StructuralUnitInternalControlFixupState,
    pub callee: MachineId,
    pub opcode_byte_offset: u16,
    pub field_byte_offset: u16,
    pub next_instruction_byte_offset: u16,
    pub field_byte_width: u8,
    pub addend: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64StructuralUnitInternalControlResolutionState {
    ResolvedInSectionV1,
}

/// Target-owned evidence that one structural Unit call fixup has been
/// discharged against concrete text-section coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64ResolvedStructuralUnitInternalControlFixup {
    pub source: X86_64StructuralUnitInternalControlFixup,
    pub state: X86_64StructuralUnitInternalControlResolutionState,
    pub caller_section_offset: u64,
    pub callee_section_offset: u64,
    pub next_instruction_section_offset: u64,
    pub displacement: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitRootRead {
    pub root: MachineRegister,
    pub byte_offset: u32,
    pub byte_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitCallerCopyWrite {
    pub stack_byte_offset: u32,
    pub byte_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86_64StructuralUnitArgumentPointerWrite {
    pub register: MachineRegister,
    pub stack_byte_offset: u32,
}

/// Independently decoded architectural footprint of the exact bounded call
/// bundle. It remains distinct from ordinary alternative effects because the
/// latter cannot express root-indirect reads, caller-copy writes, or a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64SelectedStructuralUnitCallFootprint {
    pub implicit_unit_uses: Vec<RegisterUnitId>,
    pub implicit_unit_defs: Vec<RegisterUnitId>,
    pub implicit_unit_clobbers: Vec<RegisterUnitId>,
    pub root_reads: [X86_64StructuralUnitRootRead; 4],
    pub caller_copy_writes: [X86_64StructuralUnitCallerCopyWrite; 4],
    pub scratch_register_writes: [MachineRegister; 1],
    pub argument_pointer_writes: [X86_64StructuralUnitArgumentPointerWrite; 2],
    pub writes_rflags: bool,
    pub frame_byte_count: u32,
    pub shadow_byte_count: u32,
    pub pre_call_stack_alignment: u16,
    pub frame_is_balanced: bool,
    pub trap: MachineTrapBehavior,
    pub barrier: StructuralUnitCallBarrier,
    pub call: StructuralUnitCallEffect,
    pub cleanup: MachineCleanupEffect,
}

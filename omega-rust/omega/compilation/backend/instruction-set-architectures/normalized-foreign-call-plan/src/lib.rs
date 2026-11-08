#![forbid(unsafe_code)]

/*
We own the shared admission check (`admit`), unresolved relocation fixup carrier
(`Fixup`), and validated template record (`Template`) for `NormalizedForeignCall`
selected instructions across x86-64 and AArch64. When a backend encodes or
validates an unresolved normalized foreign call, we verify that the target is
supported, the physical register model identity is canonical, the instruction
kind's `{boundary, ordinal}` matches the retained record and names a symbolic
import, the evaluated register operand views and accesses match the roster row,
and the single admitted alternative carries the exact declared machine effects.
*/

/*
We centralize normalized-foreign-call template admission and carriers here so
that x86-64 and AArch64 cannot diverge in how they authenticate foreign-call
records, operand views, or effect footprints before emitting architecture-
specific call sequences and callback materialization groups. Only the terminal
call bytes, callback destination encodings, and relocation fixup kinds remain
with the individual instruction-set crates.
*/

/*
`isa-aarch64` and `isa-x86_64` instantiate `Fixup` and `Template` with their
architecture-specific fixup kind, state, and callback destination types, and
call `admit` from their `selected_form_encoding` foreign-call encoders and
validators. Under the `test-support` feature, `fixture` supplies the shared
normalized-foreign-call test records, callback rosters, and end-to-end template
validation sweeps used by both ISA crates.
*/

use register_model::{
    PhysicalRegisterModelIdentity, RegisterInstructionConstraint, RegisterOperandAccess,
    RegisterViewId, ValidatedPhysicalRegisterModel,
};
use selected_instructions::{
    MachineAlternativeKey, MachineEffectDeclaration, MachineEncodedEffects,
    SelectedInstructionKind, SelectedNormalizedForeignCall,
};
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;

#[cfg(feature = "test-support")]
pub mod fixture;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnsupportedTarget,
    NonCanonicalPhysicalModel,
    InstructionKindMismatch,
    AlternativeMismatch,
    OperandViewMismatch,
    EffectMismatch,
    MalformedTemplate,
    FixupMismatch,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid normalized-foreign-call template: {self:?}"
        )
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixup<Kind, State, Callback> {
    pub kind: Kind, // Architecture-specific relocation kind for the terminal import call.
    pub state: State, // Architecture-specific fixup state for the unresolved import field.
    pub boundary: BoundaryMachineId, // Boundary machine owning the foreign call declaration.
    pub ordinal: u32, // Roster ordinal of the selected foreign call within the boundary.
    pub opcode_byte_offset: u16, // Byte offset of the terminal call instruction within the template.
    pub patch_byte_offset: u16,  // Byte offset of the relocation patch field within the template.
    pub reference_byte_offset: u16, // Byte offset used as the relocation reference origin.
    pub patch_byte_width: u8,    // Width in bytes of the relocation patch field.
    pub callbacks: Vec<Callback>, // Private callback address materializations in roster order.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template<Fixup> {
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: Vec<RegisterViewId>,
    effects: MachineEncodedEffects,
    bytes: Vec<u8>,
    fixup: Fixup,
}

impl<Fixup> Template<Fixup> {
    pub fn new(
        kind: SelectedInstructionKind,
        alternative: MachineAlternativeKey,
        operand_views: &[RegisterViewId],
        effects: &MachineEncodedEffects,
        bytes: &[u8],
        fixup: Fixup,
    ) -> Self {
        Self {
            kind,
            alternative,
            operand_views: operand_views.to_vec(),
            effects: effects.clone(),
            bytes: bytes.to_vec(),
            fixup,
        }
    }

    pub const fn kind(&self) -> SelectedInstructionKind {
        self.kind
    }

    pub const fn alternative(&self) -> MachineAlternativeKey {
        self.alternative
    }

    pub fn operand_views(&self) -> &[RegisterViewId] {
        &self.operand_views
    }

    pub const fn effects(&self) -> &MachineEncodedEffects {
        &self.effects
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn fixup(&self) -> &Fixup {
        &self.fixup
    }
}

pub fn admit(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    effects: &MachineEncodedEffects,
    record: &SelectedNormalizedForeignCall,
    accepts_target: impl Fn(NativeTarget) -> bool,
    canonical_physical_identity: impl Fn() -> PhysicalRegisterModelIdentity,
    constraint: impl Fn(&SelectedNormalizedForeignCall) -> Option<RegisterInstructionConstraint>,
    declaration: impl Fn(
        &SelectedNormalizedForeignCall,
        &RegisterInstructionConstraint,
    ) -> Option<MachineEffectDeclaration>,
) -> Result<(BoundaryMachineId, u32), Error> {
    if !accepts_target(target) {
        return Err(Error::UnsupportedTarget);
    }
    if physical.identity() != canonical_physical_identity() {
        return Err(Error::NonCanonicalPhysicalModel);
    }
    let SelectedInstructionKind::NormalizedForeignCall { boundary, ordinal } = kind else {
        return Err(Error::InstructionKindMismatch);
    };
    if record.call.boundary != boundary
        || record.call.binding.native_target() != Some(target)
        || record.call.binding.symbolic_import().is_none()
    {
        return Err(Error::OperandViewMismatch);
    }
    let row = constraint(record).ok_or(Error::OperandViewMismatch)?;
    if row.operands.len() != operand_views.len()
        || row.operands.len() != operand_accesses.len()
        || row
            .operands
            .iter()
            .zip(operand_views.iter().zip(operand_accesses))
            .any(|(operand, (view, access))| {
                operand.fixed_view != Some(*view) || operand.access != *access
            })
    {
        return Err(Error::OperandViewMismatch);
    }
    let declared = declaration(record, &row).ok_or(Error::EffectMismatch)?;
    let [expected] = declared.alternatives.as_slice() else {
        return Err(Error::AlternativeMismatch);
    };
    if alternative != expected.key {
        return Err(Error::AlternativeMismatch);
    }
    if effects != &expected.encoded {
        return Err(Error::EffectMismatch);
    }
    Ok((boundary, ordinal))
}

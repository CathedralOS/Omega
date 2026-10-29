#![forbid(unsafe_code)]

//! The unresolved normalized-foreign-call template both instruction sets build.
//!
//! A selected normalized-foreign call is admitted the same way on every
//! target: the instruction kind carries the `{boundary, ordinal}` roster row,
//! the retained record must agree with it and name a symbolic import, the
//! roster row's operand columns must be the evaluated ones, and that row's
//! declaration must admit exactly one alternative whose encoded effects are
//! the ones under test. What differs between instruction sets is only the
//! terminal call encoding, each callback destination's byte count, and the
//! fixup vocabulary naming its relocations; those stay with the instruction
//! set. The admission, the fixup record, and the validated template record are
//! stated here once so a second instruction set cannot admit differently.

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

/// The selected record both instruction sets' template tests admit: physical
/// call inputs with the target's own facts as arguments.
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

/// Unresolved import-field fixup for one normalized foreign call.
///
/// `{boundary, ordinal}` names the selected roster row owning the evaluated
/// locator; `callbacks` carries every private-callback materialization in the
/// call's roster order. The kind, state, and callback records belong to the
/// instruction set: each names the relocations its architecture needs and the
/// offsets they occupy inside their materialization group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixup<Kind, State, Callback> {
    pub kind: Kind,
    pub state: State,
    pub boundary: BoundaryMachineId,
    pub ordinal: u32,
    pub opcode_byte_offset: u16,
    pub patch_byte_offset: u16,
    pub reference_byte_offset: u16,
    pub patch_byte_width: u8,
    pub callbacks: Vec<Callback>,
}

/// Exact selected inputs and canonical bytes for one unresolved normalized
/// foreign call, bound to the instruction set's own fixup record.
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
    /// Bind admitted inputs to the bytes and fixup an instruction set emitted
    /// for them, after that instruction set re-derived both from the record.
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

/// Admit a selected normalized-foreign call against its own retained record.
///
/// The four hooks are the whole per-instruction-set surface the program needs:
/// whether the target is one this instruction set encodes, the canonical
/// physical register model identity it admits, the roster constraint the record
/// evaluates to, and the effect declaration that constraint carries. Everything
/// they are checked against is target-independent, so both instruction sets run
/// one admission instead of two nearly-equal ones.
///
/// Returns the `{boundary, ordinal}` row the record was admitted against, which
/// is what the caller's fixup names.
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

//! The encoding carriers: the footprint of one selected form, the validated
//! encoding, the MOVN seed, MOVK patch and shortest-materialization recipe,
//! and the encoding error.

use register_model::RegisterViewId;
use selected_instructions::MachineEncodedEffects;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aarch64SelectedFormFootprint {
    pub register_reads: Vec<RegisterViewId>,
    pub register_writes: Vec<RegisterViewId>,
    pub writes_nzcv: bool,
    pub encoded: MachineEncodedEffects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAarch64SelectedFormEncoding {
    pub(crate) bytes: Vec<u8>,
    pub(crate) footprint: Aarch64SelectedFormFootprint,
}

impl ValidatedAarch64SelectedFormEncoding {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn footprint(&self) -> &Aarch64SelectedFormFootprint {
        &self.footprint
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aarch64SelectedFormEncodingError {
    NonCanonicalPhysicalModel,
    LayoutDependentForm,
    AlternativeMismatch,
    OperandCountMismatch,
    UnknownOrNonGpr64View(RegisterViewId),
    IntegerOutsideI64Bits,
    ImmediateOutsideU12,
    BranchDisplacementMisaligned,
    BranchDisplacementOutsideImm19,
    BranchDisplacementOutsideImm26,
    MovnMaterializationDoesNotShrink,
    MalformedEncoding,
    EncodedFormMismatch,
}

impl std::fmt::Display for Aarch64SelectedFormEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 selected-form encoding: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64SelectedFormEncodingError {}

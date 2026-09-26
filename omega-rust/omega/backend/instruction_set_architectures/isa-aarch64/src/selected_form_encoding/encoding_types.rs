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

/// Canonical 64-bit `MOVN` seed for a shortest complement-seeded immediate
/// materialization. `halfword` is the architectural `hw` field, in `0..=3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Aarch64MovnSeed {
    pub(crate) halfword: u8,
    pub(crate) immediate: u16,
}

impl Aarch64MovnSeed {
    pub const fn halfword(&self) -> u8 {
        self.halfword
    }

    pub const fn immediate(&self) -> u16 {
        self.immediate
    }
}

/// One canonical 64-bit `MOVK` patch following a complement-seeded `MOVN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Aarch64MovkPatch {
    pub(crate) halfword: u8,
    pub(crate) immediate: u16,
}

impl Aarch64MovkPatch {
    pub const fn halfword(&self) -> u8 {
        self.halfword
    }

    pub const fn immediate(&self) -> u16 {
        self.immediate
    }
}

/// The unique shortest `MOVN`-seeded recipe that is strictly smaller than the
/// baseline zero-seeded `MOVZ`/`MOVK` materialization.
///
/// Equal-length recipes choose the lowest possible seed halfword. Patches are
/// then ordered by strictly ascending halfword index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Aarch64ShortestMovnMaterializationRecipe {
    pub(crate) seed: Aarch64MovnSeed,
    pub(crate) patches: Vec<Aarch64MovkPatch>,
    pub(crate) baseline_byte_count: usize,
}

impl Aarch64ShortestMovnMaterializationRecipe {
    pub const fn seed(&self) -> Aarch64MovnSeed {
        self.seed
    }

    pub fn patches(&self) -> &[Aarch64MovkPatch] {
        &self.patches
    }

    pub const fn baseline_byte_count(&self) -> usize {
        self.baseline_byte_count
    }

    pub fn encoded_byte_count(&self) -> usize {
        (1 + self.patches.len()) * 4
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

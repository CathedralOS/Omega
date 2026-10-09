//! The register vocabulary: unit, view and class identities and the
//! declarations behind them.

use sha2::Digest;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterUnitId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterViewId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegisterClassId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegisterUnitKind {
    IntegerLane,
    VectorLane,
    Flags,
    StackPointer,
    InstructionPointer,
    Zero,
    FloatingControl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUnit {
    pub id: RegisterUnitId,
    pub name: String,
    pub bits: u16,
    pub kind: RegisterUnitKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterWriteSemantics {
    ExactView,
    PreservesUnwritten,
    ZeroExtendsParent,
    ZeroExtendsWithinUnit,
    Discards,
    InstructionDefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterView {
    pub id: RegisterViewId,
    pub name: String,
    pub class: RegisterClassId,
    /// Storage units occupied by a live value in this view.
    pub units: Vec<RegisterUnitId>,
    /// Storage units modified by the view's canonical write behavior.
    pub write_units: Vec<RegisterUnitId>,
    pub bits: u16,
    pub write_semantics: RegisterWriteSemantics,
    pub allocatable: bool,
}

impl RegisterView {
    /// Whether either view stores through a unit the other occupies or writes.
    pub fn footprints_overlap(&self, other: &RegisterView) -> bool {
        self.units
            .iter()
            .chain(&self.write_units)
            .any(|unit| other.units.contains(unit) || other.write_units.contains(unit))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterClass {
    pub id: RegisterClassId,
    pub name: String,
    pub views: Vec<RegisterViewId>,
}

/// Length-prefix writer shared by identity hashes.
pub fn sha256_length(hasher: &mut sha2::Sha256, value: usize) {
    hasher.update((value as u64).to_le_bytes());
}

/// Length-framed register-unit roster writer shared by identity hashes.
pub fn sha256_register_units(hasher: &mut sha2::Sha256, values: &[RegisterUnitId]) {
    sha256_length(hasher, values.len());
    for value in values {
        hasher.update(value.0.to_le_bytes());
    }
}

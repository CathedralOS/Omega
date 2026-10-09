//! The register vocabulary: unit, view and class identities and the
//! declarations behind them.



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

/// Digest sink the identity roster writers feed — each crate's
/// domain-tagged encoder implements it.
pub trait Sha256RegisterDigest {
    fn byte(&mut self, value: u8);
    fn u64(&mut self, value: u64);
    fn raw(&mut self, bytes: &[u8]);
}

impl Sha256RegisterDigest for sha2::Sha256 {
    fn byte(&mut self, value: u8) {
        sha2::Digest::update(self, [value]);
    }

    fn u64(&mut self, value: u64) {
        sha2::Digest::update(self, value.to_le_bytes());
    }

    fn raw(&mut self, bytes: &[u8]) {
        sha2::Digest::update(self, bytes);
    }
}

/// Length-prefix writer shared by identity hashes.
pub fn sha256_length<H: Sha256RegisterDigest + ?Sized>(hasher: &mut H, value: usize) {
    hasher.u64(value as u64);
}

/// Length-framed register-unit roster writer shared by identity hashes.
pub fn sha256_register_units<H: Sha256RegisterDigest + ?Sized>(hasher: &mut H, values: &[RegisterUnitId]) {
    sha256_length(hasher, values.len());
    for value in values {
        hasher.raw(&value.0.to_le_bytes());
    }
}

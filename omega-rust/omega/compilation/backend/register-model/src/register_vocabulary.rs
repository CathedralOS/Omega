/*
We define our physical-register vocabulary (`RegisterUnit`, `RegisterView`,
`RegisterClass`, `RegisterWriteSemantics`) and the length-framed SHA-256
digest helpers shared across register-model identity calculations.
*/

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
    pub units: Vec<RegisterUnitId>, // Storage units occupied by a live value in this view.
    pub write_units: Vec<RegisterUnitId>, // Storage units modified by the view's canonical write behavior.
    pub bits: u16,
    pub write_semantics: RegisterWriteSemantics,
    pub allocatable: bool,
}

impl RegisterView {
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

impl Sha256RegisterDigest for optimization_core::digest_encoder::DomainDigestEncoder {
    fn byte(&mut self, value: u8) {
        optimization_core::digest_encoder::DomainDigestEncoder::byte(self, value);
    }

    fn u64(&mut self, value: u64) {
        optimization_core::digest_encoder::DomainDigestEncoder::u64(self, value);
    }

    fn raw(&mut self, bytes: &[u8]) {
        optimization_core::digest_encoder::DomainDigestEncoder::raw(self, bytes);
    }
}

pub fn sha256_length<H: Sha256RegisterDigest + ?Sized>(hasher: &mut H, value: usize) {
    hasher.u64(value as u64);
}

pub fn sha256_register_units<H: Sha256RegisterDigest + ?Sized>(
    hasher: &mut H,
    values: &[RegisterUnitId],
) {
    sha256_length(hasher, values.len());
    for value in values {
        hasher.raw(&value.0.to_le_bytes());
    }
}

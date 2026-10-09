//! Portable declaration identity shared by emitted bodies and their demands.
//!
//! Psi derives nominal bytes from exact package/toolchain ownership and the
//! declaration path. These records identify checked inputs; they do not grant
//! execution authority or replace the artifact's semantic proof.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalDeclarationIdentity(String);

impl NominalDeclarationIdentity {
    pub fn new(canonical: String) -> Result<Self, &'static str> {
        if canonical.is_empty() {
            return Err("nominal declaration identity is empty");
        }
        Ok(Self(canonical))
    }

    pub fn canonical(&self) -> &str {
        &self.0
    }
}

/// Exact checked callable and initial state from which an executable body is
/// produced. Optimization preserves this identity, not the source arenas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineDeclaration {
    pub machine: NominalDeclarationIdentity,
    pub state: NominalDeclarationIdentity,
    pub contract: [u8; 32],
    pub specialization: Option<CheckedMachineSpecialization>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineSpecialization {
    pub template: NominalDeclarationIdentity,
    pub commitment: [u8; 32],
}

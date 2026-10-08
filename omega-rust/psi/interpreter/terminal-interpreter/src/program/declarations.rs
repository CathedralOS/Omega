//! Read canonical declaration tables without rebuilding owned lookup maps.
//!
//! Terminal encoding establishes strictly increasing identities for these
//! tables. Binary search preserves the old keyed lookup and iteration order;
//! the returned references borrow the artifact, not this temporary view.

use terminal_psi::{Block, BoundaryMachineDeclaration, StructuralTypeDeclaration, TerminalMachine};

pub(crate) trait ProgramDeclaration {
    type Identity: Ord;
    fn identity(&self) -> &Self::Identity;
}

pub(crate) struct ProgramDeclarations<'storage, Declaration>(&'storage [Declaration]);

impl<'storage, Declaration: ProgramDeclaration> ProgramDeclarations<'storage, Declaration> {
    pub(crate) const fn new(declarations: &'storage [Declaration]) -> Self {
        Self(declarations)
    }

    pub(crate) fn get(&self, identity: &Declaration::Identity) -> Option<&'storage Declaration> {
        let position = self
            .0
            .binary_search_by(|declaration| declaration.identity().cmp(identity))
            .ok()?;
        self.0.get(position)
    }

    pub(crate) fn values(&self) -> std::slice::Iter<'storage, Declaration> {
        self.0.iter()
    }

    pub(crate) fn contains_key(&self, identity: &Declaration::Identity) -> bool {
        self.get(identity).is_some()
    }
}

impl<'storage> ProgramDeclarations<'storage, StructuralTypeDeclaration> {
    /// The record-or-mixed field roster of `identity`, absent when the type is
    /// missing or not field-shaped.
    pub(crate) fn record_fields(
        &self,
        identity: &semantic_vocabulary::StructuralTypeId,
    ) -> Option<&'storage [terminal_psi::StructuralFieldDeclaration]> {
        self.get(identity)?.shape.record_fields()
    }
}

impl ProgramDeclaration for TerminalMachine {
    type Identity = semantic_vocabulary::MachineId;
    fn identity(&self) -> &Self::Identity {
        &self.id
    }
}

impl ProgramDeclaration for Block {
    type Identity = semantic_vocabulary::BlockId;
    fn identity(&self) -> &Self::Identity {
        &self.id
    }
}

impl ProgramDeclaration for StructuralTypeDeclaration {
    type Identity = semantic_vocabulary::StructuralTypeId;
    fn identity(&self) -> &Self::Identity {
        &self.id
    }
}

impl ProgramDeclaration for BoundaryMachineDeclaration {
    type Identity = semantic_vocabulary::BoundaryMachineId;
    fn identity(&self) -> &Self::Identity {
        &self.id
    }
}

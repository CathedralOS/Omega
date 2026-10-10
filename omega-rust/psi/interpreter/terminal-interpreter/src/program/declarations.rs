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

pub(crate) struct ProgramDeclarations<'storage, Declaration> {
    declarations: &'storage [Declaration],
    replacements: &'storage [Declaration],
}

impl<'storage, Declaration: ProgramDeclaration> ProgramDeclarations<'storage, Declaration> {
    pub(crate) const fn new(declarations: &'storage [Declaration]) -> Self {
        Self {
            declarations,
            replacements: &[],
        }
    }

    /// Replacement rows belong to an admitted target closure of this exact
    /// canonical table. Only structural declarations currently have overlays.
    pub(crate) const fn with_replacements(
        declarations: &'storage [Declaration],
        replacements: &'storage [Declaration],
    ) -> Self {
        Self {
            declarations,
            replacements,
        }
    }

    pub(crate) fn get(&self, identity: &Declaration::Identity) -> Option<&'storage Declaration> {
        if let Ok(position) = self
            .replacements
            .binary_search_by(|row| row.identity().cmp(identity))
        {
            return self.replacements.get(position);
        }
        let position = self
            .declarations
            .binary_search_by(|row| row.identity().cmp(identity))
            .ok()?;
        self.declarations.get(position)
    }

    pub(crate) fn values(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'storage Declaration>
    + ExactSizeIterator
    + Clone
    + use<'storage, Declaration> {
        let replacements = self.replacements;
        self.declarations.iter().map(move |row| {
            replacements
                .binary_search_by(|replacement| replacement.identity().cmp(row.identity()))
                .ok()
                .map_or(row, |position| &replacements[position])
        })
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

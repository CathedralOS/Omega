//! Staged host replacements retain the exact captured argument and private
//! reference-selection identity. Storage contents have a separate shared owner.

use crate::effects::TerminalEffectRejection;
use crate::structural_storage::contents::TerminalBoundaryStorageValue;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use std::sync::Arc;
use terminal_psi::{StructuralAccess, StructuralPathSegment};

#[derive(Debug)]
pub struct TerminalBoundaryStructuralBuffer {
    pub(super) original_storage:
        Option<crate::effects::original_storage::TerminalOriginalStorageLoan>,
    pub(super) identity: Arc<()>,
    pub(super) argument_index: usize,
    pub(super) access: StructuralAccess,
    pub(super) structural_type: StructuralTypeId,
    pub(super) values: Vec<TerminalBoundaryStorageValue>,
    pub(super) references: Vec<BoundaryReferenceReplacement>,
}

/// An opaque selection of one endpoint in this invocation's immutable snapshot.
/// A host integer or native pointer cannot construct its private capture identity.
#[derive(Debug, Clone)]
pub struct TerminalBoundaryReferenceSelection {
    pub(super) identity: Arc<()>,
    pub(super) argument_index: usize,
    pub(super) path: Vec<Path>,
    pub(super) access: StructuralAccess,
}

#[derive(Debug)]
pub(super) struct BoundaryReferenceReplacement {
    pub path: Vec<StructuralPathSegment>,
    pub source: TerminalBoundaryReferenceSelection,
    pub write: ReferenceOutputWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReferenceOutputWrite {
    Descriptor,
    Address,
}

impl TerminalBoundaryStructuralBuffer {
    pub fn original_storage(
        &self,
    ) -> Option<&crate::effects::original_storage::TerminalOriginalStorageLoan> {
        self.original_storage.as_ref()
    }
    pub fn argument_index(&self) -> usize {
        self.argument_index
    }
    pub fn access(&self) -> StructuralAccess {
        self.access
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        self.structural_type
    }
    pub fn values(&self) -> &[TerminalBoundaryStorageValue] {
        &self.values
    }

    /// Stage an authenticated snapshot endpoint in an existing reference-output
    /// slot. This changes only the request buffer; all inputs and all output
    /// declarations are checked together before publishing any guest write.
    pub fn replace_reference(
        &mut self,
        path: &[StructuralPathSegment],
        source: TerminalBoundaryReferenceSelection,
    ) -> Result<(), TerminalEffectRejection> {
        self.stage_reference_replacement(path, source, ReferenceOutputWrite::Descriptor)
    }

    /// Stage an address-only foreign write. Completion retains the old scalar
    /// storage interpretation and requires the selected endpoint to have the
    /// same captured interpretation before publishing any guest storage.
    pub fn replace_reference_address(
        &mut self,
        path: &[StructuralPathSegment],
        source: TerminalBoundaryReferenceSelection,
    ) -> Result<(), TerminalEffectRejection> {
        self.stage_reference_replacement(path, source, ReferenceOutputWrite::Address)
    }

    fn stage_reference_replacement(
        &mut self,
        path: &[StructuralPathSegment],
        source: TerminalBoundaryReferenceSelection,
        write: ReferenceOutputWrite,
    ) -> Result<(), TerminalEffectRejection> {
        if self.access != StructuralAccess::MutableBorrow
            || self
                .references
                .iter()
                .any(|replacement| replacement.path == path)
        {
            return Err(TerminalEffectRejection::new(
                "reference output is not one unique mutable slot",
            ));
        }
        self.references.push(BoundaryReferenceReplacement {
            path: path.to_vec(),
            source,
            write,
        });
        Ok(())
    }

    /// Stage complete active contents. Completion checks nominal membership,
    /// field restrictions, original identity and retained opaque children before
    /// any interpreter storage changes. This is not a native tag constructor.
    pub fn replace(
        &mut self,
        values: Vec<TerminalBoundaryStorageValue>,
    ) -> Result<(), TerminalEffectRejection> {
        if self.access != StructuralAccess::MutableBorrow {
            return Err(TerminalEffectRejection::new(
                "owned or shared boundary storage cannot be replaced",
            ));
        }
        self.values = values;
        Ok(())
    }
}

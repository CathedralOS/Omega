//! An authenticated invocation may realize exposed scalar cells in their
//! original allocation. Geometry stays with the host's admitted target product;
//! neither a view's shape nor a per-call snapshot establishes that allocation.

use crate::{TerminalScalarBacking, TerminalScalarValue};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
use std::rc::Rc;
use terminal_psi::{StructuralAccess, TerminalPsiIdentity};

/// Execution-only resident association for an immutable byte allocation.
/// Portable byte values keep their plain backing; the owner handle pins that
/// exact allocation while native residence remains established.
pub(crate) struct OriginalByteCell {
    pub(crate) owner: crate::byte_sequences::ByteSequenceView,
    pub(crate) backing: Rc<dyn TerminalScalarBacking>,
}

/// One execution's exact original allocation. Numeric identities alone carry
/// no access; requests originate only after the engine's call preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalOriginalStorageRoot {
    pub(crate) artifact: TerminalPsiIdentity,
    pub(crate) namespace: u64,
    pub(crate) allocation: u64,
}

/// Keep the allocation declaration established during original capture.
/// Borrowed descriptors can present another type at the same storage path;
/// those presentations never replace the allocation's own geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OriginalStorageAllocation {
    pub(crate) root: TerminalOriginalStorageRoot,
    pub(crate) structural_type: StructuralTypeId,
}
impl TerminalOriginalStorageRoot {
    pub fn artifact(&self) -> TerminalPsiIdentity {
        self.artifact
    }
    pub fn namespace(&self) -> u64 {
        self.namespace
    }
    pub fn allocation(&self) -> u64 {
        self.allocation
    }
}

/// Only initialized cells inside the invocation's exposed range are supplied.
/// Unexposed sibling bytes confer no permission and cannot be fabricated.
#[derive(Debug)]
pub struct TerminalOriginalScalarValue {
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
    pub(crate) value: TerminalScalarValue,
    pub(crate) backing: Option<Rc<dyn TerminalScalarBacking>>,
}
impl TerminalOriginalScalarValue {
    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }
    pub fn value(&self) -> TerminalScalarValue {
        self.value
    }
    /// Identity of an already established scalar endpoint, for retaining the
    /// same home rather than copying it back from a captured observation.
    pub fn backing(&self) -> Option<&Rc<dyn TerminalScalarBacking>> {
        self.backing.as_ref()
    }
}

#[derive(Debug)]
pub struct TerminalOriginalStorageRequest {
    pub(crate) root: TerminalOriginalStorageRoot,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) argument_index: usize,
    pub(crate) referent: Vec<CanonicalStructuralPathSegment>,
    pub(crate) access: StructuralAccess,
    pub(crate) values: Vec<TerminalOriginalScalarValue>,
    pub(crate) argument_path: Vec<CanonicalStructuralPathSegment>,
    pub(crate) view_type: StructuralTypeId,
    pub(crate) references: Vec<TerminalOriginalReferenceValue>,
    pub(crate) scalar_projection: Option<
        Vec<(
            Vec<CanonicalStructuralPathSegment>,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    >,
    pub(crate) byte_extent: Option<usize>,
    pub(crate) byte_window: Option<(usize, usize)>,
}
impl TerminalOriginalStorageRequest {
    /// Full actual byte allocation, independently of the exposed window. A
    /// typed fixed-array root retains its ordinary layout recipe instead.
    pub fn byte_extent(&self) -> Option<usize> {
        self.byte_extent
    }
    pub fn root(&self) -> TerminalOriginalStorageRoot {
        self.root
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        self.structural_type
    }
    pub fn argument_index(&self) -> usize {
        self.argument_index
    }
    pub fn referent(&self) -> &[CanonicalStructuralPathSegment] {
        &self.referent
    }
    pub fn access(&self) -> StructuralAccess {
        self.access
    }
    pub fn argument_path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.argument_path
    }
    pub fn view_type(&self) -> StructuralTypeId {
        self.view_type
    }
    pub fn references(&self) -> &[TerminalOriginalReferenceValue] {
        &self.references
    }
    pub fn loan(&self) -> TerminalOriginalStorageLoan {
        TerminalOriginalStorageLoan {
            root: self.root,
            referent: self.referent.clone(),
            scalar_projection: self.scalar_projection.clone(),
            byte_window: self.byte_window,
        }
    }
    pub fn values(&self) -> &[TerminalOriginalScalarValue] {
        &self.values
    }
}

/// Buffer metadata retains the original root/range selected by core custody.
/// It is not constructible by an effect handler and grants no extra access.
#[derive(Debug, Clone)]
pub struct TerminalOriginalStorageLoan {
    pub(crate) root: TerminalOriginalStorageRoot,
    pub(crate) referent: Vec<CanonicalStructuralPathSegment>,
    pub(crate) scalar_projection: Option<
        Vec<(
            Vec<CanonicalStructuralPathSegment>,
            Vec<CanonicalStructuralPathSegment>,
        )>,
    >,
    pub(crate) byte_window: Option<(usize, usize)>,
}
impl TerminalOriginalStorageLoan {
    /// Captured byte coordinate and live length. For raw bytes the coordinate
    /// is root-relative. For typed arrays it is relative to the selected array;
    /// scalar_path supplies exact enclosing-root leaves, and the target layout
    /// owner must derive physical displacement from them, never this coordinate.
    pub fn byte_window(&self) -> Option<(usize, usize)> {
        self.byte_window
    }
    pub fn root(&self) -> TerminalOriginalStorageRoot {
        self.root
    }
    pub fn referent(&self) -> &[CanonicalStructuralPathSegment] {
        &self.referent
    }

    /// Resolve a logical view leaf using the exact core-captured endpoint.
    /// A re-view cannot expose siblings merely because its root owns them.
    pub fn scalar_path(
        &self,
        view_path: &[CanonicalStructuralPathSegment],
    ) -> Option<Vec<CanonicalStructuralPathSegment>> {
        if let Some(projection) = &self.scalar_projection {
            let position = projection
                .binary_search_by(|(path, _)| path.as_slice().cmp(view_path))
                .ok()?;
            Some(projection[position].1.clone())
        } else {
            let mut path = self.referent.clone();
            path.extend_from_slice(view_path);
            Some(path)
        }
    }
}

pub type TerminalOriginalScalarBackings = Vec<Vec<Rc<dyn TerminalScalarBacking>>>;

/// A descriptor edge authenticated by the engine's live typed projection.
/// The slot is relative to its original allocation; the target is another
/// sealed original range. Neither path supplies target layout or authority.
#[derive(Debug)]
pub struct TerminalOriginalReferenceValue {
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
    pub(crate) target: TerminalOriginalStorageLoan,
    pub(crate) referent_type: StructuralTypeId,
    pub(crate) access: StructuralAccess,
}
impl TerminalOriginalReferenceValue {
    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }
    pub fn target(&self) -> &TerminalOriginalStorageLoan {
        &self.target
    }
    pub fn referent_type(&self) -> StructuralTypeId {
        self.referent_type
    }
    pub fn access(&self) -> StructuralAccess {
        self.access
    }
}

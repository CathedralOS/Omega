//! An authenticated invocation may realize exposed scalar cells in their
//! original allocation. Geometry stays with the host's admitted target product;
//! neither a view's shape nor a per-call snapshot establishes that allocation.

use crate::{TerminalScalarBacking, TerminalScalarValue};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralTypeId};
use std::rc::Rc;
use terminal_psi::{StructuralAccess, TerminalPsiIdentity};

/// One execution's exact original allocation. Numeric identities alone carry
/// no access; requests originate only after the engine's call preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalOriginalStorageRoot {
    pub(crate) artifact: TerminalPsiIdentity,
    pub(crate) namespace: u64,
    pub(crate) allocation: u64,
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
}
impl TerminalOriginalStorageRequest {
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
}
impl TerminalOriginalStorageLoan {
    pub fn root(&self) -> TerminalOriginalStorageRoot {
        self.root
    }
    pub fn referent(&self) -> &[CanonicalStructuralPathSegment] {
        &self.referent
    }
}

pub type TerminalOriginalScalarBackings = Vec<Vec<Rc<dyn TerminalScalarBacking>>>;

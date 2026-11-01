//! Producer-known addresses for independent source review. These contain no
//! physical geometry and are not part of portable Terminal semantics. A review
//! consumer resolves them against its supplied candidate; an address is not
//! evidence that the candidate still declares the expected contract.

use crate::{PlanLaidTargetLayout, types::TypeReferenceHandle};
use arena::Handle;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use symbols::SymbolHandle;

/// A source occurrence and the lexical substitutions under which it was
/// collected. These are review addresses, not a target storage recipe.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct SourceTypeApplication {
    pub reference: TypeReferenceHandle,
    pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SourceRecordCorrespondence {
    pub record: StructuralTypeId,
    pub declaration: SymbolHandle,
    pub application: SourceTypeApplication,
    pub storage_identity: String,
    pub schema: SymbolHandle,
    pub policy: SymbolHandle,
    pub layout: Option<SourceLayoutPlanCoordinate>,
    pub fields: Vec<SourceFieldCorrespondence>,
    pub realizations: Vec<SourceLayoutRealizationCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SourceFieldCorrespondence {
    pub field: StructuralFieldId,
    pub declaration: SymbolHandle,
    pub schema: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SourceLayoutPlanCoordinate {
    /// PlanLaidLayout currently lives in an owner-local vector. The original
    /// application guards that coordinate against a different supplied roster.
    pub ordinal: u32,
    pub application: TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SourceLayoutRealizationCoordinate {
    /// Empty means the invariant realization, never a target representative.
    pub target: String,
    /// Zero means invariant geometry on the named plan; otherwise the exact
    /// target alternative owns its source callback-slot application telescope.
    pub member: Handle<PlanLaidTargetLayout>,
}

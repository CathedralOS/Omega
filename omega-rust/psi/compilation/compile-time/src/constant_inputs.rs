//! Actual checked constant execution inputs and concrete admission judgments.
//!
//! An ordered observation sink keeps transaction truncation common while the
//! two kinds remain distinct. A snapshot judgment never becomes an executed
//! receipt, and a typed initializer probe is not a checked runtime closure.

use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionHandle, types::TypeReferenceHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedConstantInput {
    Executed(checked_interpreter::ExecutedCheckedConstantInput),
    SnapshotAdmission(CheckedConstantSnapshotAdmission),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotOperand {
    pub receiver: bool,
    /// Handle in the retained original program.
    pub original: ExpressionHandle,
    /// Corresponding operand in the retained checked snapshot program.
    pub snapshot: ExpressionHandle,
}

#[derive(Clone)]
pub(crate) enum SnapshotSourceProgram {
    TypedProbe(Arc<TypedTrees>),
    Checked(crate::CheckedBuildProgram),
}
impl SnapshotSourceProgram {
    pub(crate) fn typed(&self) -> &TypedTrees {
        match self {
            Self::TypedProbe(program) => program,
            Self::Checked(program) => program.typed(),
        }
    }
}

/// One successful ordinary snapshot check and its original operand association.
/// Private issuance follows concrete failure and operational admission. Identity
/// is this actual judgment owner, not equality of final scalar values or trees.
#[derive(Clone)]
pub struct CheckedConstantSnapshotAdmission(Arc<SnapshotAdmissionOwner>);
struct SnapshotAdmissionOwner {
    original: SnapshotSourceProgram,
    checked: crate::CheckedBuildProgram,
    /// Original declaration used to scope the generated probe; this is not
    /// a claim that the authored call occurs in that declaration body.
    probe_source_owner: SymbolHandle,
    original_call: ExpressionHandle,
    probe_owner: SymbolHandle,
    probe_call: ExpressionHandle,
    original_return_type: TypeReferenceHandle,
    operands: Arc<[SnapshotOperand]>,
    endpoint_stand_ins: Arc<[ExpressionHandle]>,
    selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
}
impl PartialEq for CheckedConstantSnapshotAdmission {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CheckedConstantSnapshotAdmission {}
impl std::fmt::Debug for CheckedConstantSnapshotAdmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CheckedConstantSnapshotAdmission")
            .field("original_call", &self.0.original_call)
            .field("probe_owner", &self.0.probe_owner)
            .field("probe_call", &self.0.probe_call)
            .finish_non_exhaustive()
    }
}
impl CheckedConstantSnapshotAdmission {
    pub(crate) fn issue(
        original: SnapshotSourceProgram,
        checked: crate::CheckedBuildProgram,
        probe_source_owner: SymbolHandle,
        original_call: ExpressionHandle,
        probe_owner: SymbolHandle,
        probe_call: ExpressionHandle,
        original_return_type: TypeReferenceHandle,
        operands: Vec<SnapshotOperand>,
        endpoint_stand_ins: Vec<ExpressionHandle>,
        selection_authority: Option<Arc<dyn crate::BuildTimeSelectionAuthority>>,
    ) -> Self {
        Self(Arc::new(SnapshotAdmissionOwner {
            original,
            checked,
            probe_source_owner,
            original_call,
            probe_owner,
            probe_call,
            original_return_type,
            operands: operands.into(),
            endpoint_stand_ins: endpoint_stand_ins.into(),
            selection_authority,
        }))
    }
    pub fn original_program(&self) -> &TypedTrees {
        self.0.original.typed()
    }
    pub fn original_checked_program(&self) -> Option<&crate::CheckedBuildProgram> {
        match &self.0.original {
            SnapshotSourceProgram::Checked(program) => Some(program),
            SnapshotSourceProgram::TypedProbe(_) => None,
        }
    }
    pub fn checked_program(&self) -> &crate::CheckedBuildProgram {
        &self.0.checked
    }
    pub fn probe_source_owner(&self) -> SymbolHandle {
        self.0.probe_source_owner
    }
    pub fn original_call(&self) -> ExpressionHandle {
        self.0.original_call
    }
    pub fn probe_owner(&self) -> SymbolHandle {
        self.0.probe_owner
    }
    pub fn probe_call(&self) -> ExpressionHandle {
        self.0.probe_call
    }
    pub fn original_return_type(&self) -> TypeReferenceHandle {
        self.0.original_return_type
    }
    pub fn operands(&self) -> &[SnapshotOperand] {
        &self.0.operands
    }
    /// The original owner preserves pre-substitution nodes; this checked owner
    /// preserves the stand-ins at the same coordinates in its separate arena.
    pub fn endpoint_stand_ins(&self) -> &[ExpressionHandle] {
        &self.0.endpoint_stand_ins
    }
    pub fn selection_authority(&self) -> Option<&Arc<dyn crate::BuildTimeSelectionAuthority>> {
        self.0.selection_authority.as_ref()
    }
}

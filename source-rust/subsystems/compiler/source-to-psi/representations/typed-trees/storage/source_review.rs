use crate::source_state::{PlanLaidTargetLayout, types::TypeReferenceHandle};
use arena::Handle;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use symbols::SymbolHandle;

/// Typed occurrence identity belongs to the frozen source kept beside a
/// publication. Spans are diagnostic positions, never this association.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceNode {
    Expression(crate::source_state::expression::ExpressionHandle),
    Statement(crate::source_state::statement::StatementHandle),
}

/// One actual emitted invocation and the typed occurrence that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCallCorrespondence {
    /// The lowered machine that executes this invocation.
    pub machine: semantic_vocabulary::MachineId,
    /// The original member before call-component folding.
    pub source_member: semantic_vocabulary::MachineId,
    pub site: semantic_vocabulary::InvocationSite,
    /// Combining a call cycle into one loop does not change this original
    /// source owner or its static argument environment.
    pub owner: SymbolHandle,
    pub source: SourceNode,
}

/// Actual source arguments stay separate from the authored type occurrence.
/// A frame substitutes its arguments simultaneously; earlier frames supply
/// the context in which those arguments were written. Records from one
/// machine share this context instead of copying its complete call history.
#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SourceTypeScope {
    pub parent: Option<std::sync::Arc<SourceTypeScope>>,
    pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    pub rigid_parameters: Vec<SymbolHandle>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct SourceTypeApplication {
    pub reference: TypeReferenceHandle,
    pub substitutions: Vec<(SymbolHandle, TypeReferenceHandle)>,
    pub scope: std::sync::Arc<SourceTypeScope>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct SourceRecordCorrespondence {
    pub record: StructuralTypeId,
    pub declaration: SymbolHandle,
    /// The type occurrence the record was read from. `None` when the
    /// program never writes the type: a declaration with no type
    /// parameter, read as a declaration of the program, is the whole
    /// identity of its record. A generic record always has one.
    pub application: Option<SourceTypeApplication>,
    /// The authored cleanup choice belongs to this exact type application.
    pub cleanup: Option<SourceRecordCleanup>,
    pub storage_identity: String,
    pub schema: SymbolHandle,
    pub policy: SymbolHandle,
    pub layout: Option<SourceLayoutPlanCoordinate>,
    pub fields: Vec<SourceFieldCorrespondence>,
    pub realizations: Vec<SourceLayoutRealizationCoordinate>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct SourceRecordCleanup {
    pub selection: crate::source_state::domain::CleanupSelection,
    pub target: semantic_vocabulary::MachineId,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct SourceFieldCorrespondence {
    pub field: StructuralFieldId,
    pub declaration: SymbolHandle,
    pub schema: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct SourceLayoutPlanCoordinate {
    pub ordinal: u32,
    pub application: TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct SourceLayoutRealizationCoordinate {
    pub target: String,
    pub member: Handle<PlanLaidTargetLayout>,
}

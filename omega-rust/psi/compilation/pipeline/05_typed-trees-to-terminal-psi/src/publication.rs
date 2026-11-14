//! Source correspondence retained by the driver, outside Terminal judgments.

use crate::Elaboration;
use semantic_vocabulary::{IdTable, MachineId, OperationId};

#[derive(Debug, Clone, Default)]
pub struct SourceCorrespondence {
    pub records: Vec<typed_trees::SourceRecordCorrespondence>,
    pub designated: IdTable<MachineId, Vec<(OperationId, DesignatedOperands)>>,
    /// The outer table is indexed by the emitted caller. Each row also keeps
    /// the source member's specialization because a call component can place
    /// several source members in that same emitted machine. An expression may
    /// have several sites; none is selected by a source span or callee name.
    pub calls: IdTable<
        MachineId,
        Vec<(
            MachineId,
            typed_trees::expression::ExpressionHandle,
            typed_trees::statement::StatementHandle,
            semantic_vocabulary::InvocationSite,
        )>,
    >,
}

pub(crate) struct PreparedProgram<'source> {
    pub program: Elaboration,
    pub source: SourceCorrespondence,
    pub selected_calls: Vec<crate::compilation::SelectedSourceCall<'source>>,
}

/// The designated operands of one Build selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesignatedOperands {
    /// The request as the typed trees hold it
    /// (`ExpressionTable::build_selection_request`).
    pub request: arena::Handle<typed_trees::expression::BuildSelectionRequest>,
    /// The machine whose source wrote the selection. The declarations a
    /// selection names are resolved for the place that wrote it
    /// (scoped_execution.md: "using the occurrence's package authority,
    /// exact target, and complete expected slot/requirement signature"),
    /// and that is not always the machine whose lowered body holds the
    /// call: a call component lowers its members into one machine.
    pub selecting: MachineId,
}

//! Source correspondence retained by the driver, outside Terminal judgments.

use crate::Elaboration;
use semantic_vocabulary::{IdTable, MachineId, OperationId};

/// Source forms in the same order as the Terminal compilation: the primary
/// closure, its generic forms, then its conformance witnesses. A failed reader
/// keeps an absent row and cannot produce an accepted publication.
#[derive(Debug, Clone, Default)]
pub struct SourceCorrespondence {
    pub forms: Vec<Option<SourceFormCorrespondence>>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceFormCorrespondence {
    pub records: Vec<typed_trees::SourceRecordCorrespondence>,
    pub designated: IdTable<MachineId, Vec<(OperationId, DesignatedOperands)>>,
    pub calls: Vec<typed_trees::SourceCallCorrespondence>,
    pub constructions: Vec<(
        MachineId,
        OperationId,
        symbols::SymbolHandle,
        typed_trees::expression::ExpressionHandle,
    )>,
    pub folded_calls: Vec<(
        MachineId,
        semantic_vocabulary::EdgeId,
        symbols::SymbolHandle,
        typed_trees::SourceNode,
    )>,
    pub selection_questions: Option<crate::selection_questions::RawSelectionQuestions>,
}

pub(crate) struct PreparedProgram {
    pub program: Elaboration,
    pub source: SourceFormCorrespondence,
}

/// Reading and lowering retain the exact typed occurrence until its emitted
/// call coordinate is recorded. The Terminal checker receives only the
/// diagnostic position, while source publication keeps the typed association.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    Authored(source::SourceSpan),
    AuthoredNode {
        owner: symbols::SymbolHandle,
        node: typed_trees::SourceNode,
        span: Option<source::SourceSpan>,
    },
    Unpositioned(crate::Unpositioned),
}

impl SourceOrigin {
    pub fn span(self) -> Option<source::SourceSpan> {
        match self {
            Self::Authored(span) => Some(span),
            Self::AuthoredNode { span, .. } => span,
            Self::Unpositioned(_) => None,
        }
    }

    pub fn node(self) -> Option<(symbols::SymbolHandle, typed_trees::SourceNode)> {
        match self {
            Self::AuthoredNode { owner, node, .. } => Some((owner, node)),
            Self::Authored(_) | Self::Unpositioned(_) => None,
        }
    }

    pub(crate) fn of(span: source::SourceSpan) -> Self {
        if span.span.start < span.span.end {
            Self::Authored(span)
        } else {
            Self::Unpositioned(crate::Unpositioned::SpanAbsent)
        }
    }
}

impl From<SourceOrigin> for terminal_check_tables::program::SourceOrigin {
    fn from(source: SourceOrigin) -> Self {
        match source {
            SourceOrigin::Authored(span) => Self::Authored(span),
            SourceOrigin::AuthoredNode {
                span: Some(span), ..
            } => Self::Authored(span),
            SourceOrigin::AuthoredNode { span: None, .. } => {
                Self::Unpositioned(crate::Unpositioned::SpanAbsent)
            }
            SourceOrigin::Unpositioned(reason) => Self::Unpositioned(reason),
        }
    }
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

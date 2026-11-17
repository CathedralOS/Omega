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
    /// Each requirement a Build awaits a provider for
    /// (`DesignatedOperands::readiness`), with the identity of the trait that
    /// declares it. The module states the application's identity on the
    /// service row and the receiving slots on its boundaries; a Build that
    /// awaits a provider and calls none of its requirements has no boundary
    /// to read the trait from, so the trait is stated here.
    pub readiness: Vec<(
        semantic_vocabulary::ServiceId,
        terminal_psi::NominalDeclarationIdentity,
    )>,
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

pub(crate) struct PreparedProgram<'source> {
    pub program: Elaboration,
    pub outcomes: semantic_vocabulary::IdTable<MachineId, crate::outcome::Outcome>,
    pub source: SourceFormCorrespondence,
    pub selected_calls: Vec<crate::compilation::SelectedSourceCall<'source>>,
    pub prepared_applications: Vec<(MachineId, crate::declare::PreparedApplication)>,
    pub pending_publication: Option<PendingSourcePublication<'source>>,
}

/// Exact reader outputs whose publication depends on the real proof scope.
/// Provider schemas follow calling schemas, matching their shared source order.
pub(crate) struct PendingSourcePublication<'source> {
    pub source: &'source typed_trees::TypedTrees,
    pub context: MachineId,
    pub calling: Option<(
        crate::read::calling::SourceCallingPolicies,
        crate::read::storage_metadata::PreparedCallingDescriptions,
    )>,
    pub providers: Option<crate::provider_selections::SourceProviderSelections<'source>>,
}

impl PreparedProgram<'_> {
    /// Extraction cannot discard source obligations that have not been published.
    pub(crate) fn require_check_form(&self) -> terminal_check_tables::Checked<()> {
        if let Some(pending) = &self.pending_publication {
            let calling = pending
                .calling
                .as_ref()
                .is_some_and(|(policies, descriptions)| {
                    !policies.requests.is_empty()
                        || !policies.signatures.is_empty()
                        || !policies.entries.is_empty()
                        || !descriptions.signatures.is_empty()
                });
            let providers = pending
                .providers
                .as_ref()
                .is_some_and(|prepared| !prepared.rows.is_empty());
            if calling || providers {
                return Err(crate::read::services::unavailable(
                    pending.context,
                    format!(
                        "a check form still owes source publication (calling declarations: {calling}; provider declarations: {providers}); actual proof-scope publication is required"
                    ),
                ));
            }
        }
        if let Some(selected) = self.selected_calls.first() {
            return Err(crate::read::services::unavailable(
                selected.key.0,
                "a check form still has exact selected-call publication associations",
            ));
        }
        Ok(())
    }

    pub(crate) fn finalize_check_form(mut self) -> terminal_check_tables::Checked<Self> {
        if let Some((owner, _)) = self.prepared_applications.first() {
            return Err(crate::read::services::unavailable(
                *owner,
                "check-form source publication needs its completed specialization headers",
            ));
        }
        let Some(pending) = self.pending_publication.as_ref() else {
            self.require_check_form()?;
            return Ok(self);
        };
        let typed = pending.source;
        if self.selected_calls.is_empty() {
            self.selected_calls = crate::compilation::prepare_selected_signatures(
                typed,
                &self.program,
                &self.source,
            )?;
        }
        // The record and its exact selected calls are consumed by the same
        // publisher as a primary closure, after this form's headers are closed.
        let selected = std::mem::take(&mut self.selected_calls);
        let pending = self.pending_publication.take();
        finalize_source(typed, &mut self.program, &selected, pending)?;
        self.require_check_form()?;
        Ok(self)
    }

    pub(crate) fn into_check_form(self) -> terminal_check_tables::Checked<Elaboration> {
        let prepared = self.finalize_check_form()?;
        prepared.require_check_form()?;
        Ok(prepared.program)
    }
}

/// Called once after any specialization headers are closed. This is the only
/// source-publication dependency order: real proof scope, calling, providers.
pub(crate) fn finalize_source(
    typed: &typed_trees::TypedTrees,
    program: &mut Elaboration,
    selected_calls: &[crate::compilation::SelectedSourceCall<'_>],
    pending: Option<PendingSourcePublication<'_>>,
) -> terminal_check_tables::Checked<()> {
    if let Some(pending) = &pending
        && !std::ptr::eq(typed, pending.source)
    {
        return Err(crate::read::services::unavailable(
            pending.context,
            "pending source publication belongs to another frozen source owner",
        ));
    }
    crate::proof_scope::declare(typed, program, selected_calls)?;
    if let Some(pending) = pending {
        if let Some((policies, descriptions)) = pending.calling {
            crate::calling_publication::publish(
                typed,
                pending.context,
                &policies,
                &descriptions,
                program,
            )?;
        }
        if let Some(providers) = pending.providers {
            crate::provider_selections::publish(pending.context, &providers, program)?;
        }
    }
    Ok(())
}

/// Reading and lowering retain the exact typed occurrence until its emitted
/// call coordinate is recorded. The Terminal checker receives only the
/// diagnostic position, while source publication keeps the typed association.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    Authored(source::SourceSpan),
    AuthoredNode {
        source_member: MachineId,
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

    pub fn source_member(self) -> Option<MachineId> {
        match self {
            Self::AuthoredNode { source_member, .. } => Some(source_member),
            Self::Authored(_) | Self::Unpositioned(_) => None,
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
    /// The request and the protocol operation as the typed trees hold
    /// them (`ExpressionTable::build_selection_call`).
    pub request: arena::Handle<typed_trees::expression::BuildSelectionCall>,
    /// The machine whose source wrote the selection. The declarations a
    /// selection names are resolved for the place that wrote it
    /// (scoped_execution.md: "using the occurrence's package authority,
    /// exact target, and complete expected slot/requirement signature"),
    /// and that is not always the machine whose lowered body holds the
    /// call: a call component lowers its members into one machine.
    pub selecting: MachineId,
    /// For provider readiness, the awaited requirement as the module's
    /// service row: the exact application of the boundary trait the
    /// selection names (provider_selection.md, "Build provider readiness").
    pub readiness: Option<semantic_vocabulary::ServiceId>,
}

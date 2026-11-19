//! Source correspondence retained by the driver, outside Terminal judgments.

use crate::lowering::Elaboration;
use arena::IdTable;
use semantic_vocabulary::{MachineId, OperationId};

/// Source forms in the same order as the Terminal compilation: the primary
/// closure, its generic forms, then its conformance witnesses. A failed reader
/// keeps an absent row and cannot produce an accepted publication.
#[derive(Debug, Clone, Default)]
pub struct SourceCorrespondence {
    pub forms: Vec<Option<SourceFormCorrespondence>>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceFormCorrespondence {
    pub records: Vec<crate::source_state::SourceRecordCorrespondence>,
    /// Declaration-owned review facts from this exact form. They become
    /// available only after every form of the source compilation accepts.
    pub review_requirements: Vec<RequirementReview>,
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
    pub calls: Vec<crate::source_state::SourceCallCorrespondence>,
    pub constructions: Vec<(
        MachineId,
        OperationId,
        symbols::SymbolHandle,
        crate::source_state::expression::ExpressionHandle,
    )>,
    pub folded_calls: Vec<(
        MachineId,
        semantic_vocabulary::EdgeId,
        symbols::SymbolHandle,
        crate::source_state::SourceNode,
    )>,
    pub selection_questions: Option<crate::lowering::selection_questions::RawSelectionQuestions>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementReviewMetadata {
    pub parameter_types: Vec<String>,
    pub result_type: Option<String>,
    pub lifetime_parameter_count: usize,
    /// None is an open ceiling; Some(empty) is a closed empty ceiling.
    pub service_reach: Option<Vec<symbols::SymbolHandle>>,
    pub crash_is_open: bool,
    pub returned_parameters: Vec<u32>,
    pub parameter_qualifications: Vec<(u32, symbols::SymbolHandle)>,
    pub suspends: bool,
    pub blocks: bool,
    pub terminates: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementReview {
    pub declaration: symbols::SymbolHandle,
    /// Unsupported metadata cannot turn into an empty policy claim.
    pub metadata: Result<RequirementReviewMetadata, String>,
}

pub(crate) struct PreparedProgram<'source> {
    pub program: Elaboration,
    pub outcomes: arena::IdTable<MachineId, crate::lowering::outcome::Outcome>,
    pub source: SourceFormCorrespondence,
    pub selected_calls: Vec<crate::lowering::compilation::SelectedSourceCall<'source>>,
    pub prepared_applications: Vec<(MachineId, crate::lowering::declare::PreparedApplication)>,
    pub pending_publication: Option<PendingSourcePublication<'source>>,
}

/// Exact reader outputs whose publication depends on the real proof scope.
/// Provider schemas follow calling schemas, matching their shared source order.
pub(crate) struct PendingSourcePublication<'source> {
    pub source: &'source crate::source_state::TypedTrees,
    pub context: MachineId,
    pub calling: Option<(
        crate::lowering::read::calling::SourceCallingPolicies,
        crate::lowering::read::storage_metadata::PreparedCallingDescriptions,
    )>,
    pub providers: Option<crate::lowering::provider_selections::SourceProviderSelections<'source>>,
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
                return Err(crate::lowering::read::services::unavailable(
                    pending.context,
                    format!(
                        "a check form still owes source publication (calling declarations: {calling}; provider declarations: {providers}); actual proof-scope publication is required"
                    ),
                ));
            }
        }
        if let Some(selected) = self.selected_calls.first() {
            return Err(crate::lowering::read::services::unavailable(
                selected.key.0,
                "a check form still has exact selected-call publication associations",
            ));
        }
        Ok(())
    }

    pub(crate) fn finalize_check_form(mut self) -> terminal_check_tables::Checked<Self> {
        if let Some((owner, _)) = self.prepared_applications.first() {
            return Err(crate::lowering::read::services::unavailable(
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
            self.selected_calls = crate::lowering::compilation::prepare_selected_signatures(
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
    typed: &crate::source_state::TypedTrees,
    program: &mut Elaboration,
    selected_calls: &[crate::lowering::compilation::SelectedSourceCall<'_>],
    pending: Option<PendingSourcePublication<'_>>,
) -> terminal_check_tables::Checked<()> {
    if let Some(pending) = &pending
        && !std::ptr::eq(typed, pending.source)
    {
        return Err(crate::lowering::read::services::unavailable(
            pending.context,
            "pending source publication belongs to another frozen source owner",
        ));
    }
    crate::lowering::proof_scope::declare(typed, program, selected_calls)?;
    if let Some(pending) = pending {
        if let Some((policies, descriptions)) = pending.calling {
            crate::lowering::calling_publication::publish(
                typed,
                pending.context,
                &policies,
                &descriptions,
                program,
            )?;
        }
        if let Some(providers) = pending.providers {
            crate::lowering::provider_selections::publish(pending.context, &providers, program)?;
        }
    }
    Ok(())
}

/// Reading and lowering retain the exact typed occurrence until its emitted
/// call coordinate is recorded. The Terminal checker receives only the
/// diagnostic position, while source publication keeps the typed association.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    Authored(source_text::SourceSpan),
    AuthoredNode {
        source_member: MachineId,
        owner: symbols::SymbolHandle,
        node: crate::source_state::SourceNode,
        span: Option<source_text::SourceSpan>,
    },
    Unpositioned(crate::lowering::Unpositioned),
}

impl SourceOrigin {
    pub fn span(self) -> Option<source_text::SourceSpan> {
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

    pub fn node(self) -> Option<(symbols::SymbolHandle, crate::source_state::SourceNode)> {
        match self {
            Self::AuthoredNode { owner, node, .. } => Some((owner, node)),
            Self::Authored(_) | Self::Unpositioned(_) => None,
        }
    }

    pub(crate) fn of(span: source_text::SourceSpan) -> Self {
        if span.span.start < span.span.end {
            Self::Authored(span)
        } else {
            Self::Unpositioned(crate::lowering::Unpositioned::SpanAbsent)
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
                Self::Unpositioned(crate::lowering::Unpositioned::SpanAbsent)
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
    pub request: arena::Handle<crate::source_state::expression::BuildSelectionCall>,
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

//! Source correspondence retained by the driver, outside Terminal judgments.

use arena::IdTable;
use numerics::arithmetic::ArithmeticDomain;
use semantic_vocabulary::{MachineId, OperationId, ScalarType};

/// Source forms in the same order as the Terminal compilation: the primary
/// closure, its generic forms, then its conformance witnesses. A failed reader
/// keeps an absent row and cannot produce an accepted publication.
#[derive(Debug, Clone, Default)]
pub struct SourceCorrespondence {
    pub forms: Vec<Option<SourceFormCorrespondence>>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceFormCorrespondence {
    /// The public machine whose authored generic body this particular form checks.
    /// A conformance witness or an application does not review the template.
    pub generic_callable: Option<symbols::SymbolHandle>,
    pub records: Vec<crate::source_state::SourceRecordCorrespondence>,
    /// Declaration-owned review facts from this exact form. They become
    /// available only after every form of the source compilation accepts.
    pub review_requirements: Vec<RequirementReview>,
    /// The checked source predicate signature of each boundary in this form.
    /// It uses the concrete arguments and formal lanes of that occurrence.
    pub boundary_signatures: IdTable<
        semantic_vocabulary::BoundaryMachineId,
        terminal_check_tables::contracts::StaticSignature,
    >,
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
    pub selection_questions: Option<crate::selection_questions::RawSelectionQuestions>,
}

/// An effect retains its source owner across check forms. Applied terms also
/// carry the checked, package-qualified application with its complete arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementService {
    pub declaration: symbols::SymbolHandle,
    pub application: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequirementInvocation {
    Parameter(u32),
    Service(RequirementService),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementReviewMetadata {
    pub parameter_types: Vec<String>,
    /// Source parameter positions in the order used by borrow origins and qualifications.
    pub structural_parameter_positions: Vec<u32>,
    pub result_type: Option<String>,
    pub lifetime_parameter_count: usize,
    /// None is an open ceiling; Some(empty) is a closed empty ceiling.
    pub service_reach: Option<Vec<RequirementService>>,
    pub synchronous_invocations: Vec<RequirementInvocation>,
    /// None is open. A closed declaration owns all of its checked crash routes.
    pub published_crash_routes: Option<Vec<terminal_psi::CrashRouteBucket>>,
    pub returned_parameters: Vec<u32>,
    /// Root domain applications keyed by source parameter position.
    pub parameter_qualifications: Vec<(u32, terminal_check_tables::program::DomainInstance)>,
    /// Source parameter position, scalar carrier and selected core arithmetic policy.
    pub parameter_policies: Vec<(u32, ScalarType, ArithmeticDomain)>,
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

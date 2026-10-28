//! Normal-return obligations at the exact completed expression occurrence.
//!
//! Ordinary call and named-operator completion share the declared result roster
//! and publication. The roster is provisional: checked bodies owe it at exits,
//! sealed operators owe their admitted catalog contract, and routed rows still
//! require the independent invocation/custody admission in content checking.

use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;

/// Whether a return type can produce identity rows at all: types whose
/// peeled carrier is a reference yield none, so every caller can skip the
/// row build without consulting the (return_type-keyed) domain rows.
pub(super) fn carries_identity_rows(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !return_type.is_valid() {
        return false;
    }
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(return_type);
    !matches!(
        program.type_reference_table.type_reference(carrier),
        typed_trees::types::TypeReferenceNode::Reference { .. }
    )
}

/// The return-type-driven identity rows: the complete checked child duties
/// plus type-level constraint domains. Independent of the callable's explicit
/// contracts, so callers may memoize them per return type. Routed referent
/// duties are proved independently at the callee's exits, not inferred here.
pub(super) fn type_domain_rows(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let mut domains =
        validation::field_domains::declared_result_field_domain_identities(program, return_type);
    domains.extend(
        validation::field_domains::domain_constraint_identities(program, return_type)
            .into_iter()
            .map(|(symbol, identity)| (Vec::new(), symbol, identity)),
    );
    domains
}

/// Publish exact result paths without selecting a case or creating an owned
/// claim. Conditional case segments stay on each row. No receiving annotation
/// participates, and a root reference keeps its existing referent proof path.
pub(super) fn append_memberships<'rows>(
    semantic: &mut FactPlan,
    build: &mut super::FlowBuildContext<'_>,
    exit: &mut super::CallFlowContexts,
    expression: ExpressionHandle,
    point: ProgramPoint,
    evidence: QualificationEvidence,
    rows: impl IntoIterator<Item = &'rows validation::field_domains::FieldDomainIdentity>,
) {
    let mut refs = HandleSpan::empty();
    for (path, domain_symbol, semantic_domain) in rows {
        let place = crate::semantic::places::append_place(
            semantic,
            facts::PlaceRoot::Expression(expression),
            path,
        );
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::CallEnsures,
            evidence,
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol: *domain_symbol,
                semantic_domain: *semantic_domain,
            },
        });
        semantic.append_ref(&mut refs, fact);
    }
    // Ordinary calls retain their completion context even when an adapter's
    // authority fence removes every provisional row. Keep that sequencing
    // coordinate; an empty context conveys no membership.
    super::reference_spans::append_semantic_context(semantic, build, exit, point, refs);
}

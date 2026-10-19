//! Missing indexed result-establishment diagnostics. Cast obligations belong to
//! the selected domain's normalized theory, never to sibling domains on its carrier.

use super::instance_collection::{CompatibilityKey, IndexedInstance};
use super::labels::{family_label, point_label};
use diagnostics::Diagnostic;
use facts::ProgramPoint;
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

/// A restating write whose declared type names an indexed instance of a family
/// with no predicate body must be handed that instance by the value itself.
/// Membership in such a family cannot follow from proving predicates, so the
/// write establishes nothing on its own: `wiki/spec/language/domains.md` keeps
/// establishment on the value's own route (transported qualification, an
/// authorized `established by` route, or an explicit qualification of a
/// predicate-free, route-free family), and
/// `wiki/spec/resources/placed_access.md` fixes the entire route set of
/// `Extent::Resident<P, T>`: initialization from `Vacant` and an owned `T`, a
/// `ResidentContentTransfer<P, T>` issuance occurrence with its receipt, or
/// forwarding an existing custody.
///
/// A value-position call's result is exactly what its signature and contract
/// row say it is. When neither the declared return type nor an `ensures` on the
/// reserved `result` names any instance of the family, the restatement has no
/// establishment at all, and refusing it here is what stops the local's
/// declared-type seeding (`flow/transfers.rs`) from manufacturing the fact that
/// later call `requires` consume. An instance either route does name keeps its
/// ordinary index-compatibility treatment above, including the distinct
/// normalized instance refusal, and a predicate-bearing domain keeps its
/// `checks/contracts/writes.rs` discharge untouched.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_unevidenced_establishment_diagnostics(
    program: &dyn ::validation::CheckingInput,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    actual: &[IndexedInstance],
    expected: &[IndexedInstance],
    diagnostics: &mut Vec<Diagnostic>,
    unresolved: &mut Vec<CompatibilityKey>,
) {
    if !matches!(
        program.expression_table.expression(value),
        ExpressionNode::Call(_)
    ) {
        return;
    }
    for expected in expected {
        if !expected.semantic_id.is_valid()
            || expected.arguments.is_empty()
            || actual.iter().any(|actual| actual.family == expected.family)
            || !is_bodyless_domain_family(program, expected.family)
        {
            continue;
        }
        let key = CompatibilityKey {
            point,
            value,
            target_type,
            family: expected.family,
            actual: SemanticDomainId::NULL,
            expected: expected.semantic_id,
        };
        if unresolved.contains(&key) {
            continue;
        }
        unresolved.push(key);
        diagnostics.push(Diagnostic::error(format!(
            "declared instance `{}` has no establishment: the call establishes no instance of \
             domain family `{}` -- neither its declared return type nor its `ensures` names one \
             -- and a domain family with no predicate body has no predicate the write could \
             prove; the value must already carry the instance, through the callee's `ensures \
             result in {}`, a declared place of that type, or the family's authorized \
             establishment route (at {})",
            expected.label,
            family_label(&expected.label),
            expected.label,
            point_label(program, point),
        )));
    }
}

/// An unresolved domain is not bodyless: only its declaration establishes
/// that there are no predicates a receiving write could prove.
pub(super) fn is_bodyless_domain_family(
    program: &dyn ::validation::CheckingInput,
    family: SymbolHandle,
) -> bool {
    program
        .domain_definition(family)
        .is_some_and(|domain| !domain.predicate_body.is_present())
}

//! Unevidenced-establishment diagnostics: the custody-marked cast-mint
//! refusal and the domain/point label wording it prints.

use super::instance_collection::{CompatibilityKey, IndexedInstance};
use super::labels::{carrier_label, family_label, point_label, qualification_label};
use diagnostics::Diagnostic;
use facts::ProgramPoint;
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
///
/// A semantic-domain cast is the other introduction surface the same write
/// must not seed from: `x as T in D` mints a predicate-free, route-free family
/// instance unconditionally (validation's staged mint fence judges only
/// predicate-bearing or routed domains). Minting remains the sanctioned
/// introduction on carriers whose domain set is all tag-style qualifications
/// (a `Region::Left -> Region::Right` retag, a scalar's `Wrapping`), but on a
/// custody-marked carrier -- one that already declares a predicate-bearing or
/// `established by`-routed domain, like `Extent` with `Granted` -- a vacuous
/// family is a member of a managed custody system, not a tag namespace:
/// `Resident<P, T>` and `Vacant` assert facts about who owns the range, and
/// `placed_access.md` fixes `Resident`'s route set while no declaration on
/// `Extent` authorizes an `as` mint. Until such a family declares its
/// qualification-carrier route, the only establishment is refusal.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_unevidenced_establishment_diagnostics(
    program: &TypedTrees,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    actual: &[IndexedInstance],
    expected: &[IndexedInstance],
    diagnostics: &mut Vec<Diagnostic>,
    unresolved: &mut Vec<CompatibilityKey>,
) {
    if let ExpressionNode::Cast(cast) = program.expression_table.expression(value) {
        append_unevidenced_cast_mint_diagnostic(
            program,
            cast,
            value,
            target_type,
            point,
            diagnostics,
            unresolved,
        );
        return;
    }
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

/// A semantic-domain cast whose introduced family is a predicate-free,
/// route-free domain declared over a custody-managed carrier mints state the
/// caller never produced. Tag-style minting elsewhere keeps the staged `as`
/// surface; on a custody-managed carrier the vacuous member's qualification
/// names custody state, so the write must refuse the value the cast minted.
pub(super) fn append_unevidenced_cast_mint_diagnostic(
    program: &TypedTrees,
    cast: &typed_trees::expression::TableCastExpression,
    value: ExpressionHandle,
    target_type: TypeReferenceHandle,
    point: ProgramPoint,
    diagnostics: &mut Vec<Diagnostic>,
    unresolved: &mut Vec<CompatibilityKey>,
) {
    if !cast.semantic_domain_symbol.is_valid() {
        return;
    }
    let Some(domain) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == cast.semantic_domain_symbol)
    else {
        return;
    };
    if domain.predicate_body.is_present() || !domain.establishment_routes.is_empty() {
        return;
    }
    if !domain_target_is_custody_marked(program, domain.target_type) {
        return;
    }
    let key = CompatibilityKey {
        point,
        value,
        target_type,
        family: cast.semantic_domain_symbol,
        actual: SemanticDomainId::NULL,
        expected: cast.semantic_domain_id,
    };
    if unresolved.contains(&key) {
        return;
    }
    unresolved.push(key);
    diagnostics.push(Diagnostic::error(format!(
        "declared instance `{}` has no establishment: `as` mints an instance of \
         domain family `{}` on custody-marked carrier `{}` -- the family \
         declares no predicate body and no `establishment` route, and a \
         linear carrier that route-manages a domain does not admit a minted \
         member; the instance must come from an establishing call, an \
         `established by` route, or the family's declared qualification-carrier \
         route (at {})",
        qualification_label(program, cast),
        family_label(&qualification_label(program, cast)),
        carrier_label(program, domain.target_type),
        point_label(program, point),
    )));
}

/// Whether the type a domain is declared over is a custody-managed carrier: it
/// is linear, and some domain over it carries a predicate or an `established
/// by` route. A qualification on such a carrier travels with the one live
/// value and is what a provider view's parameter reads as held authority, so
/// `as` minting a vacuous member of it fabricates custody the caller never
/// received.
///
/// Both conjuncts are load-bearing, and each is pinned by a case the other
/// would decide wrongly. Linearity alone would refuse
/// `restating_let_keeps_a_cast_mint_on_an_all_vacuous_domain_carrier`, whose
/// `[linear] data Region` carries only the vacuous `Left` and `Right`: with no
/// route-managed domain over it there is no custody to fabricate, and
/// retagging is an ordinary linear move. The route-management scan alone
/// decided `5 as i32::Km` by whether an unrelated `domain i32::Positive`
/// happened to be declared elsewhere in the program, which
/// [domains](../../../../../../wiki/spec/language/domains.md) admits with no
/// obligation beyond carrier compatibility; `i32` is unrestricted, so
/// linearity excludes it whatever its siblings declare.
///
/// Whether a linear carrier's vacuous qualification should depend on its
/// siblings at all is [`linear-carrier-vacuous-qualification`](../../../../../../OWNER_QUESTIONS.md) (Q8).
/// Constrained or borrowed spellings unwrap to the named root.
pub(super) fn domain_target_is_custody_marked(
    program: &TypedTrees,
    target_type: TypeReferenceHandle,
) -> bool {
    let mut type_reference = target_type;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => {
                if !symbol.is_valid()
                    || program.type_multiplicity(type_reference)
                        != language_semantics::Multiplicity::Linear
                {
                    return false;
                }
                return program.domain_definitions().iter().any(|domain| {
                    domain_carrier_symbol(program, domain.target_type) == Some(*symbol)
                        && (domain.predicate_body.is_present()
                            || !domain.establishment_routes.is_empty())
                });
            }
            _ => return false,
        }
    }
}

/// The named carrier symbol a domain declaration attaches to, unwrapping
/// constrained or borrowed spellings.
pub(super) fn domain_carrier_symbol(
    program: &TypedTrees,
    target_type: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let mut type_reference = target_type;
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => type_reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => return Some(*symbol),
            _ => return None,
        }
    }
}

/// Display spelling of a domain's declared carrier for the mint diagnostic.
/// is not treated as bodyless: only a found declaration states that membership
/// has no predicates a write could prove.
pub(super) fn is_bodyless_domain_family(program: &TypedTrees, family: SymbolHandle) -> bool {
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == family)
        .is_some_and(|domain| !domain.predicate_body.is_present())
}

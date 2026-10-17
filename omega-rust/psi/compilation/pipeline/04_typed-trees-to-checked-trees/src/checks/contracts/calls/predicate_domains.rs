//! Predicate-only domain vocabulary: predicates, labels, intervals, and
//! the requirement subject a fact constrains.

use checked_trees::CheckFacts;
use symbols::SymbolHandle;

/// The proof-fact expressions of a PREDICATE-ONLY domain, or `None` when the
/// domain is not predicate-only.
///
/// [Domains](wiki/spec/language/domains.md#declaration-and-membership) settles
/// the rule: "Predicates alone establish predicate-only membership", and a
/// ROUTED domain is the case that "additionally needs exact authorized
/// provenance". So a routed, aliased or indexed domain returns `None` here and
/// keeps its provenance obligation; only a domain whose whole content is
/// predicates over `self` reduces, and then EVERY predicate must be
/// established, never a subset.
pub(super) fn predicate_only_domain_predicates(
    program: &dyn ::validation::CheckingInput,
    domain_symbol: SymbolHandle,
) -> Option<Vec<typed_trees::expression::ExpressionHandle>> {
    let domain = typed_trees::domain::domain_by_symbol(program, domain_symbol)?;
    // The same list `domain_byte_predicate` enforces, so one definition governs
    // both grants: an alias expands to constituent requirements, index binders
    // carry an identity this substitution cannot supply, and `established by`
    // routes restrict who may mint membership at all.
    if domain.alias.is_some()
        || !domain.index_arguments.is_empty()
        || !domain.establishment_routes.is_empty()
        || !typed_trees::domain::index_parameters(program, domain).is_empty()
    {
        return None;
    }
    let declared = program.proof_facts(domain);
    if declared.is_empty() {
        // A predicate-free domain adds no membership obligation predicates
        // could discharge; whatever it asks for is not this.
        return None;
    }
    declared
        .iter()
        .map(|declared_fact| match declared_fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            // A nested membership or a proposition needs more than this
            // substitution gives; refuse the whole domain rather than
            // establish a subset of its obligations.
            _ => None,
        })
        .collect()
}

/// The predicates a PREDICATE-ONLY domain membership reduces to, each rendered
/// at the subject, or `None` when the domain is not predicate-only.
///
/// The rendering is `instantiate_domain_expression_label`, the same
/// substitution `contracts::domains` already runs in the MEMBERSHIP ->
/// PREDICATE direction. This is that rendering read the other way.
pub(super) fn predicate_only_domain_labels(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    fact: &facts::Fact,
) -> Option<Vec<String>> {
    let Some((_, _, domain_symbol, _)) = fact.payload.domain_membership() else {
        return None;
    };
    let place_handle = fact.place.as_place()?;
    let subject_label = facts.semantic.place_label(program, place_handle);
    let domain = program.domain_definition(domain_symbol)?;
    Some(
        predicate_only_domain_predicates(program, domain_symbol)?
            .iter()
            .map(|expression| {
                super::super::labels::instantiate_domain_expression_label(
                    program,
                    domain,
                    *expression,
                    &subject_label,
                )
            })
            .collect(),
    )
}

/// The closed interval a PREDICATE-ONLY domain requires of its subject, paired
/// with that subject's label, or `None` when the domain states no interval.
///
/// Establishment direction, so the interval must be EXACT: every predicate a
/// recognized bound on `self`, or containing the subject inside it would grant
/// membership while a predicate it cannot see -- `self != 7` beside
/// `self <= 15` -- stays unproven.
pub(super) fn predicate_only_domain_interval(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    fact: &facts::Fact,
) -> Option<(String, numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let Some((_, _, domain_symbol, semantic_domain)) = fact.payload.domain_membership() else {
        return None;
    };
    let place_handle = fact.place.as_place()?;
    let domain = program.domain_definition(domain_symbol)?;
    if domain.alias.is_some()
        || !domain.index_arguments.is_empty()
        || !domain.establishment_routes.is_empty()
        || !typed_trees::domain::index_parameters(program, domain).is_empty()
    {
        return None;
    }
    let constraint = typed_trees::types::DomainConstraint {
        symbol: domain_symbol,
        semantic_id: semantic_domain,
        ..Default::default()
    };
    let (minimum, maximum) = validation::exact_declared_domain_interval(program, &constraint)?;
    Some((
        facts.semantic.place_label(program, place_handle),
        minimum.unwrap_or_else(|| numerics::bignum::BigInt::from_i64(i64::MIN)),
        maximum.unwrap_or_else(|| numerics::bignum::BigInt::from_i64(i64::MAX)),
    ))
}

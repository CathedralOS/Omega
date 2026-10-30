//! Predicate-only domain vocabulary: predicates, labels, intervals, and
//! the requirement subject a fact constrains.

use checked_trees::CheckFacts;

/// The proof-fact expressions of a PREDICATE-ONLY domain, or `None` when the
/// domain is not predicate-only.
///
/// [Domains](wiki/spec/language/domains.md#declaration-and-membership) settles
/// the rule: "Predicates alone establish predicate-only membership", and a
/// ROUTED domain is the case that "additionally needs exact authorized
/// provenance". Routed and aliased domains retain that obligation; an indexed
/// theory must already have a complete applied view. Only a theory whose whole content is
/// predicates over `self` reduces, and then EVERY predicate must be
/// established, never a subset.
pub(super) fn predicate_only_domain_predicates<'program>(
    program: &'program dyn ::validation::CheckingInput,
    domain: &'program typed_trees::domain::DomainDefinition,
) -> Option<
    Vec<(
        &'program typed_trees::domain::DomainDefinition,
        typed_trees::expression::ExpressionHandle,
    )>,
> {
    fn visit<'program>(
        program: &'program dyn ::validation::CheckingInput,
        domain: &'program typed_trees::domain::DomainDefinition,
        active: &mut Vec<(symbols::SymbolHandle, language_semantics::SemanticDomainId)>,
    ) -> Option<
        Vec<(
            &'program typed_trees::domain::DomainDefinition,
            typed_trees::expression::ExpressionHandle,
        )>,
    > {
        // The same list `domain_byte_predicate` enforces, so one definition governs
        // both grants: an alias expands to constituent requirements, index binders
        // carry an identity this substitution cannot supply, and `established by`
        // routes restrict who may mint membership at all.
        let instance = (domain.symbol, domain.semantic_id);
        if active.contains(&instance)
            || domain.alias.is_some()
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
        active.push(instance);
        let mut predicates = Vec::new();
        for fact in declared {
            match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    predicates.push((domain, *expression));
                }
                typed_trees::domain::ProofFact::Membership(membership)
                    if typed_trees::domain::domain_subject_reference(
                        program,
                        domain,
                        membership.value,
                    ) =>
                {
                    // Inheritance is a conjunction on this exact subject. Every
                    // inherited theory must be closed and predicate-only too;
                    // its declaring subject owns each leaf's substitution.
                    let parent = program.domain_definition_for_instance(
                        membership.domain_symbol,
                        membership.semantic_domain,
                    )?;
                    predicates.extend(visit(program, parent, active)?);
                }
                // Projected membership and propositions require their own proof
                // judgments. Never discharge an outer domain using a subset.
                _ => return None,
            }
        }
        active.pop();
        Some(predicates)
    }
    visit(program, domain, &mut Vec::new())
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
    let Some((_, _, domain_symbol, identity)) = fact.payload.domain_membership() else {
        return None;
    };
    let place_handle = fact.place.as_place()?;
    let subject_label = facts.semantic.place_label(program, place_handle);
    let domain = program.domain_definition_for_instance(domain_symbol, identity)?;
    Some(
        predicate_only_domain_predicates(program, domain)?
            .iter()
            .map(|(owner, expression)| {
                super::super::labels::instantiate_domain_expression_label(
                    program,
                    owner,
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

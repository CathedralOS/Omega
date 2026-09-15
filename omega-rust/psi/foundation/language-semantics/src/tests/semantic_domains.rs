use crate::{
    DomainEstablishmentRoute, DomainPredicateBody, QualificationEvidenceOrigin, SemanticDomainId,
    SemanticDomainTable,
};

#[test]
fn semantic_domain_table_is_deterministic_and_idempotent() {
    // Identity = declaration order; re-interning the same name returns
    // the same id; NULL never resolves to a name.
    let mut table = SemanticDomainTable::default();
    // Policies pre-seed with FIXED ids; declared domains follow.
    assert_eq!(
        table.lookup("Wrapping"),
        Some(SemanticDomainTable::WRAPPING)
    );
    assert_eq!(
        table.lookup("Saturating"),
        Some(SemanticDomainTable::SATURATING)
    );
    assert_eq!(
        table.lookup("Trapping"),
        Some(SemanticDomainTable::TRAPPING)
    );
    let kilometres = table.intern("Km");
    assert_eq!(kilometres, SemanticDomainId(4));
    assert_eq!(table.intern("Km"), kilometres);
    assert_eq!(table.name(kilometres), Some("Km"));
    // Re-interning a policy returns its fixed id, never a duplicate.
    assert_eq!(table.intern("Wrapping"), SemanticDomainTable::WRAPPING);
    assert_eq!(table.name(SemanticDomainId::NULL), None);
    assert_eq!(table.lookup("Miles"), None);
}

#[test]
fn establishment_routes_keep_source_and_requirement_identity_independent() {
    let checked_trait = symbols::SymbolHandle::from_arena_index(9);
    let checked_requirement = symbols::SymbolHandle::from_arena_index(10);
    let checked = DomainEstablishmentRoute::CheckedRequirement {
        trait_definition: checked_trait,
        requirement: checked_requirement,
    };
    assert_eq!(checked.kind_name(), "checked_requirement");
    assert_eq!(checked.source_symbol(), checked_trait);
    assert_eq!(checked.requirement_symbol(), checked_requirement);

    let boundary_trait = symbols::SymbolHandle::from_arena_index(11);
    let requirement = symbols::SymbolHandle::from_arena_index(12);
    let route = DomainEstablishmentRoute::BoundaryRequirement {
        boundary_trait,
        requirement,
    };
    assert_eq!(route.kind_name(), "boundary_requirement");
    assert_eq!(route.source_symbol(), boundary_trait);
    assert_eq!(route.requirement_symbol(), requirement);
}

#[test]
fn domain_predicate_body_distinguishes_bodyless_from_explicit_predicates() {
    assert_eq!(
        DomainPredicateBody::default(),
        DomainPredicateBody::Bodyless
    );
    assert!(!DomainPredicateBody::Bodyless.is_present());
    assert!(DomainPredicateBody::Present.is_present());
    assert_eq!(DomainPredicateBody::Bodyless.as_str(), "bodyless");
    assert_eq!(DomainPredicateBody::Present.as_str(), "present");
}

#[test]
fn qualification_evidence_origins_have_stable_public_names() {
    use QualificationEvidenceOrigin as Origin;

    assert_eq!(Origin::default(), Origin::None);
    assert_eq!(Origin::Prover.as_str(), "prover");
    assert_eq!(Origin::CheckedValidation.as_str(), "checked_validation");
    assert_eq!(
        Origin::AuthorizedRouteEstablishment.as_str(),
        "authorized_route_establishment"
    );
    assert_eq!(
        Origin::CheckedTransformation.as_str(),
        "checked_transformation"
    );
    assert_eq!(Origin::AdmittedReceipt.as_str(), "admitted_receipt");
    assert_eq!(Origin::Propagated.as_str(), "propagated");
    assert_eq!(
        Origin::VacuousQualification.as_str(),
        "vacuous_qualification"
    );
}

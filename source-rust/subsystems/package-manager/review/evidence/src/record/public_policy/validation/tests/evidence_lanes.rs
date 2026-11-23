use super::{U64, api, identity, objective_c, package, type_identity};
use crate::record::{
    PackageReviewCallableContract as Clause, PackageReviewContractFact as Fact,
    PackageReviewContractKind as Kind, PackageReviewEvidenceInterface as Interface,
    PackageReviewEvidenceRequirement as Requirement,
    PackageReviewPropositionApplication as Application, PackageReviewPropositionBinder as Binder,
    PackageReviewPropositionBinderArgument as Argument,
    PackageReviewPropositionBinderArgumentKind as ArgumentKind,
    PackageReviewPropositionBinderKind as BinderKind, PackageReviewPropositionBinderValue as Value,
    PackageReviewPropositionEvidence as Evidence,
};

fn application() -> Application {
    Application {
        declaration: identity("Ordering"),
        binders: Vec::new(),
        parameter_types: Vec::new(),
        binder_arguments: Vec::new(),
        arguments: Vec::new(),
        evidence: Evidence::FactOnly,
    }
}

fn source() -> Clause {
    let requirement = Requirement {
        declaring_trait: identity("Compare"),
        declaring_trait_lifetime_arguments: Vec::new(),
        declaring_trait_arguments: vec![type_identity(U64)],
        requirement: identity("Compare::compare"),
    };
    let interface = Interface {
        trait_identity: identity("Compare"),
        lifetime_arguments: Vec::new(),
        arguments: vec![type_identity(U64)],
        requirements: vec![requirement],
    };
    Clause {
        kind: Kind::Requires,
        result_case: None,
        binding: None,
        evidence_lane_position: Some(0),
        fact: Fact::Proposition(Application {
            evidence: Evidence::Witness(interface),
            ..application()
        }),
    }
}

fn projection(kind: Kind, position: u32, member: &str) -> Clause {
    let value = Value::EvidenceProjection {
        source_kind: kind,
        source_lane_position: position,
        declaring_trait: identity("Compare"),
        declaring_trait_arguments: vec![type_identity(U64)],
        requirement: identity(member),
    };
    Clause {
        kind: Kind::Ensures,
        result_case: None,
        binding: None,
        evidence_lane_position: None,
        fact: Fact::Proposition(Application {
            binders: vec![Binder {
                kind: BinderKind::Machine,
                bounds: crate::record::PackageReviewDataProperties {
                    multiplicity: Default::default(),
                },
            }],
            binder_arguments: vec![Argument {
                kind: ArgumentKind::Machine,
                value,
            }],
            ..application()
        }),
    }
}

fn validate(clauses: Vec<Clause>) -> Result<(), &'static str> {
    let mut reviewed = objective_c(0);
    reviewed.requirements[0].contracts = clauses;
    api(reviewed).validate_canonical_structure(package())
}

#[test]
fn projected_machine_uses_the_exact_requires_witness_lane_and_member() {
    let projected = || projection(Kind::Requires, 0, "Compare::compare");
    assert_eq!(validate(vec![source(), projected()]), Ok(()));
    assert!(
        validate(vec![
            source(),
            projection(Kind::Requires, 3, "Compare::compare")
        ])
        .is_err()
    );
    assert!(
        validate(vec![
            source(),
            projection(Kind::Ensures, 0, "Compare::compare")
        ])
        .is_err()
    );
    assert!(
        validate(vec![
            source(),
            projection(Kind::Requires, 0, "Compare::other")
        ])
        .is_err()
    );
    assert!(validate(vec![projected()]).is_err());
    assert!(validate(vec![source(), source(), projected()]).is_err());
    let mut fact_only = source();
    fact_only.fact = Fact::Proposition(application());
    assert!(validate(vec![fact_only, projected()]).is_err());
    let mut other_arguments = source();
    let Fact::Proposition(application) = &mut other_arguments.fact else {
        unreachable!()
    };
    let Evidence::Witness(interface) = &mut application.evidence else {
        unreachable!()
    };
    interface.requirements[0].declaring_trait_arguments.clear();
    assert!(validate(vec![other_arguments, projected()]).is_err());
}

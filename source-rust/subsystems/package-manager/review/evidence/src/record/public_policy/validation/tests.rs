// Regression tests verifying that trait requirement contract binders start at declared statics without an implicit subject binder.

use crate::record::{
    PackagePolicyPublicApi, PackagePolicyTermination, PackagePolicyTraitRequirement,
    PackagePolicyTraitShape, PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
    PackageReviewCallableContract, PackageReviewCallableContract as Clause,
    PackageReviewContractBinaryOperator, PackageReviewContractExpression as Expression,
    PackageReviewContractFact, PackageReviewContractFact as Fact, PackageReviewContractKind,
    PackageReviewContractKind as Kind, PackageReviewContractOperatorMeaning,
    PackageReviewDataProperties, PackageReviewEvidenceInterface as Interface,
    PackageReviewEvidenceRequirement as Requirement, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewPropositionApplication as Application,
    PackageReviewPropositionBinder as Binder, PackageReviewPropositionBinderArgument as Argument,
    PackageReviewPropositionBinderArgumentKind as ArgumentKind,
    PackageReviewPropositionBinderKind as BinderKind, PackageReviewPropositionBinderValue as Value,
    PackageReviewPropositionEvidence as Evidence, PackageReviewSignatureParameter,
    PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;

const U64: &str = r"14:signature-type41:named(name(compiler-type\(atom\(u64\)\)))5:named";
const BYTES_OF_COUNT: &str = r"14:signature-type86:ref(array(named(name(compiler-type\(atom\(u8\)\))),const-parameter(type-parameter:0)))36:9:reference6:elided14:5:array5:named";

fn package() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([0x56; 32]).unwrap()
}

fn identity(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package()),
        path: path.to_owned(),
    }
}

fn type_identity(canonical: &str) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: canonical.to_owned(),
    }
}

fn requires(
    operator: PackageReviewContractBinaryOperator,
    left: Expression,
    right: Expression,
) -> PackageReviewCallableContract {
    PackageReviewCallableContract {
        kind: PackageReviewContractKind::Requires,
        result_case: None,
        binding: None,
        evidence_lane_position: None,
        fact: PackageReviewContractFact::Expression(binary(operator, left, right)),
    }
}

fn binary(
    operator: PackageReviewContractBinaryOperator,
    left: Expression,
    right: Expression,
) -> Expression {
    Expression::Binary {
        meaning: PackageReviewContractOperatorMeaning::Builtin,
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn objective_c(count_binder: u32) -> PackagePolicyTraitShape {
    use PackageReviewContractBinaryOperator as Operator;
    let count = || Expression::GenericBinder(count_binder);
    let integer = |value: &str| Expression::Integer(value.to_owned());
    let last_byte = Expression::Indexed {
        meaning: PackageReviewContractOperatorMeaning::Builtin,
        collection: Box::new(Expression::Parameter(0)),
        index: Box::new(binary(Operator::Subtract, count(), integer("1"))),
    };
    PackagePolicyTraitShape {
        identity: identity("ObjectiveC"),
        is_boundary: true,
        lifetime_parameter_count: 0,
        type_parameters: Vec::new(),
        conformance_bounds: Vec::new(),
        parents: Vec::new(),
        requirements: vec![PackagePolicyTraitRequirement {
            identity: identity("ObjectiveC::get_class"),
            conformance_bounds: Vec::new(),
            spelling: None,
            has_default_realization: false,
            lifetime_parameter_count: 0,
            type_parameters: vec![PackagePolicyTypeParameter {
                kind: PackagePolicyTypeParameterKind::Const(type_identity(U64)),
                bounds: PackageReviewDataProperties {
                    multiplicity: Default::default(),
                },
            }],
            parameters: vec![PackageReviewSignatureParameter {
                name: "name".to_owned(),
                type_identity: type_identity(BYTES_OF_COUNT),
                is_const: false,
                is_mutable: false,
            }],
            structural_parameter_positions: Some(vec![0]),
            return_type: Some(type_identity(U64)),
            contracts: vec![
                requires(Operator::Greater, count(), integer("0")),
                requires(Operator::Equal, last_byte, integer("0")),
            ],
            published_crash: Some(Vec::new()),
            service_reach: Some(Vec::new()),
            service_reach_is_installation_bound: false,
            synchronous_invocations: Vec::new(),
            suspends: false,
            blocks: false,
            admitted_same_stack_bytes: 0,
            termination: PackagePolicyTermination::NoGuarantee,
            returned_borrow_origins: Vec::new(),
            parameter_qualification_preconditions: Vec::new(),
        }],
    }
}

fn api(shape: PackagePolicyTraitShape) -> PackagePolicyPublicApi {
    PackagePolicyPublicApi {
        traits: vec![shape],
        conformances: Vec::new(),
        domains: Vec::new(),
        propositions: Vec::new(),
        consts: Vec::new(),
        operators: Vec::new(),
        data: Vec::new(),
        type_aliases: None,
    }
}

#[test]
fn requirement_contract_binders_start_at_the_declared_statics() {
    assert_eq!(
        api(objective_c(0)).validate_canonical_structure(package()),
        Ok(())
    );
}

#[test]
fn a_trait_has_no_implicit_subject_binder_before_its_statics() {
    assert_eq!(
        api(objective_c(1)).validate_canonical_structure(package()),
        Err("contract generic binder escapes its static telescope")
    );
}

#[test]
fn foreign_stack_requires_a_bodyless_boundary_requirement() {
    let mut shape = objective_c(0);
    shape.requirements[0].admitted_same_stack_bytes = 128;
    assert_eq!(
        api(shape.clone()).validate_canonical_structure(package()),
        Ok(())
    );
    for (boundary, has_default) in [(false, false), (true, true)] {
        shape.is_boundary = boundary;
        shape.requirements[0].has_default_realization = has_default;
        assert_eq!(
            api(shape.clone()).validate_canonical_structure(package()),
            Err("only a bodyless boundary requirement can declare foreign stack demand")
        );
    }
}

mod structural_parameters;

#[test]
fn public_trait_parents_retain_their_local_declaration() {
    use crate::record::{PackageReviewTraitCompositionKind, PackageReviewTraitParent};
    let mut public = api(objective_c(0));
    public.traits[0].parents.push(PackageReviewTraitParent {
        kind: PackageReviewTraitCompositionKind::Policy,
        identity: identity("Parent"),
        lifetime_arguments: Vec::new(),
        arguments: Vec::new(),
    });
    let mut parent = public.traits[0].clone();
    parent.identity = identity("Parent");
    parent.parents.clear();
    parent.requirements.clear();
    public.traits.push(parent);
    assert_eq!(public.validate_canonical_structure(package()), Ok(()));
    public.traits.pop();
    assert!(public.validate_canonical_structure(package()).is_err());
    let dependency = PackageKeyIdentity::from_digest([0xa1; 32]).unwrap();
    public.traits[0].parents[0].identity.owner = PackageReviewNominalOwner::Package(dependency);
    assert_eq!(public.validate_canonical_structure(package()), Ok(()));
}

#[test]
fn public_witnesses_retain_their_local_interface_and_its_members() {
    use crate::record::{
        PackagePolicyConformanceShape, PackageReviewEvidenceInterface,
        PackageReviewEvidenceRequirement,
    };
    let mut public = api(objective_c(0));
    public.conformances.push(PackagePolicyConformanceShape {
        identity: identity("Implementation"),
        lifetime_parameter_count: 0,
        type_parameters: Vec::new(),
        interface: PackageReviewEvidenceInterface {
            trait_identity: identity("ObjectiveC"),
            lifetime_arguments: Vec::new(),
            arguments: Vec::new(),
            requirements: vec![PackageReviewEvidenceRequirement {
                declaring_trait: identity("ObjectiveC"),
                declaring_trait_lifetime_arguments: Vec::new(),
                declaring_trait_arguments: Vec::new(),
                requirement: identity("ObjectiveC::get_class"),
            }],
        },
    });
    assert_eq!(public.validate_canonical_structure(package()), Ok(()));
    public.conformances[0].interface.requirements[0].declaring_trait = identity("MissingParent");
    assert!(public.validate_canonical_structure(package()).is_err());
    public.conformances[0].interface.requirements.clear();
    public.traits.clear();
    assert!(public.validate_canonical_structure(package()).is_err());
}

pub(in crate::record::public_policy::validation::tests) fn application() -> Application {
    Application {
        declaration: identity("Ordering"),
        binders: Vec::new(),
        parameter_types: Vec::new(),
        binder_arguments: Vec::new(),
        arguments: Vec::new(),
        evidence: Evidence::FactOnly,
    }
}

pub(in crate::record::public_policy::validation::tests) fn source() -> Clause {
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

pub(in crate::record::public_policy::validation::tests) fn projection(
    kind: Kind,
    position: u32,
    member: &str,
) -> Clause {
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

pub(in crate::record::public_policy::validation::tests) fn validate(
    clauses: Vec<Clause>,
) -> Result<(), &'static str> {
    let mut reviewed = objective_c(0);
    reviewed.requirements[0].contracts = clauses;
    api(reviewed).validate_canonical_structure(package())
}

#[test]
pub(in crate::record::public_policy::validation::tests) fn projected_machine_uses_the_exact_requires_witness_lane_and_member()
 {
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

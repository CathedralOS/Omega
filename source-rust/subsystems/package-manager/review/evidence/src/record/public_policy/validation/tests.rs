// Regression tests verifying that trait requirement contract binders start at declared statics without an implicit subject binder.

use crate::record::{
    PackagePolicyPublicApi, PackagePolicyTermination, PackagePolicyTraitRequirement,
    PackagePolicyTraitShape, PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
    PackageReviewCallableContract, PackageReviewContractBinaryOperator,
    PackageReviewContractExpression as Expression, PackageReviewContractFact,
    PackageReviewContractKind, PackageReviewContractOperatorMeaning, PackageReviewDataProperties,
    PackageReviewNominalIdentity, PackageReviewNominalOwner, PackageReviewSignatureParameter,
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
    assert_eq!(api(shape.clone()).validate_canonical_structure(package()), Ok(()));
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

mod evidence_lanes;

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

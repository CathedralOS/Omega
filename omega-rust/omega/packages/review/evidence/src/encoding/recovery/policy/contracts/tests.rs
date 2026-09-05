use super::super::signatures::{conformance_bound, data_properties, machine_contract};
use super::*;
use crate::encoding::PackagePolicyRecoveryLimits;
use crate::encoding::encode::declarations::encode_machine_parameter_contract;
use crate::encoding::encode::encoder::Encoder;
use psi_language_semantics::{
    CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension, Multiplicity,
};

fn identity(name: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [7; 32],
        }),
        path: format!("signature_tests::{name}"),
    }
}

fn value_type() -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: "u64".to_owned(),
    }
}

fn properties() -> PackageReviewDataProperties {
    PackageReviewDataProperties {
        multiplicity: Multiplicity::Unrestricted,
        carry: None,
    }
}

fn signature() -> PackageReviewMachineParameterSignature {
    PackageReviewMachineParameterSignature {
        lifetime_parameter_count: 2,
        type_parameters: Vec::new(),
        parameters: vec![PackageReviewMachineParameterValue {
            name: "value".to_owned(),
            type_identity: value_type(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        }],
        return_type: value_type(),
        contracts: Vec::new(),
        published_crash: Vec::new(),
        service_reach: vec![identity("Console")],
        service_reach_is_installation_bound: true,
        synchronous_invocations: vec![
            PackageReviewSynchronousInvocation::Parameter(0),
            PackageReviewSynchronousInvocation::Service(identity("Console")),
        ],
        suspends: true,
        blocks: true,
        termination: PackageReviewTermination::Terminates {
            premises: Vec::new(),
        },
    }
}

fn proposition(evidence: PackageReviewPropositionEvidence) -> PackageReviewContractFact {
    let mut binder_arguments = vec![
        PackageReviewPropositionBinderArgument {
            kind: PackageReviewPropositionBinderArgumentKind::Type,
            value: PackageReviewPropositionBinderValue::Type(value_type()),
        },
        PackageReviewPropositionBinderArgument {
            kind: PackageReviewPropositionBinderArgumentKind::Const,
            value: PackageReviewPropositionBinderValue::GenericBinder(2),
        },
        PackageReviewPropositionBinderArgument {
            kind: PackageReviewPropositionBinderArgumentKind::Const,
            value: PackageReviewPropositionBinderValue::Integer("123".to_owned()),
        },
        PackageReviewPropositionBinderArgument {
            kind: PackageReviewPropositionBinderArgumentKind::Machine,
            value: PackageReviewPropositionBinderValue::Machine(identity("machine")),
        },
    ];
    for source_kind in [
        PackageReviewContractKind::Requires,
        PackageReviewContractKind::Ensures,
    ] {
        binder_arguments.push(PackageReviewPropositionBinderArgument {
            kind: PackageReviewPropositionBinderArgumentKind::Type,
            value: PackageReviewPropositionBinderValue::EvidenceProjection {
                source_kind,
                source_lane_position: 3,
                declaring_trait: identity("Witness"),
                declaring_trait_arguments: vec![value_type()],
                requirement: identity("Witness::item"),
            },
        });
    }
    PackageReviewContractFact::Proposition(PackageReviewPropositionApplication {
        declaration: identity("Fact"),
        binders: binder_arguments
            .iter()
            .map(|argument| PackageReviewPropositionBinder {
                kind: match argument.kind {
                    PackageReviewPropositionBinderArgumentKind::Type => {
                        PackageReviewPropositionBinderKind::Type
                    }
                    PackageReviewPropositionBinderArgumentKind::Const => {
                        PackageReviewPropositionBinderKind::Const(value_type())
                    }
                    PackageReviewPropositionBinderArgumentKind::Machine => {
                        PackageReviewPropositionBinderKind::Machine
                    }
                },
                bounds: properties(),
            })
            .collect(),
        parameter_types: vec![value_type()],
        binder_arguments,
        arguments: vec![PackageReviewContractExpression::Integer("42".to_owned())],
        evidence,
    })
}

fn round_trip(contract: &PackageReviewMachineParameterContract) -> Vec<u8> {
    let mut encoder = Encoder::bounded(1024 * 1024);
    encode_machine_parameter_contract(&mut encoder, contract).unwrap();
    let bytes = encoder.finish().unwrap();
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    let decoded = machine_contract(&mut reader).unwrap();
    reader.finish().unwrap();
    assert_eq!(&decoded, contract);
    let mut encoder = Encoder::bounded(1024 * 1024);
    encode_machine_parameter_contract(&mut encoder, &decoded).unwrap();
    assert_eq!(encoder.finish().unwrap(), bytes);
    let mut policy_encoder = Encoder::policy_bounded(1024 * 1024);
    encode_machine_parameter_contract(&mut policy_encoder, contract).unwrap();
    assert_eq!(policy_encoder.finish().unwrap(), bytes);
    bytes
}

#[test]
fn structural_machine_contract_recovers_all_parameter_and_fact_kinds() {
    let witness = PackageReviewEvidenceInterface {
        trait_identity: identity("Witness"),
        lifetime_arguments: vec![0, 1],
        arguments: vec![value_type()],
        requirements: vec![PackageReviewEvidenceRequirement {
            declaring_trait: identity("Parent"),
            declaring_trait_lifetime_arguments: vec![1, 0],
            declaring_trait_arguments: vec![value_type()],
            requirement: identity("Parent::item"),
        }],
    };
    let facts = vec![
        PackageReviewContractFact::Expression(PackageReviewContractExpression::Boolean(true)),
        PackageReviewContractFact::Membership {
            value: PackageReviewContractExpression::Integer("17".to_owned()),
            domain: identity("Bounded"),
        },
        proposition(PackageReviewPropositionEvidence::FactOnly),
        proposition(PackageReviewPropositionEvidence::Witness(witness)),
        PackageReviewContractFact::PropositionParameter(
            PackageReviewPropositionParameterApplication {
                binder_ordinal: 3,
                arguments: vec![PackageReviewContractExpression::Boolean(false)],
            },
        ),
    ];
    let mut outer = signature();
    outer.type_parameters = vec![
        PackageReviewTypeParameterKind::Type,
        PackageReviewTypeParameterKind::Const(value_type()),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Structural(
            signature(),
        )),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Nominal {
            trait_identity: identity("MachineTrait"),
            requirement_identity: identity("MachineTrait::invoke"),
        }),
        PackageReviewTypeParameterKind::Machine(
            PackageReviewMachineParameterContract::RequirementIdentity,
        ),
        PackageReviewTypeParameterKind::Proposition(PackageReviewPropositionParameterSignature {
            parameters: vec![PackageReviewPropositionParameterValue {
                type_identity: value_type(),
            }],
        }),
    ]
    .into_iter()
    .map(|kind| PackageReviewTypeParameter {
        kind,
        bounds: properties(),
    })
    .collect();
    outer.contracts = facts
        .into_iter()
        .enumerate()
        .map(|(index, fact)| PackageReviewCallableContract {
            kind: if index == 0 {
                PackageReviewContractKind::Requires
            } else {
                PackageReviewContractKind::Ensures
            },
            result_case: (index == 1).then(|| PackageReviewResultCaseIdentity {
                result_data: identity("Outcome"),
                result_case: identity("Outcome::Success"),
            }),
            binding: (index % 2 == 0).then(|| "witness".to_owned()),
            evidence_lane_position: (index % 2 == 0).then_some(index as u32),
            fact,
        })
        .collect();
    let bytes = round_trip(&PackageReviewMachineParameterContract::Structural(outer));
    for end in 0..bytes.len() {
        let mut reader =
            Reader::new(&bytes[..end], PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(
            machine_contract(&mut reader).is_err(),
            "truncated signature at {end}"
        );
    }
}

#[test]
fn property_decoder_preserves_every_closed_carry_axis_and_multiplicity() {
    for multiplicity in [
        Multiplicity::Unrestricted,
        Multiplicity::Affine,
        Multiplicity::Linear,
    ] {
        for axes in 0..16 {
            let properties = PackageReviewDataProperties {
                multiplicity,
                carry: Some(CarryPolicy {
                    suspension: if axes & 1 == 0 {
                        CarrySuspension::Forbidden
                    } else {
                        CarrySuspension::Allowed
                    },
                    cpu: if axes & 2 == 0 {
                        CarryCpu::Origin
                    } else {
                        CarryCpu::Any
                    },
                    host_thread: if axes & 4 == 0 {
                        CarryHostThread::Origin
                    } else {
                        CarryHostThread::Any
                    },
                    address: if axes & 8 == 0 {
                        CarryAddress::Stable
                    } else {
                        CarryAddress::Movable
                    },
                }),
            };
            let mut encoder = Encoder::bounded(16);
            crate::encoding::encode::declarations::encode_data_properties(&mut encoder, properties);
            let bytes = encoder.finish().unwrap();
            let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
            assert_eq!(data_properties(&mut reader).unwrap(), properties);
            reader.finish().unwrap();
        }
    }
    for bytes in [
        vec![3],
        vec![0, 2],
        vec![0, 1, 2],
        vec![0, 1, 0, 2],
        vec![0, 1, 0, 0, 2],
        vec![0, 1, 0, 0, 0, 2],
    ] {
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert_eq!(data_properties(&mut reader), Err(Error::InvalidTag));
    }
    let mut reader = Reader::new(&[255], PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(machine_contract(&mut reader), Err(Error::InvalidTag));
}

#[test]
fn recursive_machine_contracts_obey_depth_element_and_storage_limits() {
    let mut outer = signature();
    outer.type_parameters.push(PackageReviewTypeParameter {
        kind: PackageReviewTypeParameterKind::Machine(
            PackageReviewMachineParameterContract::Structural(signature()),
        ),
        bounds: properties(),
    });
    let bytes = round_trip(&PackageReviewMachineParameterContract::Structural(outer));
    for (limits, expected) in [
        (
            PackagePolicyRecoveryLimits::new(1024 * 1024, 1024, 1024, 1024 * 1024, 1),
            Error::NestingLimitExceeded,
        ),
        (
            PackagePolicyRecoveryLimits::new(1024 * 1024, 1024, 0, 1024 * 1024, 128),
            Error::ElementLimitExceeded,
        ),
        (
            PackagePolicyRecoveryLimits::new(1024 * 1024, 1024, 1024, 0, 128),
            Error::AllocationLimitExceeded,
        ),
    ] {
        let mut reader = Reader::new(&bytes, limits).unwrap();
        assert_eq!(machine_contract(&mut reader), Err(expected));
    }

    let mut recursive = PackageReviewMachineParameterContract::RequirementIdentity;
    for _ in 0..129 {
        let mut outer = signature();
        outer.type_parameters.push(PackageReviewTypeParameter {
            kind: PackageReviewTypeParameterKind::Machine(recursive),
            bounds: properties(),
        });
        recursive = PackageReviewMachineParameterContract::Structural(outer);
    }
    let mut full_review = Encoder::bounded(1024 * 1024);
    encode_machine_parameter_contract(&mut full_review, &recursive).unwrap();
    let bytes = full_review.finish().unwrap();
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(
        machine_contract(&mut reader),
        Err(Error::NestingLimitExceeded)
    );
    let mut policy = Encoder::policy_bounded(1024 * 1024);
    assert!(encode_machine_parameter_contract(&mut policy, &recursive).is_err());
}

#[test]
fn conformance_bounds_preserve_binder_free_and_selected_application_shapes() {
    let unselected = PackageReviewConformanceBound {
        binder_ordinal: None,
        subject_parameter: 2,
        selected_conformance: None,
        selected_lifetime_arguments: Vec::new(),
        selected_arguments: Vec::new(),
        selected_subject: None,
        trait_identity: identity("Witness"),
        trait_lifetime_arguments: vec![0, 1],
        arguments: vec![value_type()],
    };
    let selected = PackageReviewConformanceBound {
        binder_ordinal: Some(3),
        selected_conformance: Some(identity("SelectedWitness")),
        selected_lifetime_arguments: vec![1, 0],
        selected_arguments: vec![
            PackageReviewContractStaticArgument::Type(value_type()),
            PackageReviewContractStaticArgument::ConstInteger("7".to_owned()),
            PackageReviewContractStaticArgument::ConcreteMachine(identity("apply")),
        ],
        selected_subject: Some(PackageReviewContractStaticArgument::GenericType {
            base: value_type(),
            lifetime_arguments: vec![0],
            arguments: vec![PackageReviewContractStaticArgument::GenericTypeBinder(0)],
        }),
        ..unselected.clone()
    };
    for bound in [unselected, selected] {
        let mut encoder = Encoder::bounded(1024 * 1024);
        crate::encoding::encode::declarations::encode_conformance_bound(&mut encoder, &bound)
            .unwrap();
        let bytes = encoder.finish().unwrap();
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert_eq!(conformance_bound(&mut reader).unwrap(), bound);
        reader.finish().unwrap();
    }
}

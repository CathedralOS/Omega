use super::{
    Error, PackagePolicyRecoveryLimits, Reader, availability::conformance_shape, fixtures::nominal,
};
use crate::encoding::encode::{declarations::encode_conformance_shape, encoder::Encoder};
use crate::record::*;

fn value(canonical: &str) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: canonical.to_owned(),
    }
}

fn properties() -> PackageReviewDataProperties {
    PackageReviewDataProperties {
        multiplicity: psi_language_semantics::Multiplicity::Unrestricted,
        carry: None,
    }
}

fn structural() -> PackageReviewMachineParameterContract {
    PackageReviewMachineParameterContract::Structural(PackageReviewMachineParameterSignature {
        lifetime_parameter_count: 1,
        type_parameters: Vec::new(),
        parameters: vec![PackageReviewMachineParameterValue {
            name: "value".into(),
            type_identity: value("u64"),
            is_const: false,
            is_mutable: false,
            is_self: false,
        }],
        return_type: value("u64"),
        contracts: Vec::new(),
        published_crash: Vec::new(),
        service_reach: Vec::new(),
        service_reach_is_installation_bound: false,
        synchronous_invocations: Vec::new(),
        suspends: false,
        blocks: false,
        termination: PackageReviewTermination::NoGuarantee,
    })
}

#[test]
fn conformance_availability_inverse_retains_every_subject_and_parameter_family() {
    let kinds = [
        PackageReviewTypeParameterKind::Type,
        PackageReviewTypeParameterKind::Const(value("u64")),
        PackageReviewTypeParameterKind::Machine(
            PackageReviewMachineParameterContract::RequirementIdentity,
        ),
        PackageReviewTypeParameterKind::Machine(PackageReviewMachineParameterContract::Nominal {
            trait_identity: nominal("Callback"),
            requirement_identity: nominal("Callback::call"),
        }),
        PackageReviewTypeParameterKind::Machine(structural()),
        PackageReviewTypeParameterKind::Proposition(PackageReviewPropositionParameterSignature {
            parameters: vec![PackageReviewPropositionParameterValue {
                type_identity: value("u64"),
            }],
        }),
    ];
    for subject in [
        PackageReviewConformanceSubject::Subjectless,
        PackageReviewConformanceSubject::TypeParameter(0),
        PackageReviewConformanceSubject::Nominal(nominal("Carrier")),
    ] {
        let shape = PackageReviewConformanceShape {
            identity: nominal("Available"),
            lifetime_parameter_count: 2,
            type_parameters: kinds
                .iter()
                .cloned()
                .map(|kind| PackageReviewTypeParameter {
                    kind,
                    bounds: properties(),
                })
                .collect(),
            subject,
            interface: PackageReviewEvidenceInterface {
                trait_identity: nominal("Interface"),
                lifetime_arguments: vec![1, 0],
                arguments: vec![value("u64")],
                requirements: vec![PackageReviewEvidenceRequirement {
                    declaring_trait: nominal("Parent"),
                    declaring_trait_lifetime_arguments: vec![0, 1],
                    declaring_trait_arguments: vec![value("u32")],
                    requirement: nominal("Parent::run"),
                }],
            },
        };
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encode_conformance_shape(&mut encoder, &shape).unwrap();
        let bytes = encoder.finish().unwrap();
        let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
        assert_eq!(conformance_shape(&mut reader).unwrap(), shape);
        reader.finish().unwrap();
        for end in 0..bytes.len() {
            let mut reader =
                Reader::new(&bytes[..end], PackagePolicyRecoveryLimits::default()).unwrap();
            assert!(
                conformance_shape(&mut reader).is_err(),
                "truncated conformance prefix {end}"
            );
        }
    }
}

#[test]
fn conformance_availability_subject_tag_is_closed() {
    let shape = PackageReviewConformanceShape {
        identity: nominal("Available"),
        lifetime_parameter_count: 0,
        type_parameters: Vec::new(),
        subject: PackageReviewConformanceSubject::Subjectless,
        interface: PackageReviewEvidenceInterface {
            trait_identity: nominal("Interface"),
            lifetime_arguments: Vec::new(),
            arguments: Vec::new(),
            requirements: Vec::new(),
        },
    };
    let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
    encode_conformance_shape(&mut encoder, &shape).unwrap();
    let mut bytes = encoder.finish().unwrap();
    let subject = 1 + 32 + 8 + shape.identity.path.len() + 8 + 8;
    bytes[subject] = 3;
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(conformance_shape(&mut reader), Err(Error::InvalidTag));
}

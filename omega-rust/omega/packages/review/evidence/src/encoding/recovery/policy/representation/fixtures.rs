use crate::record::*;
use psi_core::PackageKeyIdentity;

pub(super) fn nominal(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(
            PackageKeyIdentity::from_digest([7; 32]).unwrap(),
        ),
        path: path.to_owned(),
    }
}

pub(super) fn empty() -> PackagePolicyRepresentation {
    PackagePolicyRepresentation {
        package: PackageKeyIdentity::from_digest([7; 32]).unwrap(),
        target: PackageReviewRepresentationTarget {
            profile: PackageReviewRepresentationTargetProfile::LinuxX64,
            architecture: PackageReviewRepresentationArchitecture::X86_64,
            object_format: PackageReviewRepresentationObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        },
        declarations: Vec::new(),
        producer_availability: Vec::new(),
        selected_availability: Vec::new(),
        demands: Vec::new(),
    }
}

pub(in crate::encoding::recovery::policy) fn complete() -> PackagePolicyRepresentation {
    let mut policy = empty();
    let calling = super::super::calling_application::tests::complete_fixture();
    let used = &calling.opaque_uses[0];
    let selection = PackagePolicyRepresentationSelection {
        opaque: used.opaque.clone(),
        carrier: used.carrier.clone(),
        selection_owner: used.selection_owner,
        application: used.application.clone(),
        origin: used.origin,
        lifecycle: used.lifecycle,
        copy_disposition: used.copy_disposition,
    };
    policy.declarations.push(selection.opaque.clone());
    policy
        .producer_availability
        .push(PackagePolicyRepresentationAvailability {
            opaque: selection.opaque.clone(),
            carrier: selection.carrier.clone(),
            conformance: PackagePolicyConformanceShape {
                identity: selection.application.declaration.clone(),
                lifetime_parameter_count: 0,
                type_parameters: Vec::new(),
                subject: PackageReviewConformanceSubject::Nominal(selection.carrier.clone()),
                interface: PackageReviewEvidenceInterface {
                    trait_identity: selection.application.trait_identity.clone(),
                    lifetime_arguments: Vec::new(),
                    arguments: selection.application.trait_arguments.clone(),
                    requirements: Vec::new(),
                },
            },
        });
    policy.demands.push(PackagePolicyRepresentationDemand {
        opaque: selection.opaque.clone(),
        calling,
    });
    policy.selected_availability.push(selection);
    let mut unused = policy.selected_availability[0].clone();
    unused.opaque = nominal("Unused");
    unused.application.declaration = nominal("UnusedChosen");
    unused.application.trait_arguments[0].canonical = "Unused".to_owned();
    unused.copy_disposition = PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy;
    policy.declarations.push(unused.opaque.clone());
    policy.selected_availability.push(unused);
    policy
}

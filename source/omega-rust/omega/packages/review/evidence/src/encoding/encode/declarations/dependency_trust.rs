use super::super::PackageReviewEncodingError;
use super::super::encoder::Encoder;
use super::super::values::identity::encode_nominal;
use crate::record::{
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewRepresentationTcb,
    PackageReviewRepresentationTcbKind, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
};

pub(crate) fn encode_semantic_dependency_key(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &dependency.consumer)?;
    encode_nominal(encoder, &dependency.dependency)?;
    encoder.byte(semantic_dependency_kind_tag(dependency.kind));
    Ok(())
}

pub(crate) fn encode_semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_semantic_dependency_key(encoder, dependency)?;
    encoder.byte(match dependency.exposure {
        PackageReviewSemanticDependencyExposure::PrivateImplementation => 0,
        PackageReviewSemanticDependencyExposure::PublicInterface => 1,
    });
    Ok(())
}

pub(crate) const fn semantic_dependency_kind_tag(kind: PackageReviewSemanticDependencyKind) -> u8 {
    match kind {
        PackageReviewSemanticDependencyKind::NominalIdentity => 0,
        PackageReviewSemanticDependencyKind::Layout => 1,
        PackageReviewSemanticDependencyKind::OwnershipBehavior => 2,
        PackageReviewSemanticDependencyKind::AutomaticCleanup => 3,
        PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => 4,
    }
}

pub(crate) fn encode_representation_tcb_key(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &row.declaration)?;
    match &row.kind {
        PackageReviewRepresentationTcbKind::Unbound => encoder.byte(0),
        PackageReviewRepresentationTcbKind::ProducerAvailability { conformance, .. } => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_representation_tcb(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_representation_tcb_key(encoder, row)?;
    if let PackageReviewRepresentationTcbKind::ProducerAvailability { carrier, .. } = &row.kind {
        encode_nominal(encoder, carrier)?;
    }
    Ok(())
}

pub(crate) fn encode_dangerous_authority(
    encoder: &mut Encoder,
    authority: &PackageReviewDangerousAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match authority.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &authority.service)
}

pub(crate) fn encode_dangerous_authority_slack(
    encoder: &mut Encoder,
    slack: &PackageReviewDangerousAuthoritySlack,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match slack.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &slack.callable)?;
    encode_nominal(encoder, &slack.service)
}

use crate::encoding::encode::values::identity::encode_nominal;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewSemanticDependencyKind,
};

pub(crate) const fn semantic_dependency_kind_tag(kind: PackageReviewSemanticDependencyKind) -> u8 {
    match kind {
        PackageReviewSemanticDependencyKind::NominalIdentity => 0,
        PackageReviewSemanticDependencyKind::Layout => 1,
        PackageReviewSemanticDependencyKind::OwnershipBehavior => 2,
        PackageReviewSemanticDependencyKind::AutomaticCleanup => 3,
        PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => 4,
    }
}

pub(crate) fn encode_dangerous_authority(
    encoder: &mut Encoder,
    authority: &PackageReviewDangerousAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("class", |encoder| {
        match authority.class {
            PackageReviewDangerousAuthorityClass::Filesystem => encoder.tag("filesystem", 0),
            PackageReviewDangerousAuthorityClass::MachineControl => {
                encoder.tag("machine_control", 1)
            }
            PackageReviewDangerousAuthorityClass::PortIo => encoder.tag("port_io", 2),
            PackageReviewDangerousAuthorityClass::InterruptControl => {
                encoder.tag("interrupt_control", 3)
            }
            PackageReviewDangerousAuthorityClass::InterruptEntry => {
                encoder.tag("interrupt_entry", 4)
            }
            PackageReviewDangerousAuthorityClass::RootMemory => encoder.tag("root_memory", 5),
            PackageReviewDangerousAuthorityClass::Process => encoder.tag("process", 6),
            PackageReviewDangerousAuthorityClass::ConsumerAudit => encoder.tag("consumer_audit", 7),
        };
        Ok(())
    })?;
    encoder.field_by("service", &authority.service, encode_nominal)
}

pub(crate) fn encode_dangerous_authority_slack(
    encoder: &mut Encoder,
    slack: &PackageReviewDangerousAuthoritySlack,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("class", |encoder| {
        match slack.class {
            PackageReviewDangerousAuthorityClass::Filesystem => encoder.tag("filesystem", 0),
            PackageReviewDangerousAuthorityClass::MachineControl => {
                encoder.tag("machine_control", 1)
            }
            PackageReviewDangerousAuthorityClass::PortIo => encoder.tag("port_io", 2),
            PackageReviewDangerousAuthorityClass::InterruptControl => {
                encoder.tag("interrupt_control", 3)
            }
            PackageReviewDangerousAuthorityClass::InterruptEntry => {
                encoder.tag("interrupt_entry", 4)
            }
            PackageReviewDangerousAuthorityClass::RootMemory => encoder.tag("root_memory", 5),
            PackageReviewDangerousAuthorityClass::Process => encoder.tag("process", 6),
            PackageReviewDangerousAuthorityClass::ConsumerAudit => encoder.tag("consumer_audit", 7),
        };
        Ok(())
    })?;
    encoder.field_by("callable", &slack.callable, encode_nominal)?;
    encoder.field("service", |encoder| encode_nominal(encoder, &slack.service))
}

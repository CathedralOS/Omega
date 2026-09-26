use crate::encoding::encode::values::identity::encode_nominal;
use crate::encoding::{PackageReviewEncodingError, encode::encoder::Encoder};
use crate::record::{
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewTerminalAuthorityPermission,
};

pub(crate) fn encode_terminal_authority_permission_key(
    encoder: &mut Encoder,
    permission: &PackageReviewTerminalAuthorityPermission,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, permission.service())?;
    encoder.fixed_bytes(permission.service_schema().as_bytes());
    encoder.string(permission.requirement_identity())
}

pub(crate) fn encode_terminal_authority_permission(
    encoder: &mut Encoder,
    permission: &PackageReviewTerminalAuthorityPermission,
) -> Result<(), PackageReviewEncodingError> {
    encode_terminal_authority_permission_key(encoder, permission)?;
    encoder.sequence(permission.permitted().classes(), |encoder, class| {
        encoder.byte(class.canonical_tag());
        Ok(())
    })
}

pub(crate) fn encode_semantic_dependency_key(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("consumer", |encoder| {
        encode_nominal(encoder, &dependency.consumer)
    })?;
    encoder.field("dependency", |encoder| {
        encode_nominal(encoder, &dependency.dependency)
    })?;
    encoder.field("kind", |encoder| {
        let name = match dependency.kind {
            PackageReviewSemanticDependencyKind::NominalIdentity => "nominal_identity",
            PackageReviewSemanticDependencyKind::Layout => "layout",
            PackageReviewSemanticDependencyKind::OwnershipBehavior => "ownership_behavior",
            PackageReviewSemanticDependencyKind::AutomaticCleanup => "automatic_cleanup",
            PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => {
                "automatic_cleanup_machine"
            }
        };
        encoder.tag(name, semantic_dependency_kind_tag(dependency.kind));
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn encode_semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("dependency", |encoder| {
        encode_semantic_dependency_key(encoder, dependency)
    })?;
    encoder.field("exposure", |encoder| {
        match dependency.exposure {
            PackageReviewSemanticDependencyExposure::PrivateImplementation => {
                encoder.tag("private_implementation", 0)
            }
            PackageReviewSemanticDependencyExposure::PublicInterface => {
                encoder.tag("public_interface", 1)
            }
        };
        Ok(())
    })?;
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
        };
        Ok(())
    })?;
    encoder.field("service", |encoder| {
        encode_nominal(encoder, &authority.service)
    })
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
        };
        Ok(())
    })?;
    encoder.field("callable", |encoder| {
        encode_nominal(encoder, &slack.callable)
    })?;
    encoder.field("service", |encoder| encode_nominal(encoder, &slack.service))
}

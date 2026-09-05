use super::super::identity::nominal;
use super::*;

pub(super) fn dangerous_authority(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthority, Error> {
    Ok(PackageReviewDangerousAuthority {
        class: authority_class(reader)?,
        service: nominal(reader)?,
    })
}

pub(super) fn slack(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthoritySlack, Error> {
    Ok(PackageReviewDangerousAuthoritySlack {
        class: authority_class(reader)?,
        callable: nominal(reader)?,
        service: nominal(reader)?,
    })
}

pub(super) fn semantic_dependency(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicySemanticDependency, Error> {
    Ok(PackagePolicySemanticDependency {
        consumer: match reader.byte()? {
            0 => PackagePolicySemanticDependencyConsumer::Callable(nominal(reader)?),
            1 => PackagePolicySemanticDependencyConsumer::PackageImplementation,
            _ => return Err(Error::InvalidTag),
        },
        dependency: nominal(reader)?,
        kind: match reader.byte()? {
            0 => PackageReviewSemanticDependencyKind::NominalIdentity,
            1 => PackageReviewSemanticDependencyKind::Layout,
            2 => PackageReviewSemanticDependencyKind::OwnershipBehavior,
            3 => PackageReviewSemanticDependencyKind::AutomaticCleanup,
            4 => PackageReviewSemanticDependencyKind::AutomaticCleanupMachine,
            _ => return Err(Error::InvalidTag),
        },
        exposure: match reader.byte()? {
            0 => PackageReviewSemanticDependencyExposure::PrivateImplementation,
            1 => PackageReviewSemanticDependencyExposure::PublicInterface,
            _ => return Err(Error::InvalidTag),
        },
    })
}

fn authority_class(reader: &mut Reader<'_>) -> Result<PackageReviewDangerousAuthorityClass, Error> {
    use PackageReviewDangerousAuthorityClass as Class;
    Ok(match reader.byte()? {
        0 => Class::Filesystem,
        1 => Class::MachineControl,
        2 => Class::PortIo,
        3 => Class::InterruptControl,
        4 => Class::InterruptEntry,
        5 => Class::RootMemory,
        6 => Class::Process,
        _ => return Err(Error::InvalidTag),
    })
}

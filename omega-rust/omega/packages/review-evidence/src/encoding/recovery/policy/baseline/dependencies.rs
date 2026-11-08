use super::super::identity::nominal;
use super::Error;
use crate::encoding::recovery::policy::reader::Reader;
use crate::record::PackagePolicySemanticDependency;
use crate::record::PackagePolicySemanticDependencyConsumer;
use crate::record::PackageReviewDangerousAuthority;
use crate::record::PackageReviewDangerousAuthorityClass;
use crate::record::PackageReviewDangerousAuthoritySlack;
use crate::record::PackageReviewSemanticDependencyExposure;
use crate::record::PackageReviewSemanticDependencyKind;

pub(in crate::encoding::recovery) fn dangerous_authority(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthority, Error> {
    Ok(PackageReviewDangerousAuthority {
        class: authority_class(reader)?,
        service: nominal(reader)?,
    })
}

pub(in crate::encoding::recovery) fn slack(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthoritySlack, Error> {
    Ok(PackageReviewDangerousAuthoritySlack {
        class: authority_class(reader)?,
        callable: nominal(reader)?,
        service: nominal(reader)?,
    })
}

pub(in crate::encoding::recovery) fn semantic_dependency(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicySemanticDependency, Error> {
    Ok(PackagePolicySemanticDependency {
        consumer: super::super::tag!(reader => {
            0 => PackagePolicySemanticDependencyConsumer::Callable(nominal(reader)?),
            1 => PackagePolicySemanticDependencyConsumer::PackageImplementation,
        }),
        dependency: nominal(reader)?,
        kind: super::super::tag!(reader => {
            0 => PackageReviewSemanticDependencyKind::NominalIdentity,
            1 => PackageReviewSemanticDependencyKind::Layout,
            2 => PackageReviewSemanticDependencyKind::OwnershipBehavior,
            3 => PackageReviewSemanticDependencyKind::AutomaticCleanup,
            4 => PackageReviewSemanticDependencyKind::AutomaticCleanupMachine,
        }),
        exposure: super::super::tag!(reader => {
            0 => PackageReviewSemanticDependencyExposure::PrivateImplementation,
            1 => PackageReviewSemanticDependencyExposure::PublicInterface,
        }),
    })
}

pub(in crate::encoding::recovery) fn authority_class(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthorityClass, Error> {
    use PackageReviewDangerousAuthorityClass as Class;
    Ok(super::super::tag!(reader => {
        0 => Class::Filesystem,
        1 => Class::MachineControl,
        2 => Class::PortIo,
        3 => Class::InterruptControl,
        4 => Class::InterruptEntry,
        5 => Class::RootMemory,
        6 => Class::Process,
        7 => Class::ConsumerAudit,
    }))
}

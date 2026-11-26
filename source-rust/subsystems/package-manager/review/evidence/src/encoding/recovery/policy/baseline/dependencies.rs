// Bounded recovery for dangerous authorities, authority slack, and semantic dependencies in policy baselines.

use super::super::identity::nominal;
use super::Error;
use crate::encoding::recovery::policy::identity::service;
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
        service: service(reader)?,
    })
}

pub(in crate::encoding::recovery) fn slack(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthoritySlack, Error> {
    Ok(PackageReviewDangerousAuthoritySlack {
        class: authority_class(reader)?,
        callable: nominal(reader)?,
        service: service(reader)?,
    })
}

pub(in crate::encoding::recovery) fn semantic_dependency(
    reader: &mut Reader<'_>,
    schema_version: u16,
) -> Result<PackagePolicySemanticDependency, Error> {
    let consumer = match reader.byte()? {
        0 => PackagePolicySemanticDependencyConsumer::Callable(nominal(reader)?),
        1 => PackagePolicySemanticDependencyConsumer::PackageImplementation,
        2 if schema_version >= crate::encoding::BASELINE_WITH_CARRIED_TYPES => {
            return Ok(PackagePolicySemanticDependency::CarriedType {
                dependency: nominal(reader)?,
                supplier_sources:
                    source_provenance::PackageSourceConsumptionCommitment::from_retained_digest(
                        reader.digest()?,
                    ),
                exposure: exposure(reader)?,
            });
        }
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackagePolicySemanticDependency::Declaration {
        consumer,
        dependency: nominal(reader)?,
        kind: super::super::tag!(reader => {
            0 => PackageReviewSemanticDependencyKind::NominalIdentity,
            1 => PackageReviewSemanticDependencyKind::Layout,
            2 => PackageReviewSemanticDependencyKind::OwnershipBehavior,
            3 => PackageReviewSemanticDependencyKind::AutomaticCleanup,
            4 => PackageReviewSemanticDependencyKind::AutomaticCleanupMachine,
        }),
        exposure: exposure(reader)?,
    })
}

fn exposure(reader: &mut Reader<'_>) -> Result<PackageReviewSemanticDependencyExposure, Error> {
    Ok(super::super::tag!(reader => {
        0 => PackageReviewSemanticDependencyExposure::PrivateImplementation,
        1 => PackageReviewSemanticDependencyExposure::PublicInterface,
    }))
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
        8 => Class::Network,
    }))
}

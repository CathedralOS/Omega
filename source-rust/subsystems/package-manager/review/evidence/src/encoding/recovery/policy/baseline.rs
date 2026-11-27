// Inert baseline recovery under one shared reader and allocation budget.
// `recover_canonical` restores comparison meaning without old source, proof, or native replay.

use super::identity::{nominal, operator_coordinate, package, type_identity};
use super::reader::Reader;
use super::selected_providers::target;
use super::{
    Error, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage, callable_policy, external,
    intrinsic, public_api, representation, selected_providers, terminal_permissions,
};
use crate::encoding::recovery::policy::identity::service;
use crate::encoding::{
    BASELINE_WITHOUT_STRUCTURAL_PARAMETERS, PACKAGE_POLICY_BASELINE_MAGIC,
    PACKAGE_POLICY_BASELINE_VERSION,
};
use crate::record::{
    PackagePolicyBaseline, PackagePolicyBoundaryApplicationDemand,
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization, PackagePolicyEvaluationSponsorLimits,
    PackagePolicyFilesystemSponsorLimits, PackagePolicyNativeBinding,
    PackagePolicyRestrictedBuildBounds, PackagePolicyRestrictedBuildGrant,
    PackagePolicyRestrictedBuildGrantRoot, PackagePolicyRestrictedBuildOperation,
    PackagePolicyRestrictedBuildRequest, PackagePolicySemanticDependency,
    PackagePolicySemanticDependencyConsumer, PackageReviewBoundaryApplication,
    PackageReviewBoundaryApplicationArgument, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthorityClass, PackageReviewDangerousAuthoritySlack,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSymbolicBoundaryApplicationArgument,
};

impl PackagePolicyBaseline {
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        Self::recover_canonical_with_usage(bytes, limits).map(|(policy, _)| policy)
    }

    pub(in crate::encoding::recovery) fn recover_canonical_with_usage(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<(Self, PackagePolicyRecoveryUsage), Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(PACKAGE_POLICY_BASELINE_MAGIC)?;
        let schema_version = reader.u16()?;
        if !matches!(
            schema_version,
            BASELINE_WITHOUT_STRUCTURAL_PARAMETERS
                | crate::encoding::BASELINE_WITH_STRUCTURAL_PARAMETERS
                | crate::encoding::BASELINE_WITH_PROVIDER_APPLICATION_GRANTS
                | crate::encoding::BASELINE_WITH_TYPE_ALIASES
                | crate::encoding::BASELINE_WITH_CARRIED_TYPES
                | crate::encoding::BASELINE_WITH_APPLIED_SERVICES
                | PACKAGE_POLICY_BASELINE_VERSION
        ) {
            return Err(Error::UnsupportedVersion);
        }
        reader.applied_services = schema_version >= crate::encoding::BASELINE_WITH_APPLIED_SERVICES;
        let policy = Self {
            schema_version,
            package: package(&mut reader)?,
            target: selected_providers::target(&mut reader)?,
            public_api: public_api::public_api(&mut reader, schema_version)?,
            callables: callable_policy::policy(&mut reader)?,
            selected_providers: selected_providers::policy(&mut reader)?,
            terminal_permissions: terminal_permissions::policy(&mut reader)?,
            representation: representation::policy(&mut reader)?,
            native_bindings: reader.sequence(1, |reader| native_binding(reader, schema_version))?,
            external_supplies: reader.sequence(1, external::policy)?,
            dangerous_capabilities: reader.sequence(1, dangerous_authority)?,
            slack_uses: reader.sequence(1, slack)?,
            semantic_dependencies: reader
                .sequence(1, |reader| semantic_dependency(reader, schema_version))?,
            boundary_applications: applications(&mut reader)?,
            restricted_build_requests: requests(&mut reader)?,
            service_audits: reader.sequence(1, service)?,
            component_assumptions: reader.sequence(1, component_assumption)?,
        };
        reader.finish()?;
        policy
            .validate_canonical_structure()
            .map_err(|_| Error::InvalidValue)?;
        reader.canonical_scratch(bytes.len())?;
        if policy
            .canonical_bytes_for_recovery(bytes.len())
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok((policy, reader.usage()))
    }
}

fn component_assumption(
    reader: &mut Reader<'_>,
) -> Result<crate::record::PackagePolicyComponentAssumption, Error> {
    Ok(crate::record::PackagePolicyComponentAssumption {
        component: package(reader)?,
        assumption: reader.digest()?,
        bound_rows: reader.sequence(1, |reader| reader.string())?,
    })
}

fn native_binding(
    reader: &mut Reader<'_>,
    schema_version: u16,
) -> Result<PackagePolicyNativeBinding, Error> {
    use crate::record::PackagePolicyNativeApplication as Application;
    let application = match reader.byte()? {
        0 => Application::Concrete(None),
        1 => Application::Concrete(Some(reader.string()?)),
        2 if schema_version >= crate::encoding::BASELINE_WITH_UNUSED_GENERIC_IMPORTS => {
            Application::UnusedGeneric {
                declaration: reader.string()?,
                unbound_parameters: reader.sequence(8, |reader| reader.string())?,
                custom_calling_policy: reader.option(nominal)?,
                source_occurrence: reader.digest()?,
                evaluation: reader.digest()?,
            }
        }
        _ => return Err(Error::InvalidTag),
    };
    Ok(PackagePolicyNativeBinding {
        application,
        method: selected_providers::service_method(reader)?,
        binding: selected_providers::bindings::binding(reader)?,
    })
}

// Bounded recovery for boundary application demands and realizations in policy baselines.

pub(in crate::encoding::recovery) fn applications(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyBoundaryApplications, Error> {
    Ok(PackagePolicyBoundaryApplications {
        demands: reader.sequence(1, |reader| {
            Ok(PackagePolicyBoundaryApplicationDemand {
                operator_coordinate: operator_coordinate(reader)?,
                producer_callable: nominal(reader)?,
                arguments: reader.sequence(9, |reader| {
                    Ok(super::tag!(reader => {
                        0 => PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                            requirement_binder_ordinal: reader.u32()?,
                            producer_binder_ordinal: reader.u32()?,
                        },
                    }))
                })?,
            })
        })?,
        realizations: reader.sequence(1, |reader| {
            Ok(PackagePolicyBoundaryApplicationRealization {
                operator_coordinate: operator_coordinate(reader)?,
                requirement_identity: reader.string()?,
                application: application(reader)?,
                selected_plan_index: reader.u32()?,
                realization: super::tag!(reader => {
                    0 => PackagePolicyBoundaryRealization::NongenericCheckedBody {
                        declaration: nominal(reader)?,
                        realization: nominal(reader)?,
                    },
                    1 => PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                        declaration: nominal(reader)?,
                        template: nominal(reader)?,
                    },
                    2 => PackagePolicyBoundaryRealization::ExactCompilerIntrinsic {
                        execution: intrinsic::execution(reader)?,
                    },
                }),
            })
        })?,
    })
}

pub(in crate::encoding::recovery) fn application(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewBoundaryApplication, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewBoundaryApplication::Empty,
        1 => PackageReviewBoundaryApplication::Exact(reader.sequence(1, |reader| {
            Ok(super::tag!(reader => {
                0 => PackageReviewBoundaryApplicationArgument::Type {
                    binder_ordinal: reader.u32()?,
                    type_identity: type_identity(reader)?,
                },
                1 => PackageReviewBoundaryApplicationArgument::Const {
                    binder_ordinal: reader.u32()?,
                    declared_carrier: type_identity(reader)?,
                    value_type: reader.string()?,
                    value_encoding: reader.string()?,
                },
            }))
        })?),
    }))
}

// Bounded recovery for dangerous authorities, authority slack, and semantic dependencies in policy baselines.

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
        kind: super::tag!(reader => {
            0 => PackageReviewSemanticDependencyKind::NominalIdentity,
            1 => PackageReviewSemanticDependencyKind::Layout,
            2 => PackageReviewSemanticDependencyKind::OwnershipBehavior,
            3 => PackageReviewSemanticDependencyKind::AutomaticCleanup,
            4 => PackageReviewSemanticDependencyKind::AutomaticCleanupMachine,
        }),
        exposure: exposure(reader)?,
    })
}

pub(in crate::encoding::recovery::policy::baseline) fn exposure(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewSemanticDependencyExposure, Error> {
    Ok(super::tag!(reader => {
        0 => PackageReviewSemanticDependencyExposure::PrivateImplementation,
        1 => PackageReviewSemanticDependencyExposure::PublicInterface,
    }))
}

pub(in crate::encoding::recovery) fn authority_class(
    reader: &mut Reader<'_>,
) -> Result<PackageReviewDangerousAuthorityClass, Error> {
    use PackageReviewDangerousAuthorityClass as Class;
    Ok(super::tag!(reader => {
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

// Restricted build-host request recovery under the shared reader budget.
// Minimum-encoded-byte constants bound sequence reservation before element decoding: `REQUEST_MINIMUM_BYTES` (operation tag + two grant sequence counts + bounds + two profile option tags), `GRANT_MINIMUM_BYTES` (root tag + narrowed flag + captured flag), and `REQUIRED_OUTPUT_MINIMUM_BYTES` (length-prefixed byte string length).

pub(in crate::encoding::recovery::policy::baseline) const REQUEST_MINIMUM_BYTES: usize =
    1 + 8 + 8 + 11 + 1 + 1;
pub(in crate::encoding::recovery::policy::baseline) const GRANT_MINIMUM_BYTES: usize = 3;
pub(in crate::encoding::recovery::policy::baseline) const REQUIRED_OUTPUT_MINIMUM_BYTES: usize = 8;

pub(in crate::encoding::recovery) fn request(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRestrictedBuildRequest, Error> {
    let operation = super::tag!(reader => {
        0 => PackagePolicyRestrictedBuildOperation::ScopedFilesystemExecution,
        1 => PackagePolicyRestrictedBuildOperation::UnscopedFilesystemExecution,
        2 => PackagePolicyRestrictedBuildOperation::ProgramLaunch(Box::new(build_consent::RestrictedProgramLaunch {
            owner: super::tag!(reader => {
                0 => build_consent::ProgramDeclarationOwner::RequestingSource,
                1 => build_consent::ProgramDeclarationOwner::Package(reader.digest()?),
                2 => build_consent::ProgramDeclarationOwner::Toolchain(reader.digest()?),
            }),
            declaration: reader.string()?, file: reader.string()?, image_sha256: reader.digest()?,
            readable: reader.sequence(REQUIRED_OUTPUT_MINIMUM_BYTES, Reader::bytes)?,
            writable: reader.sequence(REQUIRED_OUTPUT_MINIMUM_BYTES, Reader::bytes)?,
            timeout_ms: reader.u64()?,
        })),
    });
    Ok(PackagePolicyRestrictedBuildRequest {
        operation,
        read_grants: reader.sequence(GRANT_MINIMUM_BYTES, grant)?,
        write_grants: reader.sequence(GRANT_MINIMUM_BYTES, grant)?,
        bounds: bounds(reader)?,
        build_execution_profile: reader.option(target)?,
        selected_target_profile: reader.option(target)?,
    })
}

pub(in crate::encoding::recovery) fn grant(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRestrictedBuildGrant, Error> {
    Ok(PackagePolicyRestrictedBuildGrant {
        root: super::tag!(reader => {
            0 => PackagePolicyRestrictedBuildGrantRoot::SourceInventory,
            1 => PackagePolicyRestrictedBuildGrantRoot::StagedOutput,
            2 => PackagePolicyRestrictedBuildGrantRoot::Other(reader.u32()?),
        }),
        narrowed: reader.boolean()?,
        captured: reader.boolean()?,
    })
}

pub(in crate::encoding::recovery) fn bounds(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyRestrictedBuildBounds, Error> {
    Ok(PackagePolicyRestrictedBuildBounds {
        filesystem_sponsor_limits: reader.option(|reader| {
            Ok(PackagePolicyFilesystemSponsorLimits {
                maximum_entries: reader.u64()?,
                maximum_total_logical_bytes: reader.u64()?,
                maximum_object_extent: reader.u64()?,
            })
        })?,
        evaluation_sponsor_limits: reader.option(|reader| {
            Ok(PackagePolicyEvaluationSponsorLimits {
                work_budget: reader.u64()?,
                maximum_build_log_bytes: reader.u64()?,
                maximum_filesystem_operation_attempts: reader.u64()?,
                maximum_live_filesystem_handles: reader.u64()?,
                maximum_live_cells: reader.u64()?,
                maximum_live_text_bytes: reader.u64()?,
                maximum_result_cells: reader.u64()?,
                maximum_result_text_bytes: reader.u64()?,
            })
        })?,
        required_outputs: reader.sequence(REQUIRED_OUTPUT_MINIMUM_BYTES, Reader::bytes)?,
        artifact_only: reader.boolean()?,
    })
}

pub(in crate::encoding::recovery) fn requests(
    reader: &mut Reader<'_>,
) -> Result<Vec<PackagePolicyRestrictedBuildRequest>, Error> {
    reader.sequence(REQUEST_MINIMUM_BYTES, request)
}

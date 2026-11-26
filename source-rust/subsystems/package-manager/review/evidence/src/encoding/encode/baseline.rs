// Canonical wire framing and resource budget for the composed package policy baseline.

use super::encoder::Encoder;
use super::{
    callable_policy, declarations, public_api, representation, selected_providers,
    terminal_permissions, values,
};
use crate::encoding::encode::values::identity::encode_service;
use crate::encoding::{
    PACKAGE_POLICY_BASELINE_MAGIC, PackagePolicyRecoveryLimits, PackageReviewEncodingError,
};
use crate::record::{
    PackagePolicyBaseline, PackagePolicyBoundaryApplicationDemand,
    PackagePolicyBoundaryApplicationRealization, PackagePolicyBoundaryApplications,
    PackagePolicyBoundaryRealization, PackagePolicyEvaluationSponsorLimits,
    PackagePolicyFilesystemSponsorLimits, PackagePolicyRestrictedBuildBounds,
    PackagePolicyRestrictedBuildGrant, PackagePolicyRestrictedBuildGrantRoot,
    PackagePolicyRestrictedBuildOperation, PackagePolicyRestrictedBuildRequest,
    PackagePolicySemanticDependency, PackagePolicySemanticDependencyConsumer,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSymbolicBoundaryApplicationArgument,
};
use declarations::{
    encode_dangerous_authority, encode_dangerous_authority_slack, semantic_dependency_kind_tag,
};
use selected_providers::{encode_binding, encode_service_method};
use target::TargetProfile;
use values::declarations::encode_operator_coordinate;
use values::identity::encode_nominal;
use values::providers::{encode_boundary_application, encode_compiler_intrinsic_execution};

impl PackagePolicyBaseline {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        let mut encoder =
            Encoder::policy_bounded(PackagePolicyRecoveryLimits::default().maximum_bytes);
        framed_policy(&mut encoder, self)?;
        encoder.finish()
    }
}

pub(in crate::encoding) fn framed_policy(
    encoder: &mut Encoder,
    value: &PackagePolicyBaseline,
) -> Result<(), PackageReviewEncodingError> {
    encoder.fixed_bytes_field("binary_format", PACKAGE_POLICY_BASELINE_MAGIC)?;
    encoder.u16_field("baseline_schema", value.schema_version)?;
    encoder.record("package_policy", |encoder| policy(encoder, value))
}

pub(in crate::encoding) fn policy(
    encoder: &mut Encoder,
    value: &PackagePolicyBaseline,
) -> Result<(), PackageReviewEncodingError> {
    encoder.applied_services =
        value.schema_version >= crate::encoding::BASELINE_WITH_APPLIED_SERVICES;
    encoder.package_identity_field("package", value.package)?;
    encoder.string_field("target", value.target.identity().as_str())?;
    encoder.field("public_api", |encoder| {
        public_api::public_api(encoder, &value.public_api, value.schema_version)
    })?;
    encoder.field_by("callables", &value.callables, callable_policy::policy)?;
    encoder.field_by(
        "selected_providers",
        &value.selected_providers,
        selected_providers::policy,
    )?;
    encoder.field_by(
        "terminal_permissions",
        &value.terminal_permissions,
        terminal_permissions::policy,
    )?;
    encoder.field_by(
        "representation",
        &value.representation,
        representation::policy,
    )?;
    encoder.sequence_field("native_bindings", &value.native_bindings, native_binding)?;
    encoder.field("external_supplies", |encoder| {
        encoder.sequence(&value.external_supplies, values::external_policy::policy)
    })?;
    encoder.field("dangerous_capabilities", |encoder| {
        encoder.sequence(&value.dangerous_capabilities, encode_dangerous_authority)
    })?;
    encoder.field("slack_uses", |encoder| {
        encoder.sequence(&value.slack_uses, encode_dangerous_authority_slack)
    })?;
    encoder.field("semantic_dependencies", |encoder| {
        encoder.sequence(
            value
                .validated_semantic_dependencies()
                .map_err(PackageReviewEncodingError::new)?,
            semantic_dependency,
        )
    })?;
    encoder.field_by(
        "boundary_applications",
        &value.boundary_applications,
        applications,
    )?;
    encoder.field("restricted_build_requests", |encoder| {
        encoder.sequence(&value.restricted_build_requests, request)
    })?;
    encoder.field("service_audits", |encoder| {
        encoder.sequence(&value.service_audits, encode_service)
    })?;
    encoder.sequence_field(
        "component_assumptions",
        &value.component_assumptions,
        component_assumption,
    )
}

pub(super) fn component_assumption(
    encoder: &mut Encoder,
    value: &crate::record::PackagePolicyComponentAssumption,
) -> Result<(), PackageReviewEncodingError> {
    encoder.package_identity_field("component", value.component)?;
    encoder.fixed_bytes_field("assumption", &value.assumption)?;
    encoder.field("bound_rows", |encoder| {
        encoder.sequence(&value.bound_rows, |encoder, row| encoder.string(row))
    })
}

pub(super) fn carried_type_key(
    encoder: &mut Encoder,
    nominal: &crate::record::PackageReviewNominalIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.tag("carried_type", 2);
    encoder.field_by("dependency", nominal, encode_nominal)
}

pub(super) fn semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackagePolicySemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    match dependency {
        PackagePolicySemanticDependency::CarriedType {
            dependency,
            supplier_sources,
            ..
        } => {
            carried_type_key(encoder, dependency)?;
            encoder.fixed_bytes_field("supplier_sources", &supplier_sources.digest())?;
        }
        PackagePolicySemanticDependency::Declaration {
            consumer,
            dependency,
            kind,
            ..
        } => {
            encoder.field("consumer", |encoder| {
                match consumer {
                    PackagePolicySemanticDependencyConsumer::Callable(identity) => {
                        encoder.tag("callable", 0);
                        encoder.field_by("identity", identity, encode_nominal)?;
                    }
                    PackagePolicySemanticDependencyConsumer::PackageImplementation => {
                        encoder.tag("package_implementation", 1)
                    }
                }
                Ok(())
            })?;
            encoder.field_by("dependency", dependency, encode_nominal)?;
            encoder.field("kind", |encoder| {
                let name = match kind {
                    PackageReviewSemanticDependencyKind::NominalIdentity => "nominal_identity",
                    PackageReviewSemanticDependencyKind::Layout => "layout",
                    PackageReviewSemanticDependencyKind::OwnershipBehavior => "ownership_behavior",
                    PackageReviewSemanticDependencyKind::AutomaticCleanup => "automatic_cleanup",
                    PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => {
                        "automatic_cleanup_machine"
                    }
                };
                encoder.tag(name, semantic_dependency_kind_tag(*kind));
                Ok(())
            })?;
        }
    }
    encoder.field("exposure", |encoder| {
        match dependency.exposure() {
            PackageReviewSemanticDependencyExposure::PrivateImplementation => {
                encoder.tag("private_implementation", 0)
            }
            PackageReviewSemanticDependencyExposure::PublicInterface => {
                encoder.tag("public_interface", 1)
            }
        }
        Ok(())
    })
}

pub(super) fn native_binding(
    encoder: &mut Encoder,
    value: &crate::record::PackagePolicyNativeBinding,
) -> Result<(), PackageReviewEncodingError> {
    encoder.option_field(
        "application",
        value.application.as_deref(),
        |encoder, value| encoder.string(value),
    )?;
    encoder.field_by("requirement", &value.method, encode_service_method)?;
    encoder.field_by("binding", &value.binding, encode_binding)
}

// Canonical baseline verification encoding using preallocated recovery scratch capacity.

impl PackagePolicyBaseline {
    pub(in crate::encoding) fn canonical_bytes_for_recovery(
        &self,
        expected_length: usize,
    ) -> Result<Vec<u8>, PackageReviewEncodingError> {
        let mut encoder = Encoder::policy_preallocated(expected_length)?;
        framed_policy(&mut encoder, self)?;
        let bytes = encoder.finish()?;
        if bytes.len() != expected_length {
            return Err(PackageReviewEncodingError::new(
                "package policy verification differs from its expected length",
            ));
        }
        Ok(bytes)
    }
}

// Canonical encoding for boundary application demands and realizations in the package policy baseline.

pub(super) fn applications(
    encoder: &mut Encoder,
    applications: &PackagePolicyBoundaryApplications,
) -> Result<(), PackageReviewEncodingError> {
    encoder.sequence_field("demands", &applications.demands, demand)?;
    encoder.sequence_field("realizations", &applications.realizations, realization)
}

pub(super) fn demand(
    encoder: &mut Encoder,
    demand: &PackagePolicyBoundaryApplicationDemand,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by(
        "operator_coordinate",
        &demand.operator_coordinate,
        encode_operator_coordinate,
    )?;
    encoder.field_by(
        "producer_callable",
        &demand.producer_callable,
        encode_nominal,
    )?;
    encoder.field("arguments", |encoder| {
        encoder.sequence(&demand.arguments, |encoder, argument| {
            match argument {
                PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    requirement_binder_ordinal,
                    producer_binder_ordinal,
                } => {
                    encoder.tag("type_binder", 0);
                    encoder.u32_field("requirement_binder_ordinal", *requirement_binder_ordinal)?;
                    encoder.u32_field("producer_binder_ordinal", *producer_binder_ordinal)?;
                }
            }
            Ok(())
        })
    })
}

pub(super) fn realization(
    encoder: &mut Encoder,
    realization: &PackagePolicyBoundaryApplicationRealization,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field_by(
        "operator_coordinate",
        &realization.operator_coordinate,
        encode_operator_coordinate,
    )?;
    encoder.string_field("requirement_identity", &realization.requirement_identity)?;
    encoder.field_by(
        "application",
        &realization.application,
        encode_boundary_application,
    )?;
    encoder.u32_field("selected_plan_index", realization.selected_plan_index)?;
    encoder.field("realization", |encoder| {
        match &realization.realization {
            PackagePolicyBoundaryRealization::NongenericCheckedBody {
                declaration,
                realization,
            } => {
                encoder.tag("nongeneric_checked_body", 0);
                encoder.field_by("declaration", declaration, encode_nominal)?;
                encoder.field_by("realization", realization, encode_nominal)?;
            }
            PackagePolicyBoundaryRealization::SpecializedCheckedBody {
                declaration,
                template,
            } => {
                encoder.tag("specialized_checked_body", 1);
                encoder.field_by("declaration", declaration, encode_nominal)?;
                encoder.field("template", |encoder| encode_nominal(encoder, template))?;
            }
            PackagePolicyBoundaryRealization::ExactCompilerIntrinsic { execution } => {
                encoder.tag("exact_compiler_intrinsic", 2);
                encoder.field_by("execution", execution, encode_compiler_intrinsic_execution)?;
            }
        }
        Ok(())
    })
}

// Canonical encoding for restricted build-host requests in grant order, retaining captured-inventory presence without binding volatile source entry counts.

pub(super) fn operation(operation: &PackagePolicyRestrictedBuildOperation) -> (&'static str, u8) {
    match operation {
        PackagePolicyRestrictedBuildOperation::ProgramLaunch(_) => ("program_launch", 2),
        PackagePolicyRestrictedBuildOperation::ScopedFilesystemExecution => {
            ("scoped_filesystem_execution", 0)
        }
        PackagePolicyRestrictedBuildOperation::UnscopedFilesystemExecution => {
            ("unscoped_filesystem_execution", 1)
        }
    }
}

pub(super) fn request(
    encoder: &mut Encoder,
    request: &PackagePolicyRestrictedBuildRequest,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("operation", |encoder| {
        let (name, tag) = operation(&request.operation);
        encoder.tag(name, tag);
        Ok(())
    })?;
    if let PackagePolicyRestrictedBuildOperation::ProgramLaunch(program) = &request.operation {
        encoder.field("program_owner", |encoder| {
            use build_consent::ProgramDeclarationOwner;
            match program.owner {
                ProgramDeclarationOwner::RequestingSource => encoder.tag("requesting_source", 0),
                ProgramDeclarationOwner::Package(digest) => {
                    encoder.tag("package", 1);
                    encoder.fixed_bytes_field("key", &digest)?;
                }
                ProgramDeclarationOwner::Toolchain(digest) => {
                    encoder.tag("toolchain_source", 2);
                    encoder.fixed_bytes_field("source", &digest)?;
                }
            }
            Ok(())
        })?;
        encoder.string_field("declaration", &program.declaration)?;
        encoder.string_field("file", &program.file)?;
        encoder.fixed_bytes_field("image_sha256", &program.image_sha256)?;
        encoder.sequence_field("readable", &program.readable, |encoder, path| {
            encoder.bytes(path)
        })?;
        encoder.sequence_field("writable", &program.writable, |encoder, path| {
            encoder.bytes(path)
        })?;
        encoder.u64_field("timeout_ms", program.timeout_ms)?;
    }
    encoder.sequence_field("read_grants", &request.read_grants, grant)?;
    encoder.sequence_field("write_grants", &request.write_grants, grant)?;
    encoder.field("bounds", |encoder| bounds(encoder, &request.bounds))?;
    encoder.field_by(
        "build_execution_profile",
        request.build_execution_profile,
        profile,
    )?;
    encoder.field_by(
        "selected_target_profile",
        request.selected_target_profile,
        profile,
    )
}

pub(in crate::encoding::encode::baseline) fn grant(
    encoder: &mut Encoder,
    grant: &PackagePolicyRestrictedBuildGrant,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("root", |encoder| {
        match grant.root {
            PackagePolicyRestrictedBuildGrantRoot::SourceInventory => {
                encoder.tag("source_inventory", 0)
            }
            PackagePolicyRestrictedBuildGrantRoot::StagedOutput => encoder.tag("staged_output", 1),
            PackagePolicyRestrictedBuildGrantRoot::Other(identity) => {
                encoder.tag("other", 2);
                encoder.u32_field("identity", identity)?;
            }
        }
        Ok(())
    })?;
    encoder.boolean_field("narrowed", grant.narrowed)?;
    encoder.boolean_field("captured", grant.captured)
}

pub(in crate::encoding::encode::baseline) fn bounds(
    encoder: &mut Encoder,
    bounds: &PackagePolicyRestrictedBuildBounds,
) -> Result<(), PackageReviewEncodingError> {
    encoder.option_field(
        "filesystem_sponsor_limits",
        bounds.filesystem_sponsor_limits.as_ref(),
        filesystem_sponsor_limits,
    )?;
    encoder.option_field(
        "evaluation_sponsor_limits",
        bounds.evaluation_sponsor_limits.as_ref(),
        evaluation_sponsor_limits,
    )?;
    encoder.field("required_outputs", |encoder| {
        encoder.sequence(&bounds.required_outputs, |encoder, output| {
            encoder.bytes(output)
        })
    })?;
    encoder.boolean_field("artifact_only", bounds.artifact_only)
}

pub(in crate::encoding::encode::baseline) fn filesystem_sponsor_limits(
    encoder: &mut Encoder,
    limits: &PackagePolicyFilesystemSponsorLimits,
) -> Result<(), PackageReviewEncodingError> {
    encoder.u64_field("maximum_entries", limits.maximum_entries)?;
    encoder.u64_field(
        "maximum_total_logical_bytes",
        limits.maximum_total_logical_bytes,
    )?;
    encoder.u64_field("maximum_object_extent", limits.maximum_object_extent)
}

pub(in crate::encoding::encode::baseline) fn evaluation_sponsor_limits(
    encoder: &mut Encoder,
    limits: &PackagePolicyEvaluationSponsorLimits,
) -> Result<(), PackageReviewEncodingError> {
    for (name, value) in [
        ("work_budget", limits.work_budget),
        ("maximum_build_log_bytes", limits.maximum_build_log_bytes),
        (
            "maximum_filesystem_operation_attempts",
            limits.maximum_filesystem_operation_attempts,
        ),
        (
            "maximum_live_filesystem_handles",
            limits.maximum_live_filesystem_handles,
        ),
        ("maximum_live_cells", limits.maximum_live_cells),
        ("maximum_live_text_bytes", limits.maximum_live_text_bytes),
        ("maximum_result_cells", limits.maximum_result_cells),
        (
            "maximum_result_text_bytes",
            limits.maximum_result_text_bytes,
        ),
    ] {
        encoder.field(name, |encoder| {
            encoder.u64(value);
            Ok(())
        })?;
    }
    Ok(())
}

pub(in crate::encoding::encode::baseline) fn profile(
    encoder: &mut Encoder,
    profile: Option<TargetProfile>,
) -> Result<(), PackageReviewEncodingError> {
    encoder.option(profile.as_ref(), |encoder, profile| {
        encoder.string(profile.identity().as_str())
    })
}

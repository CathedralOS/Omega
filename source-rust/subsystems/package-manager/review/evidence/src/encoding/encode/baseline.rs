// Canonical wire framing and resource budget for the composed package policy baseline.

use crate::encoding::encode::values::identity::encode_service;
use crate::record::{
    PackagePolicyBaseline, PackagePolicySemanticDependency,
    PackagePolicySemanticDependencyConsumer, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind,
};
pub(super) mod boundary;
mod recovery;
pub(super) mod restricted_build;

use super::{
    callable_policy, declarations, encoder::Encoder, public_api, representation,
    selected_providers, terminal_permissions, values,
};
use crate::encoding::{
    PACKAGE_POLICY_BASELINE_MAGIC, PackagePolicyRecoveryLimits, PackageReviewEncodingError,
};
use declarations::encode_dangerous_authority;
use declarations::encode_dangerous_authority_slack;
use declarations::semantic_dependency_kind_tag;
use selected_providers::encode_binding;
use selected_providers::encode_service_method;
use values::identity::encode_nominal;

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
        boundary::applications,
    )?;
    encoder.field("restricted_build_requests", |encoder| {
        encoder.sequence(&value.restricted_build_requests, restricted_build::request)
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

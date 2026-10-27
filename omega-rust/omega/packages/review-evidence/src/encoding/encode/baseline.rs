//! One envelope and resource budget for the composed package policy.

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
    PACKAGE_POLICY_BASELINE_MAGIC, PACKAGE_POLICY_BASELINE_VERSION, PackagePolicyRecoveryLimits,
    PackageReviewEncodingError,
};

impl PackagePolicyBaseline {
    /// Deterministic comparison payload. These bytes grant no acceptance or
    /// executable authority and contain no replay certificate.
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
    encoder.field("binary_format", |encoder| {
        encoder.fixed_bytes(PACKAGE_POLICY_BASELINE_MAGIC);
        Ok(())
    })?;
    encoder.field("baseline_schema", |encoder| {
        encoder.u16(PACKAGE_POLICY_BASELINE_VERSION);
        Ok(())
    })?;
    encoder.record("package_policy", |encoder| policy(encoder, value))
}

pub(in crate::encoding) fn policy(
    encoder: &mut Encoder,
    value: &PackagePolicyBaseline,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("package", |encoder| {
        encoder.package_identity(value.package);
        Ok(())
    })?;
    encoder.field("target", |encoder| {
        encoder.string(value.target.identity().as_str())
    })?;
    encoder.field_by("public_api", &value.public_api, public_api::public_api)?;
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
    encoder.field("external_supplies", |encoder| {
        encoder.sequence(&value.external_supplies, values::external_policy::policy)
    })?;
    encoder.field("dangerous_capabilities", |encoder| {
        encoder.sequence(
            &value.dangerous_capabilities,
            declarations::encode_dangerous_authority,
        )
    })?;
    encoder.field("slack_uses", |encoder| {
        encoder.sequence(
            &value.slack_uses,
            declarations::encode_dangerous_authority_slack,
        )
    })?;
    encoder.field("semantic_dependencies", |encoder| {
        encoder.sequence(&value.semantic_dependencies, semantic_dependency)
    })?;
    encoder.field_by(
        "boundary_applications",
        &value.boundary_applications,
        boundary::applications,
    )?;
    encoder.field("restricted_build_requests", |encoder| {
        encoder.sequence(&value.restricted_build_requests, restricted_build::request)
    })?;
    encoder.field("assumed_bindings", |encoder| {
        encoder.sequence(&value.assumed_bindings, |encoder, row| encoder.string(row))
    })?;
    encoder.field("service_audits", |encoder| {
        encoder.sequence(&value.service_audits, values::identity::encode_nominal)
    })
}

pub(super) fn semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackagePolicySemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encoder.field("consumer", |encoder| {
        match &dependency.consumer {
            PackagePolicySemanticDependencyConsumer::Callable(identity) => {
                encoder.tag("callable", 0);
                encoder.field_by("identity", identity, values::identity::encode_nominal)?;
            }
            PackagePolicySemanticDependencyConsumer::PackageImplementation => {
                encoder.tag("package_implementation", 1)
            }
        }
        Ok(())
    })?;
    encoder.field_by(
        "dependency",
        &dependency.dependency,
        values::identity::encode_nominal,
    )?;
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
        encoder.tag(
            name,
            declarations::semantic_dependency_kind_tag(dependency.kind),
        );
        Ok(())
    })?;
    encoder.field("exposure", |encoder| {
        match dependency.exposure {
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

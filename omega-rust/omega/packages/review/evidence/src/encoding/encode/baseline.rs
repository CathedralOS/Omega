//! One envelope and resource budget for the composed package policy.

mod boundary;

use super::{
    callable_policy, declarations, encoder::Encoder, public_api, representation,
    selected_providers, terminal_permissions, values,
};
use crate::encoding::{
    PACKAGE_POLICY_BASELINE_MAGIC, PACKAGE_POLICY_BASELINE_VERSION, PackageReviewEncodingError,
};
use crate::record::*;

impl PackagePolicyBaseline {
    /// Deterministic comparison payload. These bytes grant no acceptance or
    /// executable authority and contain no replay certificate.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        self.validate_canonical_structure()
            .map_err(PackageReviewEncodingError::new)?;
        let mut encoder = Encoder::policy_bounded(4 * 1024 * 1024);
        encoder.fixed_bytes(PACKAGE_POLICY_BASELINE_MAGIC);
        encoder.u16(PACKAGE_POLICY_BASELINE_VERSION);
        policy(&mut encoder, self)?;
        encoder.finish()
    }
}

pub(in crate::encoding) fn policy(
    encoder: &mut Encoder,
    value: &PackagePolicyBaseline,
) -> Result<(), PackageReviewEncodingError> {
    encoder.package_identity(value.package);
    encoder.string(value.target.identity().as_str())?;
    public_api::public_api(encoder, &value.public_api)?;
    callable_policy::policy(encoder, &value.callables)?;
    selected_providers::policy(encoder, &value.selected_providers)?;
    terminal_permissions::policy(encoder, &value.terminal_permissions)?;
    representation::policy(encoder, &value.representation)?;
    encoder.sequence(&value.external_supplies, values::external_policy::policy)?;
    encoder.sequence(
        &value.dangerous_capabilities,
        declarations::encode_dangerous_authority,
    )?;
    encoder.sequence(
        &value.slack_uses,
        declarations::encode_dangerous_authority_slack,
    )?;
    encoder.sequence(&value.semantic_dependencies, semantic_dependency)?;
    boundary::applications(encoder, &value.boundary_applications)
}

fn semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackagePolicySemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    match &dependency.consumer {
        PackagePolicySemanticDependencyConsumer::Callable(identity) => {
            encoder.byte(0);
            values::identity::encode_nominal(encoder, identity)?;
        }
        PackagePolicySemanticDependencyConsumer::PackageImplementation => encoder.byte(1),
    }
    values::identity::encode_nominal(encoder, &dependency.dependency)?;
    encoder.byte(declarations::semantic_dependency_kind_tag(dependency.kind));
    encoder.byte(match dependency.exposure {
        PackageReviewSemanticDependencyExposure::PrivateImplementation => 0,
        PackageReviewSemanticDependencyExposure::PublicInterface => 1,
    });
    Ok(())
}

// Inert baseline recovery under one shared reader and allocation budget.
// `recover_canonical` restores comparison meaning without old source, proof, or native replay.

use crate::encoding::recovery::policy::identity::service;
use crate::record::PackagePolicyBaseline;
pub(in crate::encoding::recovery) mod boundary;
pub(in crate::encoding::recovery) mod dependencies;
pub(in crate::encoding::recovery) mod restricted_build;

use super::{
    Error, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage, callable_policy, external,
    identity::package, public_api, reader::Reader, representation, selected_providers,
    terminal_permissions,
};
use crate::encoding::{
    BASELINE_WITHOUT_STRUCTURAL_PARAMETERS, PACKAGE_POLICY_BASELINE_MAGIC,
    PACKAGE_POLICY_BASELINE_VERSION,
};
use crate::record::PackagePolicyNativeBinding;

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
            native_bindings: reader.sequence(1, native_binding)?,
            external_supplies: reader.sequence(1, external::policy)?,
            dangerous_capabilities: reader.sequence(1, dependencies::dangerous_authority)?,
            slack_uses: reader.sequence(1, dependencies::slack)?,
            semantic_dependencies: reader.sequence(1, |reader| {
                dependencies::semantic_dependency(reader, schema_version)
            })?,
            boundary_applications: boundary::applications(&mut reader)?,
            restricted_build_requests: restricted_build::requests(&mut reader)?,
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

fn native_binding(reader: &mut Reader<'_>) -> Result<PackagePolicyNativeBinding, Error> {
    Ok(PackagePolicyNativeBinding {
        application: reader.option(|reader| reader.string())?,
        method: selected_providers::service_method(reader)?,
        binding: selected_providers::bindings::binding(reader)?,
    })
}

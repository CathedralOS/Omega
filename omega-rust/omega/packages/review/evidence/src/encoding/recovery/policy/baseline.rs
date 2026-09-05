//! Inert baseline recovery under one shared reader and allocation budget.

mod boundary;
mod dependencies;
#[cfg(test)]
mod external_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod text_tests;

use super::{
    Error, PackagePolicyRecoveryLimits, callable_policy, external, identity::package, public_api,
    reader::Reader, representation, selected_providers, terminal_permissions,
};
use crate::encoding::{PACKAGE_POLICY_BASELINE_MAGIC, PACKAGE_POLICY_BASELINE_VERSION};
use crate::record::*;

impl PackagePolicyBaseline {
    /// Recover comparison meaning without old source, proof, or native replay.
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(PACKAGE_POLICY_BASELINE_MAGIC)?;
        if reader.u16()? != PACKAGE_POLICY_BASELINE_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let policy = Self {
            package: package(&mut reader)?,
            target: selected_providers::target(&mut reader)?,
            public_api: public_api::public_api(&mut reader)?,
            callables: callable_policy::policy(&mut reader)?,
            selected_providers: selected_providers::policy(&mut reader)?,
            terminal_permissions: terminal_permissions::policy(&mut reader)?,
            representation: representation::policy(&mut reader)?,
            external_supplies: reader.sequence(1, external::policy)?,
            dangerous_capabilities: reader.sequence(1, dependencies::dangerous_authority)?,
            slack_uses: reader.sequence(1, dependencies::slack)?,
            semantic_dependencies: reader.sequence(1, dependencies::semantic_dependency)?,
            boundary_applications: boundary::applications(&mut reader)?,
        };
        reader.finish()?;
        policy
            .validate_canonical_structure()
            .map_err(|_| Error::InvalidValue)?;
        reader.canonical_scratch(bytes.len())?;
        if policy
            .canonical_bytes()
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(policy)
    }
}

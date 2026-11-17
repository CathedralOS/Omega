/*
Runtime permission snapshots retain source association and public Build metadata.
At consumption the same source must have an accepted Product judgment, and its
complete supported policy inventory supplies the consent comparison rows.
*/

use diagnostics::Diagnostic;
use products::CheckedProductSource;
use std::sync::Arc;
use typed_trees::ProductSourceIdentity;

pub struct PreparedRuntimePolicy {
    owner: Result<ProductSourceIdentity, Diagnostic>,
    sources: Arc<package_compilation::CheckedSourceCustody>,
}

impl PreparedRuntimePolicy {
    pub fn capture(source: &CheckedProductSource) -> Self {
        Self {
            owner: source
                .sources
                .require_owner(&source.program)
                .map(|()| ProductSourceIdentity::new(&source.program)),
            sources: Arc::clone(&source.sources),
        }
    }

    pub(crate) fn rows(
        &self,
        input: &crate::permission_acceptance::RuntimePermissionInput<'_>,
        target: target::TargetProfile,
        package: semantic_vocabulary::PackageKeyIdentity,
    ) -> Result<Vec<crate::record::PackagePolicyRow>, Vec<Diagnostic>> {
        let owner = self.owner.as_ref().map_err(|error| vec![error.clone()])?;
        let checked = input.compilation;
        if !owner.same_source(checked.sources().source_identity())
            || self.sources.as_ref() != checked.sources()
        {
            return Err(vec![Diagnostic::error(
                "saved permission declarations belong to another typed source owner",
            )]);
        }
        if checked.sources().package_identity() != Some(package)
            || checked.execution().target_profile() != Some(target)
            || checked.execution().native_target() != Some(target.native_target())
        {
            return Err(vec![Diagnostic::error(
                "policy package or target differs from the exact root activation",
            )]);
        }
        let policy = super::project_checked_package_policy(checked, target, package)?;
        let (rows, _) = policy
            .acceptance_rows_with_limits(Default::default())
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "cannot encode the accepted runtime permission policy: {error:?}"
                ))]
            })?;
        Ok(rows)
    }
}

/*
Runtime permission snapshots retain source association and public Build metadata.
The retired checker's contract plans cannot supply route-owned permission proof.
Until the accepted route publishes that custody, a request for policy rows refuses
rather than emitting an empty permission baseline or borrowing old checked facts.
*/

use compilation_products::CheckedProductSource;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::ProductSourceIdentity;

pub struct PreparedRuntimePolicy {
    owner: Result<ProductSourceIdentity, Diagnostic>,
    sources: Arc<package_compilation::CheckedSourceCustody>,
    evaluated_assumptions: usize,
}

impl PreparedRuntimePolicy {
    pub fn capture(source: &CheckedProductSource) -> Self {
        Self {
            owner: source
                .sources
                .require_owner(&source.program)
                .map(|()| ProductSourceIdentity::new(&source.program)),
            sources: Arc::clone(&source.sources),
            evaluated_assumptions: source.computed_build_config.config.assumed_bindings.len(),
        }
    }

    pub(crate) fn evaluated_assumptions(&self) -> usize {
        self.evaluated_assumptions
    }

    pub(crate) fn rows(
        &self,
        input: &crate::permission_acceptance::RuntimePermissionInput<'_>,
        target: target::TargetProfile,
        package: semantic_vocabulary::PackageKeyIdentity,
    ) -> Result<Vec<crate::record::PackagePolicyRow>, Vec<Diagnostic>> {
        let owner = self.owner.as_ref().map_err(|error| vec![error.clone()])?;
        if !owner.same_source(input.sources.source_identity())
            || self.sources.as_ref() != input.sources
        {
            return Err(vec![Diagnostic::error(
                "saved permission declarations belong to another typed source owner",
            )]);
        }
        if input.sources.package_identity() != Some(package)
            || input.execution.target_profile() != Some(target)
            || input.execution.native_target() != Some(target.native_target())
        {
            return Err(vec![Diagnostic::error(
                "policy package or target differs from the exact root activation",
            )]);
        }
        Err(vec![Diagnostic::error(
            "no product route yet: runtime permission and assumed-binding policy require route-owned contract custody",
        )])
    }
}

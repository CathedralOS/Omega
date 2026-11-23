/*
    Parked from `source-rust/subsystems/package-manager/review/restricted_build_grants.rs`
    on 2026-11-23. Its only caller is this directory's `accepted_compilation.rs`
    (`RestrictedBuildCheckpoint::derive`); restore both together. While parked,
    a Build that consumes an accepted lock receives no restricted grants, and
    `BuildExecutionInputs` with no grants reports restricted requests without
    executing them.

    The lock stores decisions rather than credentials, so
    `RestrictedBuildCheckpoint` and `CheckpointRestrictedBuildGrants` re-project
    each occurrence's acceptance rows under its exact package identity and
    `PackageCheckedContext`, comparing complete normalized request meaning
    before the request's build effect executes or its generated sources are
    consumed. Restoring them also restores
    `UngrantedRestrictedBuildRequest::request_meaning`:

        pub(crate) fn request_meaning(&self) -> &str {
            &self.request_meaning
        }
*/

use std::collections::BTreeSet;

use diagnostics::Diagnostic;
use review_evidence::record::PackagePolicyRowKind;
use semantic_vocabulary::PackageKeyIdentity;

use crate::lock::{PackageCheckedContext, PackageLockTarget};
use crate::resolution::graph::DependencyRequestPath;
use crate::review::UngrantedRestrictedBuildRequest;
use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::RestrictedBuildRequest;

#[derive(Debug, Clone)]
pub(crate) struct RestrictedBuildCheckpoint {
    grants: Vec<(PackageKeyIdentity, PackageCheckedContext, BTreeSet<String>)>,
}

impl RestrictedBuildCheckpoint {
    pub fn derive(accepted: &PackageLockTarget) -> Self {
        Self {
            grants: accepted
                .occurrences()
                .iter()
                .map(|occurrence| {
                    (
                        occurrence.acceptance().package(),
                        occurrence.context(),
                        occurrence
                            .acceptance()
                            .rows()
                            .iter()
                            .filter(|row| {
                                row.kind() == PackagePolicyRowKind::RestrictedBuildRequest
                            })
                            .map(|row| row.canonical_text().to_owned())
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    pub fn grants(
        &self,
        package: PackageKeyIdentity,
        context: PackageCheckedContext,
        request_path: Option<DependencyRequestPath>,
    ) -> CheckpointRestrictedBuildGrants {
        CheckpointRestrictedBuildGrants {
            package,
            context,
            request_path,
            granted: self
                .grants
                .iter()
                .find(|(identity, granted_context, _)| {
                    *identity == package && *granted_context == context
                })
                .map(|(_, _, texts)| texts.clone())
                .unwrap_or_default(),
        }
    }
}

pub(crate) struct CheckpointRestrictedBuildGrants {
    package: PackageKeyIdentity,
    context: PackageCheckedContext,
    request_path: Option<DependencyRequestPath>,
    granted: BTreeSet<String>,
}

impl RestrictedBuildGrants for CheckpointRestrictedBuildGrants {
    fn admit(&mut self, request: &RestrictedBuildRequest) -> Result<(), Vec<Diagnostic>> {
        let pending = UngrantedRestrictedBuildRequest::project(
            self.package,
            self.context,
            self.request_path.clone(),
            request,
        )
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
        if self.granted.contains(pending.request_meaning()) {
            return Ok(());
        }
        Err(vec![Diagnostic::error(pending.to_string())])
    }
}

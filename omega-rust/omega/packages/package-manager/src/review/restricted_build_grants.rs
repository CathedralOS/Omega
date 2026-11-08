/*
    We perform the execution-time grant join between a fresh compile and the
    consuming project's accepted lock policy. Review compiles are observational:
    private snapshot/staging builds run without restricted authority, while a
    non-exempt request stops before execution and remains available for a
    decision. A lock that retains decisions is different: an operation consuming
    that compile refuses any occurrence whose projected restricted build
    requests lack retained accepted-request meaning.

    Because the lock stores decisions rather than credentials,
    `RestrictedBuildCheckpoint` and `CheckpointRestrictedBuildGrants` re-project
    each occurrence's acceptance rows under its exact package identity and
    `PackageCheckedContext`, comparing complete normalized request meaning
    before the request's build effect executes or its generated sources are
    consumed.
*/

use std::collections::BTreeSet;

use diagnostics::Diagnostic;
use package_evidence::record::PackagePolicyRowKind;
use semantic_vocabulary::PackageKeyIdentity;

use crate::Hex;
use crate::lock::{PackageCheckedContext, PackageLockTarget};
use crate::resolution::graph::DependencyRequestPath;
use package_compilation::build_execution::RestrictedBuildGrants;
use package_compilation::build_execution::RestrictedBuildRequest;
use package_evidence::encoding::restricted_build_request_acceptance_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UngrantedRestrictedBuildRequest {
    package: PackageKeyIdentity,
    context: PackageCheckedContext,
    request_meaning: String,
    request_path: Option<DependencyRequestPath>,
}

impl UngrantedRestrictedBuildRequest {
    pub(crate) fn project(
        package: PackageKeyIdentity,
        context: PackageCheckedContext,
        request_path: Option<DependencyRequestPath>,
        request: &RestrictedBuildRequest,
    ) -> Result<Self, package_evidence::encoding::PackageReviewEncodingError> {
        let request = request.clone().for_target(Some(context.target()));
        let request_meaning =
            restricted_build_request_acceptance_text(package, context.target(), &request)?;
        Ok(Self {
            package,
            context,
            request_meaning,
            request_path,
        })
    }

    pub(crate) fn request_meaning(&self) -> &str {
        &self.request_meaning
    }
}

impl std::fmt::Display for UngrantedRestrictedBuildRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let identity = Hex(&self.package.digest()[..4]).to_string();
        writeln!(
            formatter,
            "package {identity}… {} {} occurrence requests:\n    {}",
            self.context.purpose().name(),
            self.context.target().identity().as_str(),
            self.request_meaning
        )?;
        if let Some(profile) = self.context.build_execution_profile() {
            writeln!(formatter, "    build host {}", profile.identity().as_str())?;
        }
        match &self.request_path {
            Some(path) => {
                write!(formatter, "    path ")?;
                write!(formatter, "{}", Hex(&path.root().identity().digest()))?;
                for step in path.steps() {
                    write!(formatter, " -> {:?} ", step.alias().as_str())?;
                    write!(formatter, "{}", Hex(&step.target().identity().digest()))?;
                }
                Ok(())
            }
            None => write!(formatter, "    path none"),
        }
    }
}

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

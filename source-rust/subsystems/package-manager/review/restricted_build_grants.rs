/*
    We describe a restricted build request that the consuming project has not
    granted. Review compiles are observational: private snapshot/staging builds
    run without restricted authority, and a non-exempt request stops before
    execution and remains available for a decision. The command reports each
    stopped request with its package, checked context, normalized request
    meaning and dependency path.

    The lock-checkpoint grants that re-admitted accepted requests for an
    operation consuming a locked compile are parked with their caller in
    `_recycling/source-rust/subsystems/package-manager/v1-project-admission/`.
*/

use semantic_vocabulary::PackageKeyIdentity;

use crate::Hex;
use crate::lock::PackageCheckedContext;
use crate::resolution::graph::DependencyRequestPath;
use package_compilation::build_execution::RestrictedBuildRequest;
use review_evidence::encoding::restricted_build_request_acceptance_text;

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
    ) -> Result<Self, review_evidence::encoding::PackageReviewEncodingError> {
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

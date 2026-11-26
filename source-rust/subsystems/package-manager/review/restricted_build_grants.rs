/*
    We describe a restricted build request that the consuming project has not
    granted. Review compiles are observational: private snapshot/staging builds
    run without restricted authority, and a non-exempt request stops before
    execution and remains available for a decision. The command reports each
    stopped request with its package, checked context, normalized request
    meaning and dependency path.

    Root execution consumes the matching reviewed section for every selected
    deployment after immutable source acquisition and compiler identity checks.
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

/// A shared root Build executes once for its requested deployments. Every
/// participating deployment must have accepted the complete normalized row
/// for this exact occurrence, source and evaluator.
pub(crate) struct RootRestrictedBuildGrants {
    pub(crate) name: Option<String>,
    pub(crate) phase: Option<crate::transaction::restricted_build::BuildPhaseGrants>,
    deployments: Vec<(
        PackageKeyIdentity,
        PackageCheckedContext,
        std::collections::BTreeSet<String>,
    )>,
}

impl RootRestrictedBuildGrants {
    pub(crate) fn for_phase(phase: crate::transaction::restricted_build::BuildPhaseGrants) -> Self {
        Self {
            name: None,
            phase: Some(phase),
            deployments: Vec::new(),
        }
    }

    pub(crate) fn for_source(
        sections: &[&crate::lock::PackageLockTarget],
        acquired: &crate::resolution::source::AcquiredBuildSource,
        host: target::TargetProfile,
    ) -> Option<Self> {
        use review_evidence::record::PackagePolicyRowKind;
        let root_key = sections.first()?.source().root().selected().key();
        let origin = acquired.acquisition_origin();
        let mut deployments = Vec::new();
        for section in sections {
            let source = section.source();
            let root = source.root().selected();
            if root.key() != root_key
                || root.key().source_lineage() != origin.lineage
                || root.resolution() != origin.resolution
                || source.package_navigation(root.key()) != Some(origin.navigation)
            {
                return None;
            }
            let package = root.key().identity();
            let context = PackageCheckedContext::new(
                build_declarations::DependencyPurpose::Product,
                section.target(),
                Some(host),
            );
            let occurrence = section
                .occurrences()
                .iter()
                .find(|row| row.acceptance().package() == package && row.context() == context)?;
            let rows = occurrence
                .acceptance()
                .rows()
                .iter()
                .filter(|row| row.kind() == PackagePolicyRowKind::RestrictedBuildRequest)
                .map(|row| row.canonical_text().to_owned())
                .collect();
            deployments.push((package, context, rows));
        }
        Some(Self {
            name: Some(root_key.name().as_str().to_owned()),
            phase: None,
            deployments,
        })
    }
}

impl package_compilation::build_execution::RestrictedBuildGrants for RootRestrictedBuildGrants {
    fn admits(
        &mut self,
        site: &package_compilation::build_execution::RestrictedBuildSite,
        request: &RestrictedBuildRequest,
    ) -> Result<bool, Vec<diagnostics::Diagnostic>> {
        if let Some(phase) = self.phase.as_mut()
            && phase.admits(site, request)?
        {
            return Ok(true);
        }
        for (package, context, accepted) in &self.deployments {
            let requested =
                UngrantedRestrictedBuildRequest::project(*package, *context, None, request)
                    .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])?;
            if !accepted.contains(&requested.request_meaning) {
                return Ok(false);
            }
        }
        Ok(!self.deployments.is_empty())
    }
}

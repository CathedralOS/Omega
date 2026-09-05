//! Reacquire recorded source content without resolving mutable selectors.

mod acquisition;
mod comparison;
mod errors;
mod paths;

pub use errors::ResolveLockedPackageClosureError;

use super::super::reconcile::resolve_package_source_closure_with_indexed_limits;
use super::super::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, PackageRootSourceRequest,
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
};
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::resolution::source::{PackageSourceCustody, ResolvePackageSourceError};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{
    ExternalSourceContext, GitSourceRequest, LocalSourceLimits, SourceResolverStorage,
    WorkspaceLineageIdentity,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Rebuild a recorded graph using newly issued source custody. This operation
/// does not compile, compare policy, approve admissions, or modify the subject.
/// The caller selects the exact lock target before invoking this source owner.
/// Local roots must still exist: no cache-only local custody issuer is implied.
#[allow(clippy::too_many_arguments)]
pub fn resolve_locked_package_source_closure_with_storage(
    subject: &CanonicalSourceClosureSubject,
    root_request: &PackageRootSourceRequest,
    acquisition: GitExactRevisionAcquisition,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    subject_limits: CanonicalSourceClosureSubjectLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveLockedPackageClosureError> {
    resolve(
        subject,
        root_request,
        acquisition,
        storage,
        source_limits,
        closure_limits,
        subject_limits,
        RootPolicy::Strict,
    )
}

/// Prepare accepted dependencies for an editable local APPLICATION or PACKAGE.
/// The caller's live root must retain its canonical package key, role,
/// navigation and complete dependency projection. Only its source resolution
/// may change. Every dependency retains strict locked custody.
///
/// Historical local request spellings (for example `.`) are inert metadata:
/// this entrance checks the physical caller through fresh canonical lineage,
/// never by decoding recorded path bytes. The strict entrance still requires
/// the original exact request spelling as well as immutable root content.
#[allow(clippy::too_many_arguments)]
pub fn resolve_locked_local_project_closure_with_storage(
    subject: &CanonicalSourceClosureSubject,
    root_request: &PackageRootSourceRequest,
    acquisition: GitExactRevisionAcquisition,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    subject_limits: CanonicalSourceClosureSubjectLimits,
) -> Result<ResolvedPackageSourceClosure, ResolveLockedPackageClosureError> {
    resolve(
        subject,
        root_request,
        acquisition,
        storage,
        source_limits,
        closure_limits,
        subject_limits,
        RootPolicy::MutableLocal,
    )
}

#[derive(Clone, Copy)]
enum RootPolicy {
    Strict,
    MutableLocal,
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    subject: &CanonicalSourceClosureSubject,
    root_request: &PackageRootSourceRequest,
    acquisition: GitExactRevisionAcquisition,
    storage: &SourceResolverStorage,
    source_limits: LocalSourceLimits,
    closure_limits: PackageSourceClosureLimits,
    subject_limits: CanonicalSourceClosureSubjectLimits,
    root_policy: RootPolicy,
) -> Result<ResolvedPackageSourceClosure, ResolveLockedPackageClosureError> {
    let request_matches = match root_policy {
        RootPolicy::Strict => {
            comparison::root_request_matches(subject.root().request(), root_request)
        }
        RootPolicy::MutableLocal => {
            comparison::local_root_context_matches(subject.root().request(), root_request)
        }
    };
    if !request_matches {
        return Err(ResolveLockedPackageClosureError::RootRequestMismatch);
    }
    // Reject known graph/record size ceilings before opening source. The final
    // subject owner also checks every identity/request field against its limits.
    comparison::limits(subject, closure_limits, subject_limits)?;
    storage
        .verify_path_identity()
        .map_err(ResolvePackageSourceError::from)?;
    let mut resolver = Resolver {
        subject,
        root_request,
        root_policy,
        acquisition,
        storage,
        source_limits: source_limits.compiler_bounded(),
        local_roots: BTreeMap::new(),
        git_requests: BTreeMap::new(),
        workspaces: BTreeMap::new(),
        local_workspace_root: None,
        external_context: match root_request {
            PackageRootSourceRequest::ExternalLocal { source_context, .. } => {
                Some(source_context.clone())
            }
            _ => None,
        },
    };
    let result = (|| {
        let root = resolver.root()?;
        comparison::requester_custody(subject, subject.root().selected(), &root, root_policy)?;
        if root.role() != subject.root_role() {
            return Err(ResolveLockedPackageClosureError::mismatch(
                root.key(),
                "root declaration role differs",
            ));
        }
        let closure = resolve_package_source_closure_with_indexed_limits(
            root_request.clone(),
            root,
            closure_limits,
            |requester, ordinal, request| resolver.dependency(requester, ordinal, request),
        )
        .map_err(|error| ResolveLockedPackageClosureError::Closure(Box::new(error)))?;
        let matches = match root_policy {
            RootPolicy::Strict => subject.matches_resolved(
                &closure.for_exact_target(subject.target_profile()),
                subject_limits,
            )?,
            RootPolicy::MutableLocal => {
                comparison::mutable_local_graph_matches(subject, &closure, subject_limits)?
            }
        };
        if !matches {
            return Err(ResolveLockedPackageClosureError::mismatch(
                subject.root().selected().key(),
                "recovered source graph differs",
            ));
        }
        Ok(closure)
    })();
    storage
        .verify_path_identity()
        .map_err(ResolvePackageSourceError::from)?;
    result
}

struct Resolver<'a> {
    subject: &'a CanonicalSourceClosureSubject,
    root_request: &'a PackageRootSourceRequest,
    root_policy: RootPolicy,
    acquisition: GitExactRevisionAcquisition,
    storage: &'a SourceResolverStorage,
    source_limits: LocalSourceLimits,
    local_roots: BTreeMap<PackageKey, PathBuf>,
    git_requests: BTreeMap<PackageKey, GitSourceRequest>,
    workspaces: BTreeMap<WorkspaceLineageIdentity, super::dependencies::WorkspaceContext>,
    external_context: Option<ExternalSourceContext>,
    local_workspace_root: Option<PathBuf>,
}

impl Resolver<'_> {
    fn dependency(
        &mut self,
        requester: &PackageSourceCustody,
        ordinal: usize,
        request: &DependencySourceRequest,
    ) -> Result<PackageSourceCustody, ResolveLockedPackageClosureError> {
        let edge = comparison::edge(self.subject, requester, ordinal, request, self.root_policy)?;
        let selected = match request {
            DependencySourceRequest::Git {
                repository,
                revision,
                selection,
                ..
            } => {
                let acquisition = GitSourceRequest::new(repository.clone(), Some(revision.clone()))
                    .map_err(super::ResolveDependencySourceError::from)?;
                let request = crate::resolution::source::GitPackageSourceRequest::new(
                    acquisition,
                    selection.clone(),
                );
                self.git(&request, edge.selected(), false)?
            }
            DependencySourceRequest::Path { location, .. } => {
                self.path(requester, location, edge.selected())?
            }
        };
        comparison::custody(self.subject, edge.selected(), &selected)?;
        Ok(selected)
    }
}

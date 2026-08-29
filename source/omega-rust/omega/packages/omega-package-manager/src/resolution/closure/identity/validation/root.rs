use super::super::{
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectLimits,
};
use super::source::{validate_git_selection, validate_request_bytes, validate_source_lineage};
use crate::resolution::closure::reconciliation::PackageRootSourceRequest;
use crate::resolution::source::PackageSourceNavigation;
use omega_package_source::{
    GitSourceRequest, ImmutableSourceResolution, SourceLineage, WorkspaceLineageIdentity,
};

pub(in super::super) fn canonical_root_request(
    request: &PackageRootSourceRequest,
) -> CanonicalRootSourceRequest {
    match request {
        PackageRootSourceRequest::Git(request) => CanonicalRootSourceRequest::Git {
            requested_locator: request.acquisition().requested_locator().to_owned(),
            requested_revision: request.acquisition().requested_revision().to_owned(),
            selection: request.selection().clone(),
        },
        PackageRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source: workspace_root_source.clone(),
            member_path: member_path.clone(),
            requested_workspace_root: requested_workspace_root
                .as_os_str()
                .as_encoded_bytes()
                .to_vec(),
        },
        PackageRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: requested_root.as_os_str().as_encoded_bytes().to_vec(),
            source_context: source_context.clone(),
        },
    }
}

pub(super) fn validate_root_request(
    root: &CanonicalRootSourceSelection,
    navigation: &PackageSourceNavigation,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &root.request {
        CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
            selection,
        } => {
            validate_request_bytes(requested_locator.as_bytes(), limits.maximum_request_bytes)?;
            validate_request_bytes(requested_revision.as_bytes(), limits.maximum_request_bytes)?;
            let request =
                GitSourceRequest::new(requested_locator.clone(), Some(requested_revision.clone()))
                    .map_err(|_| {
                        CanonicalSourceClosureSubjectError::new("invalid root Git request")
                    })?;
            if request.lineage() != root.selected.key().source_lineage()
                || !matches!(
                    root.selected.resolution(),
                    ImmutableSourceResolution::Git { .. }
                )
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "root Git request disagrees with its selected source",
                ));
            }
            validate_git_selection(selection, &root.selected, navigation)?;
        }
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => {
            validate_source_lineage(workspace_root_source, limits.maximum_identity_bytes)?;
            validate_request_bytes(
                member_path.as_str().as_bytes(),
                limits.maximum_request_bytes,
            )?;
            validate_request_bytes(requested_workspace_root, limits.maximum_request_bytes)?;
            let identity = WorkspaceLineageIdentity::from_root_source(workspace_root_source)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid workspace root source in root request",
                    )
                })?;
            if !matches!(
                root.selected.key().source_lineage(),
                SourceLineage::Workspace(lineage)
                    if lineage.workspace_identity() == &identity
                        && lineage.member_path() == member_path
            ) || !matches!(
                root.selected.resolution(),
                ImmutableSourceResolution::Workspace { .. }
            ) {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "workspace root request disagrees with its selected source",
                ));
            }
            if navigation != &PackageSourceNavigation::Root {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "workspace root request has non-root package navigation",
                ));
            }
        }
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => {
            validate_request_bytes(requested_root, limits.maximum_request_bytes)?;
            if !matches!(
                root.selected.key().source_lineage(),
                SourceLineage::ExternalLocal(lineage)
                    if lineage.source_context() == source_context
            ) || !matches!(
                root.selected.resolution(),
                ImmutableSourceResolution::ExternalLocal { .. }
            ) {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "external-local root request disagrees with its selected source",
                ));
            }
            if navigation != &PackageSourceNavigation::Root {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "external-local root request has non-root package navigation",
                ));
            }
        }
    }
    Ok(())
}

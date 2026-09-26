use super::super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalSourceClosureSubjectError, request_view::Request, usage::Budget,
};
use super::source::{validate_git_selection, validate_request_bytes};
use crate::declarations::AliasName;
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;
use package_source::{ImmutableSourceResolution, SourceLineage};

pub(super) fn validate_dependency_selection_kind(
    selection: &CanonicalDependencySourceSelection,
    requester: &ResolvedSourceIdentity,
    _requester_navigation: &PackageSourceNavigation,
    selected_navigation: &PackageSourceNavigation,
    budget: &mut Budget,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &selection.request {
        CanonicalDependencySourceRequest::Path { .. } => {
            match selection.selected.key().source_lineage() {
                SourceLineage::Workspace(_) | SourceLineage::ExternalLocal(_) => {
                    if selected_navigation != &PackageSourceNavigation::Root {
                        return Err(CanonicalSourceClosureSubjectError::new(
                            "path request selected invalid package navigation",
                        ));
                    }
                }
                SourceLineage::GitHub(_) | SourceLineage::GitLab(_) | SourceLineage::Git(_) => {
                    if requester.key().source_lineage() != selection.selected.key().source_lineage()
                        || requester.resolution() != selection.selected.resolution()
                        || !matches!(selected_navigation, PackageSourceNavigation::Member(_))
                    {
                        return Err(CanonicalSourceClosureSubjectError::new(
                            "Git member path request escaped its repository resolution",
                        ));
                    }
                }
            }
        }
        CanonicalDependencySourceRequest::Git {
            repository,
            revision,
            selection: package_selection,
            ..
        } => {
            let request = super::source::git_request(repository, revision, budget)?;
            if request.lineage() != selection.selected.key().source_lineage()
                || !matches!(
                    selection.selected.resolution(),
                    ImmutableSourceResolution::Git { .. }
                )
            {
                return Err(CanonicalSourceClosureSubjectError::new(
                    "dependency Git request disagrees with its selected source",
                ));
            }
            validate_git_selection(package_selection, &selection.selected, selected_navigation)?;
        }
    }
    Ok(())
}

pub(super) fn validate_dependency_request(
    request: Request<'_>,
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match request {
        Request::Path {
            explicit_alias,
            location,
        } => {
            validate_optional_alias(explicit_alias)?;
            validate_request_bytes(location.as_bytes(), maximum_request_bytes)
        }
        Request::Git {
            explicit_alias,
            repository,
            revision,
            selection: _,
        } => {
            validate_optional_alias(explicit_alias)?;
            validate_request_bytes(repository.as_bytes(), maximum_request_bytes)?;
            validate_request_bytes(revision.as_bytes(), maximum_request_bytes)
        }
    }
}

fn validate_optional_alias(
    alias: Option<&AliasName>,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    if alias
        .as_ref()
        .is_some_and(|alias| alias.as_str().is_empty())
    {
        Err(CanonicalSourceClosureSubjectError::new(
            "dependency request contains an empty explicit alias",
        ))
    } else {
        Ok(())
    }
}

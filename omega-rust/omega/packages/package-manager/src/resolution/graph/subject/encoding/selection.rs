//! Canonical root and dependency source-selection encoding.

use super::super::SOURCE_CLOSURE_SUBJECT_MAGIC;
use super::super::{
    CanonicalDependencySourceSelection, CanonicalRootSourceRequest, CanonicalRootSourceSelection,
    CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectLimits,
    SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
use super::super::{request_view::Request, usage::Budget};
use super::framing::Encoder;
use super::source::{encode_package_key, encode_source_identity, encode_source_lineage};
use crate::declarations::AliasName;
use crate::declarations::dependencies::read::{
    DependencyProjections, DependencyPurpose, PackageSelection, ProjectedDependencies,
};
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;
use package_compilation::review_record_encoding::{declaration_kind_tag, purpose_tag};
use target::TargetProfile;

pub(in super::super) fn encode_subject(
    target_profile: TargetProfile,
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    package_navigations: &[PackageSourceNavigation],
    package_dependency_projections: &[DependencyProjections],
    dependency_requests: &[CanonicalDependencySourceSelection],
    catalog_dependencies: &[super::super::CanonicalCatalogDependency],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<Vec<u8>, CanonicalSourceClosureSubjectError> {
    let mut encoder = Encoder::new();
    encode_subject_to(
        &mut encoder,
        target_profile,
        root,
        packages,
        package_navigations,
        package_dependency_projections,
        dependency_requests,
        catalog_dependencies,
        limits,
    )?;
    encoder.check()?;
    Ok(encoder.finish())
}

pub(in super::super) fn encode_subject_with_budget(
    target_profile: TargetProfile,
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    package_navigations: &[PackageSourceNavigation],
    package_dependency_projections: &[DependencyProjections],
    dependency_requests: &[CanonicalDependencySourceSelection],
    catalog_dependencies: &[super::super::CanonicalCatalogDependency],
    limits: CanonicalSourceClosureSubjectLimits,
    budget: &mut Budget,
) -> Result<Vec<u8>, CanonicalSourceClosureSubjectError> {
    let mut encoder = Encoder::budgeted(budget, limits.maximum_record_bytes);
    encode_subject_to(
        &mut encoder,
        target_profile,
        root,
        packages,
        package_navigations,
        package_dependency_projections,
        dependency_requests,
        catalog_dependencies,
        limits,
    )?;
    encoder.check()?;
    Ok(encoder.finish())
}

fn encode_subject_to(
    encoder: &mut Encoder,
    target_profile: TargetProfile,
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    package_navigations: &[PackageSourceNavigation],
    package_dependency_projections: &[DependencyProjections],
    dependency_requests: &[CanonicalDependencySourceSelection],
    catalog_dependencies: &[super::super::CanonicalCatalogDependency],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encoder.fixed(SOURCE_CLOSURE_SUBJECT_MAGIC);
    // Existing subjects retain their exact v7 identity and decision bindings.
    // Build-only catalog subjects retain v8. Queries add Product-purpose rows
    // whose explicit purpose is part of v9 identity, never a new import grant.
    let purpose_tagged_catalog = catalog_dependencies
        .iter()
        .any(|dependency| dependency.purpose != DependencyPurpose::Build);
    encoder.u16(if catalog_dependencies.is_empty() {
        7
    } else if purpose_tagged_catalog {
        SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION
    } else {
        8
    });
    encode_target_profile(encoder, target_profile, limits)?;
    encode_root_selection(encoder, root, limits)?;
    encoder.count(packages.len())?;
    for ((source, navigation), projection) in packages
        .iter()
        .zip(package_navigations)
        .zip(package_dependency_projections)
    {
        encode_source_identity(encoder, source, limits.maximum_identity_bytes)?;
        encode_package_navigation(encoder, navigation, limits.maximum_request_bytes)?;
        encode_dependency_projection(encoder, projection, limits)?;
    }
    encoder.count(dependency_requests.len())?;
    for request in dependency_requests {
        encode_dependency_selection(encoder, request, limits)?;
    }
    if !catalog_dependencies.is_empty() {
        encoder.count(catalog_dependencies.len())?;
        for dependency in catalog_dependencies {
            encode_source_identity(encoder, &dependency.observer, limits.maximum_identity_bytes)?;
            encode_package_navigation(
                encoder,
                &dependency.navigation,
                limits.maximum_request_bytes,
            )?;
            encoder.count(dependency.ordinal)?;
            if purpose_tagged_catalog {
                encoder.byte(match dependency.purpose {
                    DependencyPurpose::Product => 0,
                    DependencyPurpose::Build => 1,
                });
            }
            encode_dependency_request(encoder, Request::from(&dependency.request), limits)?;
            encoder.bytes_bounded(
                dependency.provider.canonical_bytes(),
                limits.maximum_record_bytes,
            )?;
        }
    }
    encoder.check()
}

fn encode_target_profile(
    encoder: &mut Encoder,
    profile: TargetProfile,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encoder.bytes_bounded(
        profile.identity().as_str().as_bytes(),
        limits.maximum_identity_bytes,
    )
}

fn encode_dependency_projection(
    encoder: &mut Encoder,
    dependencies: &DependencyProjections,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_dependency_scope(encoder, dependencies.product(), limits)?;
    encode_dependency_scope(encoder, dependencies.build(), limits)
}

fn encode_dependency_scope(
    encoder: &mut Encoder,
    dependencies: &ProjectedDependencies,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encoder.count(dependencies.authored_dependencies().len())?;
    for request in dependencies.authored_dependencies() {
        encode_dependency_request(encoder, Request::from(request), limits)?;
    }
    Ok(())
}

fn encode_root_selection(
    encoder: &mut Encoder,
    root: &CanonicalRootSourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match &root.request {
        CanonicalRootSourceRequest::Git {
            requested_locator,
            requested_revision,
            selection,
        } => {
            encoder.byte(0);
            encoder.bytes_bounded(requested_locator.as_bytes(), limits.maximum_request_bytes)?;
            encoder.bytes_bounded(requested_revision.as_bytes(), limits.maximum_request_bytes)?;
            encode_package_selection(encoder, selection, limits.maximum_identity_bytes)?;
        }
        CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source,
            member_path,
            requested_workspace_root,
        } => {
            encoder.byte(1);
            encode_source_lineage(
                encoder,
                workspace_root_source,
                limits.maximum_identity_bytes,
            )?;
            encoder.bytes_bounded(
                member_path.as_str().as_bytes(),
                limits.maximum_request_bytes,
            )?;
            encoder.bytes_bounded(requested_workspace_root, limits.maximum_request_bytes)?;
        }
        CanonicalRootSourceRequest::ExternalLocal {
            requested_root,
            source_context,
        } => {
            encoder.byte(2);
            encoder.bytes_bounded(requested_root, limits.maximum_request_bytes)?;
            encoder.fixed(source_context.digest());
        }
    }
    encoder.byte(declaration_kind_tag(root.role));
    encode_source_identity(encoder, &root.selected, limits.maximum_identity_bytes)
}

fn encode_dependency_selection(
    encoder: &mut Encoder,
    selection: &CanonicalDependencySourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, &selection.requester, limits.maximum_identity_bytes)?;
    encoder.byte(purpose_tag(selection.purpose));
    encoder.u32(u32::try_from(selection.dependency_index).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("dependency ordinal exceeds canonical range")
    })?);
    encode_dependency_request(encoder, Request::from(&selection.request), limits)?;
    encoder.bytes_bounded(
        selection.alias.as_str().as_bytes(),
        limits.maximum_identity_bytes,
    )?;
    encode_source_identity(encoder, &selection.selected, limits.maximum_identity_bytes)
}

pub(super) fn encode_dependency_request(
    encoder: &mut Encoder,
    request: Request<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match request {
        Request::Path {
            explicit_alias,
            location,
        } => {
            encoder.byte(0);
            encode_optional_alias(encoder, explicit_alias, limits.maximum_identity_bytes)?;
            encoder.bytes_bounded(location.as_bytes(), limits.maximum_request_bytes)?;
        }
        Request::Git {
            explicit_alias,
            repository,
            revision,
            selection,
        } => {
            encoder.byte(1);
            encode_optional_alias(encoder, explicit_alias, limits.maximum_identity_bytes)?;
            encoder.bytes_bounded(repository.as_bytes(), limits.maximum_request_bytes)?;
            encoder.bytes_bounded(revision.as_bytes(), limits.maximum_request_bytes)?;
            encode_package_selection(encoder, selection, limits.maximum_identity_bytes)?;
        }
    }
    Ok(())
}

fn encode_package_selection(
    encoder: &mut Encoder,
    selection: &PackageSelection,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match selection {
        PackageSelection::Root => encoder.byte(0),
        PackageSelection::Named(package) => {
            encoder.byte(1);
            encoder.bytes_bounded(package.as_str().as_bytes(), maximum_identity_bytes)?;
        }
    }
    Ok(())
}

fn encode_package_navigation(
    encoder: &mut Encoder,
    navigation: &PackageSourceNavigation,
    maximum_request_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match navigation {
        PackageSourceNavigation::Root => encoder.byte(0),
        PackageSourceNavigation::Member(path) => {
            encoder.byte(1);
            encoder.bytes_bounded(path.as_str().as_bytes(), maximum_request_bytes)?;
        }
    }
    Ok(())
}

fn encode_optional_alias(
    encoder: &mut Encoder,
    alias: Option<&AliasName>,
    maximum_identity_bytes: usize,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    match alias {
        None => encoder.byte(0),
        Some(alias) => {
            encoder.byte(1);
            encoder.bytes_bounded(alias.as_str().as_bytes(), maximum_identity_bytes)?;
        }
    }
    Ok(())
}

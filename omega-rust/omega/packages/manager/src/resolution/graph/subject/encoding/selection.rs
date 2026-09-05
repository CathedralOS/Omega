//! Canonical root and dependency source-selection encoding.

use super::super::model::SOURCE_CLOSURE_SUBJECT_MAGIC;
use super::super::{
    CanonicalDependencySourceRequest, CanonicalDependencySourceSelection,
    CanonicalRootSourceRequest, CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectLimits, SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION,
};
use super::super::{request_view::Request, usage::Budget};
use super::framing::{Decoder, Encoder, encode_hex};
use super::source::{
    decode_package_key, decode_source_identity, decode_source_lineage, encode_package_key,
    encode_source_identity, encode_source_lineage,
};
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::{
    DependencySourceRequest, PackageSelection, ProjectedDependencies,
};
use crate::declarations::{AliasName, PackageName};
use crate::resolution::graph::ResolvedSourceIdentity;
use crate::resolution::source::PackageSourceNavigation;
use omega_package_source::{ExternalSourceContext, SourceRelativePath};
use omega_target::TargetProfile;

pub(in super::super) fn encode_subject(
    target_profile: TargetProfile,
    root: &CanonicalRootSourceSelection,
    packages: &[ResolvedSourceIdentity],
    package_navigations: &[PackageSourceNavigation],
    package_dependency_projections: &[ProjectedDependencies],
    dependency_requests: &[CanonicalDependencySourceSelection],
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
    package_dependency_projections: &[ProjectedDependencies],
    dependency_requests: &[CanonicalDependencySourceSelection],
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
    package_dependency_projections: &[ProjectedDependencies],
    dependency_requests: &[CanonicalDependencySourceSelection],
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encoder.fixed(SOURCE_CLOSURE_SUBJECT_MAGIC);
    encoder.u16(SOURCE_CLOSURE_SUBJECT_ENCODING_VERSION);
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

pub(in super::super) fn decode_target_profile(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<TargetProfile, CanonicalSourceClosureSubjectError> {
    let identity = decoder.string(limits.maximum_identity_bytes)?;
    TargetProfile::ALL
        .into_iter()
        .find(|profile| profile.identity().as_str() == identity)
        .ok_or_else(|| CanonicalSourceClosureSubjectError::new("unknown target-profile identity"))
}

fn encode_dependency_projection(
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

pub(in super::super) fn decode_dependency_projection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<ProjectedDependencies, CanonicalSourceClosureSubjectError> {
    let request_count = decoder.count(limits.maximum_dependency_requests)?;
    let mut requests = Vec::with_capacity(request_count);
    for _ in 0..request_count {
        requests.push(dependency_request(decode_dependency_request(
            decoder, limits,
        )?));
    }
    Ok(ProjectedDependencies::from(requests))
}

fn dependency_request(request: CanonicalDependencySourceRequest) -> DependencySourceRequest {
    match request {
        CanonicalDependencySourceRequest::Path {
            explicit_alias,
            location,
        } => DependencySourceRequest::Path {
            explicit_alias,
            location,
        },
        CanonicalDependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
            selection,
        } => DependencySourceRequest::Git {
            explicit_alias,
            repository,
            revision,
            selection,
        },
    }
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
    encode_build_declaration_kind(encoder, root.role);
    encode_source_identity(encoder, &root.selected, limits.maximum_identity_bytes)
}

pub(in super::super) fn decode_root_selection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalRootSourceSelection, CanonicalSourceClosureSubjectError> {
    let request = match decoder.byte()? {
        0 => CanonicalRootSourceRequest::Git {
            requested_locator: decoder.string(limits.maximum_request_bytes)?,
            requested_revision: decoder.string(limits.maximum_request_bytes)?,
            selection: decode_package_selection(decoder, limits.maximum_identity_bytes)?,
        },
        1 => CanonicalRootSourceRequest::WorkspaceMember {
            workspace_root_source: decode_source_lineage(decoder, limits.maximum_identity_bytes)?,
            member_path: SourceRelativePath::parse(&decoder.string(limits.maximum_request_bytes)?)
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid workspace member path in root request",
                    )
                })?,
            requested_workspace_root: decoder.bytes(limits.maximum_request_bytes)?.to_vec(),
        },
        2 => CanonicalRootSourceRequest::ExternalLocal {
            requested_root: decoder.bytes(limits.maximum_request_bytes)?.to_vec(),
            source_context: ExternalSourceContext::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| {
                    CanonicalSourceClosureSubjectError::new(
                        "invalid external source context in root request",
                    )
                })?,
        },
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid root source-request tag",
            ));
        }
    };
    let role = decode_build_declaration_kind(decoder)?;
    let selected = decode_source_identity(decoder, limits.maximum_identity_bytes)?;
    Ok(CanonicalRootSourceSelection {
        request,
        role,
        selected,
    })
}

fn encode_build_declaration_kind(encoder: &mut Encoder, role: BuildDeclarationKind) {
    encoder.byte(match role {
        BuildDeclarationKind::Package => 0,
        BuildDeclarationKind::Application => 1,
        BuildDeclarationKind::Workspace => 2,
    });
}

fn decode_build_declaration_kind(
    decoder: &mut Decoder<'_>,
) -> Result<BuildDeclarationKind, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
        0 => Ok(BuildDeclarationKind::Package),
        1 => Ok(BuildDeclarationKind::Application),
        2 => Ok(BuildDeclarationKind::Workspace),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid root declaration-role tag",
        )),
    }
}

fn encode_dependency_selection(
    encoder: &mut Encoder,
    selection: &CanonicalDependencySourceSelection,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encode_package_key(encoder, &selection.requester, limits.maximum_identity_bytes)?;
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

pub(in super::super) fn decode_dependency_selection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalDependencySourceSelection, CanonicalSourceClosureSubjectError> {
    let requester = decode_package_key(decoder, limits.maximum_identity_bytes)?;
    let dependency_index = usize::try_from(decoder.u32()?).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("dependency ordinal exceeds platform range")
    })?;
    let request = decode_dependency_request(decoder, limits)?;
    let alias = AliasName::parse(decoder.string(limits.maximum_identity_bytes)?).map_err(|_| {
        CanonicalSourceClosureSubjectError::new("invalid resolved dependency alias")
    })?;
    let selected = decode_source_identity(decoder, limits.maximum_identity_bytes)?;
    Ok(CanonicalDependencySourceSelection {
        requester,
        dependency_index,
        request,
        alias,
        selected,
    })
}

pub(super) fn decode_dependency_request(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<CanonicalDependencySourceRequest, CanonicalSourceClosureSubjectError> {
    Ok(match decoder.byte()? {
        0 => CanonicalDependencySourceRequest::Path {
            explicit_alias: decode_optional_alias(decoder, limits.maximum_identity_bytes)?,
            location: decoder.string(limits.maximum_request_bytes)?,
        },
        1 => CanonicalDependencySourceRequest::Git {
            explicit_alias: decode_optional_alias(decoder, limits.maximum_identity_bytes)?,
            repository: decoder.string(limits.maximum_request_bytes)?,
            revision: decoder.string(limits.maximum_request_bytes)?,
            selection: decode_package_selection(decoder, limits.maximum_identity_bytes)?,
        },
        _ => {
            return Err(CanonicalSourceClosureSubjectError::new(
                "invalid dependency source-request tag",
            ));
        }
    })
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

fn decode_package_selection(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<PackageSelection, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
        0 => Ok(PackageSelection::Root),
        1 => PackageName::parse(decoder.string(maximum_identity_bytes)?)
            .map(PackageSelection::Named)
            .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid selected package name")),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid package-selection tag",
        )),
    }
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

pub(in super::super) fn decode_package_navigation(
    decoder: &mut Decoder<'_>,
    maximum_request_bytes: usize,
) -> Result<PackageSourceNavigation, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
        0 => Ok(PackageSourceNavigation::Root),
        1 => SourceRelativePath::parse(&decoder.string(maximum_request_bytes)?)
            .map(PackageSourceNavigation::Member)
            .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid package navigation")),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid package-navigation tag",
        )),
    }
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

fn decode_optional_alias(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<Option<AliasName>, CanonicalSourceClosureSubjectError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => AliasName::parse(decoder.string(maximum_identity_bytes)?)
            .map(Some)
            .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid explicit alias")),
        _ => Err(CanonicalSourceClosureSubjectError::new(
            "invalid explicit-alias option tag",
        )),
    }
}

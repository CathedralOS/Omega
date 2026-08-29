//! Canonical target-conditioned dependency projection framing.

use super::super::{
    CanonicalDependencySourceRequest, CanonicalSourceClosureSubjectError,
    CanonicalSourceClosureSubjectLimits,
};
use super::framing::{Decoder, Encoder};
use super::selection::{decode_dependency_request, encode_dependency_request};
use crate::manifest::dependencies::read::{
    DependencySourceRequest, ProjectedDependencies, TargetDependencyColumn,
};
use omega_target::TargetProfile;

pub(super) fn encode_target_profile(
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

pub(super) fn encode_dependency_projection(
    encoder: &mut Encoder,
    projection: &ProjectedDependencies,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encoder.u32(projection.condition_schema().version());
    encoder.count(
        projection
            .condition_schema()
            .referenced_profile_identities()
            .len(),
    )?;
    for identity in projection
        .condition_schema()
        .referenced_profile_identities()
    {
        encoder.bytes_bounded(identity.as_str().as_bytes(), limits.maximum_identity_bytes)?;
    }

    encoder.count(projection.authored_dependencies().len())?;
    for request in projection.authored_dependencies() {
        encode_dependency_request(
            encoder,
            &CanonicalDependencySourceRequest::from(request),
            limits,
        )?;
    }

    encode_occurrence_indices(encoder, projection.common_occurrence_indices())?;
    encoder.count(projection.by_profile().len())?;
    for column in projection.by_profile() {
        encode_target_profile(encoder, column.profile(), limits)?;
        encode_occurrence_indices(encoder, column.occurrence_indices())?;
    }
    Ok(())
}

pub(in super::super) fn decode_dependency_projection(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<ProjectedDependencies, CanonicalSourceClosureSubjectError> {
    let condition_schema_version = decoder.u32()?;
    let referenced_count = decoder.count(TargetProfile::ALL.len())?;
    let mut referenced_profile_identities = Vec::with_capacity(referenced_count);
    for _ in 0..referenced_count {
        referenced_profile_identities.push(decode_target_profile(decoder, limits)?.identity());
    }

    let occurrence_count = decoder.count(limits.maximum_dependency_requests)?;
    let mut occurrences = Vec::with_capacity(occurrence_count);
    for _ in 0..occurrence_count {
        occurrences.push(projected_request(decode_dependency_request(
            decoder, limits,
        )?));
    }

    let common_occurrence_indices = decode_occurrence_indices(decoder, limits)?;
    let column_count = decoder.count(TargetProfile::ALL.len())?;
    let mut by_profile = Vec::with_capacity(column_count);
    for _ in 0..column_count {
        by_profile.push(TargetDependencyColumn::new(
            decode_target_profile(decoder, limits)?,
            decode_occurrence_indices(decoder, limits)?,
        ));
    }
    Ok(ProjectedDependencies::from_retained_parts(
        occurrences,
        common_occurrence_indices,
        by_profile,
        condition_schema_version,
        referenced_profile_identities,
    ))
}

fn encode_occurrence_indices(
    encoder: &mut Encoder,
    indices: &[usize],
) -> Result<(), CanonicalSourceClosureSubjectError> {
    encoder.count(indices.len())?;
    for index in indices {
        encoder.u32(u32::try_from(*index).map_err(|_| {
            CanonicalSourceClosureSubjectError::new(
                "dependency occurrence index exceeds canonical range",
            )
        })?);
    }
    Ok(())
}

fn decode_occurrence_indices(
    decoder: &mut Decoder<'_>,
    limits: CanonicalSourceClosureSubjectLimits,
) -> Result<Vec<usize>, CanonicalSourceClosureSubjectError> {
    let count = decoder.count(limits.maximum_dependency_requests)?;
    let mut indices = Vec::with_capacity(count);
    for _ in 0..count {
        indices.push(usize::try_from(decoder.u32()?).map_err(|_| {
            CanonicalSourceClosureSubjectError::new(
                "dependency occurrence index exceeds platform range",
            )
        })?);
    }
    Ok(indices)
}

fn projected_request(request: CanonicalDependencySourceRequest) -> DependencySourceRequest {
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

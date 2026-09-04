use psi_core::PackageKeyIdentity;

use super::codec::{RecoveryDecoder, RecoveryEncoder, clone_string};
use super::model::{
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
};
use crate::record::{
    PackageReviewCanonicalRowSource, PackageReviewSourceLocation, PackageReviewSourceLocationOwner,
    PackageReviewSourceLocationRole, PackageReviewSyntheticSourceKind,
    PackageReviewToolchainSourceIdentity,
};

pub(super) fn encode_location(
    encoder: &mut RecoveryEncoder,
    location: &PackageReviewSourceLocation,
) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
    match location.owner {
        PackageReviewSourceLocationOwner::Package(package) => {
            encoder.byte(0);
            encoder.fixed_bytes(&package.digest());
        }
        PackageReviewSourceLocationOwner::Toolchain(source) => {
            encoder.byte(1);
            encoder.fixed_bytes(&source.digest());
        }
    }
    encoder.string(&location.relative_path)?;
    encoder.u64(location.start_byte);
    encoder.u64(location.end_byte);
    encoder.byte(source_location_role_tag(location.role));
    Ok(())
}

pub(super) fn decode_location(
    decoder: &mut RecoveryDecoder<'_>,
    limits: PackageReviewCanonicalRowRecoveryLimits,
    total_path_bytes: &mut usize,
) -> Result<PackageReviewSourceLocation, PackageReviewCanonicalRowRecoveryError> {
    let owner_tag = decoder.byte()?;
    let digest = decoder.array_32()?;
    let owner = match owner_tag {
        0 => PackageReviewSourceLocationOwner::Package(
            PackageKeyIdentity::from_digest(digest).ok_or_else(|| {
                PackageReviewCanonicalRowRecoveryError::new(
                    "canonical-row recovery source contains an invalid package owner",
                )
            })?,
        ),
        1 => PackageReviewSourceLocationOwner::Toolchain(PackageReviewToolchainSourceIdentity {
            digest,
        }),
        _ => {
            return Err(PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery source contains an unknown owner tag",
            ));
        }
    };
    let relative_path = clone_string(
        decoder.string(limits.maximum_source_path_bytes)?,
        "canonical-row recovery source-path allocation failed",
    )?;
    *total_path_bytes = total_path_bytes
        .checked_add(relative_path.len())
        .ok_or_else(|| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery source-path byte count overflow",
            )
        })?;
    if *total_path_bytes > limits.maximum_total_source_path_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source paths exceed their total byte ceiling",
        ));
    }
    let start_byte = decoder.u64()?;
    let end_byte = decoder.u64()?;
    let role = decode_source_location_role(decoder.byte()?)?;
    Ok(PackageReviewSourceLocation {
        owner,
        relative_path,
        start_byte,
        end_byte,
        role,
    })
}

pub(super) fn validate_source(
    source: &PackageReviewCanonicalRowSource,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
    if source.authored_locations.is_empty() && source.compiler_derivations.is_empty() {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source has no authored location or compiler derivation",
        ));
    }
    if source.authored_locations.len() > limits.maximum_source_locations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source-location count exceeds its ceiling",
        ));
    }
    if source.compiler_derivations.len() > limits.maximum_compiler_derivations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery compiler-derivation count exceeds its ceiling",
        ));
    }
    if source
        .authored_locations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source locations are not strictly ordered",
        ));
    }
    if source
        .compiler_derivations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery compiler derivations are not strictly ordered",
        ));
    }
    let mut total_path_bytes = 0usize;
    for location in &source.authored_locations {
        validate_location(location, limits)?;
        total_path_bytes = total_path_bytes
            .checked_add(location.relative_path.len())
            .ok_or_else(|| {
                PackageReviewCanonicalRowRecoveryError::new(
                    "canonical-row recovery source-path byte count overflow",
                )
            })?;
    }
    if total_path_bytes > limits.maximum_total_source_path_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source paths exceed their total byte ceiling",
        ));
    }
    Ok(())
}

fn validate_location(
    location: &PackageReviewSourceLocation,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<(), PackageReviewCanonicalRowRecoveryError> {
    if location.relative_path.is_empty()
        || location.relative_path.len() > limits.maximum_source_path_bytes
        || location.relative_path.starts_with('/')
        || location
            .relative_path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains a noncanonical relative path",
        ));
    }
    if location.start_byte >= location.end_byte {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains an invalid byte span",
        ));
    }
    Ok(())
}

pub(super) const fn synthetic_source_tag(kind: PackageReviewSyntheticSourceKind) -> u8 {
    match kind {
        PackageReviewSyntheticSourceKind::ProjectionHeader => 0,
        PackageReviewSyntheticSourceKind::EmptySelectedProviderSet => 1,
        PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection => 2,
        PackageReviewSyntheticSourceKind::FreeExternalProviderType => 3,
        PackageReviewSyntheticSourceKind::ConsumerTerminalAuthorityPermission => 4,
    }
}

pub(super) fn decode_synthetic_source(
    tag: u8,
) -> Result<PackageReviewSyntheticSourceKind, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewSyntheticSourceKind::ProjectionHeader),
        1 => Ok(PackageReviewSyntheticSourceKind::EmptySelectedProviderSet),
        2 => Ok(PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection),
        3 => Ok(PackageReviewSyntheticSourceKind::FreeExternalProviderType),
        4 => Ok(PackageReviewSyntheticSourceKind::ConsumerTerminalAuthorityPermission),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains an unknown compiler-derivation tag",
        )),
    }
}

const fn source_location_role_tag(role: PackageReviewSourceLocationRole) -> u8 {
    match role {
        PackageReviewSourceLocationRole::Declaration => 0,
        PackageReviewSourceLocationRole::DerivationOrigin => 1,
        PackageReviewSourceLocationRole::AuthorityDeclaration => 2,
        PackageReviewSourceLocationRole::AuthorityExposure => 3,
        PackageReviewSourceLocationRole::ProviderSelection => 4,
        PackageReviewSourceLocationRole::ProviderGrant => 25,
        PackageReviewSourceLocationRole::BoundaryApplicationUse => 26,
        PackageReviewSourceLocationRole::ProviderSchemaDeclaration => 5,
        PackageReviewSourceLocationRole::ProviderTypeDeclaration => 6,
        PackageReviewSourceLocationRole::ProviderRealization => 7,
        PackageReviewSourceLocationRole::SemanticDependencyConsumer => 8,
        PackageReviewSourceLocationRole::SemanticDependencyDeclaration => 9,
        PackageReviewSourceLocationRole::ProviderRequirementDeclaration => 10,
        PackageReviewSourceLocationRole::TraitParent => 11,
        PackageReviewSourceLocationRole::ContractClause => 12,
        PackageReviewSourceLocationRole::BodyCall => 13,
        PackageReviewSourceLocationRole::SynchronousInvocation => 14,
        PackageReviewSourceLocationRole::ServiceReach => 15,
        PackageReviewSourceLocationRole::Suspension => 16,
        PackageReviewSourceLocationRole::Blocking => 17,
        PackageReviewSourceLocationRole::ExternalBinding => 18,
        PackageReviewSourceLocationRole::ConstInitializer => 19,
        PackageReviewSourceLocationRole::PropositionFormula => 20,
        PackageReviewSourceLocationRole::ProofFact => 21,
        PackageReviewSourceLocationRole::TraitRequirement => 22,
        PackageReviewSourceLocationRole::DataMember => 23,
        PackageReviewSourceLocationRole::CallableParameter => 24,
        PackageReviewSourceLocationRole::QuotientOperationDeclaration => 27,
        PackageReviewSourceLocationRole::RepresentationSelection => 28,
    }
}

fn decode_source_location_role(
    tag: u8,
) -> Result<PackageReviewSourceLocationRole, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewSourceLocationRole::Declaration),
        1 => Ok(PackageReviewSourceLocationRole::DerivationOrigin),
        2 => Ok(PackageReviewSourceLocationRole::AuthorityDeclaration),
        3 => Ok(PackageReviewSourceLocationRole::AuthorityExposure),
        4 => Ok(PackageReviewSourceLocationRole::ProviderSelection),
        25 => Ok(PackageReviewSourceLocationRole::ProviderGrant),
        26 => Ok(PackageReviewSourceLocationRole::BoundaryApplicationUse),
        5 => Ok(PackageReviewSourceLocationRole::ProviderSchemaDeclaration),
        6 => Ok(PackageReviewSourceLocationRole::ProviderTypeDeclaration),
        7 => Ok(PackageReviewSourceLocationRole::ProviderRealization),
        8 => Ok(PackageReviewSourceLocationRole::SemanticDependencyConsumer),
        9 => Ok(PackageReviewSourceLocationRole::SemanticDependencyDeclaration),
        10 => Ok(PackageReviewSourceLocationRole::ProviderRequirementDeclaration),
        11 => Ok(PackageReviewSourceLocationRole::TraitParent),
        12 => Ok(PackageReviewSourceLocationRole::ContractClause),
        13 => Ok(PackageReviewSourceLocationRole::BodyCall),
        14 => Ok(PackageReviewSourceLocationRole::SynchronousInvocation),
        15 => Ok(PackageReviewSourceLocationRole::ServiceReach),
        16 => Ok(PackageReviewSourceLocationRole::Suspension),
        17 => Ok(PackageReviewSourceLocationRole::Blocking),
        18 => Ok(PackageReviewSourceLocationRole::ExternalBinding),
        19 => Ok(PackageReviewSourceLocationRole::ConstInitializer),
        20 => Ok(PackageReviewSourceLocationRole::PropositionFormula),
        21 => Ok(PackageReviewSourceLocationRole::ProofFact),
        22 => Ok(PackageReviewSourceLocationRole::TraitRequirement),
        23 => Ok(PackageReviewSourceLocationRole::DataMember),
        24 => Ok(PackageReviewSourceLocationRole::CallableParameter),
        27 => Ok(PackageReviewSourceLocationRole::QuotientOperationDeclaration),
        28 => Ok(PackageReviewSourceLocationRole::RepresentationSelection),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source contains an unknown role tag",
        )),
    }
}

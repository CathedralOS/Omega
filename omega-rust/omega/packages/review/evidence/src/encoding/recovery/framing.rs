use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

use super::codec::{RecoveryDecoder, clone_bytes};
use super::model::{
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
};
use crate::encoding::encode::ROW_MAGIC;
use crate::encoding::{PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION};
use crate::record::{PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk};

pub(super) struct ParsedCanonicalRow {
    pub(super) package: PackageKeyIdentity,
    pub(super) target: TargetProfile,
    pub(super) kind: PackageReviewCanonicalRowKind,
    pub(super) risk: PackageReviewCanonicalRowRisk,
    pub(super) key_bytes: Vec<u8>,
}

pub(super) fn parse_canonical_row(
    bytes: &[u8],
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<ParsedCanonicalRow, PackageReviewCanonicalRowRecoveryError> {
    if bytes.len() > limits.maximum_canonical_row_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row exceeds its byte ceiling",
        ));
    }
    let mut decoder = RecoveryDecoder::new(bytes);
    decoder.fixed_bytes(ROW_MAGIC)?;
    if decoder.u16()? != PACKAGE_REVIEW_ROW_ENCODING_VERSION {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "unsupported canonical package-review row version",
        ));
    }
    if decoder.u16()? != PACKAGE_REVIEW_ENCODING_VERSION {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row names an unsupported review schema",
        ));
    }
    let package_digest = decoder.array_32()?;
    let package = PackageKeyIdentity::from_digest(package_digest).ok_or_else(|| {
        PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains an invalid package identity",
        )
    })?;
    let target_name = decoder.string(limits.maximum_target_bytes)?;
    let target = TargetProfile::from_canonical_target_name(target_name).map_err(|_| {
        PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains a noncanonical target",
        )
    })?;
    if target.target_name() != target_name {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains a noncanonical target",
        ));
    }
    let kind = decode_kind(decoder.byte()?)?;
    let risk = decode_risk(decoder.byte()?)?;
    if risk != canonical_risk(kind) {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains a noncanonical risk for its kind",
        ));
    }
    let key_bytes = clone_bytes(
        decoder.bytes(limits.maximum_row_key_bytes)?,
        "canonical package-review row key allocation failed",
    )?;
    let _value_bytes = decoder.bytes(limits.maximum_row_value_bytes)?;
    decoder.finish()?;
    Ok(ParsedCanonicalRow {
        package,
        target,
        kind,
        risk,
        key_bytes,
    })
}

const fn canonical_risk(kind: PackageReviewCanonicalRowKind) -> PackageReviewCanonicalRowRisk {
    match kind {
        PackageReviewCanonicalRowKind::RepresentationTcb
        | PackageReviewCanonicalRowKind::DangerousAuthoritySlack => {
            PackageReviewCanonicalRowRisk::AuditRecommended
        }
        PackageReviewCanonicalRowKind::SelectedProviderSet
        | PackageReviewCanonicalRowKind::ExternalExecutableSupply => {
            PackageReviewCanonicalRowRisk::OpaqueBlocking
        }
        PackageReviewCanonicalRowKind::ProjectionHeader
        | PackageReviewCanonicalRowKind::PublicTrait
        | PackageReviewCanonicalRowKind::PublicDomain
        | PackageReviewCanonicalRowKind::PublicData
        | PackageReviewCanonicalRowKind::PublicProposition
        | PackageReviewCanonicalRowKind::PublicConst
        | PackageReviewCanonicalRowKind::PublicOperator
        | PackageReviewCanonicalRowKind::PublicConformance
        | PackageReviewCanonicalRowKind::Callable
        | PackageReviewCanonicalRowKind::DangerousAuthority
        | PackageReviewCanonicalRowKind::AcceptedClaim
        | PackageReviewCanonicalRowKind::SemanticDependency
        | PackageReviewCanonicalRowKind::BoundaryApplicationRealization
        | PackageReviewCanonicalRowKind::BoundaryApplicationDemand
        | PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence
        | PackageReviewCanonicalRowKind::ContractEntailmentAssumptionDischarge
        | PackageReviewCanonicalRowKind::TerminalAuthorityPermission => {
            PackageReviewCanonicalRowRisk::Blocking
        }
        PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation => {
            PackageReviewCanonicalRowRisk::Blocking
        }
    }
}

fn decode_kind(
    tag: u8,
) -> Result<PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewCanonicalRowKind::ProjectionHeader),
        1 => Ok(PackageReviewCanonicalRowKind::PublicTrait),
        2 => Ok(PackageReviewCanonicalRowKind::PublicDomain),
        3 => Ok(PackageReviewCanonicalRowKind::PublicData),
        4 => Ok(PackageReviewCanonicalRowKind::RepresentationTcb),
        5 => Ok(PackageReviewCanonicalRowKind::Callable),
        6 => Ok(PackageReviewCanonicalRowKind::DangerousAuthority),
        7 => Ok(PackageReviewCanonicalRowKind::SelectedProviderSet),
        8 => Ok(PackageReviewCanonicalRowKind::AcceptedClaim),
        9 => Ok(PackageReviewCanonicalRowKind::DangerousAuthoritySlack),
        10 => Ok(PackageReviewCanonicalRowKind::SemanticDependency),
        11 => Ok(PackageReviewCanonicalRowKind::PublicProposition),
        12 => Ok(PackageReviewCanonicalRowKind::PublicConst),
        13 => Ok(PackageReviewCanonicalRowKind::PublicOperator),
        14 => Ok(PackageReviewCanonicalRowKind::PublicConformance),
        15 => Ok(PackageReviewCanonicalRowKind::ExternalExecutableSupply),
        16 => Ok(PackageReviewCanonicalRowKind::BoundaryApplicationRealization),
        17 => Ok(PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence),
        18 => Ok(PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation),
        19 => Ok(PackageReviewCanonicalRowKind::TerminalAuthorityPermission),
        20 => Ok(PackageReviewCanonicalRowKind::BoundaryApplicationDemand),
        21 => Ok(PackageReviewCanonicalRowKind::ContractEntailmentAssumptionDischarge),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains an unknown kind tag",
        )),
    }
}

fn decode_risk(
    tag: u8,
) -> Result<PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowRecoveryError> {
    match tag {
        0 => Ok(PackageReviewCanonicalRowRisk::Blocking),
        1 => Ok(PackageReviewCanonicalRowRisk::AuditRecommended),
        2 => Ok(PackageReviewCanonicalRowRisk::OpaqueBlocking),
        _ => Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row contains an unknown risk tag",
        )),
    }
}

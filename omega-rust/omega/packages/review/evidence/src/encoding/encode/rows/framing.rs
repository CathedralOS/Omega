//! Canonical row envelope construction, source lookup, and wire tags.

use super::super::encoder::Encoder;
use super::super::{
    PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION,
    PackageReviewEncodingError, PackageReviewEncodingLimits, ROW_MAGIC,
};
use crate::record::{
    CheckedPackageReviewProjection, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};

pub(super) fn push_row(
    rows: &mut Vec<PackageReviewCanonicalRow>,
    total_row_bytes: &mut usize,
    limits: PackageReviewEncodingLimits,
    row: PackageReviewCanonicalRow,
) -> Result<(), PackageReviewEncodingError> {
    *total_row_bytes = total_row_bytes
        .checked_add(row.key_bytes.len())
        .and_then(|total| total.checked_add(row.canonical_bytes.len()))
        .ok_or_else(|| {
            PackageReviewEncodingError::new(
                "package review exceeds the total canonical-row byte ceiling",
            )
        })?;
    if *total_row_bytes > limits.maximum_total_row_bytes {
        return Err(PackageReviewEncodingError::new(
            "package review exceeds the total canonical-row byte ceiling",
        ));
    }
    rows.push(row);
    Ok(())
}

pub(super) fn encode_row(
    review: &CheckedPackageReviewProjection,
    limits: PackageReviewEncodingLimits,
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    source: PackageReviewCanonicalRowSource,
    encode_key: impl FnOnce(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
    encode_value: impl FnOnce(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
) -> Result<PackageReviewCanonicalRow, PackageReviewEncodingError> {
    encode_subject_row(
        review.package,
        review.target,
        limits,
        kind,
        risk,
        source,
        encode_key,
        encode_value,
    )
}

pub(crate) fn encode_subject_row(
    package: psi_core::PackageKeyIdentity,
    target: omega_target::TargetProfile,
    limits: PackageReviewEncodingLimits,
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    source: PackageReviewCanonicalRowSource,
    encode_key: impl FnOnce(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
    encode_value: impl FnOnce(&mut Encoder) -> Result<(), PackageReviewEncodingError>,
) -> Result<PackageReviewCanonicalRow, PackageReviewEncodingError> {
    let mut key = Encoder::bounded(limits.maximum_row_key_bytes);
    encode_key(&mut key)?;
    let key_bytes = key.finish()?;
    let mut value = Encoder::bounded(limits.maximum_row_bytes);
    encode_value(&mut value)?;
    let value_bytes = value.finish()?;
    let mut canonical = Encoder::bounded(limits.maximum_row_bytes);
    canonical.fixed_bytes(ROW_MAGIC);
    canonical.u16(PACKAGE_REVIEW_ROW_ENCODING_VERSION);
    canonical.u16(PACKAGE_REVIEW_ENCODING_VERSION);
    canonical.package_identity(package);
    canonical.string(target.target_name())?;
    canonical.byte(canonical_row_kind_tag(kind));
    canonical.byte(canonical_row_risk_tag(risk));
    canonical.bytes(&key_bytes)?;
    canonical.bytes(&value_bytes)?;
    Ok(PackageReviewCanonicalRow {
        kind,
        risk,
        key_bytes,
        canonical_bytes: canonical.finish()?,
        source,
    })
}

pub(super) fn row_source(
    sources: &[PackageReviewCanonicalRowSource],
    index: usize,
) -> Result<PackageReviewCanonicalRowSource, PackageReviewEncodingError> {
    sources.get(index).cloned().ok_or_else(|| {
        PackageReviewEncodingError::new(
            "package review canonical row has no compiler-issued source disposition",
        )
    })
}

const fn canonical_row_risk_tag(risk: PackageReviewCanonicalRowRisk) -> u8 {
    match risk {
        PackageReviewCanonicalRowRisk::Blocking => 0,
        PackageReviewCanonicalRowRisk::AuditRecommended => 1,
        PackageReviewCanonicalRowRisk::OpaqueBlocking => 2,
    }
}

const fn canonical_row_kind_tag(kind: PackageReviewCanonicalRowKind) -> u8 {
    match kind {
        PackageReviewCanonicalRowKind::ProjectionHeader => 0,
        PackageReviewCanonicalRowKind::PublicTrait => 1,
        PackageReviewCanonicalRowKind::PublicDomain => 2,
        PackageReviewCanonicalRowKind::PublicData => 3,
        PackageReviewCanonicalRowKind::RepresentationTcb => 4,
        PackageReviewCanonicalRowKind::Callable => 5,
        PackageReviewCanonicalRowKind::DangerousAuthority => 6,
        PackageReviewCanonicalRowKind::SelectedProviderSet => 7,
        PackageReviewCanonicalRowKind::AcceptedClaim => 8,
        PackageReviewCanonicalRowKind::DangerousAuthoritySlack => 9,
        PackageReviewCanonicalRowKind::SemanticDependency => 10,
        PackageReviewCanonicalRowKind::PublicProposition => 11,
        PackageReviewCanonicalRowKind::PublicConst => 12,
        PackageReviewCanonicalRowKind::PublicOperator => 13,
        PackageReviewCanonicalRowKind::PublicConformance => 14,
        PackageReviewCanonicalRowKind::ExternalExecutableSupply => 15,
        PackageReviewCanonicalRowKind::BoundaryApplicationRealization => 16,
        PackageReviewCanonicalRowKind::NonExecutableQuotientCorrespondence => 17,
        PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation => 18,
        PackageReviewCanonicalRowKind::TerminalAuthorityPermission => 19,
        PackageReviewCanonicalRowKind::BoundaryApplicationDemand => 20,
        PackageReviewCanonicalRowKind::ContractEntailmentAssumptionDischarge => 21,
    }
}

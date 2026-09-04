use super::codec::{RecoveryDecoder, RecoveryEncoder, clone_bytes};
use super::framing::parse_canonical_row;
use super::model::{
    DecodedPackageReviewCanonicalRow, PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION,
    PackageReviewCanonicalRowRecoveryError, PackageReviewCanonicalRowRecoveryLimits,
};
use super::source::{
    decode_location, decode_synthetic_source, encode_location, synthetic_source_tag,
    validate_source,
};
use crate::record::{PackageReviewCanonicalRow, PackageReviewCanonicalRowSource};

const RECOVERY_MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-ROW-RECOVERY\0";

pub fn encode_package_review_canonical_row(
    row: &PackageReviewCanonicalRow,
) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
    encode_package_review_canonical_row_with_limits(
        row,
        PackageReviewCanonicalRowRecoveryLimits::default(),
    )
}

pub fn encode_package_review_canonical_row_with_limits(
    row: &PackageReviewCanonicalRow,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<Vec<u8>, PackageReviewCanonicalRowRecoveryError> {
    let framing = parse_canonical_row(&row.canonical_bytes, limits)?;
    if row.kind != framing.kind || row.risk != framing.risk || row.key_bytes != framing.key_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical package-review row fields disagree with its canonical outer frame",
        ));
    }
    validate_source(&row.source, limits)?;

    let mut encoder = RecoveryEncoder::bounded(limits.maximum_recovery_bytes);
    encoder.fixed_bytes(RECOVERY_MAGIC);
    encoder.u16(PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION);
    encoder.bytes(&row.canonical_bytes)?;
    encoder.usize(row.source.authored_locations.len())?;
    for location in &row.source.authored_locations {
        encode_location(&mut encoder, location)?;
    }
    encoder.usize(row.source.compiler_derivations.len())?;
    for derivation in &row.source.compiler_derivations {
        encoder.byte(synthetic_source_tag(*derivation));
    }
    encoder.finish()
}

pub fn decode_package_review_canonical_row(
    bytes: &[u8],
) -> Result<DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRowRecoveryError> {
    decode_package_review_canonical_row_with_limits(
        bytes,
        PackageReviewCanonicalRowRecoveryLimits::default(),
    )
}

pub fn decode_package_review_canonical_row_with_limits(
    bytes: &[u8],
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRowRecoveryError> {
    if bytes.len() > limits.maximum_recovery_bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery envelope exceeds its byte ceiling",
        ));
    }
    let mut decoder = RecoveryDecoder::new(bytes);
    decoder.fixed_bytes(RECOVERY_MAGIC)?;
    if decoder.u16()? != PACKAGE_REVIEW_CANONICAL_ROW_RECOVERY_VERSION {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "unsupported canonical-row recovery envelope version",
        ));
    }
    let canonical_bytes = clone_bytes(
        decoder.bytes(limits.maximum_canonical_row_bytes)?,
        "canonical package-review row allocation failed",
    )?;
    let framing = parse_canonical_row(&canonical_bytes, limits)?;

    let location_count = decoder.usize()?;
    if location_count > limits.maximum_source_locations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery source-location count exceeds its ceiling",
        ));
    }
    let mut authored_locations = Vec::new();
    authored_locations
        .try_reserve(location_count)
        .map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery source-location allocation failed",
            )
        })?;
    let mut total_path_bytes = 0usize;
    for _ in 0..location_count {
        authored_locations.push(decode_location(
            &mut decoder,
            limits,
            &mut total_path_bytes,
        )?);
    }

    let derivation_count = decoder.usize()?;
    if derivation_count > limits.maximum_compiler_derivations {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery compiler-derivation count exceeds its ceiling",
        ));
    }
    let mut compiler_derivations = Vec::new();
    compiler_derivations
        .try_reserve(derivation_count)
        .map_err(|_| {
            PackageReviewCanonicalRowRecoveryError::new(
                "canonical-row recovery compiler-derivation allocation failed",
            )
        })?;
    for _ in 0..derivation_count {
        compiler_derivations.push(decode_synthetic_source(decoder.byte()?)?);
    }
    decoder.finish()?;

    let source = PackageReviewCanonicalRowSource::mixed(authored_locations, compiler_derivations);
    validate_source(&source, limits)?;
    let decoded = DecodedPackageReviewCanonicalRow {
        package: framing.package,
        target: framing.target,
        row: PackageReviewCanonicalRow {
            kind: framing.kind,
            risk: framing.risk,
            key_bytes: framing.key_bytes,
            canonical_bytes,
            source,
        },
    };
    if encode_package_review_canonical_row_with_limits(&decoded.row, limits)? != bytes {
        return Err(PackageReviewCanonicalRowRecoveryError::new(
            "canonical-row recovery input is not in canonical form",
        ));
    }
    Ok(decoded)
}

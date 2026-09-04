use super::super::*;
use super::fixture::optimized;
use super::wire_offsets::locate;

#[test]
fn closed_envelope_tags_and_nested_payloads_fail_with_exact_decode_errors() {
    let encoded = optimized().pre_physical_manifest().record().encode();
    let offsets = locate(&encoded);

    corrupt(
        &encoded,
        0,
        0xff,
        PrePhysicalOptimizationManifestDecodeError::WrongMagic,
    );

    let mut version = encoded.clone();
    version[8..12].copy_from_slice(&99_u32.to_le_bytes());
    assert_decode_error(
        version,
        PrePhysicalOptimizationManifestDecodeError::UnsupportedVersion(99),
    );

    corrupt(
        &encoded,
        12,
        encoded[12] ^ 1,
        PrePhysicalOptimizationManifestDecodeError::IdentityMismatch,
    );
    corrupt(
        &encoded,
        offsets.stage,
        99,
        PrePhysicalOptimizationManifestDecodeError::UnknownStage(99),
    );
    corrupt(
        &encoded,
        offsets.physical,
        99,
        PrePhysicalOptimizationManifestDecodeError::UnknownPhysicalStatus(99),
    );

    let mut vocabulary = encoded.clone();
    vocabulary[offsets.vocabulary..offsets.vocabulary + 2].copy_from_slice(&0_u16.to_le_bytes());
    assert_decode_error(
        vocabulary,
        PrePhysicalOptimizationManifestDecodeError::UnsupportedVocabulary(0),
    );

    let mut fuel = encoded.clone();
    fuel[offsets.fuel..offsets.fuel + 4].copy_from_slice(&0_u32.to_le_bytes());
    assert_decode_error(
        fuel,
        PrePhysicalOptimizationManifestDecodeError::InvalidFuelSchedule,
    );

    for (offset, expected) in [
        (
            offsets.selections_payload,
            PrePhysicalOptimizationManifestDecodeError::InvalidSelections,
        ),
        (
            offsets.psi_selections_payload,
            PrePhysicalOptimizationManifestDecodeError::InvalidSelections,
        ),
        (
            offsets.decision_payload,
            PrePhysicalOptimizationManifestDecodeError::InvalidDecisionLog,
        ),
        (
            offsets.first_pass_payload,
            PrePhysicalOptimizationManifestDecodeError::InvalidPassManifest,
        ),
        (
            offsets.ledger_payload,
            PrePhysicalOptimizationManifestDecodeError::InvalidTransformationLedger,
        ),
        (
            offsets.bundle_payload,
            PrePhysicalOptimizationManifestDecodeError::InvalidIdentityBundle,
        ),
    ] {
        corrupt(&encoded, offset, encoded[offset] ^ 0xff, expected);
    }

    let mut budget = encoded.clone();
    budget[offsets.budget..offsets.budget + 8].copy_from_slice(&0_u64.to_le_bytes());
    assert_decode_error(
        budget,
        PrePhysicalOptimizationManifestDecodeError::InvalidWorkBudget,
    );
}

#[test]
fn framing_rejects_truncation_and_trailing_bytes() {
    let encoded = optimized().pre_physical_manifest().record().encode();

    assert_decode_error(
        encoded[..encoded.len() - 1].to_vec(),
        PrePhysicalOptimizationManifestDecodeError::Truncated,
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_decode_error(
        trailing,
        PrePhysicalOptimizationManifestDecodeError::TrailingBytes,
    );
}

fn corrupt(
    encoded: &[u8],
    offset: usize,
    value: u8,
    expected: PrePhysicalOptimizationManifestDecodeError,
) {
    let mut corrupted = encoded.to_vec();
    corrupted[offset] = value;
    assert_decode_error(corrupted, expected);
}

fn assert_decode_error(encoded: Vec<u8>, expected: PrePhysicalOptimizationManifestDecodeError) {
    assert_eq!(
        PrePhysicalOptimizationManifest::decode(&encoded),
        Err(expected)
    );
}

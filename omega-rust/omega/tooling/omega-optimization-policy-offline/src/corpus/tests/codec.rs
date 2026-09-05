use super::fixture::{chosen_point, encoded_log, skipped_point, source};
use crate::{OfflinePolicyCorpusError, admit_external_decision_logs, decode_offline_policy_corpus};
use omega_optimization_core::ExternalDecisionSchemaError;

const HEADER: usize = 8 + 4 + 32 + 4;

#[test]
fn strict_codec_round_trip_reconstructs_examples_and_receipt() {
    let admitted = admit_external_decision_logs([
        encoded_log(source(b"codec-a"), [chosen_point(b"codec-a")]),
        encoded_log(source(b"codec-b"), [skipped_point(b"codec-b")]),
    ])
    .unwrap();
    let decoded = decode_offline_policy_corpus(&admitted.encode()).unwrap();
    assert_eq!(decoded, admitted);
}

#[test]
fn codec_rejects_envelope_corruption_and_trailing_bytes() {
    let admitted = admit_external_decision_logs([encoded_log(
        source(b"codec-corruption"),
        [chosen_point(b"codec-corruption")],
    )])
    .unwrap();
    let encoded = admitted.encode();

    let mut wrong_magic = encoded.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        decode_offline_policy_corpus(&wrong_magic),
        Err(OfflinePolicyCorpusError::WrongMagic)
    );
    let mut wrong_version = encoded.clone();
    wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
    assert_eq!(
        decode_offline_policy_corpus(&wrong_version),
        Err(OfflinePolicyCorpusError::UnsupportedVersion(2))
    );
    let mut wrong_identity = encoded.clone();
    wrong_identity[12] ^= 1;
    assert_eq!(
        decode_offline_policy_corpus(&wrong_identity),
        Err(OfflinePolicyCorpusError::CorpusIdentityMismatch)
    );
    let mut unknown_split = encoded.clone();
    unknown_split[HEADER] = 9;
    assert_eq!(
        decode_offline_policy_corpus(&unknown_split),
        Err(OfflinePolicyCorpusError::UnknownSplit(9))
    );
    assert_eq!(
        decode_offline_policy_corpus(&encoded[..encoded.len() - 1]),
        Err(OfflinePolicyCorpusError::Truncated)
    );
    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        decode_offline_policy_corpus(&trailing),
        Err(OfflinePolicyCorpusError::TrailingBytes)
    );
}

#[test]
fn codec_rejects_noncanonical_record_order() {
    let admitted = admit_external_decision_logs([
        encoded_log(source(b"order-a"), [chosen_point(b"order-a")]),
        encoded_log(source(b"order-b"), [skipped_point(b"order-b")]),
    ])
    .unwrap();
    let mut encoded = admitted.encode();
    let first_length =
        u32::from_le_bytes(encoded[HEADER + 1..HEADER + 5].try_into().unwrap()) as usize;
    let second_start = HEADER + 5 + first_length;
    let second_length = u32::from_le_bytes(
        encoded[second_start + 1..second_start + 5]
            .try_into()
            .unwrap(),
    ) as usize;
    let first = encoded[HEADER..second_start].to_vec();
    let second = encoded[second_start..second_start + 5 + second_length].to_vec();
    encoded.splice(
        HEADER..second_start + 5 + second_length,
        [second, first].concat(),
    );
    assert_eq!(
        decode_offline_policy_corpus(&encoded),
        Err(OfflinePolicyCorpusError::NonCanonicalLogs)
    );
}

#[test]
fn codec_rejects_action_corruption_inside_the_retained_v2_log() {
    let admitted = admit_external_decision_logs([encoded_log(
        source(b"action-corruption"),
        [chosen_point(b"action-corruption")],
    )])
    .unwrap();
    let mut encoded = admitted.encode();
    let last = encoded.last_mut().unwrap();
    *last ^= 1;
    let result = decode_offline_policy_corpus(&encoded);
    assert!(
        matches!(
            result,
            Err(OfflinePolicyCorpusError::ExternalSchema(
                ExternalDecisionSchemaError::IllegalAction
            ))
        ),
        "{result:?}"
    );
}

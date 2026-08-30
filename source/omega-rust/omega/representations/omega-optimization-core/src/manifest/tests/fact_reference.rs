//! Closed fact-family tags and exact framing.

use super::*;

#[test]
fn fact_reference_codec_round_trips_every_closed_variant_with_stable_tags() {
    let cases = [
        (fact(b"scalar"), 1),
        (obligation_fact(b"obligation"), 2),
        (ownership_fact(b"ownership"), 3),
        (range_fact(b"range"), 4),
    ];

    for (fact, expected_tag) in cases {
        let encoded = fact.encode();
        assert_eq!(encoded.len(), OptimizationFactReference::ENCODED_LENGTH);
        assert_eq!(encoded[0], expected_tag);
        assert_eq!(OptimizationFactReference::decode(&encoded), Ok(fact));
        assert_eq!(
            OptimizationFactReference::decode(&fact.encode())
                .unwrap()
                .encode(),
            encoded
        );
    }
}

#[test]
fn fact_reference_codec_detects_tag_and_identity_corruption_for_every_variant() {
    let facts = [
        fact(b"scalar"),
        obligation_fact(b"obligation"),
        ownership_fact(b"ownership"),
        range_fact(b"range"),
    ];

    for fact in facts {
        let mut unknown_tag = fact.encode();
        unknown_tag[0] = 255;
        assert_eq!(
            OptimizationFactReference::decode(&unknown_tag),
            Err(OptimizationFactReferenceDecodeError::UnknownTag(255))
        );

        let mut changed_identity = fact.encode();
        changed_identity[1] ^= 1;
        let decoded = OptimizationFactReference::decode(&changed_identity).unwrap();
        assert_ne!(decoded, fact);
        assert_eq!(decoded.encode(), changed_identity);
    }
}

#[test]
fn fact_reference_codec_rejects_truncated_and_trailing_framing() {
    let encoded = fact(b"framing").encode();
    assert_eq!(
        OptimizationFactReference::decode(&encoded[..encoded.len() - 1]),
        Err(OptimizationFactReferenceDecodeError::WrongLength {
            expected: OptimizationFactReference::ENCODED_LENGTH,
            actual: OptimizationFactReference::ENCODED_LENGTH - 1,
        })
    );

    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert_eq!(
        OptimizationFactReference::decode(&trailing),
        Err(OptimizationFactReferenceDecodeError::WrongLength {
            expected: OptimizationFactReference::ENCODED_LENGTH,
            actual: OptimizationFactReference::ENCODED_LENGTH + 1,
        })
    );
}

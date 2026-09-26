use super::{tests::fixture, *};
use crate::encoding::PackagePolicyTextRecoveryLimits;

fn limits(
    text: &str,
    binary_bytes: usize,
    elements: usize,
    owned: usize,
) -> PackagePolicyTextRecoveryLimits {
    PackagePolicyTextRecoveryLimits::new(
        text.len(),
        PackagePolicyRecoveryLimits::new(binary_bytes, binary_bytes, elements, owned, 128),
    )
}

#[test]
fn text_usage_includes_requested_binary_reserve_and_canonical_scratch() {
    let value = fixture();
    let text = value.canonical_text().unwrap();
    let defaults = PackagePolicyRecoveryLimits::default();
    let (binary, reserved) =
        crate::encoding::recovery::decode_policy_text_scalars(&text, defaults).unwrap();
    let (from_binary, binary_usage) =
        PackagePolicyBaseline::recover_canonical_with_usage(&binary, defaults).unwrap();
    let (from_text, text_usage) = PackagePolicyBaseline::recover_text_with_usage(
        &text,
        PackagePolicyTextRecoveryLimits::default(),
    )
    .unwrap();
    let (counted_text, encoded_elements) = value.canonical_text_with_element_count().unwrap();
    assert_eq!(counted_text, text);
    assert_eq!(encoded_elements, text_usage.sequence_elements());
    assert_eq!(from_binary, value);
    assert_eq!(from_text, value);
    assert_eq!(
        text_usage.owned_bytes(),
        reserved + binary_usage.owned_bytes()
    );
    assert_eq!(
        text_usage.sequence_elements(),
        binary_usage.sequence_elements()
    );
    assert!(binary_usage.owned_bytes() > binary.len());

    // An exact binary ceiling removes geometric reserve slack; every charged
    // byte and element then has a separately testable successful frontier.
    let generous = limits(&text, binary.len(), 65_536, 64 * 1024 * 1024);
    let (_, usage) = PackagePolicyBaseline::recover_text_with_usage(&text, generous).unwrap();
    assert_eq!(
        usage.owned_bytes(),
        binary.len() + binary_usage.owned_bytes()
    );
    let exact = limits(
        &text,
        binary.len(),
        usage.sequence_elements(),
        usage.owned_bytes(),
    );
    assert_eq!(
        PackagePolicyBaseline::recover_text_with_usage(&text, exact).unwrap(),
        (value.clone(), usage)
    );
    assert_eq!(
        PackagePolicyBaseline::recover_text(&text, exact).unwrap(),
        value
    );
    for (elements, owned, expected) in [
        (
            usage.sequence_elements() - 1,
            usage.owned_bytes(),
            Error::ElementLimitExceeded,
        ),
        (
            usage.sequence_elements(),
            usage.owned_bytes() - 1,
            Error::AllocationLimitExceeded,
        ),
    ] {
        let too_small = limits(&text, binary.len(), elements, owned);
        assert_eq!(
            PackagePolicyBaseline::recover_text_with_usage(&text, too_small),
            Err(expected)
        );
        assert_eq!(
            PackagePolicyBaseline::recover_text(&text, too_small),
            Err(expected)
        );
    }
}

#[test]
fn canonical_verification_reserves_only_its_exact_charged_length() {
    let value = fixture();
    let expected = value.canonical_bytes().unwrap();
    assert_eq!(
        value.canonical_bytes_for_recovery(expected.len()).unwrap(),
        expected
    );
    for length in [0, expected.len() - 1, expected.len() + 1, usize::MAX] {
        assert!(value.canonical_bytes_for_recovery(length).is_err());
    }
    let limits = PackagePolicyRecoveryLimits::default();
    let (_, usage) =
        PackagePolicyBaseline::recover_canonical_with_usage(&expected, limits).unwrap();
    let exact = PackagePolicyRecoveryLimits::new(
        expected.len(),
        expected.len(),
        usage.sequence_elements(),
        usage.owned_bytes(),
        128,
    );
    assert_eq!(
        PackagePolicyBaseline::recover_canonical_with_usage(&expected, exact).unwrap(),
        (value, usage),
    );
    assert_eq!(
        PackagePolicyBaseline::recover_canonical_with_usage(
            &expected,
            PackagePolicyRecoveryLimits::new(
                expected.len(),
                expected.len(),
                usage.sequence_elements(),
                usage.owned_bytes() - 1,
                128,
            )
        ),
        Err(Error::AllocationLimitExceeded),
    );
}

#[test]
fn individually_valid_baselines_cannot_reset_enclosing_storage_or_elements() {
    let first = fixture();
    let mut second = first.clone();
    second.public_api.consts[0].canonical_value_encoding = "43".to_owned();
    let first_text = first.canonical_text().unwrap();
    let second_text = second.canonical_text().unwrap();
    let binary_bytes = first.canonical_bytes().unwrap().len();
    assert_eq!(binary_bytes, second.canonical_bytes().unwrap().len());
    let generous = limits(&first_text, binary_bytes, 65_536, 64 * 1024 * 1024);
    let (_, usage) = PackagePolicyBaseline::recover_text_with_usage(&first_text, generous).unwrap();
    assert_eq!(
        PackagePolicyBaseline::recover_text_with_usage(&second_text, generous)
            .unwrap()
            .1,
        usage,
    );
    for (elements, owned, expected) in [
        (
            usage.sequence_elements() * 2 - 1,
            usage.owned_bytes() * 2,
            Error::ElementLimitExceeded,
        ),
        (
            usage.sequence_elements() * 2,
            usage.owned_bytes() * 2 - 1,
            Error::AllocationLimitExceeded,
        ),
    ] {
        let enclosing = limits(&first_text, binary_bytes, elements, owned);
        // Either child is independently valid with this ceiling.
        assert!(PackagePolicyBaseline::recover_text(&second_text, enclosing).is_ok());
        let (recovered, consumed) =
            PackagePolicyBaseline::recover_text_with_usage(&first_text, enclosing).unwrap();
        assert_eq!(recovered, first);
        let remaining = limits(
            &second_text,
            binary_bytes,
            elements.checked_sub(consumed.sequence_elements()).unwrap(),
            owned.checked_sub(consumed.owned_bytes()).unwrap(),
        );
        assert_eq!(
            PackagePolicyBaseline::recover_text_with_usage(&second_text, remaining),
            Err(expected)
        );
    }
    let total_owned = usage.owned_bytes() * 2;
    let total_elements = usage.sequence_elements() * 2;
    let (_, consumed) = PackagePolicyBaseline::recover_text_with_usage(
        &first_text,
        limits(&first_text, binary_bytes, total_elements, total_owned),
    )
    .unwrap();
    assert_eq!(
        PackagePolicyBaseline::recover_text_with_usage(
            &second_text,
            limits(
                &second_text,
                binary_bytes,
                total_elements - consumed.sequence_elements(),
                total_owned - consumed.owned_bytes()
            ),
        )
        .unwrap(),
        (second, usage)
    );
}

#[test]
fn reader_usage_counts_vectors_boxes_nested_entries_and_scratch_exactly() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&2u64.to_le_bytes());
    for value in [b"one", b"two"] {
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(value);
    }
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(reader.usage(), PackagePolicyRecoveryUsage::default());
    let values = reader
        .sequence(1, |reader| {
            reader.nested(|reader| reader.boxed(Reader::string))
        })
        .unwrap();
    reader.finish().unwrap();
    assert_eq!(&**values[0], "one");
    assert_eq!(&**values[1], "two");
    let typed_owned =
        2 * std::mem::size_of::<Box<String>>() + 2 * std::mem::size_of::<String>() + 6;
    assert_eq!(reader.usage().owned_bytes(), typed_owned);
    assert_eq!(reader.usage().sequence_elements(), 4);
    reader.canonical_scratch(bytes.len()).unwrap();
    assert_eq!(reader.usage().owned_bytes(), typed_owned + bytes.len());
    assert_eq!(reader.usage().sequence_elements(), 4);
}

#[test]
fn legacy_and_usage_text_entrances_reject_the_same_invalid_input() {
    let text = fixture().canonical_text().unwrap();
    for changed in [
        text.replacen("field public_api", "field private_api", 1),
        text.replacen("tag layout 1", "tag layout 255", 1),
        text[..text.len() - 1].to_owned(),
        format!("{text}\n"),
    ] {
        let limits = PackagePolicyTextRecoveryLimits::default();
        let legacy = PackagePolicyBaseline::recover_text(&changed, limits);
        assert!(legacy.is_err());
        assert_eq!(
            PackagePolicyBaseline::recover_text_with_usage(&changed, limits)
                .map(|(policy, _)| policy),
            legacy,
        );
    }
}

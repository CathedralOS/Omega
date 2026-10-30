use crate::{Record, RecordLimits, decode, encode};
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, Record)]
enum Payload {
    Empty,
    Count(u64),
    Named { name: Arc<str>, data: Vec<u16> },
}

#[derive(Debug, Record)]
struct Shared {
    first: Arc<Payload>,
    second: Arc<Payload>,
    #[record(skip)]
    derived: Vec<usize>,
}

#[test]
fn shares_one_graph_object_and_rebuilds_derived_state() {
    let value = Arc::new(Payload::Named {
        name: Arc::from("declaration"),
        data: vec![1, 2, 65535],
    });
    let original = Shared {
        first: Arc::clone(&value),
        second: value,
        derived: vec![3, 9],
    };
    let bytes = encode(&original, RecordLimits::default()).unwrap();
    let restored: Shared = decode(&bytes, RecordLimits::default()).unwrap();
    assert_eq!(restored.first, original.first);
    assert!(Arc::ptr_eq(&restored.first, &restored.second));
    assert!(restored.derived.is_empty());
}

#[test]
fn rejects_every_truncation_trailing_data_and_foreign_variants() {
    let value = Payload::Named {
        name: Arc::from("source"),
        data: vec![4, 7],
    };
    let bytes = encode(&value, RecordLimits::default()).unwrap();
    for length in 0..bytes.len() {
        assert!(decode::<Payload>(&bytes[..length], RecordLimits::default()).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode::<Payload>(&trailing, RecordLimits::default()).is_err());
    let mut foreign = bytes;
    foreign[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode::<Payload>(&foreign, RecordLimits::default()).is_err());
}

#[test]
fn rejects_allocation_abuse_and_unpublished_shared_references() {
    let bytes = u64::MAX.to_le_bytes();
    assert!(decode::<Vec<u64>>(&bytes, RecordLimits::default()).is_err());
    let mut reference = 1u64.to_le_bytes().to_vec();
    reference.push(0);
    assert!(decode::<Arc<u64>>(&reference, RecordLimits::default()).is_err());
    let mut definition = 2u64.to_le_bytes().to_vec();
    definition.push(1);
    definition.extend_from_slice(&4u64.to_le_bytes());
    assert!(decode::<Arc<u64>>(&definition, RecordLimits::default()).is_err());
}

#[derive(Debug, Record)]
struct Link(Option<Box<Link>>);

#[test]
fn bounds_both_encoding_and_decoding_depth() {
    let value = Link(Some(Box::new(Link(Some(Box::new(Link(None)))))));
    let limits = RecordLimits {
        depth: 3,
        ..RecordLimits::default()
    };
    assert!(encode(&value, limits).is_err());
    let bytes = encode(&value, RecordLimits::default()).unwrap();
    assert!(decode::<Link>(&bytes, limits).is_err());
}

#[test]
fn fixed_width_sequences_keep_little_endian_bits_and_shared_array_identity() {
    let values = [0_u32, 1, u32::MAX, 0x10203040];
    let mut bytes = 4_u64.to_le_bytes().to_vec();
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(
        decode::<Vec<u32>>(&bytes, RecordLimits::default()).unwrap(),
        values
    );
    assert_eq!(
        decode::<[u32; 4]>(&bytes, RecordLimits::default()).unwrap(),
        values
    );
    assert_eq!(encode(&values, RecordLimits::default()).unwrap(), bytes);
    let shared = Arc::new([0_u8, 7, 128, 255]);
    let original = (shared.clone(), shared);
    let restored: (Arc<[u8; 4]>, Arc<[u8; 4]>) = decode(
        &encode(&original, RecordLimits::default()).unwrap(),
        RecordLimits::default(),
    )
    .unwrap();
    assert_eq!(*restored.0, [0, 7, 128, 255]);
    assert!(Arc::ptr_eq(&restored.0, &restored.1));
    let bits = [
        0x7ff8000000000001_u64,
        0x8000000000000000,
        0xfff0000000000000,
    ];
    let floats = bits.map(f64::from_bits);
    let restored: [f64; 3] = decode(
        &encode(&floats, RecordLimits::default()).unwrap(),
        RecordLimits::default(),
    )
    .unwrap();
    assert_eq!(restored.map(f64::to_bits), bits);
    let signed = [i16::MIN, -1, 0, i16::MAX];
    assert_eq!(
        decode::<[i16; 4]>(
            &encode(&signed, RecordLimits::default()).unwrap(),
            RecordLimits::default()
        )
        .unwrap(),
        signed
    );
}

#[test]
fn fixed_width_sequences_keep_size_budget_and_value_refusals() {
    let bytes = encode(&[1_u32, 2, 3], RecordLimits::default()).unwrap();
    for length in 0..bytes.len() {
        assert!(decode::<[u32; 3]>(&bytes[..length], RecordLimits::default()).is_err());
        assert!(decode::<Vec<u32>>(&bytes[..length], RecordLimits::default()).is_err());
    }
    assert!(decode::<[u32; 2]>(&bytes, RecordLimits::default()).is_err());
    for limits in [
        RecordLimits {
            allocation: 11,
            ..RecordLimits::default()
        },
        RecordLimits {
            elements: 2,
            ..RecordLimits::default()
        },
    ] {
        assert!(decode::<[u32; 3]>(&bytes, limits).is_err());
        assert!(decode::<Vec<u32>>(&bytes, limits).is_err());
    }
    let mut invalid_bool = 2_u64.to_le_bytes().to_vec();
    invalid_bool.extend_from_slice(&[0, 2]);
    assert!(decode::<Vec<bool>>(&invalid_bool, RecordLimits::default()).is_err());
    assert!(decode::<[bool; 2]>(&invalid_bool, RecordLimits::default()).is_err());
    assert_eq!(
        decode::<[u32; 0]>(&0_u64.to_le_bytes(), RecordLimits::default()).unwrap(),
        []
    );
}

#[test]
fn generation_domains_preserve_semantic_handles_and_separate_equal_old_tags() {
    struct Text;
    struct OtherText;
    struct Semantic;
    let mut input = crate::RecordReader::<()>::new(&[], RecordLimits::default()).unwrap();
    input.remap_generations::<Text>();
    assert_eq!(input.generation::<Semantic>(3).unwrap(), 3);
    assert!(input.generation::<Text>(3).is_err());
    assert_eq!(
        input
            .define_generation::<Text, _>(3, &"source", || Ok(11))
            .unwrap(),
        11
    );
    assert_eq!(
        input
            .define_generation::<OtherText, _>(3, &"other", || Ok(22))
            .unwrap(),
        22
    );
    input.remap_generations::<Text>();
    assert_eq!(
        input
            .define_generation::<Text, _>(3, &"source", || panic!(
                "must reuse the existing correspondence"
            ))
            .unwrap(),
        11
    );
    assert!(
        input
            .define_generation::<Text, _>(3, &"changed", || Ok(33))
            .is_err()
    );
    assert!(
        input
            .define_generation::<Text, _>(3, &17_u32, || Ok(33))
            .is_err()
    );
    assert_eq!(input.generation::<Text>(3).unwrap(), 11);
    assert_eq!(input.generation::<OtherText>(3).unwrap(), 22);
    assert_eq!(input.generation::<Semantic>(3).unwrap(), 3);
}

#[test]
fn failed_generation_publication_does_not_register_or_publish_a_domain() {
    struct Text;
    for limits in [
        RecordLimits {
            allocation: 0,
            ..RecordLimits::default()
        },
        RecordLimits {
            elements: 0,
            ..RecordLimits::default()
        },
    ] {
        let mut input = crate::RecordReader::<()>::new(&[], limits).unwrap();
        assert!(
            input
                .define_generation::<Text, _>(3, &"source", || panic!(
                    "budget must refuse before allocating a generation"
                ))
                .is_err()
        );
        assert_eq!(input.generation::<Text>(3).unwrap(), 3);
    }
    let mut input = crate::RecordReader::<()>::new(&[], RecordLimits::default()).unwrap();
    assert!(
        input
            .define_generation::<Text, _>(3, &"source", || Err("generation exhausted"))
            .is_err()
    );
    assert_eq!(input.generation::<Text>(3).unwrap(), 3);
    input.remap_generations::<Text>();
    assert!(
        input
            .define_generation::<Text, _>(3, &"source", || Err("generation exhausted"))
            .is_err()
    );
    assert!(input.generation::<Text>(3).is_err());
}

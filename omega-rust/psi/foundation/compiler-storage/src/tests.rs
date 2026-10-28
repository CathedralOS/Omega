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

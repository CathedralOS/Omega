use super::{ServiceNormalResultMembership, ServiceResultProjection};
use record_storage::{Record, decode, encode};
use semantic_vocabulary::{
    ContentPlaceSegment, ContentPlaceVersion, ContentStructuralPlace, PlaceId,
};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

// Reference declarations from 1b54028582c. Rust derives the expected ordering
// and Hasher calls independently of the current shared segment implementation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Record)]
enum OriginalContentSegment {
    Case(String),
    Field(String),
    FixedIndex(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum OriginalServiceSegment {
    Field(String),
    Case(String),
    FixedIndex(u64),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Record)]
struct OriginalContentPlace {
    version: ContentPlaceVersion,
    root: PlaceId,
    segments: Vec<OriginalContentSegment>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
struct OriginalMembership {
    path: Vec<OriginalServiceSegment>,
    domain: String,
}

#[derive(Default)]
struct HashInput(Vec<u8>);

impl Hasher for HashInput {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
    fn write_isize(&mut self, ordinal: isize) {
        self.0.push(0xdb);
        self.write(&ordinal.to_ne_bytes());
    }
}

fn hash_input(value: &impl Hash) -> Vec<u8> {
    let mut input = HashInput::default();
    value.hash(&mut input);
    input.0
}

fn check_comparison_and_hash<O: Ord + Hash, N: Ord + Hash>(original: &[O], current: &[N]) {
    assert_eq!(original.len(), current.len());
    for (left, new_left) in original.iter().zip(current) {
        assert_eq!(hash_input(left), hash_input(new_left));
        for (right, new_right) in original.iter().zip(current) {
            assert_eq!(left.cmp(right), new_left.cmp(new_right));
        }
    }
}

fn check_record<O: Record + Eq + Debug, N: Record + Eq + Debug>(original: &[O], current: &[N]) {
    assert_eq!(original.len(), current.len());
    for (old, new) in original.iter().zip(current) {
        let previous = encode(old, Default::default()).unwrap();
        let latest = encode(new, Default::default()).unwrap();
        assert_eq!(previous, latest);
        assert_eq!(&decode::<N>(&previous, Default::default()).unwrap(), new);
        assert_eq!(&decode::<O>(&latest, Default::default()).unwrap(), old);
    }
}

fn samples<S>(field: fn(String) -> S, case: fn(String) -> S, index: fn(u64) -> S) -> Vec<S> {
    vec![
        field("a".into()),
        field("z".into()),
        field("Ω".into()),
        case("a".into()),
        case("z".into()),
        index(0),
        index(2),
        index(u64::MAX),
    ]
}

fn paths<S: Clone>(segments: &[S]) -> Vec<Vec<S>> {
    let mut paths = vec![vec![]];
    for first in segments {
        paths.push(vec![first.clone()]);
        for second in segments {
            paths.push(vec![first.clone(), second.clone()]);
        }
    }
    paths
}

#[test]
fn content_segment_preserves_historical_order_hash_and_records() {
    use ContentPlaceSegment as Current;
    use OriginalContentSegment as Old;
    let original = samples(Old::Field, Old::Case, Old::FixedIndex);
    let current = samples(Current::Field, Current::Case, Current::FixedIndex);
    check_comparison_and_hash(&original, &current);
    check_record(&original, &current);

    let original = paths(&original);
    let current = paths(&current);
    check_comparison_and_hash(&original, &current);
    check_record(&original, &current);

    let version = ContentPlaceVersion::Entry;
    let root = PlaceId::new(1).unwrap();
    let original: Vec<_> = original
        .into_iter()
        .map(|segments| OriginalContentPlace {
            version,
            root,
            segments,
        })
        .collect();
    let current: Vec<_> = current
        .into_iter()
        .map(|segments| ContentStructuralPlace {
            version,
            root,
            segments,
        })
        .collect();
    check_comparison_and_hash(&original, &current);
    check_record(&original, &current);
}

#[test]
fn service_segment_preserves_historical_order_and_hash() {
    use OriginalServiceSegment as Old;
    use ServiceResultProjection as Current;
    let original = samples(Old::Field, Old::Case, Old::FixedIndex);
    let current = samples(Current::Field, Current::Case, Current::FixedIndex);
    check_comparison_and_hash(&original, &current);

    let original = paths(&original);
    let current = paths(&current);
    check_comparison_and_hash(&original, &current);

    let mut memberships = Vec::new();
    let mut current_memberships = Vec::new();
    for (old, new) in original.iter().zip(&current) {
        for domain in ["", "scalar", "Ω(domain)"] {
            memberships.push(OriginalMembership {
                path: old.clone(),
                domain: domain.into(),
            });
            current_memberships.push(ServiceNormalResultMembership {
                path: new.clone(),
                domain: domain.into(),
            });
        }
    }
    check_comparison_and_hash(&memberships, &current_memberships);
}

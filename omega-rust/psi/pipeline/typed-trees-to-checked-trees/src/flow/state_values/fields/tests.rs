use super::*;

fn field(symbol: u32, literal: u32, predicates: Vec<ByteSequencePredicate>) -> FieldValue {
    FieldValue {
        segments: vec![facts::PlaceSegment::Field {
            symbol: SymbolHandle::from_arena_index(symbol),
        }],
        literal: ExpressionHandle::from_arena_index(literal),
        predicates,
    }
}

#[test]
fn field_meet_loses_literal_identity_but_keeps_common_predicates() {
    let predicates = ByteSequencePredicate::ALL.to_vec();
    let mut previous = vec![field(1, 2, predicates.clone())];
    assert!(meet(&mut previous, &[field(1, 3, predicates.clone())]));
    assert!(!previous[0].literal.is_valid());
    assert_eq!(previous[0].predicates, predicates);
    assert!(!meet(&mut previous, &[field(1, 2, predicates.clone())]));
    assert!(!previous[0].literal.is_valid());
}

#[test]
fn missing_field_evidence_is_absorbing_and_exact_paths_do_not_merge() {
    let original = vec![field(1, 2, ByteSequencePredicate::ALL.to_vec())];
    let mut previous = original.clone();
    assert!(meet(
        &mut previous,
        &[field(2, 2, ByteSequencePredicate::ALL.to_vec())]
    ));
    assert!(previous.is_empty());
    assert!(!meet(&mut previous, &original));
    assert!(previous.is_empty());
}

#[test]
fn field_height_bounds_each_monotone_evidence_loss() {
    let mut previous = vec![field(1, 2, ByteSequencePredicate::ALL.to_vec())];
    let initial_height = height(&previous);
    let mut losses = 0;
    let mut incoming = previous.clone();
    incoming[0].literal = ExpressionHandle::invalid();
    losses += usize::from(meet(&mut previous, &incoming));
    while incoming[0].predicates.pop().is_some() {
        losses += usize::from(meet(&mut previous, &incoming));
    }
    assert_eq!(losses, initial_height);
    assert!(previous.is_empty());
}

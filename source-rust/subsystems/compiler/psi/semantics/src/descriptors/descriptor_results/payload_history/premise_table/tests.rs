use super::*;
use semantic_vocabulary::ScalarType;

fn condition(value: u64) -> Proposition {
    Proposition::Equal(
        ScalarTerm::Boolean(true),
        ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Boolean),
    )
}

#[test]
fn handle_order_matches_proposition_order_through_renumbering() {
    let mut table = PremiseTable::default();
    let mut values = Vec::new();
    let (mut low, mut high) = (1_000u64, 1_001u64);
    for round in 0..80 {
        if round % 2 == 0 {
            low -= 1;
            values.push(low);
        } else {
            high += 1;
            values.push(high);
        }
    }
    values.extend((0..64).map(|step| 500 + step));
    let handles = values
        .iter()
        .map(|value| table.intern(condition(*value)))
        .collect::<Vec<_>>();
    let mut by_handle = handles.clone();
    by_handle.sort();
    let mut by_proposition = values
        .iter()
        .map(|value| condition(*value))
        .collect::<Vec<_>>();
    by_proposition.sort();
    assert_eq!(
        by_handle
            .iter()
            .map(|premise| premise.as_ref().clone())
            .collect::<Vec<_>>(),
        by_proposition
    );
    for (value, handle) in values.iter().zip(&handles) {
        assert!(table.intern(condition(*value)) == *handle);
        assert!(table.intern_ref(&condition(*value)) == *handle);
    }
    assert!(handles[0] != handles[1]);
}

#[test]
fn arrival_keeps_unrelated_rows_and_substitutes_rebound_values() {
    let mut table = PremiseTable::default();
    let unrelated = table.intern(condition(1));
    let rebound = table.intern(condition(2));
    let retired = table.intern(condition(3));
    let actual = ValueId::new(2).unwrap();
    let formal = ValueId::new(20).unwrap();
    let arrived = table.arrival_requirements(
        &[unrelated.clone(), rebound, retired],
        &[(formal, ScalarTerm::value(actual, ScalarType::Boolean))],
        &[ValueId::new(3).unwrap(), formal],
    );
    assert!(arrived[0] == unrelated);
    assert_eq!(arrived[1].as_ref(), &condition(20));
    assert_eq!(
        arrived[2..]
            .iter()
            .map(|premise| premise.as_ref().clone())
            .collect::<Vec<_>>(),
        vec![
            Proposition::Equal(
                ScalarTerm::value(formal, ScalarType::Boolean),
                ScalarTerm::value(formal, ScalarType::Boolean),
            ),
            Proposition::Equal(
                ScalarTerm::value(formal, ScalarType::Boolean),
                ScalarTerm::value(actual, ScalarType::Boolean),
            ),
        ]
    );
    assert_eq!(arrived.len(), 4);
}

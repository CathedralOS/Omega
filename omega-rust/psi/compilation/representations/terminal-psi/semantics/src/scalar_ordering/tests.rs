use super::*;

fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}
fn position(index: u32) -> Position {
    (BlockId::new(1).unwrap(), index)
}
fn index_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}
fn edge(target: usize) -> Edge {
    Edge {
        target,
        enters_block: false,
        parameters: Vec::new(),
        bindings: Vec::new(),
        condition: None,
    }
}
fn literals() -> Vec<Step> {
    vec![
        Step {
            position: position(0),
            definitions: vec![(value(1), index_type())],
            effect: Effect::UnsignedLiteral {
                result: value(1),
                value: 2,
            },
            successors: vec![edge(1)],
        },
        Step {
            position: position(1),
            definitions: vec![(value(2), index_type())],
            effect: Effect::UnsignedLiteral {
                result: value(2),
                value: 4,
            },
            successors: vec![edge(2)],
        },
        Step {
            position: position(2),
            definitions: Vec::new(),
            effect: Effect::None,
            successors: Vec::new(),
        },
    ]
}

#[test]
fn unsigned_literal_ordering_requires_exact_index_types_and_values() {
    let ordering = ScalarOrdering::new(literals(), position(0), Vec::new());
    assert!(ordering.proves_less_than(position(2), value(1), value(2)));
    assert!(ordering.proves_less_or_equal(position(2), value(1), value(1)));
    assert!(!ordering.proves_less_or_equal(position(2), value(2), value(1)));
    for scalar_type in [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
    ] {
        let mut steps = literals();
        steps[0].definitions[0].1 = scalar_type;
        let ordering = ScalarOrdering::new(steps, position(0), Vec::new());
        assert!(!ordering.proves_less_than(position(2), value(1), value(2)));
    }
    let mut steps = literals();
    steps[0].effect = Effect::UnsignedLiteral {
        result: value(1),
        value: u128::from(u64::MAX) + 1,
    };
    let ordering = ScalarOrdering::new(steps, position(0), Vec::new());
    assert!(!ordering.proves_less_than(position(2), value(1), value(2)));
}

#[test]
fn literal_values_follow_simultaneous_bindings_and_redefinition() {
    let mut steps = literals();
    steps[1].successors[0] = Edge {
        target: 2,
        enters_block: true,
        parameters: vec![(value(3), index_type()), (value(4), index_type())],
        bindings: vec![
            Binding {
                parameter: value(3),
                argument: value(2),
                scalar_type: index_type(),
            },
            Binding {
                parameter: value(4),
                argument: value(1),
                scalar_type: index_type(),
            },
        ],
        condition: None,
    };
    steps[2].definitions = vec![(value(4), index_type())];
    steps[2].effect = Effect::UnsignedLiteral {
        result: value(4),
        value: 5,
    };
    steps[2].successors.push(edge(3));
    steps.push(Step {
        position: position(3),
        definitions: Vec::new(),
        effect: Effect::None,
        successors: Vec::new(),
    });
    let ordering = ScalarOrdering::new(steps, position(0), Vec::new());
    assert!(ordering.proves_less_than(position(2), value(4), value(3)));
    assert!(!ordering.proves_less_than(position(3), value(4), value(3)));
    assert!(ordering.proves_less_than(position(3), value(3), value(4)));
    assert!(!ordering.values_unchanged_since(position(2), position(3), &[value(4)]));
}

#[test]
fn a_join_does_not_keep_a_literal_defined_on_only_one_arrival() {
    let mut steps = literals();
    steps[1].successors = vec![edge(2), edge(3)];
    steps[2].definitions = vec![(value(1), index_type())];
    steps[2].effect = Effect::None;
    steps[2].successors = vec![edge(4)];
    steps.push(Step {
        position: position(3),
        definitions: Vec::new(),
        effect: Effect::None,
        successors: vec![edge(4)],
    });
    steps.push(Step {
        position: position(4),
        definitions: Vec::new(),
        effect: Effect::None,
        successors: Vec::new(),
    });
    let ordering = ScalarOrdering::new(steps, position(0), Vec::new());
    assert!(!ordering.proves_less_than(position(4), value(1), value(2)));
    assert!(!ordering.values_unchanged_since(position(1), position(4), &[value(1)]));
}

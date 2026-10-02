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

fn lower_bound(identity: u64, bound: u128) -> Proposition {
    let ScalarType::Integer(scalar_type) = index_type() else {
        unreachable!()
    };
    Proposition::LessOrEqual(
        semantic_vocabulary::ScalarTerm::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(bound),
        },
        semantic_vocabulary::ScalarTerm::Value {
            id: value(identity),
            scalar_type: index_type(),
        },
    )
}

#[test]
fn entry_ordering_uses_only_exact_typed_unconditional_contract_bounds() {
    let mut steps = literals();
    steps[0].effect = Effect::UnsignedLiteral {
        result: value(1),
        value: 0,
    };
    let bound = lower_bound(3, 2);
    let ordering = ScalarOrdering::with_contracts(
        steps.clone(),
        position(0),
        vec![(value(3), index_type())],
        std::slice::from_ref(&bound),
        Vec::new(),
    );
    assert!(ordering.proves_less_than(position(2), value(1), value(3)));
    for uncertain in [
        Proposition::Disjunction(vec![bound.clone(), Proposition::Truth]),
        Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(bound.clone()),
        },
        lower_bound(9, 2),
    ] {
        let ordering = ScalarOrdering::with_contracts(
            steps.clone(),
            position(0),
            vec![(value(3), index_type())],
            &[uncertain],
            Vec::new(),
        );
        assert!(!ordering.proves_less_than(position(2), value(1), value(3)));
    }
    let ordering = ScalarOrdering::with_contracts(
        steps,
        position(0),
        vec![(value(3), ScalarType::Boolean)],
        &[bound],
        Vec::new(),
    );
    assert!(!ordering.proves_less_than(position(2), value(1), value(3)));
}

#[test]
fn call_ordering_begins_after_result_definition_and_ends_at_redefinition() {
    let mut steps = literals();
    steps[0].effect = Effect::UnsignedLiteral {
        result: value(1),
        value: 0,
    };
    steps[1].definitions = vec![(value(3), index_type())];
    steps[1].effect = Effect::None;
    steps[2].definitions = vec![(value(3), index_type())];
    steps[2].effect = Effect::UnsignedLiteral {
        result: value(3),
        value: 0,
    };
    steps[2].successors = vec![edge(3)];
    steps.push(Step {
        position: position(3),
        definitions: Vec::new(),
        effect: Effect::None,
        successors: Vec::new(),
    });
    let ordering = ScalarOrdering::with_contracts(
        steps,
        position(0),
        Vec::new(),
        &[],
        vec![(position(1), vec![lower_bound(3, 2)])],
    );
    assert!(!ordering.proves_less_than(position(1), value(1), value(3)));
    assert!(ordering.proves_less_than(position(2), value(1), value(3)));
    assert!(!ordering.proves_less_than(position(3), value(1), value(3)));
}

#[test]
fn contract_ordering_requires_every_arrival_and_survives_exact_alias_transport() {
    let mut steps = literals();
    steps[0].effect = Effect::UnsignedLiteral {
        result: value(1),
        value: 0,
    };
    steps[1].successors = vec![edge(2), edge(3)];
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
    let ordering = ScalarOrdering::with_contracts(
        steps.clone(),
        position(0),
        vec![(value(3), index_type())],
        &[],
        vec![(position(2), vec![lower_bound(3, 2)])],
    );
    assert!(!ordering.proves_less_than(position(4), value(1), value(3)));
    for predecessor in [2, 3] {
        steps[predecessor].successors[0] = Edge {
            target: 4,
            enters_block: true,
            parameters: vec![(value(4), index_type())],
            bindings: vec![Binding {
                parameter: value(4),
                argument: value(3),
                scalar_type: index_type(),
            }],
            condition: None,
        };
    }
    let ordering = ScalarOrdering::with_contracts(
        steps,
        position(0),
        vec![(value(3), index_type())],
        &[lower_bound(3, 2)],
        Vec::new(),
    );
    assert!(ordering.proves_less_than(position(4), value(1), value(4)));
}

#[test]
fn contract_ordering_substitution_never_retains_an_unmapped_callee_identity() {
    use semantic_vocabulary::ScalarTerm;
    let actual = |identity, scalar_type| ScalarTerm::Value {
        id: value(identity),
        scalar_type,
    };
    let fact = lower_bound(3, 2);
    assert_eq!(
        instantiate_contract_ordering(&fact, &[(value(3), actual(9, index_type()))]),
        vec![lower_bound(9, 2)]
    );
    for bindings in [
        vec![],
        vec![(value(2), actual(3, index_type()))],
        vec![(value(3), actual(9, ScalarType::Boolean))],
        vec![
            (value(3), actual(9, index_type())),
            (value(3), actual(10, index_type())),
        ],
    ] {
        assert!(instantiate_contract_ordering(&fact, &bindings).is_empty());
    }
    let Proposition::LessOrEqual(constant, _) = lower_bound(9, 4) else {
        unreachable!()
    };
    let mapped = instantiate_contract_ordering(&fact, &[(value(3), constant.clone())]);
    assert!(matches!(&mapped[..], [Proposition::LessOrEqual(_, right)] if *right == constant));
}

#[test]
fn fixed_indices_use_current_window_bounds_without_inventing_scalar_values() {
    use crate::borrowed_storage::{Projection, Region, regions_overlap_at};
    use semantic_vocabulary::PlaceId;
    use terminal_psi::StructuralPathSegment;

    let mut steps = literals();
    steps[2].successors = vec![edge(3)];
    steps.push(Step {
        position: position(3),
        definitions: Vec::new(),
        effect: Effect::None,
        successors: Vec::new(),
    });
    let ordering = ScalarOrdering::with_contracts(
        steps.clone(),
        position(0),
        vec![(value(3), index_type())],
        &[lower_bound(3, 2)],
        Vec::new(),
    );
    let prefix = Projection::new(&[StructuralPathSegment::Field("items".into())]);
    let window = Region {
        root: Some(PlaceId::new(1).unwrap()),
        projection: prefix.then(&Projection::window(value(3), value(2), position(2))),
    };
    for (index, overlaps) in [(0, false), (1, false), (2, true), (3, true), (4, false)] {
        let selected = Region {
            root: window.root,
            projection: prefix.then(&Projection::new(&[StructuralPathSegment::FixedIndex(
                index,
            )])),
        };
        assert_eq!(
            regions_overlap_at(&window, &selected, position(3), &ordering),
            overlaps
        );
        let mut indirect = selected.clone();
        indirect.projection.indirect = true;
        assert!(regions_overlap_at(
            &window,
            &indirect,
            position(3),
            &ordering
        ));
    }
    steps[2].definitions = vec![(value(3), index_type())];
    let changed = ScalarOrdering::with_contracts(
        steps,
        position(0),
        vec![(value(3), index_type())],
        &[lower_bound(3, 2)],
        Vec::new(),
    );
    let selected = Region {
        root: window.root,
        projection: prefix.then(&Projection::new(&[StructuralPathSegment::FixedIndex(0)])),
    };
    assert!(regions_overlap_at(
        &window,
        &selected,
        position(3),
        &changed
    ));
}

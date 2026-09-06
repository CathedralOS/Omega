use super::*;

fn place(root: u32, projections: &[u32]) -> ProgressSubject {
    ProgressSubject {
        root: symbol(root),
        projections: projections.iter().copied().map(symbol).collect(),
    }
}

fn edge(source: ProgressSubject, destination: ProgressSubject) -> transfers::ParameterTransfer {
    transfers::ParameterTransfer {
        destination,
        source: Some(source),
    }
}

fn close(
    values: Vec<(ProgressSubject, ParameterLineage)>,
    mut transfers: Vec<transfers::ParameterTransfer>,
    reversed: bool,
) -> StateParameterLineage {
    if reversed {
        transfers.reverse();
    }
    StateParameterLineage::close(values, transfers)
}

fn retained<'lineage>(
    lineage: &'lineage StateParameterLineage,
    place: &ProgressSubject,
) -> &'lineage ParameterLineage {
    &lineage
        .values
        .iter()
        .find(|(candidate, _)| candidate == place)
        .expect("catalogued place")
        .1
}

fn assert_exact(
    lineage: &StateParameterLineage,
    place: &ProgressSubject,
    expected: &[ProgressSubject],
) {
    let ParameterLineage::Exact(actual) = retained(lineage, place) else {
        panic!("expected exact origins for {place:?}");
    };
    // Predecessor order may change insertion order, but never the finite set.
    assert_eq!(actual.len(), expected.len(), "origins for {place:?}");
    for expected in expected {
        assert!(
            actual.contains(expected),
            "missing {expected:?} in {actual:?}"
        );
    }
}

#[test]
fn field_swap_retains_both_origins_under_an_ambiguous_aggregate() {
    for reversed in [false, true] {
        let left = place(1, &[100]);
        let right = place(1, &[101]);
        let lineage = close(
            vec![
                (subject(1), exact(1)),
                (left.clone(), ParameterLineage::Exact(vec![left.clone()])),
                (right.clone(), ParameterLineage::Exact(vec![right.clone()])),
                (subject(2), ParameterLineage::Ambiguous),
                (place(2, &[100]), ParameterLineage::Unseen),
                (place(2, &[101]), ParameterLineage::Unseen),
            ],
            vec![
                edge(left.clone(), place(2, &[100])),
                edge(right.clone(), place(2, &[101])),
                edge(place(2, &[100]), place(2, &[101])),
                edge(place(2, &[101]), place(2, &[100])),
            ],
            reversed,
        );

        assert_exact(&lineage, &place(2, &[100]), &[left.clone(), right.clone()]);
        assert_exact(&lineage, &place(2, &[101]), &[left, right]);
        assert_eq!(
            retained(&lineage, &subject(2)),
            &ParameterLineage::Ambiguous
        );
    }
}

#[test]
fn temporary_prefix_growth_consumed_by_a_catalogue_key_has_finite_union() {
    // The cycle transports p.a into q.b.a and then back into p.a. The longer
    // concrete key consumes b.a; no residual projection accumulates per lap.
    for reversed in [false, true] {
        let first = place(1, &[100]);
        let second = place(1, &[102]);
        let lineage = close(
            vec![
                (subject(1), exact(1)),
                (first.clone(), ParameterLineage::Exact(vec![first.clone()])),
                (
                    second.clone(),
                    ParameterLineage::Exact(vec![second.clone()]),
                ),
                (subject(2), ParameterLineage::Ambiguous),
                (place(2, &[100]), ParameterLineage::Unseen),
                (subject(3), ParameterLineage::Ambiguous),
                (place(3, &[101]), ParameterLineage::Ambiguous),
                (place(3, &[101, 100]), ParameterLineage::Unseen),
                (subject(4), ParameterLineage::Unseen),
            ],
            vec![
                edge(first.clone(), place(2, &[100])),
                edge(second.clone(), place(3, &[101, 100])),
                edge(place(2, &[100]), place(3, &[101, 100])),
                edge(place(3, &[101, 100]), place(2, &[100])),
                edge(place(3, &[101, 100, 103]), subject(4)),
            ],
            reversed,
        );

        assert_exact(
            &lineage,
            &place(2, &[100]),
            &[first.clone(), second.clone()],
        );
        assert_exact(&lineage, &place(3, &[101, 100]), &[first, second]);
        assert_exact(
            &lineage,
            &subject(4),
            &[place(1, &[100, 103]), place(1, &[102, 103])],
        );
    }
}

#[test]
fn unseen_or_unknown_child_never_falls_back_to_an_exact_ancestor() {
    for child in [ParameterLineage::Unseen, ParameterLineage::Ambiguous] {
        for reversed_catalogue in [false, true] {
            for reversed in [false, true] {
                let mut values = vec![
                    (subject(1), exact(1)),
                    (place(1, &[100]), exact(5)),
                    (place(1, &[100, 101]), child.clone()),
                    (subject(2), exact(7)),
                    (subject(3), ParameterLineage::Unseen),
                ];
                if reversed_catalogue {
                    values.reverse();
                }
                let lineage = close(
                    values,
                    vec![
                        edge(place(1, &[100, 101]), subject(2)),
                        edge(place(1, &[100, 101, 102]), subject(3)),
                    ],
                    reversed,
                );

                let expected_join = if child == ParameterLineage::Unseen {
                    exact(7)
                } else {
                    ParameterLineage::Ambiguous
                };
                assert_eq!(retained(&lineage, &subject(2)), &expected_join);
                assert_eq!(retained(&lineage, &subject(3)), &child);
                for demand in [place(1, &[100, 101]), place(1, &[100, 101, 102])] {
                    assert_eq!(resolve_subject_lineage(&lineage.values, demand), child);
                }
            }
        }
    }
}

#[test]
fn growing_reference_leaf_preserves_reset_and_finitely_projected_siblings() {
    for reversed in [false, true] {
        let lineage = close(
            vec![
                (subject(1), exact(1)),
                (
                    place(1, &[100]),
                    ParameterLineage::Exact(vec![place(1, &[100])]),
                ),
                (
                    place(1, &[101]),
                    ParameterLineage::Exact(vec![place(1, &[101])]),
                ),
                (subject(2), ParameterLineage::Ambiguous),
                (place(2, &[100]), ParameterLineage::Unseen),
                (place(2, &[101]), ParameterLineage::Unseen),
                (place(2, &[103]), ParameterLineage::Unseen),
                (subject(3), ParameterLineage::Ambiguous),
                (place(3, &[100]), ParameterLineage::Unseen),
                (place(3, &[101]), ParameterLineage::Unseen),
            ],
            vec![
                edge(place(1, &[100]), place(2, &[100])),
                edge(place(2, &[100, 100]), place(2, &[100])),
                edge(place(2, &[100]), place(3, &[100])),
                edge(place(1, &[101]), place(2, &[101])),
                edge(place(2, &[101]), place(3, &[101])),
                edge(place(3, &[101]), place(2, &[101])),
                // Root 2 reaches root 3 elsewhere, but this projected sibling
                // has no return path to its source key. Its suffix is finite.
                edge(place(3, &[101, 102]), place(2, &[103])),
            ],
            reversed,
        );

        for root in [2, 3] {
            assert_eq!(
                retained(&lineage, &place(root, &[100])),
                &ParameterLineage::Ambiguous
            );
            assert_exact(&lineage, &place(root, &[101]), &[place(1, &[101])]);
        }
        assert_exact(&lineage, &place(2, &[103]), &[place(1, &[101, 102])]);
    }
}

#[test]
fn unseeded_growing_child_does_not_borrow_its_parent_seed_or_poison_a_join() {
    for reversed in [false, true] {
        let lineage = close(
            vec![
                (subject(1), exact(1)),
                (
                    place(1, &[101]),
                    ParameterLineage::Exact(vec![place(1, &[101])]),
                ),
                (subject(2), exact(2)),
                (place(2, &[100]), ParameterLineage::Unseen),
                (subject(3), ParameterLineage::Ambiguous),
                (place(3, &[101]), ParameterLineage::Unseen),
            ],
            vec![
                edge(place(1, &[101]), place(3, &[101])),
                edge(place(2, &[100, 100]), place(2, &[100])),
                edge(place(2, &[100]), place(3, &[101])),
            ],
            reversed,
        );

        assert_eq!(
            retained(&lineage, &place(2, &[100])),
            &ParameterLineage::Unseen
        );
        assert_exact(&lineage, &subject(2), &[subject(2)]);
        assert_exact(&lineage, &place(3, &[101]), &[place(1, &[101])]);
    }
}

#[test]
fn nested_captured_origin_appends_only_the_unconsumed_suffix() {
    // The exact child row represents an already captured source. Rebinding
    // through two destination keys must neither revive the parent origin nor
    // duplicate the consumed field prefixes.
    for reversed in [false, true] {
        let lineage = close(
            vec![
                (subject(1), exact(1)),
                (place(1, &[100]), exact(5)),
                (
                    place(1, &[100, 101]),
                    ParameterLineage::Exact(vec![place(4, &[200, 201])]),
                ),
                (subject(2), ParameterLineage::Ambiguous),
                (place(2, &[300]), ParameterLineage::Ambiguous),
                (place(2, &[300, 301]), ParameterLineage::Unseen),
                (subject(3), ParameterLineage::Ambiguous),
                (place(3, &[400]), ParameterLineage::Unseen),
            ],
            vec![
                edge(place(1, &[100, 101, 102, 103]), place(2, &[300, 301])),
                edge(place(2, &[300, 301, 104]), place(3, &[400])),
            ],
            reversed,
        );

        assert_exact(
            &lineage,
            &place(2, &[300, 301]),
            &[place(4, &[200, 201, 102, 103])],
        );
        assert_exact(
            &lineage,
            &place(3, &[400]),
            &[place(4, &[200, 201, 102, 103, 104])],
        );
    }
}

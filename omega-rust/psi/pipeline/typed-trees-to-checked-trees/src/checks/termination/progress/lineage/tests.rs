use super::*;

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn subject(root: u32) -> ProgressSubject {
    ProgressSubject {
        root: symbol(root),
        projections: Vec::new(),
    }
}

fn exact(root: u32) -> ParameterLineage {
    ParameterLineage::Exact(vec![subject(root)])
}

fn initial_values(cycle_entry: ParameterLineage) -> Vec<(SymbolHandle, ParameterLineage)> {
    vec![
        (symbol(1), exact(1)),
        (symbol(2), cycle_entry),
        (symbol(3), ParameterLineage::Unseen),
        (symbol(4), ParameterLineage::Unseen),
    ]
}

fn transfer(source: u32, destination: u32, projected: bool) -> transfers::ParameterTransfer {
    let mut source = subject(source);
    if projected {
        source.projections.push(symbol(100));
    }
    transfers::ParameterTransfer {
        destination: symbol(destination),
        source: Some(source),
    }
}

fn growing_cycle_with_finite_join() -> Vec<transfers::ParameterTransfer> {
    // Parameter 1 supplies the finite join at 4. The separate 2 -> 3 -> 2
    // component appends one field per graph traversal and also feeds 4.
    vec![
        transfer(1, 4, false),
        transfer(2, 3, true),
        transfer(3, 2, false),
        transfer(3, 4, false),
    ]
}

fn value(lineage: &StateParameterLineage, parameter: u32) -> &ParameterLineage {
    &lineage
        .values
        .iter()
        .find(|(candidate, _)| *candidate == symbol(parameter))
        .expect("retained parameter")
        .1
}

#[test]
fn unseeded_growing_cycle_does_not_poison_a_finite_join() {
    for reversed in [false, true] {
        let mut transfers = growing_cycle_with_finite_join();
        if reversed {
            transfers.reverse();
        }
        let lineage =
            StateParameterLineage::close(initial_values(ParameterLineage::Unseen), transfers);

        assert_eq!(value(&lineage, 1), &exact(1));
        assert_eq!(value(&lineage, 2), &ParameterLineage::Unseen);
        assert_eq!(value(&lineage, 3), &ParameterLineage::Unseen);
        assert_eq!(value(&lineage, 4), &exact(1));
    }
}

#[test]
fn seeded_growing_cycle_makes_its_finite_join_ambiguous() {
    for reversed in [false, true] {
        let mut transfers = growing_cycle_with_finite_join();
        if reversed {
            transfers.reverse();
        }
        let lineage = StateParameterLineage::close(initial_values(exact(2)), transfers);

        assert_eq!(value(&lineage, 1), &exact(1));
        for parameter in [2, 3, 4] {
            assert_eq!(value(&lineage, parameter), &ParameterLineage::Ambiguous);
        }
    }
}

#[test]
fn unknown_incoming_activates_a_growing_cycle_and_poisons_its_join() {
    // A computed argument has no structural subject. A structural root outside
    // the retained parameters is also unknown, rather than an unseeded input.
    for source in [None, Some(subject(99))] {
        for reversed in [false, true] {
            let mut transfers = growing_cycle_with_finite_join();
            transfers.push(transfers::ParameterTransfer {
                destination: symbol(2),
                source: source.clone(),
            });
            if reversed {
                transfers.reverse();
            }
            let lineage =
                StateParameterLineage::close(initial_values(ParameterLineage::Unseen), transfers);

            assert_eq!(value(&lineage, 1), &exact(1));
            for parameter in [2, 3, 4] {
                assert_eq!(value(&lineage, parameter), &ParameterLineage::Ambiguous);
            }
        }
    }
}

#[test]
fn unknown_incoming_still_poisons_a_finite_join_without_activating_the_cycle() {
    for reversed in [false, true] {
        let mut transfers = growing_cycle_with_finite_join();
        transfers.push(transfers::ParameterTransfer {
            destination: symbol(4),
            source: None,
        });
        if reversed {
            transfers.reverse();
        }
        let lineage =
            StateParameterLineage::close(initial_values(ParameterLineage::Unseen), transfers);

        assert_eq!(value(&lineage, 1), &exact(1));
        assert_eq!(value(&lineage, 2), &ParameterLineage::Unseen);
        assert_eq!(value(&lineage, 3), &ParameterLineage::Unseen);
        assert_eq!(value(&lineage, 4), &ParameterLineage::Ambiguous);
    }
}

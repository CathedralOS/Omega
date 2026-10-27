//! The storage regression is independent of source spelling. A dense reference
//! answers the same mathematical queries while the production graph must retain
//! only real components and their shared zero. Inconsistent graphs have no finite
//! bound model; compare their contradiction verdict rather than closure history.
use super::{BigInt, Engine, Polynomial, ZERO_ATOM};

fn polynomial(name: &str) -> Polynomial {
    if name == ZERO_ATOM {
        Polynomial::default()
    } else {
        Polynomial::atom(name.to_owned())
    }
}

fn add_seed(engine: &mut Engine<'_>, from: &str, to: &str, bound: i64) {
    engine.bounds.push((
        polynomial(from).sub(&polynomial(to)),
        BigInt::from_i64(bound),
    ));
}

fn dense_closure(names: &[&str], seeds: &[(&str, &str, i64)]) -> Vec<Vec<Option<i64>>> {
    let mut bounds = vec![vec![None; names.len()]; names.len()];
    for (position, row) in bounds.iter_mut().enumerate() {
        row[position] = Some(0);
    }
    for (from, to, value) in seeds {
        let from = names.iter().position(|name| name == from).unwrap();
        let to = names.iter().position(|name| name == to).unwrap();
        bounds[from][to] = Some(bounds[from][to].map_or(*value, |old| old.max(*value)));
    }
    for via in 0..names.len() {
        for from in 0..names.len() {
            for to in 0..names.len() {
                if let (Some(left), Some(right)) = (bounds[from][via], bounds[via][to]) {
                    let combined = left + right;
                    bounds[from][to] =
                        Some(bounds[from][to].map_or(combined, |old| old.max(combined)));
                }
            }
        }
    }
    bounds
}

#[test]
fn factored_difference_refresh_matches_dense_queries_after_each_batch() {
    let program = typed_trees::TypedTrees::default();
    let mut engine = Engine::for_explicit_bindings(&program);
    let names = [ZERO_ATOM, "a", "b", "c", "d"];
    let batches: &[&[(&str, &str, i64)]] = &[
        &[("a", ZERO_ATOM, 2), (ZERO_ATOM, "a", -20)],
        &[("b", ZERO_ATOM, 4), (ZERO_ATOM, "b", -30)],
        &[("c", ZERO_ATOM, 6), (ZERO_ATOM, "c", -40)],
        &[("d", ZERO_ATOM, 8), (ZERO_ATOM, "d", -50)],
        &[("a", "b", -3), ("c", "d", -4)],
        &[("b", ZERO_ATOM, 9), (ZERO_ATOM, "d", -25)],
        &[("b", "c", -2)],
    ];
    let mut seeds = Vec::new();
    for batch in batches {
        for &(from, to, bound) in *batch {
            seeds.push((from, to, bound));
            add_seed(&mut engine, from, to, bound);
        }
        engine.refresh_difference_bounds();
        let reference = dense_closure(&names, &seeds);
        assert!(!engine.requires_unsatisfiable);
        for (from, row) in reference.iter().enumerate() {
            for (to, bound) in row.iter().enumerate() {
                assert_eq!(
                    engine.matrix_bound(names[from], names[to]),
                    bound.map(BigInt::from_i64),
                    "{} -> {} after {seeds:?}",
                    names[from],
                    names[to]
                );
            }
        }
    }
    seeds.push(("a", "b", 1));
    seeds.push(("b", "a", 1));
    add_seed(&mut engine, "a", "b", 1);
    add_seed(&mut engine, "b", "a", 1);
    engine.refresh_difference_bounds();
    let reference = dense_closure(&names, &seeds);
    assert!(
        reference
            .iter()
            .enumerate()
            .any(|(index, row)| row[index].unwrap() > 0)
    );
    assert!(engine.requires_unsatisfiable);
}

#[test]
fn independent_intervals_keep_linear_storage_and_virtual_cross_bounds() {
    let program = typed_trees::TypedTrees::default();
    let mut engine = Engine::for_explicit_bindings(&program);
    let names: Vec<String> = (0..128).map(|index| format!("value_{index:03}")).collect();
    for (index, name) in names.iter().enumerate() {
        add_seed(&mut engine, name, ZERO_ATOM, index as i64);
        add_seed(&mut engine, ZERO_ATOM, name, -(index as i64 + 100));
        engine.refresh_difference_bounds();
    }
    let cells: usize = engine
        .matrix
        .lexical_indices()
        .iter()
        .map(|index| engine.matrix.stored_targets(*index).count())
        .sum();
    assert_eq!(cells, 2 * names.len());
    for (from, from_name) in names.iter().enumerate() {
        for (to, to_name) in names.iter().enumerate() {
            let expected = if from == to {
                0
            } else {
                from as i64 - to as i64 - 100
            };
            assert_eq!(
                engine.matrix_bound(from_name, to_name),
                Some(BigInt::from_i64(expected))
            );
        }
    }
}

#[test]
fn zero_tightening_beats_a_stored_path_and_exposes_a_reverse_cycle() {
    let program = typed_trees::TypedTrees::default();
    let mut engine = Engine::for_explicit_bindings(&program);
    add_seed(&mut engine, "a", "b", -10);
    engine.refresh_difference_bounds();
    add_seed(&mut engine, "a", ZERO_ATOM, 5);
    add_seed(&mut engine, ZERO_ATOM, "b", -1);
    engine.refresh_difference_bounds();
    assert!(!engine.requires_unsatisfiable);
    assert_eq!(
        engine.matrix.stored_bound("a", "b"),
        Some(&BigInt::from_i64(-10))
    );
    assert_eq!(engine.matrix_bound("a", "b"), Some(BigInt::from_i64(4)));
    add_seed(&mut engine, "b", "a", -3);
    engine.refresh_difference_bounds();
    assert!(engine.requires_unsatisfiable);
}

#[test]
fn additive_batches_keep_zero_endpoints_and_restore_before_a_late_cycle() {
    let program = typed_trees::TypedTrees::default();
    let mut engine = Engine::for_explicit_bindings(&program);
    let names = [ZERO_ATOM, "a", "b", "c", "d"];
    let batches: &[&[(&str, &str, i64)]] = &[
        &[("c", "a", 2), ("a", "b", 3), ("b", "d", 4)],
        &[(ZERO_ATOM, "a", -20)],
        &[("a", ZERO_ATOM, 10)],
        &[("a", "b", 3), ("a", "b", 2)],
    ];
    let mut seeds = Vec::new();
    for batch in batches {
        for &(from, to, bound) in *batch {
            seeds.push((from, to, bound));
            add_seed(&mut engine, from, to, bound);
        }
        engine.refresh_difference_bounds();
        assert!(!engine.requires_unsatisfiable);
        for (from, row) in dense_closure(&names, &seeds).iter().enumerate() {
            for (to, bound) in row.iter().enumerate() {
                assert_eq!(
                    engine.matrix_bound(names[from], names[to]),
                    bound.map(BigInt::from_i64)
                );
            }
        }
    }
    // This refresh first adds a new vertex, then contradicts a prior path.
    // The normal rollback/seed/closure route must retain every actual seed.
    add_seed(&mut engine, "new", ZERO_ATOM, 7);
    add_seed(&mut engine, "d", "c", 1);
    engine.refresh_difference_bounds();
    assert!(engine.requires_unsatisfiable);
    assert!(engine.matrix.position("new").is_some());
}

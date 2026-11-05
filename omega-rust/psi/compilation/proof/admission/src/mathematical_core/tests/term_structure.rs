use crate::mathematical_core::{Level, Sort, Term, TermArena, TermHandle};

#[test]
fn structural_lookup_keys_preserve_complete_payloads_and_root_order() {
    let mut arena = TermArena::new();
    let first = arena.insert(Term::Constant {
        declaration: 7,
        levels: vec![Level::Constant(0)],
    });
    let same = arena.insert(Term::Constant {
        declaration: 7,
        levels: vec![Level::Constant(0)],
    });
    let other_owner = arena.insert(Term::Constant {
        declaration: 8,
        levels: vec![Level::Constant(0)],
    });
    let other_level = arena.insert(Term::Constant {
        declaration: 7,
        levels: vec![Level::Constant(1)],
    });
    let relevant = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let strict = arena.insert(Term::Sort(Sort::Strict(Level::Constant(0))));
    let before = arena.len();
    let keys = arena
        .structural_hashes(&[first, same, other_owner, other_level, relevant, strict])
        .unwrap();
    assert_eq!(keys[0], keys[1]);
    assert!(arena.structurally_equal(first, same));
    for position in 2..keys.len() {
        assert_ne!(keys[0], keys[position]);
    }
    assert_ne!(keys[4], keys[5]);
    assert_eq!(
        arena.structural_hashes(&[other_level, first]).unwrap(),
        [keys[3], keys[0]]
    );
    assert_eq!(arena.len(), before, "lookup does not construct terms");
}

#[test]
fn structural_lookup_visits_shared_paths_without_expanding_histories() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let mut left = arena.insert(Term::TwoZero);
        let mut right = arena.insert(Term::TwoZero);
        for _ in 0..16_384 {
            left = arena.insert(Term::Pair {
                first: left,
                second: left,
            });
            right = arena.insert(Term::Pair {
                first: right,
                second: right,
            });
        }
        let before = arena.len();
        let keys = arena.structural_hashes(&[left, right]).unwrap();
        assert_eq!(keys[0], keys[1]);
        assert!(arena.structurally_equal(left, right));
        assert_eq!(arena.len(), before);
    })
    .join()
    .expect("structural lookup uses a default-stack handle worklist");
}

#[test]
fn structural_lookup_withholds_keys_for_unavailable_nodes() {
    let mut arena = TermArena::new();
    let dummy = arena.insert(Term::Dummy);
    assert!(arena.structural_hashes(&[dummy]).is_none());
    assert!(arena.structural_hashes(&[TermHandle::invalid()]).is_none());
    let valid = arena.insert(Term::Two);
    let other_generation =
        TermHandle::from_parts(valid.arena_index(), valid.generation().wrapping_add(1));
    assert!(
        arena
            .structural_hashes(&[valid, other_generation])
            .is_none()
    );
}

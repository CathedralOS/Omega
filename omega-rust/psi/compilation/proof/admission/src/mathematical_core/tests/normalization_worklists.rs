//! Normalization keeps evaluation context frames in a handle worklist.

use super::{constant, default_budget, pi, two};
use crate::mathematical_core::{
    Budget, CoreError, Declaration, Term, TermArena, check_signature, weak_head_normalize,
};

#[test]
fn deep_neutral_application_normalization_stays_on_default_stack() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let two = two(&mut arena);
        let mut function_type = two;
        for _ in 0..131_072 {
            function_type = pi(&mut arena, two, function_type);
        }
        let signature = check_signature(
            &mut arena,
            &[Declaration::assumption(0, function_type)],
            &mut default_budget(),
        )
        .expect("the neutral function has a well-formed complete telescope");
        let mut term = constant(&mut arena, 0, vec![]);
        let zero = arena.insert(Term::TwoZero);
        for _ in 0..131_072 {
            term = arena.insert(Term::Apply {
                function: term,
                argument: zero,
            });
        }
        let before = arena.len();
        let mut budget = Budget::new(0);
        assert_eq!(
            weak_head_normalize(&mut arena, &signature, term, &mut budget),
            Ok(term)
        );
        assert_eq!(
            arena.len(),
            before,
            "a neutral spine keeps all original handles"
        );
        assert_eq!(budget.remaining(), 0, "inspection is not a reduction step");
    })
    .join()
    .expect("normalization uses a default-stack handle worklist");
}

#[test]
fn normalization_frames_preserve_reduction_budgets_and_refusals() {
    let mut arena = TermArena::new();
    let two = two(&mut arena);
    let zero = arena.insert(Term::TwoZero);
    let one = arena.insert(Term::TwoOne);
    let pair = arena.insert(Term::Pair {
        first: zero,
        second: one,
    });
    let body = arena.insert(Term::Variable(0));
    let identity = arena.insert(Term::Lambda { domain: two, body });
    let applied = arena.insert(Term::Apply {
        function: identity,
        argument: pair,
    });
    let first = arena.insert(Term::Fst { pair: applied });
    let signature = crate::mathematical_core::Signature::new();
    assert_eq!(
        weak_head_normalize(&mut arena, &signature, first, &mut Budget::new(1)),
        Err(CoreError::StepCeiling)
    );
    let mut budget = Budget::new(2);
    assert_eq!(
        weak_head_normalize(&mut arena, &signature, first, &mut budget),
        Ok(zero)
    );
    assert_eq!(budget.remaining(), 0);
    let unknown = constant(&mut arena, 9, vec![]);
    let nested = arena.insert(Term::Snd { pair: unknown });
    assert_eq!(
        weak_head_normalize(&mut arena, &signature, nested, &mut default_budget()),
        Err(CoreError::UnknownDeclaration {
            declaration: 9,
            signature_len: 0
        })
    );
}

#[test]
fn deep_structural_comparison_keeps_every_payload_and_tail() {
    std::thread::spawn(|| {
        let mut arena = TermArena::new();
        let mut left = two(&mut arena);
        let mut equal = two(&mut arena);
        let mut different = arena.insert(Term::TwoOne);
        let domain = two(&mut arena);
        for _ in 0..131_072 {
            left = pi(&mut arena, domain, left);
            equal = pi(&mut arena, domain, equal);
            different = pi(&mut arena, domain, different);
        }
        assert!(arena.structurally_equal(left, equal));
        assert!(!arena.structurally_equal(left, different));
        let dummy = arena.insert(Term::Dummy);
        assert!(
            !arena.structurally_equal(dummy, dummy),
            "invalid placeholders are never equality evidence"
        );
        let parameter = crate::mathematical_core::Level::Parameter(0);
        let levels_a = vec![
            parameter
                .clone()
                .maximum(crate::mathematical_core::Level::Constant(0)),
        ];
        let levels_b = vec![parameter];
        let a = constant(&mut arena, 0, levels_a);
        let b = constant(&mut arena, 0, levels_b);
        assert!(
            !arena.structurally_equal(a, b),
            "structural comparison does not silently become semantic level conversion"
        );
    })
    .join()
    .expect("structural comparison uses a default-stack worklist");
}

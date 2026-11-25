//! The profile's "Vector length indices" demonstration: a native
//! length-indexed inductive family checked end-to-end by the kernel.
//!
//! ```text
//! Vec  : Π(n : Nat). Type 0
//! nil  : Vec zeroN
//! cons : Π(e : Elem). Π(n : Nat). Π(t : Vec n). Vec (succN n)
//! ```
//!
//! `Nat`, `zeroN`, `succN` and `Elem` are signature *assumptions* at positions
//! 0–3; `Vec`, `nil`, `cons`, and `Vec.rec` sit at positions 4–7.

use crate::common::*;
use proof_admission::{
    Context, CoreError, Declaration, Level, Signature, TermArena, TermHandle, check_signature,
    check_type, convertible, declare_inductive_family,
};

const VEC: u32 = 4;
const VEC_NIL: u32 = 5;
const VEC_CONS: u32 = 6;
const VEC_REC: u32 = 7;

fn vector_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = nat_succ_declarations(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // 3: Elem
    declarations.extend(declare_vector_family(arena, VEC));
    declarations
}

fn vector_signature(arena: &mut TermArena) -> Signature {
    let declarations = vector_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

fn vec_at(arena: &mut TermArena, length: TermHandle) -> TermHandle {
    let head = constant(arena, VEC, Vec::new());
    apply(arena, head, length)
}

fn vec_nil(arena: &mut TermArena) -> TermHandle {
    constant(arena, VEC_NIL, Vec::new())
}

fn vec_cons(
    arena: &mut TermArena,
    element: TermHandle,
    length: TermHandle,
    tail: TermHandle,
) -> TermHandle {
    let head = constant(arena, VEC_CONS, Vec::new());
    apps(arena, head, &[element, length, tail])
}

/// `Q : Π(i : Nat). Π(_ : Vec i). Type 0`,
/// `m_nil : Q zeroN nil`,
/// `m_cons : Π(e : Elem). Π(n : Nat). Π(t : Vec n). Π(ih : Q n t). Q (succN n) (cons e n t)`.
fn eliminator_context(arena: &mut TermArena) -> Context {
    let motive_type = {
        let index = variable(arena, 0);
        let family_at = vec_at(arena, index);
        let type_zero = type_sort(arena, 0);
        let inner = pi(arena, family_at, type_zero);
        let index_type = nat(arena);
        pi(arena, index_type, inner)
    };
    // Under [Q]: Q = 0.
    let m_nil_type = {
        let q = variable(arena, 0);
        let zero = nat_zero(arena);
        let nil = vec_nil(arena);
        apps(arena, q, &[zero, nil])
    };
    // Under [Q, m_nil]: Q = 1.
    let m_cons_type = {
        let e_ty = elem(arena);
        let n_ty = nat(arena);
        // Under [e, n]: n = 0.
        let t_ty = {
            let n = variable(arena, 0);
            vec_at(arena, n)
        };
        // Under [e, n, t]: t = 0, n = 1, e = 2, m_nil = 3, Q = 4.
        let ih_ty = {
            let q = variable(arena, 4);
            let n = variable(arena, 1);
            let t = variable(arena, 0);
            apps(arena, q, &[n, t])
        };
        // Under [e, n, t, ih]: ih = 0, t = 1, n = 2, e = 3, m_nil = 4, Q = 5.
        let ret_ty = {
            let q = variable(arena, 5);
            let e = variable(arena, 3);
            let n = variable(arena, 2);
            let t = variable(arena, 1);
            let succ_n = nat_succ(arena, n);
            let c = vec_cons(arena, e, n, t);
            apps(arena, q, &[succ_n, c])
        };
        let over_ih = pi(arena, ih_ty, ret_ty);
        let over_t = pi(arena, t_ty, over_ih);
        let over_n = pi(arena, n_ty, over_t);
        pi(arena, e_ty, over_n)
    };
    let element_type = elem(arena);
    let index_type = nat(arena);
    let tail_type = {
        let length = variable(arena, 0);
        vec_at(arena, length)
    };
    Context::empty()
        .with_signature(vector_signature(arena))
        .extend(motive_type)
        .extend(m_nil_type)
        .extend(m_cons_type)
        .extend(element_type)
        .extend(index_type)
        .extend(tail_type)
}

#[test]
fn vectors_compute_and_reject_wrong_indices_and_malformed_declarations() {
    let mut arena = TermArena::new();
    let mut budget = budget();

    // Positive: `Vec.rec[0] Q m_nil m_cons (succN n) (cons e n tail)` computes
    // in one iota-step to `m_cons e n tail (Vec.rec[0] Q m_nil m_cons n tail)`.
    let context = eliminator_context(&mut arena);
    // In Γ: tail = 0, n = 1, e = 2, m_cons = 3, m_nil = 4, Q = 5.
    let e = variable(&mut arena, 2);
    let n = variable(&mut arena, 1);
    let tail = variable(&mut arena, 0);
    let cons = vec_cons(&mut arena, e, n, tail);
    let succ_n = nat_succ(&mut arena, n);
    let q = variable(&mut arena, 5);
    let m_nil = variable(&mut arena, 4);
    let m_cons = variable(&mut arena, 3);
    let rec = constant(&mut arena, VEC_REC, vec![Level::Constant(0)]);
    let elim = apps(&mut arena, rec, &[q, m_nil, m_cons, succ_n, cons]);
    let expected_ty = apps(&mut arena, q, &[succ_n, cons]);
    check_type(&mut arena, &context, elim, expected_ty, &mut budget).unwrap();

    let rec_tail = apps(&mut arena, rec, &[q, m_nil, m_cons, n, tail]);
    let expected_red = apps(&mut arena, m_cons, &[e, n, tail, rec_tail]);
    assert!(
        convertible(
            &mut arena,
            &context,
            elim,
            expected_red,
            expected_ty,
            &mut budget
        )
        .unwrap()
    );

    // A cons never sits at the predecessor's own index: `succN n ≢ n`.
    let same_length = vec_at(&mut arena, n);
    let error = check_type(&mut arena, &context, cons, same_length, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // …nor at `zeroN`, and `nil` never lands at `succN n`.
    let zero = nat_zero(&mut arena);
    let at_zero = vec_at(&mut arena, zero);
    let error = check_type(&mut arena, &context, cons, at_zero, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    let nil = vec_nil(&mut arena);
    let at_successor = vec_at(&mut arena, succ_n);
    let error = check_type(&mut arena, &context, nil, at_successor, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // The eliminator cannot run at a mismatched index either:
    // `Vec.rec Q m_nil m_cons zeroN (cons …)` needs `cons : Vec zeroN`.
    let bad_elim = apps(&mut arena, rec, &[q, m_nil, m_cons, zero, cons]);
    let bad_ty = apps(&mut arena, q, &[zero, cons]);
    assert_check_type_mismatch(&mut arena, &context, bad_elim, bad_ty, &mut budget);

    // The eliminator is universe-polymorphic over the motive level `w`:
    // claiming `w = 1` while `Q` lands at `Type 0` is a bad universe.
    let rec_high = constant(&mut arena, VEC_REC, vec![Level::Constant(1)]);
    let bad_univ = apps(&mut arena, rec_high, &[q, m_nil, m_cons, succ_n, cons]);
    assert_check_type_mismatch(&mut arena, &context, bad_univ, expected_ty, &mut budget);

    // A malformed inductive declaration whose constructor supplies an `Elem`
    // where a `Nat` index is required is rejected by `check_signature`.
    let mut bad_decls = nat_succ_declarations(&mut arena);
    bad_decls.push(Declaration::assumption(0, type_sort(&mut arena, 0))); // 3: Elem
    let nat_ty = nat(&mut arena);
    let type0 = type_sort(&mut arena, 0);
    let vec_ty = pi(&mut arena, nat_ty, type0);
    let bad_ctor = {
        let e_ty = elem(&mut arena);
        let e_var = variable(&mut arena, 0);
        let head = constant(&mut arena, VEC, Vec::new());
        let app = apply(&mut arena, head, e_var);
        pi(&mut arena, e_ty, app)
    };
    bad_decls.extend(declare_inductive_family(
        &mut arena,
        VEC,
        0,
        0,
        vec_ty,
        &[bad_ctor],
    ));
    let err = check_signature(&mut arena, &bad_decls, &mut budget).unwrap_err();
    assert!(matches!(
        err,
        CoreError::ArgumentTypeMismatch { .. } | CoreError::TypeMismatch { .. }
    ));

    // A `Strict`-codomain motive is refused by `Vec.rec`.
    let strict_motive = {
        let index = variable(&mut arena, 0);
        let family_at = vec_at(&mut arena, index);
        let strict0 = strict_sort(&mut arena, 0);
        let inner = pi(&mut arena, family_at, strict0);
        let index_type = nat(&mut arena);
        pi(&mut arena, index_type, inner)
    };
    let strict_ctx = context.extend(strict_motive);
    let q_strict = variable(&mut arena, 0);
    let bad_strict = apply(&mut arena, rec, q_strict);
    assert_type_mismatch(&mut arena, &strict_ctx, bad_strict, &mut budget);
}

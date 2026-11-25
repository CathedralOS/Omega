//! The profile's "mutual families" demonstration: a native mutually recursive
//! `Tree`/`Forest` inductive block checked end-to-end by the kernel.
//!
//! ```text
//! Tree   : Type 0
//! Forest : Type 0
//! node   : Π(e : Elem). Π(f : Forest). Tree
//! fnil   : Forest
//! fcons  : Π(t : Tree). Π(f : Forest). Forest
//! ```
//!
//! `Elem` is a signature assumption at position 0; the mutual block sits at
//! positions 1–7 (`Tree`, `Forest`, `node`, `fnil`, `fcons`, `Tree.rec`,
//! `Forest.rec`).

use crate::common::*;
use proof_admission::{
    Context, CoreError, Declaration, Level, Signature, TermArena, TermHandle, check_signature,
    check_type, convertible, declare_mutual_inductive_family,
};

const MUTUAL_ELEM: u32 = 0;
const TREE: u32 = 1;
const FOREST: u32 = 2;
const NODE: u32 = 3;
const FNIL: u32 = 4;
const FCONS: u32 = 5;
const TREE_REC: u32 = 6;
const FOREST_REC: u32 = 7;

fn elem_ty(arena: &mut TermArena) -> TermHandle {
    constant(arena, MUTUAL_ELEM, Vec::new())
}

fn tree(arena: &mut TermArena) -> TermHandle {
    constant(arena, TREE, Vec::new())
}

fn forest(arena: &mut TermArena) -> TermHandle {
    constant(arena, FOREST, Vec::new())
}

fn mutual_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let type0 = type_sort(arena, 0);
    let mut declarations = vec![Declaration::assumption(0, type0)]; // 0: Elem

    let e = elem_ty(arena);
    let t = tree(arena);
    let f = forest(arena);

    // `node : Π(e : Elem). Π(f : Forest). Tree`
    let node_ty = {
        let inner = pi(arena, f, t);
        pi(arena, e, inner)
    };
    // `fnil : Forest`
    let fnil_ty = f;
    // `fcons : Π(t : Tree). Π(f : Forest). Forest`
    let fcons_ty = {
        let inner = pi(arena, f, f);
        pi(arena, t, inner)
    };

    declarations.extend(declare_mutual_inductive_family(
        arena,
        TREE,
        0,
        0,
        &[type0, type0],
        &[(0, node_ty), (1, fnil_ty), (1, fcons_ty)],
    ));
    declarations
}

fn mutual_signature(arena: &mut TermArena) -> Signature {
    let declarations = mutual_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

#[test]
fn mutual_constructions_compute_and_reject_wrong_sorts_and_non_positivity() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let signature = mutual_signature(&mut arena);

    // Context: `e : Elem, t : Tree, f : Forest` (f = 0, t = 1, e = 2).
    let e_ty = elem_ty(&mut arena);
    let t_ty = tree(&mut arena);
    let f_ty = forest(&mut arena);
    let context = Context::empty()
        .with_signature(signature.clone())
        .extend(e_ty)
        .extend(t_ty)
        .extend(f_ty);

    let e = variable(&mut arena, 2);
    let t = variable(&mut arena, 1);
    let f = variable(&mut arena, 0);

    let node_head = constant(&mut arena, NODE, Vec::new());
    let fcons_head = constant(&mut arena, FCONS, Vec::new());
    let node = apps(&mut arena, node_head, &[e, f]);
    let fnil = constant(&mut arena, FNIL, Vec::new());
    let fcons = apps(&mut arena, fcons_head, &[t, f]);

    check_type(&mut arena, &context, node, t_ty, &mut budget).unwrap();
    check_type(&mut arena, &context, fnil, f_ty, &mut budget).unwrap();
    check_type(&mut arena, &context, fcons, f_ty, &mut budget).unwrap();

    // Mutual elimination: `Forest.rec[0] Q_t Q_f m_node m_fnil m_fcons (fcons t f)`
    // computes in one step to `m_fcons t f (Tree.rec ... t) (Forest.rec ... f)`.
    let type0 = type_sort(&mut arena, 0);
    let q_t_ty = pi(&mut arena, t_ty, type0);
    let q_f_ty = pi(&mut arena, f_ty, type0);
    // Under [Q_t, Q_f]: Q_f = 0, Q_t = 1.
    let m_node_ty = {
        // Π(e:Elem). Π(f:Forest). Π(ih:Q_f f). Q_t (node e f)
        let ih_ty = {
            let q_f = variable(&mut arena, 2);
            let f_var = variable(&mut arena, 0);
            apply(&mut arena, q_f, f_var)
        };
        let ret = {
            let q_t = variable(&mut arena, 4);
            let e_var = variable(&mut arena, 2);
            let f_var = variable(&mut arena, 1);
            let n = apps(&mut arena, node_head, &[e_var, f_var]);
            apply(&mut arena, q_t, n)
        };
        let l3 = pi(&mut arena, ih_ty, ret);
        let l2 = pi(&mut arena, f_ty, l3);
        pi(&mut arena, e_ty, l2)
    };
    // Under [Q_t, Q_f, m_node]: Q_f = 1.
    let m_fnil_ty = {
        let q_f = variable(&mut arena, 1);
        apply(&mut arena, q_f, fnil)
    };
    // Under [Q_t, Q_f, m_node, m_fnil]: Q_f = 2, Q_t = 3.
    let m_fcons_ty = {
        // Π(t:Tree). Π(f:Forest). Π(ih_t:Q_t t). Π(ih_f:Q_f f). Q_f (fcons t f)
        let ih_t_ty = {
            let q_t = variable(&mut arena, 5);
            let t_var = variable(&mut arena, 1);
            apply(&mut arena, q_t, t_var)
        };
        let ih_f_ty = {
            let q_f = variable(&mut arena, 5);
            let f_var = variable(&mut arena, 1);
            apply(&mut arena, q_f, f_var)
        };
        let ret = {
            let q_f = variable(&mut arena, 6);
            let t_var = variable(&mut arena, 3);
            let f_var = variable(&mut arena, 2);
            let fc = apps(&mut arena, fcons_head, &[t_var, f_var]);
            apply(&mut arena, q_f, fc)
        };
        let l4 = pi(&mut arena, ih_f_ty, ret);
        let l3 = pi(&mut arena, ih_t_ty, l4);
        let l2 = pi(&mut arena, f_ty, l3);
        pi(&mut arena, t_ty, l2)
    };
    let elim_ctx = Context::empty()
        .with_signature(signature)
        .extend(q_t_ty)
        .extend(q_f_ty)
        .extend(m_node_ty)
        .extend(m_fnil_ty)
        .extend(m_fcons_ty)
        .extend(t_ty)
        .extend(f_ty);
    // In elim_ctx: f = 0, t = 1, m_fcons = 2, m_fnil = 3, m_node = 4, Q_f = 5, Q_t = 6.
    let q_t = variable(&mut arena, 6);
    let q_f = variable(&mut arena, 5);
    let m_node = variable(&mut arena, 4);
    let m_fnil = variable(&mut arena, 3);
    let m_fcons = variable(&mut arena, 2);
    let t_v = variable(&mut arena, 1);
    let f_v = variable(&mut arena, 0);
    let fc = apps(&mut arena, fcons_head, &[t_v, f_v]);
    let t_rec = constant(&mut arena, TREE_REC, vec![Level::Constant(0)]);
    let f_rec = constant(&mut arena, FOREST_REC, vec![Level::Constant(0)]);
    let elim = apps(&mut arena, f_rec, &[q_t, q_f, m_node, m_fnil, m_fcons, fc]);
    let expected_ty = apply(&mut arena, q_f, fc);
    check_type(&mut arena, &elim_ctx, elim, expected_ty, &mut budget).unwrap();

    let ih_t = apps(&mut arena, t_rec, &[q_t, q_f, m_node, m_fnil, m_fcons, t_v]);
    let ih_f = apps(&mut arena, f_rec, &[q_t, q_f, m_node, m_fnil, m_fcons, f_v]);
    let expected_red = apps(&mut arena, m_fcons, &[t_v, f_v, ih_t, ih_f]);
    assert!(
        convertible(
            &mut arena,
            &elim_ctx,
            elim,
            expected_red,
            expected_ty,
            &mut budget
        )
        .unwrap()
    );

    // A `node` is a `Tree`, never a `Forest`, and `fcons` is never a `Tree`.
    let err = check_type(&mut arena, &context, node, f_ty, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::TypeMismatch { .. }));
    let err = check_type(&mut arena, &context, fcons, t_ty, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::TypeMismatch { .. }));

    // Passing a `Tree` where `node` requires `Forest`, or swapping `fcons` arguments, fails.
    let wrong_child = apps(&mut arena, node_head, &[e, t]);
    assert_check_type_mismatch(&mut arena, &context, wrong_child, t_ty, &mut budget);
    let swapped = apps(&mut arena, fcons_head, &[f, t]);
    assert_check_type_mismatch(&mut arena, &context, swapped, f_ty, &mut budget);

    // Wrong motive universe `w = 1` when `Q_t, Q_f` land in `Type 0`.
    let f_rec_high = constant(&mut arena, FOREST_REC, vec![Level::Constant(1)]);
    let bad_univ = apps(
        &mut arena,
        f_rec_high,
        &[q_t, q_f, m_node, m_fnil, m_fcons, fc],
    );
    assert_check_type_mismatch(&mut arena, &elim_ctx, bad_univ, expected_ty, &mut budget);

    // Non-positive mutual occurrence: `mk : Π(_ : Π(_ : Forest). Two). Tree`
    // is rejected by `check_signature` with `NonStrictlyPositiveOccurrence`.
    let c0 = constant(&mut arena, 0, Vec::new());
    let c1 = constant(&mut arena, 1, Vec::new());
    let two_ty = two(&mut arena);
    let neg_dom = pi(&mut arena, c1, two_ty);
    let bad_ctor = pi(&mut arena, neg_dom, c0);
    let bad_decls =
        declare_mutual_inductive_family(&mut arena, 0, 0, 0, &[type0, type0], &[(0, bad_ctor)]);
    let err = check_signature(&mut arena, &bad_decls, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::NonStrictlyPositiveOccurrence));
}

//! The profile's "nested strictly-positive families" demonstration: a native
//! level-indexed rose tree whose constructor payload carries the previously
//! declared native `Vec k` family and whose children are ω-branching over `Nat`:
//!
//! ```text
//! Rose  : Π(n : Nat). Type 0
//! leaf  : Rose zeroN
//! rnode : Π(k : Nat). Π(v : Vec k). Π(g : Π(_ : Nat). Rose (succN k)). Rose k
//! ```
//!
//! `Nat`, `zeroN`, `succN`, `Elem` sit at 0–3; `Vec`..`Vec.rec` sit at 4–7;
//! `Rose`, `leaf`, `rnode`, `Rose.rec` sit at 8–11.

use crate::common::*;
use proof_admission::{
    Context, CoreError, Declaration, Level, Signature, TermArena, TermHandle, check_signature,
    check_type, convertible, declare_inductive_family,
};

const VEC: u32 = 4;
const ROSE: u32 = 8;
const LEAF: u32 = 9;
const RNODE: u32 = 10;
const ROSE_REC: u32 = 11;

fn vec_at(arena: &mut TermArena, length: TermHandle) -> TermHandle {
    let head = constant(arena, VEC, Vec::new());
    apply(arena, head, length)
}

fn rose_at(arena: &mut TermArena, level: TermHandle) -> TermHandle {
    let head = constant(arena, ROSE, Vec::new());
    apply(arena, head, level)
}

fn nested_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = nat_succ_declarations(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // 3: Elem
    declarations.extend(declare_vector_family(arena, VEC)); // 4..=7

    let nat_ty = nat(arena);
    let type0 = type_sort(arena, 0);
    let rose_ty = pi(arena, nat_ty, type0);

    // `leaf : Rose zeroN`
    let leaf_ty = {
        let zero = nat_zero(arena);
        rose_at(arena, zero)
    };

    // `rnode : Π(k : Nat). Π(v : Vec k). Π(g : Π(_ : Nat). Rose (succN k)). Rose k`
    let rnode_ty = {
        let v_ty = {
            let k = variable(arena, 0);
            vec_at(arena, k)
        };
        let g_ty = {
            // Under [k, v]: k = 1; under `b : Nat`: k = 2.
            let k = variable(arena, 2);
            let succ_k = nat_succ(arena, k);
            let cod = rose_at(arena, succ_k);
            pi(arena, nat_ty, cod)
        };
        let ret = {
            let k = variable(arena, 2);
            rose_at(arena, k)
        };
        let over_g = pi(arena, g_ty, ret);
        let over_v = pi(arena, v_ty, over_g);
        pi(arena, nat_ty, over_v)
    };

    declarations.extend(declare_inductive_family(
        arena,
        ROSE,
        0,
        0,
        rose_ty,
        &[leaf_ty, rnode_ty],
    ));
    declarations
}

fn nested_signature(arena: &mut TermArena) -> Signature {
    let declarations = nested_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

#[test]
fn nested_applications_compute_and_reject_wrong_indices_and_malformed_declarations() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let signature = nested_signature(&mut arena);

    // Context: `k : Nat, v : Vec k, g : Π(_ : Nat). Rose (succN k), e : Elem`
    // Indices: e = 0, g = 1, v = 2, k = 3.
    let nat_ty = nat(&mut arena);
    let v_ty = {
        let k = variable(&mut arena, 0);
        vec_at(&mut arena, k)
    };
    let g_ty = {
        let k = variable(&mut arena, 2);
        let succ_k = nat_succ(&mut arena, k);
        let cod = rose_at(&mut arena, succ_k);
        pi(&mut arena, nat_ty, cod)
    };
    let e_ty = elem(&mut arena);
    let context = Context::empty()
        .with_signature(signature.clone())
        .extend(nat_ty)
        .extend(v_ty)
        .extend(g_ty)
        .extend(e_ty);

    let k = variable(&mut arena, 3);
    let v = variable(&mut arena, 2);
    let g = variable(&mut arena, 1);
    let e = variable(&mut arena, 0);

    // Positive: `rnode k v g : Rose k`
    let rnode_head = constant(&mut arena, RNODE, Vec::new());
    let rnode = apps(&mut arena, rnode_head, &[k, v, g]);
    let at_k = rose_at(&mut arena, k);
    check_type(&mut arena, &context, rnode, at_k, &mut budget).unwrap();

    // Positive: `Rose.rec` on `rnode k v g` computes in one step with ω-branching IH
    // `λ(b : Nat). Rose.rec Q m_leaf m_rnode (succN k) (g b)`.
    let motive = {
        let idx = variable(&mut arena, 0);
        let r_dom = rose_at(&mut arena, idx);
        let inner = lambda(&mut arena, r_dom, nat_ty);
        lambda(&mut arena, nat_ty, inner)
    };
    let m_leaf = nat_zero(&mut arena);
    let m_rnode = {
        // λ(k:Nat). λ(v:Vec k). λ(g:Nat -> Rose (succN k)). λ(ih:Nat -> Nat). k
        let v_d = {
            let kv = variable(&mut arena, 0);
            vec_at(&mut arena, kv)
        };
        let g_d = {
            let kv = variable(&mut arena, 2);
            let sk = nat_succ(&mut arena, kv);
            let cod = rose_at(&mut arena, sk);
            pi(&mut arena, nat_ty, cod)
        };
        let ih_d = pi(&mut arena, nat_ty, nat_ty);
        let body = variable(&mut arena, 3);
        let l4 = lambda(&mut arena, ih_d, body);
        let l3 = lambda(&mut arena, g_d, l4);
        let l2 = lambda(&mut arena, v_d, l3);
        lambda(&mut arena, nat_ty, l2)
    };
    let rec = constant(&mut arena, ROSE_REC, vec![Level::Constant(0)]);
    let elim = apps(&mut arena, rec, &[motive, m_leaf, m_rnode, k, rnode]);
    check_type(&mut arena, &context, elim, nat_ty, &mut budget).unwrap();
    assert!(convertible(&mut arena, &context, elim, k, nat_ty, &mut budget).unwrap());

    // Wrong level: `rnode k v g` inhabits `Rose k`, never `Rose (succN k)`.
    let succ_k = nat_succ(&mut arena, k);
    let wrong_level = rose_at(&mut arena, succ_k);
    let err = check_type(&mut arena, &context, rnode, wrong_level, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::TypeMismatch { .. }));

    // Malformed payload: bare `e : Elem` where `Vec k` is required.
    let bad_payload = apps(&mut arena, rnode_head, &[k, e, g]);
    assert_check_type_mismatch(&mut arena, &context, bad_payload, at_k, &mut budget);

    // Wrong child level: a constant child function returning `Rose k` instead of
    // `Rose (succN k)` is refused.
    let leaf_c = constant(&mut arena, LEAF, Vec::new());
    let bad_g = lambda(&mut arena, nat_ty, leaf_c);
    let bad_children = apps(&mut arena, rnode_head, &[k, v, bad_g]);
    assert_check_type_mismatch(&mut arena, &context, bad_children, at_k, &mut budget);

    // Malformed inductive declaration: value `zeroN` as index domain is refused.
    let mut bad_decls = nat_succ_declarations(&mut arena);
    let zero = nat_zero(&mut arena);
    let type0 = type_sort(&mut arena, 0);
    let bad_ind_ty = pi(&mut arena, zero, type0);
    bad_decls.extend(declare_inductive_family(
        &mut arena,
        3,
        0,
        0,
        bad_ind_ty,
        &[],
    ));
    assert!(check_signature(&mut arena, &bad_decls, &mut budget).is_err());

    // Malformed inductive declaration: `Strict 0` codomain for inductive type is refused.
    let mut strict_decls = nat_succ_declarations(&mut arena);
    let strict0 = strict_sort(&mut arena, 0);
    let strict_ind_ty = pi(&mut arena, nat_ty, strict0);
    strict_decls.extend(declare_inductive_family(
        &mut arena,
        3,
        0,
        0,
        strict_ind_ty,
        &[],
    ));
    assert!(check_signature(&mut arena, &strict_decls, &mut budget).is_err());
}

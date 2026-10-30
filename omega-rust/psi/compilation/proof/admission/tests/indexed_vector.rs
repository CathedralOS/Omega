//! The profile's "Vector length indices" demonstration: a concrete
//! indexed family built on the derived [`indexed_scheme`] and checked
//! end-to-end by the kernel.
//!
//! The vector's description is
//!
//! ```text
//! I    = Nat                            (assumed index type)
//! A    = Σ(tag : Two). Payload(tag)     nil/cons tag plus payload
//! B a  = caseTwo(…, Id Two zero one,    nil has no reachable child
//!                Id Two zero zero,      cons has one child position
//!                fst a)
//! out  = λa. caseTwo(…, λ_. zeroN,      nil ↦ zero, cons(e,n) ↦ succ n
//!                    λp. succN (snd p),
//!                    fst a) (snd a)
//! next = λa. caseTwo(…, λp.λ_. zeroN,   cons's child must index at the
//!                    λp.λ_. snd p,      predecessor `snd p = n`
//!                    fst a) (snd a)
//! ```
//!
//! so `Vec n := IW … n` is the family of length-`n` vectors over the
//! assumed element type `Elem`: a nil node is `⟨zero, ·⟩` landing at
//! `out ⟨zero,·⟩ ≡ zeroN`, and `cons e n tail` is
//! `isup ⟨one, ⟨e, n⟩⟩ (λ_. tail) : Vec (succN n)` for `tail : Vec n`.
//! `Payload`, `B`, `out` and `next` all compute by `caseTwo` on the
//! constructor tag, so unlike the kernel's `Two`-indexed smoke test the
//! index discipline here is *computed*: the child's required index and
//! the parent's produced index are decided by reduction, and `iindW`'s
//! induction hypothesis lands at the predecessor length.
//!
//! `Nat`, `zeroN`, `succN` and `Elem` are signature *assumptions* — the
//! demonstration exercises the indexed-family scheme against an
//! arbitrary producer index type, and the certificate's assumption
//! closure records exactly those four axioms. The kernel has no
//! primitive naturals; a faithful `Nat` would need the strict empty
//! type's elimination, which the reference core keeps outside this
//! profile — so the index type is assumed, exactly as a producer would
//! supply it, and every use is still re-decided by the checker.

use crate::common::*;
use proof_admission::{
    Context, CoreError, Declaration, IndexedFamily, Level, Signature, TermArena, TermHandle,
    check_signature, check_type,
};

// ── The producer signature ────────────────────────────────────────────
//
// Positions 0–4 are the derived scheme (`IndexedAt`, `IW`, `iwPack`,
// `isup`, `iindW`); the vector's index arithmetic and element type are
// the appended assumptions. The family's own description — `B_v`, `out_v`,
// `next_v` and their `A_v` carrier — is `common::vector_family`, the one
// vector encoding every indexed fixture shares.

/// The producer signature: the five scheme declarations then the four
/// vector assumptions `Nat`, `zeroN`, `succN`, `Elem`.
fn vector_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = nat_succ_declarations(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Elem
    declarations
}

fn vector_signature(arena: &mut TermArena) -> Signature {
    let declarations = vector_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

/// `Vec n` — the family applied at a length.
fn vec_at(arena: &mut TermArena, family: &IndexedFamily, length: TermHandle) -> TermHandle {
    family.indexed_w(arena, length)
}

/// `⟨one, ⟨e, n⟩⟩` — a cons node whose payload records the element and
/// the predecessor length.
fn cons_node(arena: &mut TermArena, element: TermHandle, predecessor: TermHandle) -> TermHandle {
    let payload_value = pair(arena, element, predecessor);
    let tag = two_one(arena);
    pair(arena, tag, payload_value)
}

/// `⟨zero, zero⟩` — a nil node; its `Two` payload is a dummy.
fn nil_node(arena: &mut TermArena) -> TermHandle {
    let dummy = two_zero(arena);
    let tag = two_zero(arena);
    pair(arena, tag, dummy)
}

/// The constructor-side context `e : Elem, n : Nat, tail : Vec n` —
/// indices tail = 0, n = 1, e = 2.
fn vector_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let tail_type = {
        let length = variable(arena, 0);
        vec_at(arena, family, length)
    };
    let element_type = elem(arena);
    let index_type = nat(arena);
    Context::empty()
        .with_signature(vector_signature(arena))
        .extend(element_type)
        .extend(index_type)
        .extend(tail_type)
}

/// The eliminator context `Q : Π(i : Nat). Π(_ : Vec i). Type 0`,
/// `s : <iindW step>`, `e : Elem`, `n : Nat`, `tail : Vec n` —
/// indices tail = 0, n = 1, e = 2, s = 3, Q = 4.
fn eliminator_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let motive_type = {
        let index = variable(arena, 0);
        let family_at = vec_at(arena, family, index);
        let type_zero = type_sort(arena, 0);
        let inner = pi(arena, family_at, type_zero);
        let index_type = nat(arena);
        pi(arena, index_type, inner)
    };
    // The shared eliminator step type, at this family's own carrier.
    let domain = carrier(arena, elem_nat_payload);
    let step_type = step_type(arena, family, domain);
    let element_type = elem(arena);
    let index_type = nat(arena);
    let tail_type = {
        let length = variable(arena, 0);
        vec_at(arena, family, length)
    };
    Context::empty()
        .with_signature(vector_signature(arena))
        .extend(motive_type)
        .extend(step_type)
        .extend(element_type)
        .extend(index_type)
        .extend(tail_type)
}

#[test]
fn vectors_reject_wrong_indices_and_malformed_descriptions() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = vector_family(&mut arena);
    let context = vector_context(&mut arena, &family);
    // In Γ: tail = 0, n = 1, e = 2.

    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);

    // A cons never sits at the predecessor's own index: `succN n ≢ n`.
    let predecessor = variable(&mut arena, 1);
    let same_length = vec_at(&mut arena, &family, predecessor);
    let error = check_type(&mut arena, &context, cons, same_length, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // …nor at `zeroN`, and a nil-node never lands at `succN n`.
    let zero = nat_zero(&mut arena);
    let at_zero = vec_at(&mut arena, &family, zero);
    let error = check_type(&mut arena, &context, cons, at_zero, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    let nil_children_type = {
        let node = nil_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let codomain = {
            let zero = nat_zero(&mut arena);
            vec_at(&mut arena, &family, zero)
        };
        pi(&mut arena, domain, codomain)
    };
    let context_with_nil = context.extend(nil_children_type);
    let nil = {
        let node = nil_node(&mut arena);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, node, function)
    };
    // In Γ': g = 0, tail = 1, n = 2.
    let predecessor = variable(&mut arena, 2);
    let successor = nat_succ(&mut arena, predecessor);
    let at_successor = vec_at(&mut arena, &family, successor);
    let error = check_type(
        &mut arena,
        &context_with_nil,
        nil,
        at_successor,
        &mut budget,
    )
    .unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // The eliminator cannot run at a mismatched index either:
    // `iindW Q s zeroN (cons …)` needs `cons : Vec zeroN`.
    let context = eliminator_context(&mut arena, &family);
    // In Γ: tail = 0, n = 1, e = 2, s = 3, Q = 4.
    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);
    let motive = variable(&mut arena, 4);
    let step = variable(&mut arena, 3);
    let index = nat_zero(&mut arena);
    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, index, cons);
    let shared = {
        let motive = variable(&mut arena, 4);
        let zero = nat_zero(&mut arena);
        let at_index = apply(&mut arena, motive, zero);
        apply(&mut arena, at_index, cons)
    };
    assert_check_type_mismatch(&mut arena, &context, elimination, shared, &mut budget);

    // A `next` that grabs the payload's element rather than the
    // predecessor is a malformed description, not a different vector:
    // `fst p : Elem` never answers `Nat`.
    let mut captured = family.clone();
    captured.next = {
        let motive = {
            let tag = variable(&mut arena, 0);
            let payload_fn = elem_nat_payload(&mut arena);
            let payload_at = apply(&mut arena, payload_fn, tag);
            let branching_at = {
                let motive = type_motive(&mut arena);
                let empty = empty_positions(&mut arena);
                let unit = unit_position(&mut arena);
                let tag = variable(&mut arena, 1);
                case_two(&mut arena, motive, empty, unit, tag)
            };
            let length = nat(&mut arena);
            let inner = pi(&mut arena, branching_at, length);
            let body = pi(&mut arena, payload_at, inner);
            let domain = two(&mut arena);
            lambda(&mut arena, domain, body)
        };
        let zero_branch = zero_nat_branch(&mut arena);
        let one_branch = {
            // `λp.λ_. fst p : Elem` where `Nat` is required.
            let payload_domain = {
                let element = elem(&mut arena);
                let length = nat(&mut arena);
                sigma(&mut arena, element, length)
            };
            let position_domain = unit_position(&mut arena);
            let bound = variable(&mut arena, 1);
            let body = fst(&mut arena, bound);
            let inner = lambda(&mut arena, position_domain, body);
            lambda(&mut arena, payload_domain, inner)
        };
        let carrier = carrier(&mut arena, elem_nat_payload);
        tag_selected_children(&mut arena, carrier, motive, zero_branch, one_branch)
    };
    let length = variable(&mut arena, 1);
    let malformed = captured.indexed_w(&mut arena, length);
    assert_type_mismatch(&mut arena, &context, malformed, &mut budget);

    // A branching family into `Strict` is an illegal strict elimination,
    // rejected by `caseTwo`'s own motive rule before `W` formation ever
    // runs.
    let children = {
        let carrier = carrier(&mut arena, elem_nat_payload);
        let empty = empty_positions(&mut arena);
        let unit = unit_position(&mut arena);
        strict_branching(&mut arena, carrier, empty, |_, _| unit)
    };
    let length = variable(&mut arena, 1);
    let error = strict_children_error(&mut arena, &context, &family, children, length, &mut budget);
    assert!(matches!(
        error,
        CoreError::StrictCaseMotiveCodomain { .. } | CoreError::TypeMismatch { .. }
    ));

    // The eliminator is universe-polymorphic over the motive level `w`:
    // claiming `w = 1` while `Q` lands at `Type 0` is a bad universe,
    // not a different judgment.
    let motive = variable(&mut arena, 4);
    let step = variable(&mut arena, 3);
    let predecessor = variable(&mut arena, 1);
    let successor = nat_succ(&mut arena, predecessor);
    let elimination = family.ind(
        &mut arena,
        Level::Constant(1),
        motive,
        step,
        successor,
        cons,
    );
    let shared = {
        let motive = variable(&mut arena, 4);
        let predecessor = variable(&mut arena, 1);
        let successor = nat_succ(&mut arena, predecessor);
        let at_index = apply(&mut arena, motive, successor);
        apply(&mut arena, at_index, cons)
    };
    assert_check_type_mismatch(&mut arena, &context, elimination, shared, &mut budget);
}

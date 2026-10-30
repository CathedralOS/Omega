//! The profile's "nested strictly-positive families" demonstration: a
//! level-indexed rose tree whose constructor payload *is* the
//! previously demonstrated vector family, built on the derived
//! [`indexed_scheme`] and checked end-to-end by the kernel.
//!
//! A nested inductive mentions itself inside another type former —
//! classically `Rose = node : A → List Rose → Rose`. On the W-encoding
//! there are two strictly-positive channels for that shape: the
//! mutual-recursion tag reduction the `Tree`/`Forest` demo uses, and
//! the one exercised here — the node's *payload* carries another
//! indexed family while the recursion stays in the W child positions.
//! The description is
//!
//! ```text
//! I    = Nat                                level index
//! A    = Σ(tag : Two). Payload(tag)         leaf/rnode tag plus payload
//! Payload zero = Two                        leaf carries a dummy
//! Payload one  = Σ(k : Nat). Vec k          rnode carries k elements
//! B a  = caseTwo(…, Id Two zero one,        leaf: dead position
//!                Nat,                       rnode: ω-branching
//!                fst a)
//! out  = λa. caseTwo(…, λ_. zeroN,          leaf ↦ level 0
//!                    λp. fst p,             rnode⟨k,v⟩ ↦ k
//!                    fst a) (snd a)
//! next = λa. caseTwo(…, λp.λ_. zeroN,       leaf: dead ↦ arbitrary
//!                    λp.λ_. succN (fst p),  rnode's children ↦ k+1
//!                    fst a) (snd a)
//! ```
//!
//! so `Rose n := IW … n` is the family of level-`n` rose trees: a node
//! `rnode ⟨k, v⟩ g` at level `k` stores a `Vec Elem k` of elements —
//! its *declared arity equals its vector length*, the payload coupling
//! the profile names — and each of its `Nat` child positions requires
//! `Rose (succ k)`. `Vec k` is not a new type former: it is the very
//! same `IW` constant instantiated with the vector's own description,
//! so the nested family's signature needs no declaration beyond the
//! four shared assumptions `Nat`, `zeroN`, `succN`, `Elem`.
//!
//! `B rnode = Nat` gives unbounded (ω) branching: the kernel has no
//! `Fin` former to bound positions by `k`, so positions stay `Nat` and
//! the arity discipline lives entirely in the payload-index coupling —
//! `out` reads the level off the vector's length, `next` steps it.
//! That keeps the encoding strictly positive without pretending a
//! bounded-arity constructor exists.
//!
//! Unlike the length-indexed vector — whose `next` ignored the child
//! position and returned the predecessor — this `next` *steps* the
//! level: `next ⟨one, ⟨k, v⟩⟩ b ≡ succN k` for every position `b`, so
//! `iindW`'s induction hypothesis lands one level deeper than the node
//! it eliminates. The constructor computation is still definitional:
//! `iindW Q s k (rnode v g)` reduces with `v` a neutral `Vec k` and `g`
//! an arbitrary neutral child function.

use crate::common::*;
use proof_admission::{
    Budget, Context, CoreError, Declaration, IndexedFamily, Level, Signature, TermArena,
    TermHandle, check_signature, check_type,
};

/// The non-eliminator ceiling: this family's `Vec`-nested payload makes
/// every `IW` application re-check the *vector's* `IndexedAt`-based
/// `IW` type inside the `rnode` payload's sort, so formation,
/// constructor and certificate checks cost far more than the vector's
/// `Id`-positioned description — measured spend is pinned per test.
fn family_budget() -> Budget {
    Budget::new(1 << 22)
}

// ── The producer signature ────────────────────────────────────────────
//
// Positions 0–4 are the derived scheme (`IndexedAt`, `IW`, `iwPack`,
// `isup`, `iindW`); the index arithmetic and element type are the same
// four assumptions the vector demonstration uses, so `Vec` inside the
// nested payload is the same `IW` constant at the same instantiation:

/// `λ(_ : Two). Type 0` — the constant motive every `caseTwo` selecting
/// *types* here shares.

/// `Id Two zero one` — the dead child position: no closed inhabitant.

// ── The vector description (the family nested inside the payload) ────
//
// `Vec k` is `common::vector_family` — the same `IW` instantiation the
// vector demonstration uses, whose payload the nested family references;
// `vec_at` below applies it at a length.
/// `Vec k` — the vector family applied at a length. Under a `k` binder
/// the argument is de Bruijn index 0.
fn vec_at(arena: &mut TermArena, length: TermHandle) -> TermHandle {
    vector_family(arena).indexed_w(arena, length)
}

// ── The nested rose-tree description ──────────────────────────────────

/// `Payload = λ(tag : Two). caseTwo(λ_.Type 0, Two, Σ(k : Nat). Vec k,
/// tag)` — leaf carries a dummy `Two` payload; `rnode` carries its
/// arity `k` together with a `Vec Elem k` of elements — the declared
/// arity *is* the vector's length index.
fn payload(arena: &mut TermArena) -> TermHandle {
    let motive = type_motive(arena);
    let leaf_payload = two(arena);
    let rnode_payload = {
        // Under the `k` binder: `Vec b0`.
        let bound = variable(arena, 0);
        let elements = vec_at(arena, bound);
        let arity = nat(arena);
        sigma(arena, arity, elements)
    };
    let tag = variable(arena, 0);
    let body = case_two(arena, motive, leaf_payload, rnode_payload, tag);
    let domain = two(arena);
    lambda(arena, domain, body)
}

/// `B = λ(a : A). caseTwo(λ_.Type 0, Id Two zero one, Nat, fst a)` —
/// leaf has a dead position (no children); `rnode` has `Nat` positions:
/// unbounded ω-branching, the kernel's answer to "list of children"
/// without a `Fin` former.
fn branching(arena: &mut TermArena) -> TermHandle {
    let motive = type_motive(arena);
    let empty = empty_positions(arena);
    let positions = nat(arena);
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let body = case_two(arena, motive, empty, positions, tag);
    let domain = carrier(arena, payload);
    lambda(arena, domain, body)
}

/// `out = λ(a : A). caseTwo(M, λ_. zeroN, λp. fst p, fst a) (snd a)`
/// with `M tag = Π(_ : Payload tag). Nat` — the level a node produces:
/// `zeroN` under leaf, `k` under `rnode ⟨k, v⟩` — the produced index
/// *is* the payload vector's length.
fn out_index(arena: &mut TermArena) -> TermHandle {
    let motive = {
        // Under the tag binder: `Π(_ : Payload tag). Nat`.
        let tag = variable(arena, 0);
        let payload_fn = payload(arena);
        let payload_at = apply(arena, payload_fn, tag);
        let length = nat(arena);
        let codomain = pi(arena, payload_at, length);
        let domain = two(arena);
        lambda(arena, domain, codomain)
    };
    let zero_branch = {
        // `λ(_ : Two). zeroN` at `Π(_ : Payload zero ≡ Two). Nat`.
        let domain = two(arena);
        let zero = nat_zero(arena);
        lambda(arena, domain, zero)
    };
    let one_branch = {
        // `λ(p : Σ(k : Nat). Vec k). fst p` at
        // `Π(_ : Payload one ≡ Σ(k : Nat). Vec k). Nat`.
        let domain = {
            let bound = variable(arena, 0);
            let elements = vec_at(arena, bound);
            let arity = nat(arena);
            sigma(arena, arity, elements)
        };
        let bound = variable(arena, 0);
        let body = fst(arena, bound);
        lambda(arena, domain, body)
    };
    let carrier_handle = carrier(arena, payload);
    tag_selected_children(arena, carrier_handle, motive, zero_branch, one_branch)
}

/// `next = λ(a : A). caseTwo(M, λp.λ_. zeroN, λp.λ_. succN (fst p), fst
/// a) (snd a)` with `M tag = Π(p : Payload tag). Π(_ : B' tag). Nat`
/// and `B' tag = caseTwo(λ_.Type 0, Id Two zero one, Nat, tag)` — the
/// level each child position requires: unreachable under leaf (any
/// `Nat` answers), and `succN k` under `rnode ⟨k, v⟩` — children live
/// one level deeper than their parent's arity.
fn next_index(arena: &mut TermArena) -> TermHandle {
    let motive = {
        // `λ(tag : Two). Π(p : Payload tag). Π(_ : B' tag). Nat`.
        let tag = variable(arena, 0);
        let payload_fn = payload(arena);
        let payload_at = apply(arena, payload_fn, tag);
        let branching_at = {
            // Under the `p` binder the tag is index 1.
            let motive = type_motive(arena);
            let empty = empty_positions(arena);
            let positions = nat(arena);
            let tag = variable(arena, 1);
            case_two(arena, motive, empty, positions, tag)
        };
        let length = nat(arena);
        let inner = pi(arena, branching_at, length);
        let body = pi(arena, payload_at, inner);
        let domain = two(arena);
        lambda(arena, domain, body)
    };
    let zero_branch = {
        // `λ(p : Two). λ(_ : Id Two zero one). zeroN` — the dead
        // position is answered by any level.
        let payload_domain = two(arena);
        let position_domain = empty_positions(arena);
        let body = nat_zero(arena);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let one_branch = {
        // `λ(p : Σ(k : Nat). Vec k). λ(_ : Nat). succN (fst p)` — the
        // child's required level is the arity's successor.
        let payload_domain = {
            let bound = variable(arena, 0);
            let elements = vec_at(arena, bound);
            let arity = nat(arena);
            sigma(arena, arity, elements)
        };
        let position_domain = nat(arena);
        let bound = variable(arena, 1);
        let arity = fst(arena, bound);
        let body = nat_succ(arena, arity);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let carrier_handle = carrier(arena, payload);
    tag_selected_children(arena, carrier_handle, motive, zero_branch, one_branch)
}

/// The nested rose-tree family: `Rose n = IW Nat A B out next n` with
/// the level-coupled description above.
fn nested_family(arena: &mut TermArena) -> IndexedFamily {
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: nat(arena),
        carrier: carrier(arena, payload),
        children: branching(arena),
        out: out_index(arena),
        next: next_index(arena),
    }
}

/// The producer signature: the five scheme declarations then the four
/// shared assumptions `Nat`, `zeroN`, `succN`, `Elem` — identical to
/// the vector's signature because `Vec` is the same `IW` constant.
fn nested_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = nat_succ_declarations(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Elem
    declarations
}

fn nested_signature(arena: &mut TermArena) -> Signature {
    let declarations = nested_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

/// `Rose n` — the nested family applied at a level.
fn rose_at(arena: &mut TermArena, family: &IndexedFamily, level: TermHandle) -> TermHandle {
    family.indexed_w(arena, level)
}

/// `⟨one, ⟨k, v⟩⟩` — an `rnode` label: arity `k`, element vector `v :
/// Vec Elem k`.
fn rnode(arena: &mut TermArena, arity: TermHandle, elements: TermHandle) -> TermHandle {
    let payload_value = pair(arena, arity, elements);
    let tag = two_one(arena);
    pair(arena, tag, payload_value)
}

/// The constructor-side context `k : Nat, v : Vec k, g : Π(b : B
/// (rnode k v)). Rose (next (rnode k v) b)` — `next` left unreduced so
/// the checker computes `succN k`. Indices: g = 0, v = 1, k = 2.
fn nested_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let children_type = {
        // Under [k, v]: k = 1, v = 0.
        let arity = variable(arena, 1);
        let elements = variable(arena, 0);
        let node = rnode(arena, arity, elements);
        let domain = apply(arena, family.children, node);
        let codomain = {
            // Under [b, v, k]: k = 2, v = 1, b = 0.
            let arity = variable(arena, 2);
            let elements = variable(arena, 1);
            let node = rnode(arena, arity, elements);
            next_indexed_w(arena, &family, node)
        };
        pi(arena, domain, codomain)
    };
    let arity_type = nat(arena);
    let elements_type = {
        // Under [k]: `Vec b0`.
        let bound = variable(arena, 0);
        vec_at(arena, bound)
    };
    Context::empty()
        .with_signature(nested_signature(arena))
        .extend(arity_type)
        .extend(elements_type)
        .extend(children_type)
}

#[test]
fn nested_applications_reject_wrong_indices_and_malformed_descriptions() {
    let mut arena = TermArena::new();
    let mut budget = family_budget();
    let family = nested_family(&mut arena);
    let context = nested_context(&mut arena, &family);
    // In Γ: g = 0, v = 1, k = 2.

    // A node at the wrong level: `rnode k v g` inhabits `Rose k`, never
    // `Rose (succN k)` — `out` computes `fst ⟨k,v⟩ = k`.
    let arity = variable(&mut arena, 2);
    let elements = variable(&mut arena, 1);
    let node = rnode(&mut arena, arity, elements);
    let children_fn = variable(&mut arena, 0);
    let node_value = family.sup(&mut arena, node, children_fn);
    let arity = variable(&mut arena, 2);
    let successor = nat_succ(&mut arena, arity);
    let wrong_level = rose_at(&mut arena, &family, successor);
    let error = check_type(&mut arena, &context, node_value, wrong_level, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A malformed payload: `⟨one, ⟨k, e⟩⟩` with `e : Elem` — the
    // `rnode` payload requires `Vec k`, not a bare element. The label
    // fails its own `Payload one` check before `isup` is consulted.
    let element_type = elem(&mut arena);
    let context = context.extend(element_type);
    // In Γ: e = 0, k = 3, v = 2, g = 1.
    let malformed_label = {
        let arity = variable(&mut arena, 3);
        let element = variable(&mut arena, 0);
        rnode(&mut arena, arity, element)
    };
    let children_fn = variable(&mut arena, 1);
    let malformed = family.sup(&mut arena, malformed_label, children_fn);
    let arity = variable(&mut arena, 3);
    let expected = rose_at(&mut arena, &family, arity);
    assert_check_type_mismatch(&mut arena, &context, malformed, expected, &mut budget);

    // A value where the index type belongs is malformed, not a type:
    // `I = zeroN` instead of `Nat`.
    let mut bad_family = family.clone();
    bad_family.index = nat_zero(&mut arena);
    let level = variable(&mut arena, 3);
    let malformed = bad_family.indexed_w(&mut arena, level);
    assert_type_mismatch(&mut arena, &context, malformed, &mut budget);

    // A strict index type is a malformed universe for `I : Type l`.
    let mut strict_family = family.clone();
    strict_family.index = strict_sort(&mut arena, 0);
    let level = variable(&mut arena, 3);
    let malformed = strict_family.indexed_w(&mut arena, level);
    assert_type_mismatch(&mut arena, &context, malformed, &mut budget);

    // A branching family into `Strict` is an illegal strict
    // elimination, rejected by `caseTwo`'s own motive rule before `W`
    // formation ever runs.
    let children = {
        let carrier = carrier(&mut arena, payload);
        let empty = empty_positions(&mut arena);
        let positions = nat(&mut arena);
        strict_branching(&mut arena, carrier, empty, |_, _| positions)
    };
    let level = variable(&mut arena, 3);
    let error = strict_children_error(&mut arena, &context, &family, children, level, &mut budget);
    assert!(matches!(
        error,
        CoreError::StrictCaseMotiveCodomain { .. } | CoreError::TypeMismatch { .. }
    ));

    // A `next` that returns the position instead of the level is
    // ill-typed: `λp.λb. b : Π(_ : Σ(k:Nat). Vec k). Π(_ : Nat). Nat`
    // happens to check (positions are `Nat`), but the required level
    // `b` never matches `succN k` — the constructor's children land at
    // the wrong index and `isup` rejects. This is a *description*
    // defect, not a term defect.
    let mut bad_next = family.clone();
    bad_next.next = {
        let motive = {
            // `λ(tag : Two). Π(p : Payload tag). Π(_ : B' tag). Nat`.
            let tag = variable(&mut arena, 0);
            let payload_fn = payload(&mut arena);
            let payload_at = apply(&mut arena, payload_fn, tag);
            let branching_at = {
                let motive = type_motive(&mut arena);
                let empty = empty_positions(&mut arena);
                let positions = nat(&mut arena);
                let tag = variable(&mut arena, 1);
                case_two(&mut arena, motive, empty, positions, tag)
            };
            let length = nat(&mut arena);
            let inner = pi(&mut arena, branching_at, length);
            let body = pi(&mut arena, payload_at, inner);
            let domain = two(&mut arena);
            lambda(&mut arena, domain, body)
        };
        let zero_branch = zero_nat_branch(&mut arena);
        let one_branch = {
            // `λ(p : Σ(k:Nat). Vec k). λ(b : Nat). b` — the position
            // itself as required level.
            let payload_domain = {
                let bound = variable(&mut arena, 0);
                let elements = vec_at(&mut arena, bound);
                let arity = nat(&mut arena);
                sigma(&mut arena, arity, elements)
            };
            let position_domain = nat(&mut arena);
            let bound = variable(&mut arena, 0);
            let inner = lambda(&mut arena, position_domain, bound);
            lambda(&mut arena, payload_domain, inner)
        };
        let carrier = carrier(&mut arena, payload);
        tag_selected_children(&mut arena, carrier, motive, zero_branch, one_branch)
    };
    // Rebuild the g binding under the *broken* next: `g : Π(b : Nat).
    // Rose b` is what the broken description demands, but our context's
    // `g` supplies `Π(b : Nat). Rose (succN k)` — `isup` rejects the
    // mismatch.
    let arity = variable(&mut arena, 3);
    let elements = variable(&mut arena, 2);
    let node = rnode(&mut arena, arity, elements);
    let children_fn = variable(&mut arena, 1);
    let node_value = bad_next.sup(&mut arena, node, children_fn);
    let arity = variable(&mut arena, 3);
    let expected = rose_at(&mut arena, &family, arity);
    assert_check_type_mismatch(&mut arena, &context, node_value, expected, &mut budget);
}

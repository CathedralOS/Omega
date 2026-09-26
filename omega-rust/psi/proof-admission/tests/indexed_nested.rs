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

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
    MathematicalCertificate, Signature, Sort, Term, TermArena, TermHandle,
    certificate_assumption_closure, check_signature, check_type, convertible, indexed_scheme,
    infer_sort, infer_type, verify_mathematical_certificate,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

/// The non-eliminator ceiling: this family's `Vec`-nested payload makes
/// every `IW` application re-check the *vector's* `IndexedAt`-based
/// `IW` type inside the `rnode` payload's sort, so formation,
/// constructor and certificate checks cost far more than the vector's
/// `Id`-positioned description — measured spend is pinned per test.
fn family_budget() -> Budget {
    Budget::new(1 << 22)
}

/// The eliminator tests' ceiling: `iindW`'s typing and computation
/// unfold the whole `IndexedAt`/`IW` encoding — the `J` transport, the
/// repacked child functions, the pair-eta closures — for every
/// application, and this family's `next` runs `caseTwo` plus a pair
/// projection and a `succN` per position. Measured spend on this host
/// is 683,485 steps for the leaf computation and 696,985 for the
/// `rnode` one; the bound leaves ~6× headroom so the run stays
/// budgeted rather than open-ended — `StepCeiling` is still the
/// decidability witness.
fn measure_budget() -> Budget {
    Budget::new(1 << 22)
}

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
}

fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
}

fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Sigma { domain, codomain })
}

fn pair(arena: &mut TermArena, first: TermHandle, second: TermHandle) -> TermHandle {
    arena.insert(Term::Pair { first, second })
}

fn fst(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair })
}

fn snd(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Snd { pair })
}

fn two(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Two)
}

fn two_zero(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoZero)
}

fn two_one(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoOne)
}

fn case_two(
    arena: &mut TermArena,
    motive: TermHandle,
    zero_branch: TermHandle,
    one_branch: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::CaseTwo {
        motive,
        zero_branch,
        one_branch,
        scrutinee,
    })
}

fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn w_type(arena: &mut TermArena, carrier: TermHandle, children: TermHandle) -> TermHandle {
    arena.insert(Term::W { carrier, children })
}

fn sup(
    arena: &mut TermArena,
    carrier: TermHandle,
    children: TermHandle,
    label: TermHandle,
    function: TermHandle,
) -> TermHandle {
    arena.insert(Term::Sup {
        carrier,
        children,
        label,
        function,
    })
}

fn constant(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

// ── The producer signature ────────────────────────────────────────────
//
// Positions 0–4 are the derived scheme (`IndexedAt`, `IW`, `iwPack`,
// `isup`, `iindW`); the index arithmetic and element type are the same
// four assumptions the vector demonstration uses, so `Vec` inside the
// nested payload is the same `IW` constant at the same instantiation:

/// `Nat : Type 0` — the index type, declaration 5.
const NAT: u32 = 5;
/// `zeroN : Nat` — declaration 6.
const NAT_ZERO: u32 = 6;
/// `succN : Π(_ : Nat). Nat` — declaration 7.
const NAT_SUCC: u32 = 7;
/// `Elem : Type 0` — the element type, declaration 8.
const ELEM: u32 = 8;

fn nat(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT, Vec::new())
}

fn nat_zero(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT_ZERO, Vec::new())
}

fn nat_succ(arena: &mut TermArena, predecessor: TermHandle) -> TermHandle {
    let succ = constant(arena, NAT_SUCC, Vec::new());
    apply(arena, succ, predecessor)
}

fn elem(arena: &mut TermArena) -> TermHandle {
    constant(arena, ELEM, Vec::new())
}

/// `λ(_ : Two). Type 0` — the constant motive every `caseTwo` selecting
/// *types* here shares.
fn type_motive(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let codomain = type_sort(arena, 0);
    lambda(arena, domain, codomain)
}

/// `Id Two zero one` — the dead child position: no closed inhabitant.
fn empty_positions(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_one(arena);
    id(arena, ty, left, right)
}

// ── The vector description (the family nested inside the payload) ────
//
// `Vec k` is `IW` applied to the vector's own description — repeated
// here exactly as `indexed_vector.rs` encodes it, since the nested
// family's payload references it.

/// `Payload_v = λ(tag : Two). caseTwo(λ_.Type 0, Two, Σ(e : Elem). Nat,
/// tag)` — nil carries a dummy `Two` payload; cons carries
/// `⟨element, predecessor-length⟩`.
fn vec_payload(arena: &mut TermArena) -> TermHandle {
    let motive = type_motive(arena);
    let nil_payload = two(arena);
    let cons_payload = {
        let element = elem(arena);
        let length = nat(arena);
        sigma(arena, element, length)
    };
    let tag = variable(arena, 0);
    let body = case_two(arena, motive, nil_payload, cons_payload, tag);
    let domain = two(arena);
    lambda(arena, domain, body)
}

/// The vector's constructor carrier `A_v = Σ(tag : Two). Payload_v tag`.
fn vec_carrier(arena: &mut TermArena) -> TermHandle {
    let tag = variable(arena, 0);
    let payload_fn = vec_payload(arena);
    let codomain = apply(arena, payload_fn, tag);
    let domain = two(arena);
    sigma(arena, domain, codomain)
}

/// `Id Two zero zero` — the single live child position of a cons.
fn unit_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_zero(arena);
    id(arena, ty, left, right)
}

/// `B_v = λ(a : A_v). caseTwo(λ_.Type 0, Id Two zero one, Id Two zero
/// zero, fst a)` — child positions: none under nil, one under cons.
fn vec_branching(arena: &mut TermArena) -> TermHandle {
    let motive = type_motive(arena);
    let empty = empty_positions(arena);
    let unit = unit_position(arena);
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let body = case_two(arena, motive, empty, unit, tag);
    let domain = vec_carrier(arena);
    lambda(arena, domain, body)
}

/// `out_v = λ(a : A_v). caseTwo(M, λ_. zeroN, λp. succN (snd p), fst a)
/// (snd a)` with `M tag = Π(_ : Payload_v tag). Nat`.
fn vec_out(arena: &mut TermArena) -> TermHandle {
    let motive = {
        let tag = variable(arena, 0);
        let payload_fn = vec_payload(arena);
        let payload_at = apply(arena, payload_fn, tag);
        let length = nat(arena);
        let codomain = pi(arena, payload_at, length);
        let domain = two(arena);
        lambda(arena, domain, codomain)
    };
    let zero_branch = {
        let domain = two(arena);
        let zero = nat_zero(arena);
        lambda(arena, domain, zero)
    };
    let one_branch = {
        let domain = {
            let element = elem(arena);
            let length = nat(arena);
            sigma(arena, element, length)
        };
        let bound = variable(arena, 0);
        let predecessor = snd(arena, bound);
        let body = nat_succ(arena, predecessor);
        lambda(arena, domain, body)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload_component = snd(arena, bound);
    let body = apply(arena, selected, payload_component);
    let domain = vec_carrier(arena);
    lambda(arena, domain, body)
}

/// `next_v = λ(a : A_v). caseTwo(M, λp.λ_. zeroN, λp.λ_. snd p, fst a)
/// (snd a)` with `M tag = Π(p : Payload_v tag). Π(_ : B_v' tag). Nat`.
fn vec_next(arena: &mut TermArena) -> TermHandle {
    let motive = {
        let tag = variable(arena, 0);
        let payload_fn = vec_payload(arena);
        let payload_at = apply(arena, payload_fn, tag);
        let branching_at = {
            // Under the `p` binder the tag is index 1.
            let motive = type_motive(arena);
            let empty = empty_positions(arena);
            let unit = unit_position(arena);
            let tag = variable(arena, 1);
            case_two(arena, motive, empty, unit, tag)
        };
        let length = nat(arena);
        let inner = pi(arena, branching_at, length);
        let body = pi(arena, payload_at, inner);
        let domain = two(arena);
        lambda(arena, domain, body)
    };
    let zero_branch = {
        let payload_domain = two(arena);
        let position_domain = empty_positions(arena);
        let body = nat_zero(arena);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let one_branch = {
        let payload_domain = {
            let element = elem(arena);
            let length = nat(arena);
            sigma(arena, element, length)
        };
        let position_domain = unit_position(arena);
        let bound = variable(arena, 1);
        let body = snd(arena, bound);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload_component = snd(arena, bound);
    let body = apply(arena, selected, payload_component);
    let domain = vec_carrier(arena);
    lambda(arena, domain, body)
}

/// The length-indexed vector family, reused as the nested payload:
/// `Vec k = IW Nat A_v B_v out_v next_v k`.
fn vector_family(arena: &mut TermArena) -> IndexedFamily {
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: nat(arena),
        carrier: vec_carrier(arena),
        children: vec_branching(arena),
        out: vec_out(arena),
        next: vec_next(arena),
    }
}

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

/// The constructor carrier `A = Σ(tag : Two). Payload tag`.
fn carrier(arena: &mut TermArena) -> TermHandle {
    let tag = variable(arena, 0);
    let payload_fn = payload(arena);
    let codomain = apply(arena, payload_fn, tag);
    let domain = two(arena);
    sigma(arena, domain, codomain)
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
    let domain = carrier(arena);
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
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload_component = snd(arena, bound);
    let body = apply(arena, selected, payload_component);
    let domain = carrier(arena);
    lambda(arena, domain, body)
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
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload_component = snd(arena, bound);
    let body = apply(arena, selected, payload_component);
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// The nested rose-tree family: `Rose n = IW Nat A B out next n` with
/// the level-coupled description above.
fn nested_family(arena: &mut TermArena) -> IndexedFamily {
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: nat(arena),
        carrier: carrier(arena),
        children: branching(arena),
        out: out_index(arena),
        next: next_index(arena),
    }
}

/// The producer signature: the five scheme declarations then the four
/// shared assumptions `Nat`, `zeroN`, `succN`, `Elem` — identical to
/// the vector's signature because `Vec` is the same `IW` constant.
fn nested_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Nat
    declarations.push(Declaration::assumption(0, nat(arena))); // zeroN
    let succ_domain = nat(arena);
    let succ_codomain = nat(arena);
    let succ_statement = pi(arena, succ_domain, succ_codomain);
    declarations.push(Declaration::assumption(0, succ_statement)); // succN
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

/// `⟨zero, zero⟩` — a leaf node; its `Two` payload is a dummy.
fn leaf_node(arena: &mut TermArena) -> TermHandle {
    let dummy = two_zero(arena);
    let tag = two_zero(arena);
    pair(arena, tag, dummy)
}

/// `⟨one, ⟨k, v⟩⟩` — an `rnode` label: arity `k`, element vector `v :
/// Vec Elem k`.
fn rnode(arena: &mut TermArena, arity: TermHandle, elements: TermHandle) -> TermHandle {
    let payload_value = pair(arena, arity, elements);
    let tag = two_one(arena);
    pair(arena, tag, payload_value)
}

#[test]
fn the_nested_signature_checks_and_the_family_forms() {
    let mut arena = TermArena::new();
    let mut budget = family_budget();
    let family = nested_family(&mut arena);
    let declarations = nested_declarations(&mut arena);
    assert_eq!(declarations.len(), 9);

    let before = budget.remaining();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 9);
    assert!(before - budget.remaining() > 0);

    // Formation: under `n : Nat`, `Rose n` is a `Type 0` — the family's
    // `max(l, u, v)` lands on the closed level-0 description.
    let index_type = nat(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);
    let level = variable(&mut arena, 0);
    let rose = rose_at(&mut arena, &family, level);
    let sort = infer_sort(&mut arena, &context, rose, &mut budget).unwrap();
    assert!(matches!(sort, Sort::Type(_)));
    let type_zero = type_sort(&mut arena, 0);
    check_type(&mut arena, &context, rose, type_zero, &mut budget).unwrap();
}

#[test]
fn the_indexing_condition_couples_the_vector_payload_and_child_level() {
    let mut arena = TermArena::new();
    let mut budget = family_budget();
    let family = nested_family(&mut arena);
    let signature = nested_signature(&mut arena);

    // Γ = k : Nat, v : Vec k, f : Π(b : B (rnode k v)). W A B, i : Nat —
    // the node, its vector payload and the index all stay symbolic
    // while `out` and `next` compute the level coupling.
    let child_function_type = {
        // Under [k, v]: k = 1, v = 0.
        let arity = variable(&mut arena, 1);
        let elements = variable(&mut arena, 0);
        let node = rnode(&mut arena, arity, elements);
        let domain = apply(&mut arena, family.children, node);
        let carrier = carrier(&mut arena);
        let children = branching(&mut arena);
        let codomain = w_type(&mut arena, carrier, children);
        pi(&mut arena, domain, codomain)
    };
    let arity_type = nat(&mut arena);
    let elements_type = {
        // Under [k]: `Vec b0`.
        let bound = variable(&mut arena, 0);
        vec_at(&mut arena, bound)
    };
    let index_binding = nat(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(arity_type)
        .extend(elements_type)
        .extend(child_function_type)
        .extend(index_binding);
    // In Γ: i = 0, f = 1, v = 2, k = 3.

    // `IndexedAt i (sup (rnode k v) f)` unfolds to
    //   `Σ(_ : Id Nat k i). Π(b : Nat). IndexedAt (succN k) (f b)`
    // — produced index `k` (the vector's length) and every `Nat`
    // position requiring the successor level.
    let condition = {
        let arity = variable(&mut arena, 3);
        let elements = variable(&mut arena, 2);
        let node = rnode(&mut arena, arity, elements);
        let carrier = carrier(&mut arena);
        let children = branching(&mut arena);
        let function = variable(&mut arena, 1);
        let tree = sup(&mut arena, carrier, children, node, function);
        let index = variable(&mut arena, 0);
        family.indexed_at(&mut arena, index, tree)
    };
    let expected = {
        let domain = {
            let ty = nat(&mut arena);
            let left = variable(&mut arena, 3); // k
            let right = variable(&mut arena, 0); // i
            id(&mut arena, ty, left, right)
        };
        let codomain = {
            // Under [b, _, i, f, v, k]: b = 0, f = 3, k = 5.
            let arity = variable(&mut arena, 5);
            let required = nat_succ(&mut arena, arity);
            let f = variable(&mut arena, 3);
            let b = variable(&mut arena, 0);
            let child = apply(&mut arena, f, b);
            family.indexed_at(&mut arena, required, child)
        };
        let positions = nat(&mut arena);
        let body = pi(&mut arena, positions, codomain);
        sigma(&mut arena, domain, body)
    };
    let type_zero = type_sort(&mut arena, 0);
    // Both sides are checked types before conversion is consulted.
    check_type(&mut arena, &context, condition, type_zero, &mut budget).unwrap();
    check_type(&mut arena, &context, expected, type_zero, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &context,
            condition,
            expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // The arity coupling is what *distinguishes* the level: the same
    // `Id Nat k i` front with children required at `k` instead of
    // `succN k` is a different, false equation.
    let shifted_children = {
        let domain = {
            let ty = nat(&mut arena);
            let left = variable(&mut arena, 3); // k
            let right = variable(&mut arena, 0); // i
            id(&mut arena, ty, left, right)
        };
        let positions = nat(&mut arena);
        let codomain = {
            // Under [b, _, i, f, v, k]: b = 0, f = 3, k = 5 — children
            // required at `k`, not `succN k`.
            let arity = variable(&mut arena, 5);
            let f = variable(&mut arena, 3);
            let b = variable(&mut arena, 0);
            let child = apply(&mut arena, f, b);
            family.indexed_at(&mut arena, arity, child)
        };
        let body = pi(&mut arena, positions, codomain);
        sigma(&mut arena, domain, body)
    };
    assert!(
        !convertible(
            &mut arena,
            &context,
            condition,
            shifted_children,
            type_zero,
            &mut budget
        )
        .unwrap()
    );
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
            let next_a = apply(arena, family.next, node);
            let bound = variable(arena, 0);
            let required = apply(arena, next_a, bound);
            family.indexed_w(arena, required)
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
fn constructors_build_at_their_level_indices() {
    let mut arena = TermArena::new();
    let mut budget = family_budget();
    let family = nested_family(&mut arena);
    let context = nested_context(&mut arena, &family);
    // In Γ: g = 0, v = 1, k = 2.

    // `rnode k v g := isup ⟨one, ⟨k, v⟩⟩ g : Rose k` — the vector
    // payload `v` and the child function `g` are both context-bound,
    // hence neutral; `out` computes the produced level `fst ⟨k,v⟩ = k`.
    let arity = variable(&mut arena, 2);
    let elements = variable(&mut arena, 1);
    let node = rnode(&mut arena, arity, elements);
    let children_fn = variable(&mut arena, 0);
    let node_value = family.sup(&mut arena, node, children_fn);
    let arity = variable(&mut arena, 2);
    let expected = rose_at(&mut arena, &family, arity);
    check_type(&mut arena, &context, node_value, expected, &mut budget).unwrap();

    // `leaf := isup ⟨zero, zero⟩ g'` for a context-bound — hence
    // neutral — `g' : Π(b : B leaf). Rose (next leaf b) ≡ Π(_ : Id Two
    // zero one). Rose zeroN`: every child position is dead, so the
    // function is vacuous and supplied abstractly.
    let leaf_children_type = {
        let node = leaf_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let codomain = {
            let zero = nat_zero(&mut arena);
            rose_at(&mut arena, &family, zero)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = context.extend(leaf_children_type);
    // In Γ: g' = 0.
    let leaf = {
        let node = leaf_node(&mut arena);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, node, function)
    };
    let zero = nat_zero(&mut arena);
    let expected = rose_at(&mut arena, &family, zero);
    check_type(&mut arena, &context, leaf, expected, &mut budget).unwrap();
}

/// The eliminator context `Q : Π(i : Nat). Π(_ : Rose i). Type 0`,
/// `s : <iindW step>`, `k : Nat`, `v : Vec k`, `g : Π(b : B (rnode k
/// v)). Rose (next (rnode k v) b)` — indices g = 0, v = 1, k = 2,
/// s = 3, Q = 4.
fn eliminator_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let motive_type = {
        let index = variable(arena, 0);
        let family_at = rose_at(arena, family, index);
        let type_zero = type_sort(arena, 0);
        let inner = pi(arena, family_at, type_zero);
        let index_type = nat(arena);
        pi(arena, index_type, inner)
    };
    let step_type = {
        // `Π(a : A). Π(g : Π(b : B a). Rose (next a b)).
        //   Π(_ : Π(b : B a). Q (next a b) (g b)). Q (out a) (isup a g)`
        // written under the one-binding prefix [Q].
        let g_type = {
            // Under a: a = 0, Q = 1.
            let bound = variable(arena, 0);
            let b_domain = apply(arena, family.children, bound);
            let codomain = {
                // Under b: a = 1.
                let bound_a = variable(arena, 1);
                let next_a = apply(arena, family.next, bound_a);
                let bound = variable(arena, 0);
                let required = apply(arena, next_a, bound);
                rose_at(arena, family, required)
            };
            pi(arena, b_domain, codomain)
        };
        let hypothesis_type = {
            // Under g, a: a = 1, g = 0, Q = 2.
            let bound_a = variable(arena, 1);
            let b_domain = apply(arena, family.children, bound_a);
            let codomain = {
                // Under b: a = 2, g = 1, Q = 3.
                let bound_a = variable(arena, 2);
                let next_a = apply(arena, family.next, bound_a);
                let bound = variable(arena, 0);
                let required = apply(arena, next_a, bound);
                let motive = variable(arena, 3);
                let at_index = apply(arena, motive, required);
                let bound_g = variable(arena, 1);
                let bound = variable(arena, 0);
                let child = apply(arena, bound_g, bound);
                apply(arena, at_index, child)
            };
            pi(arena, b_domain, codomain)
        };
        let result = {
            // Under the hypothesis, g, a: a = 2, g = 1, Q = 3.
            let bound_a = variable(arena, 2);
            let produced = apply(arena, family.out, bound_a);
            let bound_a = variable(arena, 2);
            let bound_g = variable(arena, 1);
            let node = family.sup(arena, bound_a, bound_g);
            let motive = variable(arena, 3);
            let at_index = apply(arena, motive, produced);
            apply(arena, at_index, node)
        };
        let inner = pi(arena, hypothesis_type, result);
        let middle = pi(arena, g_type, inner);
        let domain = carrier(arena);
        pi(arena, domain, middle)
    };
    let arity_type = nat(arena);
    let elements_type = {
        // Under [k]: `Vec b0`.
        let bound = variable(arena, 0);
        vec_at(arena, bound)
    };
    let children_type = {
        // Installed at position 0; the prefix's last two entries are
        // v (index 0) and k (index 1).
        let arity = variable(arena, 1);
        let elements = variable(arena, 0);
        let node = rnode(arena, arity, elements);
        let domain = apply(arena, family.children, node);
        let codomain = {
            // Under the `b` binder: k = 2, v = 1, b = 0.
            let arity = variable(arena, 2);
            let elements = variable(arena, 1);
            let node = rnode(arena, arity, elements);
            let next_a = apply(arena, family.next, node);
            let bound = variable(arena, 0);
            let required = apply(arena, next_a, bound);
            family.indexed_w(arena, required)
        };
        pi(arena, domain, codomain)
    };
    Context::empty()
        .with_signature(nested_signature(arena))
        .extend(motive_type)
        .extend(step_type)
        .extend(arity_type)
        .extend(elements_type)
        .extend(children_type)
}

#[test]
fn induction_computes_on_rnode_with_neutral_payload_and_children() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = nested_family(&mut arena);
    let context = eliminator_context(&mut arena, &family);
    // In Γ: g = 0, v = 1, k = 2, s = 3, Q = 4.

    // The `rnode` construction under the eliminator context:
    // `rnode k v g = isup ⟨one, ⟨k, v⟩⟩ g` — `v` a neutral `Vec k`, `g`
    // a neutral `Π(b : Nat). Rose (succN k)` child function.
    let arity = variable(&mut arena, 2);
    let elements = variable(&mut arena, 1);
    let node = rnode(&mut arena, arity, elements);
    let children_fn = variable(&mut arena, 0);
    let rnode_value = family.sup(&mut arena, node, children_fn);

    // `iindW Q s k (rnode k v g) : Q k (rnode k v g)`.
    let arity = variable(&mut arena, 2);
    let motive = variable(&mut arena, 4);
    let step = variable(&mut arena, 3);
    let elimination = family.ind(
        &mut arena,
        Level::Constant(0),
        motive,
        step,
        arity,
        rnode_value,
    );
    let shared = {
        let motive = variable(&mut arena, 4);
        let arity = variable(&mut arena, 2);
        let at_index = apply(&mut arena, motive, arity);
        apply(&mut arena, at_index, rnode_value)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // The profile's constructor computation judgment, with `v` and `g`
    // still neutral variables:
    //   `iindW Q s k (rnode k v g)`
    //     `≡ s ⟨one,⟨k,v⟩⟩ g (b ↦ iindW Q s (next (rnode k v) b) (g b))`
    //     `≡ s ⟨one,⟨k,v⟩⟩ g (b ↦ iindW Q s (succN k) (g b))`
    // — the induction hypothesis lands one level deeper than the node.
    let hypothesis = {
        // The domain `B (rnode k v)` is written at top level (k = 2,
        // v = 1); under the hypothesis's own `b` binder everything
        // shifts one further: g = 1, k = 3, v = 2, s = 4, Q = 5.
        let arity = variable(&mut arena, 2);
        let elements = variable(&mut arena, 1);
        let node = rnode(&mut arena, arity, elements);
        let domain = apply(&mut arena, family.children, node);
        let body = {
            let arity = variable(&mut arena, 3);
            let elements = variable(&mut arena, 2);
            let node = rnode(&mut arena, arity, elements);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            let g = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, g, bound);
            let motive = variable(&mut arena, 5);
            let step = variable(&mut arena, 4);
            family.ind(
                &mut arena,
                Level::Constant(0),
                motive,
                step,
                required,
                child,
            )
        };
        lambda(&mut arena, domain, body)
    };
    let expected = {
        let step = variable(&mut arena, 3);
        let arity = variable(&mut arena, 2);
        let elements = variable(&mut arena, 1);
        let node = rnode(&mut arena, arity, elements);
        let at_node = apply(&mut arena, step, node);
        let g = variable(&mut arena, 0);
        let at_children = apply(&mut arena, at_node, g);
        apply(&mut arena, at_children, hypothesis)
    };
    check_type(&mut arena, &context, expected, shared, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &context,
            elimination,
            expected,
            shared,
            &mut budget
        )
        .unwrap()
    );

    // Work receipt: checking the `iindW` application and closing the
    // computation across the packed `IW` pair over this description.
    // Measured, not a quota — see the run evidence in the commit.
    let spent = total - budget.remaining();
    eprintln!("rnode elimination + computation spent {spent} steps");
    assert!(spent > 0);
}

#[test]
fn induction_computes_on_leaf_with_a_vacuous_child_function() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = nested_family(&mut arena);

    // Γ = Q, s, g' where `g' : Π(b : B leaf). Rose (next leaf b)` is the
    // vacuous — and therefore neutral — child function of a leaf node.
    let leaf_children_type = {
        let node = leaf_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let codomain = {
            let node = leaf_node(&mut arena);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            rose_at(&mut arena, &family, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let motive_type = {
        let index = variable(&mut arena, 0);
        let family_at = rose_at(&mut arena, &family, index);
        let type_zero = type_sort(&mut arena, 0);
        let inner = pi(&mut arena, family_at, type_zero);
        let index_type = nat(&mut arena);
        pi(&mut arena, index_type, inner)
    };
    let step_type = {
        let g_type = {
            let bound = variable(&mut arena, 0);
            let b_domain = apply(&mut arena, family.children, bound);
            let codomain = {
                let bound_a = variable(&mut arena, 1);
                let next_a = apply(&mut arena, family.next, bound_a);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                rose_at(&mut arena, &family, required)
            };
            pi(&mut arena, b_domain, codomain)
        };
        let hypothesis_type = {
            let bound_a = variable(&mut arena, 1);
            let b_domain = apply(&mut arena, family.children, bound_a);
            let codomain = {
                let bound_a = variable(&mut arena, 2);
                let next_a = apply(&mut arena, family.next, bound_a);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let motive = variable(&mut arena, 3);
                let at_index = apply(&mut arena, motive, required);
                let bound_g = variable(&mut arena, 1);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, bound_g, bound);
                apply(&mut arena, at_index, child)
            };
            pi(&mut arena, b_domain, codomain)
        };
        let result = {
            let bound_a = variable(&mut arena, 2);
            let produced = apply(&mut arena, family.out, bound_a);
            let bound_a = variable(&mut arena, 2);
            let bound_g = variable(&mut arena, 1);
            let node = family.sup(&mut arena, bound_a, bound_g);
            let motive = variable(&mut arena, 3);
            let at_index = apply(&mut arena, motive, produced);
            apply(&mut arena, at_index, node)
        };
        let inner = pi(&mut arena, hypothesis_type, result);
        let middle = pi(&mut arena, g_type, inner);
        let domain = carrier(&mut arena);
        pi(&mut arena, domain, middle)
    };
    let context = Context::empty()
        .with_signature(nested_signature(&mut arena))
        .extend(motive_type)
        .extend(step_type)
        .extend(leaf_children_type);
    // In Γ: g' = 0, s = 1, Q = 2.

    // `leaf := isup ⟨zero, zero⟩ g'`, `iindW Q s zeroN leaf : Q zeroN
    // leaf`.
    let leaf = {
        let node = leaf_node(&mut arena);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, node, function)
    };
    let zero = nat_zero(&mut arena);
    let motive = variable(&mut arena, 2);
    let step = variable(&mut arena, 1);
    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, zero, leaf);
    let shared = {
        let motive = variable(&mut arena, 2);
        let zero = nat_zero(&mut arena);
        let at_index = apply(&mut arena, motive, zero);
        apply(&mut arena, at_index, leaf)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // `iindW Q s zeroN leaf ≡ s ⟨zero, zero⟩ g' ih'` where `ih' : Π(b :
    // Id Two zero one). …` is vacuous — every hypothesis lands on a
    // dead position.
    let hypothesis = {
        let node = leaf_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let body = {
            // Under b: g' = 1, s = 2, Q = 3.
            let node = leaf_node(&mut arena);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            let g = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, g, bound);
            let motive = variable(&mut arena, 3);
            let step = variable(&mut arena, 2);
            family.ind(
                &mut arena,
                Level::Constant(0),
                motive,
                step,
                required,
                child,
            )
        };
        lambda(&mut arena, domain, body)
    };
    let expected = {
        let step = variable(&mut arena, 1);
        let node = leaf_node(&mut arena);
        let at_node = apply(&mut arena, step, node);
        let g = variable(&mut arena, 0);
        let at_children = apply(&mut arena, at_node, g);
        apply(&mut arena, at_children, hypothesis)
    };
    check_type(&mut arena, &context, expected, shared, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &context,
            elimination,
            expected,
            shared,
            &mut budget
        )
        .unwrap()
    );

    let spent = total - budget.remaining();
    eprintln!("leaf elimination + computation spent {spent} steps");
    assert!(spent > 0);
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
    let error = check_type(&mut arena, &context, malformed, expected, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A value where the index type belongs is malformed, not a type:
    // `I = zeroN` instead of `Nat`.
    let mut bad_family = family.clone();
    bad_family.index = nat_zero(&mut arena);
    let level = variable(&mut arena, 3);
    let malformed = bad_family.indexed_w(&mut arena, level);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A strict index type is a malformed universe for `I : Type l`.
    let mut strict_family = family.clone();
    strict_family.index = strict_sort(&mut arena, 0);
    let level = variable(&mut arena, 3);
    let malformed = strict_family.indexed_w(&mut arena, level);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A branching family into `Strict` is an illegal strict
    // elimination, rejected by `caseTwo`'s own motive rule before `W`
    // formation ever runs.
    let mut strict = family.clone();
    strict.children = {
        let motive = {
            let domain = two(&mut arena);
            let codomain = strict_sort(&mut arena, 0);
            lambda(&mut arena, domain, codomain)
        };
        let empty = empty_positions(&mut arena);
        let positions = nat(&mut arena);
        let bound = variable(&mut arena, 0);
        let tag = fst(&mut arena, bound);
        let body = case_two(&mut arena, motive, empty, positions, tag);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let level = variable(&mut arena, 3);
    let malformed = strict.indexed_w(&mut arena, level);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
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
        let zero_branch = {
            let payload_domain = two(&mut arena);
            let position_domain = empty_positions(&mut arena);
            let body = nat_zero(&mut arena);
            let inner = lambda(&mut arena, position_domain, body);
            lambda(&mut arena, payload_domain, inner)
        };
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
        let bound = variable(&mut arena, 0);
        let tag = fst(&mut arena, bound);
        let selected = case_two(&mut arena, motive, zero_branch, one_branch, tag);
        let bound = variable(&mut arena, 0);
        let payload_component = snd(&mut arena, bound);
        let body = apply(&mut arena, selected, payload_component);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, body)
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
    let error = check_type(&mut arena, &context, node_value, expected, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));
}

#[test]
fn a_nested_certificate_verifies_with_exact_closure_and_bounded_cost() {
    let mut arena = TermArena::new();
    let mut budget = family_budget();
    let family = nested_family(&mut arena);
    let declarations = nested_declarations(&mut arena);

    // Γ = k : Nat, v : Vec k, g : Π(b : B (rnode k v)). Rose (next
    // (rnode k v) b) ⊢ rnode k v g : Rose k — the whole judgment,
    // signature included, re-decided from data.
    let children_type = {
        // Under [k, v]: k = 1, v = 0.
        let arity = variable(&mut arena, 1);
        let elements = variable(&mut arena, 0);
        let node = rnode(&mut arena, arity, elements);
        let domain = apply(&mut arena, family.children, node);
        let codomain = {
            // Under [b, v, k]: k = 2, v = 1, b = 0.
            let arity = variable(&mut arena, 2);
            let elements = variable(&mut arena, 1);
            let node = rnode(&mut arena, arity, elements);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            family.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let arity = variable(&mut arena, 2);
    let elements = variable(&mut arena, 1);
    let node = rnode(&mut arena, arity, elements);
    let children_fn = variable(&mut arena, 0);
    let node_value = family.sup(&mut arena, node, children_fn);
    let expected = {
        let arity = variable(&mut arena, 2);
        rose_at(&mut arena, &family, arity)
    };
    let arity_type = nat(&mut arena);
    let elements_type = {
        // Under [k]: `Vec b0`.
        let bound = variable(&mut arena, 0);
        vec_at(&mut arena, bound)
    };
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![arity_type, elements_type, children_type],
        term: node_value,
        expected,
    };
    let before = budget.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    let spent = before - budget.remaining();
    assert!(spent > 0, "checking the certificate must do real work");

    // The judgment commits to exactly the four producer assumptions —
    // `Nat`, `zeroN`, `succN`, `Elem` — and to none of the scheme's
    // *definitions*, which carry no assumption force.
    let closure = certificate_assumption_closure(&arena, &certificate);
    assert_eq!(
        closure,
        [NAT, NAT_ZERO, NAT_SUCC, ELEM].into_iter().collect()
    );

    // The certificate claiming `Rose (succN k)` — the children's level —
    // is a different, false judgment.
    let wrong = {
        let arity = variable(&mut arena, 2);
        let successor = nat_succ(&mut arena, arity);
        rose_at(&mut arena, &family, successor)
    };
    let arity_type = nat(&mut arena);
    let elements_type = {
        let bound = variable(&mut arena, 0);
        vec_at(&mut arena, bound)
    };
    let forged = MathematicalCertificate {
        signature: nested_declarations(&mut arena),
        level_arity: 0,
        context: vec![arity_type, elements_type, children_type],
        term: node_value,
        expected: wrong,
    };
    assert!(matches!(
        verify_mathematical_certificate(&mut arena, &forged, &mut budget),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Retained-storage and work receipts: checking the certificate —
    // the full nine-declaration signature plus the `isup` judgment over
    // a payload that itself instantiates `IW` — re-materializes
    // substituted instances of the encoding. Both numbers are
    // measured, not quotas; the exact values are pinned in the commit.
    assert!(!arena.is_empty());
    assert!(spent > 0);
}

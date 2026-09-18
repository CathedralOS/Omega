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

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
    MathematicalCertificate, Signature, Sort, Term, TermArena, TermHandle,
    certificate_assumption_closure, check_signature, check_type, convertible, indexed_scheme,
    infer_sort, infer_type, shift, verify_mathematical_certificate,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

/// The eliminator tests' ceiling: `iindW`'s typing and computation
/// unfold the whole `IndexedAt`/`IW` encoding — the `J` transport, the
/// repacked child functions, the pair-eta closures — for every
/// application over this larger description. Measured spend is
/// ~703–708 thousand steps per test below; this bound leaves roughly
/// half-again headroom so the run stays budgeted rather than
/// open-ended — `StepCeiling` is still the decidability witness.
fn measure_budget() -> Budget {
    Budget::new(1 << 20)
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
// `isup`, `iindW`); the vector's index arithmetic and element type are
// the appended assumptions:

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

/// `Id Two zero one` — the empty child position: no closed inhabitant,
/// so a node whose `B` selects it has no reachable children.
fn empty_positions(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_one(arena);
    id(arena, ty, left, right)
}

/// `Id Two zero zero` — the single child position, inhabited by `refl`.
fn unit_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_zero(arena);
    id(arena, ty, left, right)
}

/// `Payload = λ(tag : Two). caseTwo(λ_.Type 0, Two, Σ(e : Elem). Nat,
/// tag)` — nil carries a dummy `Two` payload; cons carries
/// `⟨element, predecessor-length⟩`.
fn payload(arena: &mut TermArena) -> TermHandle {
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

/// The constructor carrier `A = Σ(tag : Two). Payload tag`.
fn carrier(arena: &mut TermArena) -> TermHandle {
    let tag = variable(arena, 0);
    let payload_fn = payload(arena);
    let codomain = apply(arena, payload_fn, tag);
    let domain = two(arena);
    sigma(arena, domain, codomain)
}

/// `B = λ(a : A). caseTwo(λ_.Type 0, Id Two zero one, Id Two zero zero,
/// fst a)` — child positions selected by tag: none under nil, one under
/// cons.
fn branching(arena: &mut TermArena) -> TermHandle {
    let motive = type_motive(arena);
    let empty = empty_positions(arena);
    let unit = unit_position(arena);
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let body = case_two(arena, motive, empty, unit, tag);
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// `out = λ(a : A). caseTwo(M, λ_. zeroN, λp. succN (snd p), fst a)
/// (snd a)` with `M tag = Π(_ : Payload tag). Nat` — the index a node
/// produces: `zeroN` under nil, `succN n` under `cons ⟨e, n⟩`.
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
        // `λ(_ : Two). zeroN`, checked at `Π(_ : Payload zero ≡ Two). Nat`.
        let domain = two(arena);
        let zero = nat_zero(arena);
        lambda(arena, domain, zero)
    };
    let one_branch = {
        // `λ(p : Σ(e : Elem). Nat). succN (snd p)` at
        // `Π(_ : Payload one ≡ Σ(e : Elem). Nat). Nat`.
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
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// `next = λ(a : A). caseTwo(M, λp.λ_. zeroN, λp.λ_. snd p, fst a)
/// (snd a)` with `M tag = Π(p : Payload tag). Π(_ : B' tag). Nat` and
/// `B' tag = caseTwo(λ_.Type 0, Id Two zero one, Id Two zero zero, tag)`
/// — the index each child position requires: unreachable under nil
/// (any `Nat` answers), and `snd p = n`, the predecessor, under cons.
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
        // `λ(p : Two). λ(_ : Id Two zero one). zeroN` at
        // `Π(_ : Payload zero). Π(_ : B' zero). Nat` — the dead position
        // is answered by any length.
        let payload_domain = two(arena);
        let position_domain = empty_positions(arena);
        let body = nat_zero(arena);
        let inner = lambda(arena, position_domain, body);
        lambda(arena, payload_domain, inner)
    };
    let one_branch = {
        // `λ(p : Σ(e : Elem). Nat). λ(_ : Id Two zero zero). snd p` —
        // the child's required index is the recorded predecessor.
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
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// The length-indexed vector family over the assumed `Nat`/`Elem`:
/// `Vec n = IW Nat A B out next n` with the computed description above.
fn vector_family(arena: &mut TermArena) -> IndexedFamily {
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
/// vector assumptions `Nat`, `zeroN`, `succN`, `Elem`.
fn vector_declarations(arena: &mut TermArena) -> Vec<Declaration> {
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

#[test]
fn the_vector_scheme_signature_checks_and_the_family_forms() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = vector_family(&mut arena);
    let declarations = vector_declarations(&mut arena);
    assert_eq!(declarations.len(), 9);

    let before = budget.remaining();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 9);
    assert!(before - budget.remaining() > 0);

    // Formation: under `n : Nat`, `Vec n` is a `Type 0` — the family's
    // `max(l, u, v)` lands on the closed level-0 description.
    let index_type = nat(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);
    let length = variable(&mut arena, 0);
    let vector = vec_at(&mut arena, &family, length);
    let sort = infer_sort(&mut arena, &context, vector, &mut budget).unwrap();
    assert!(matches!(sort, Sort::Type(_)));
    let type_zero = type_sort(&mut arena, 0);
    check_type(&mut arena, &context, vector, type_zero, &mut budget).unwrap();
}

#[test]
fn the_indexing_condition_couples_parent_and_child_lengths() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = vector_family(&mut arena);
    let signature = vector_signature(&mut arena);

    // Γ = e : Elem, n : Nat, k : Π(b : B (cons e n)). W A B, i : Nat —
    // the node and the index stay symbolic while `out` and `next`
    // compute the length coupling.
    let child_function_type = {
        let element = variable(&mut arena, 1);
        let predecessor = variable(&mut arena, 0);
        let node = cons_node(&mut arena, element, predecessor);
        let domain = apply(&mut arena, family.children, node);
        let carrier = carrier(&mut arena);
        let children = branching(&mut arena);
        let codomain = w_type(&mut arena, carrier, children);
        pi(&mut arena, domain, codomain)
    };
    let element_type = elem(&mut arena);
    let index_type = nat(&mut arena);
    let index_binding = nat(&mut arena);
    let cons_context = Context::empty()
        .with_signature(signature)
        .extend(element_type)
        .extend(index_type)
        .extend(child_function_type)
        .extend(index_binding);
    // In Γ: i = 0, k = 1, n = 2, e = 3.

    let node = {
        let element = variable(&mut arena, 3);
        let predecessor = variable(&mut arena, 2);
        let label = cons_node(&mut arena, element, predecessor);
        let carrier = carrier(&mut arena);
        let children = branching(&mut arena);
        let function = variable(&mut arena, 1);
        sup(&mut arena, carrier, children, label, function)
    };
    let index = variable(&mut arena, 0);
    let condition = family.indexed_at(&mut arena, index, node);

    // `IndexedAt i (sup (cons e n) k)` unfolds to
    //   `Σ(_ : Id Nat (succN n) i). Π(b : Id Two zero zero).
    //      IndexedAt n (k b)`
    // — the node's produced index is `succN n` and every child's
    // required index is the predecessor `n`.
    let expected = {
        let domain = {
            let ty = nat(&mut arena);
            let predecessor = variable(&mut arena, 2);
            let produced = nat_succ(&mut arena, predecessor);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, produced, index)
        };
        let codomain = {
            // Under the dead-name binder: i = 1, k = 2, n = 3, e = 4.
            let element = variable(&mut arena, 4);
            let predecessor = variable(&mut arena, 3);
            let node = cons_node(&mut arena, element, predecessor);
            let b_domain = apply(&mut arena, family.children, node);
            let body = {
                // Under b: i = 2, k = 3, n = 4, e = 5.
                let element = variable(&mut arena, 5);
                let predecessor = variable(&mut arena, 4);
                let node = cons_node(&mut arena, element, predecessor);
                let next_a = apply(&mut arena, family.next, node);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, required, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    let type_zero = type_sort(&mut arena, 0);
    check_type(&mut arena, &cons_context, condition, type_zero, &mut budget).unwrap();
    check_type(&mut arena, &cons_context, expected, type_zero, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &cons_context,
            condition,
            expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // And at nil the same equation gives `zeroN` on both sides:
    // `IndexedAt i (sup nil k') ≡ Σ(_ : Id Nat zeroN i). Π(b : Id Two
    // zero one). IndexedAt zeroN (k' b)` — a leaf's children are all
    // unreachable, and the produced index is exactly `zeroN`.
    let nil_function_type = {
        let node = nil_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let carrier = carrier(&mut arena);
        let children = branching(&mut arena);
        let codomain = w_type(&mut arena, carrier, children);
        pi(&mut arena, domain, codomain)
    };
    let nil_index_binding = nat(&mut arena);
    let context = Context::empty()
        .with_signature(vector_signature(&mut arena))
        .extend(nil_function_type)
        .extend(nil_index_binding);
    // In Γ': i = 0, k' = 1.
    let node = {
        let label = nil_node(&mut arena);
        let carrier = carrier(&mut arena);
        let children = branching(&mut arena);
        let function = variable(&mut arena, 1);
        sup(&mut arena, carrier, children, label, function)
    };
    let index = variable(&mut arena, 0);
    let nil_condition = family.indexed_at(&mut arena, index, node);
    let nil_expected = {
        let domain = {
            let ty = nat(&mut arena);
            let produced = nat_zero(&mut arena);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, produced, index)
        };
        let codomain = {
            // Under the binder: i = 1, k' = 2.
            let node = nil_node(&mut arena);
            let b_domain = apply(&mut arena, family.children, node);
            let body = {
                // Under b: i = 2, k' = 3.
                let node = nil_node(&mut arena);
                let next_a = apply(&mut arena, family.next, node);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, required, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(&mut arena, &context, nil_condition, type_zero, &mut budget).unwrap();
    check_type(&mut arena, &context, nil_expected, type_zero, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &context,
            nil_condition,
            nil_expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // And the produced index is the *successor*, not the predecessor:
    // back in the cons context `e, n, k, i`, the cons condition never
    // collapses to `Σ(_ : Id Nat n i). Π(b : B (cons e n)). IndexedAt
    // (next (cons e n) b) (k b)` — `succN n ≢ n` in the `Id` domain is
    // what separates the constructors.
    let wrong_index_form = {
        let domain = {
            let ty = nat(&mut arena);
            let predecessor = variable(&mut arena, 2);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, predecessor, index)
        };
        let codomain = {
            // Under the binder: i = 1, k = 2, n = 3, e = 4.
            let element = variable(&mut arena, 4);
            let predecessor = variable(&mut arena, 3);
            let node = cons_node(&mut arena, element, predecessor);
            let b_domain = apply(&mut arena, family.children, node);
            let body = {
                // Under b: i = 2, k = 3, n = 4, e = 5.
                let element = variable(&mut arena, 5);
                let predecessor = variable(&mut arena, 4);
                let node = cons_node(&mut arena, element, predecessor);
                let next_a = apply(&mut arena, family.next, node);
                let bound = variable(&mut arena, 0);
                let required = apply(&mut arena, next_a, bound);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, required, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(
        &mut arena,
        &cons_context,
        wrong_index_form,
        type_zero,
        &mut budget,
    )
    .unwrap();
    assert!(
        !convertible(
            &mut arena,
            &cons_context,
            condition,
            wrong_index_form,
            type_zero,
            &mut budget
        )
        .unwrap()
    );
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

#[test]
fn constructors_build_at_their_length_indices() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = vector_family(&mut arena);
    let context = vector_context(&mut arena, &family);
    // In Γ: tail = 0, n = 1, e = 2.

    // `cons e n tail := isup ⟨one, ⟨e, n⟩⟩ (λ(_ : B (cons e n)). tail)`
    // — the child function ignores its single `Id Two zero zero`
    // position and returns the tail, whose required index `next`
    // computes to `n`.
    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);
    let predecessor = variable(&mut arena, 1);
    let successor = nat_succ(&mut arena, predecessor);
    let expected = vec_at(&mut arena, &family, successor);
    check_type(&mut arena, &context, cons, expected, &mut budget).unwrap();

    // `nil := isup ⟨zero, zero⟩ g` for a context-bound — hence neutral —
    // `g : Π(b : B nil). Vec (next nil b) ≡ Π(_ : Id Two zero one).
    // Vec zeroN`: every child position is dead, so the function is
    // vacuous and supplied abstractly.
    let nil_children_type = {
        let node = nil_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let codomain = {
            let zero = nat_zero(&mut arena);
            vec_at(&mut arena, &family, zero)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = context.extend(nil_children_type);
    // In Γ, g : g = 0.
    let nil = {
        let node = nil_node(&mut arena);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, node, function)
    };
    let zero = nat_zero(&mut arena);
    let expected = vec_at(&mut arena, &family, zero);
    check_type(&mut arena, &context, nil, expected, &mut budget).unwrap();
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
    let step_type = {
        // `Π(a : A). Π(g : Π(b : B a). Vec (next a b)).
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
                vec_at(arena, family, required)
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
fn induction_computes_on_cons_with_a_neutral_tail() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = vector_family(&mut arena);
    let context = eliminator_context(&mut arena, &family);
    // In Γ: tail = 0, n = 1, e = 2, s = 3, Q = 4.

    // The cons construction again under the eliminator context:
    // `cons e n tail = isup ⟨one, ⟨e, n⟩⟩ (λ_. tail)`.
    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);

    // `iindW Q s (succN n) (cons e n tail) : Q (succN n) (cons e n tail)`.
    let predecessor = variable(&mut arena, 1);
    let successor = nat_succ(&mut arena, predecessor);
    let motive = variable(&mut arena, 4);
    let step = variable(&mut arena, 3);
    let elimination = family.ind(
        &mut arena,
        Level::Constant(0),
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
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // The profile's constructor computation judgment, with `tail` still
    // a neutral variable:
    //   `iindW Q s (succN n) (cons e n tail)`
    //     `≡ s ⟨one,⟨e,n⟩⟩ (λ_. tail) (b ↦ iindW Q s (next node b)
    //          ((λ_.tail) b))`
    //     `≡ s ⟨one,⟨e,n⟩⟩ (λ_. tail) (b ↦ iindW Q s n tail)`
    // — the induction hypothesis lands at the predecessor length.
    let hypothesis = {
        // The domain `B (cons e n)` is written at top level (e = 2,
        // n = 1); under the hypothesis's own `b` binder everything
        // shifts one further: tail = 1, n = 2, e = 3, s = 4, Q = 5.
        let element = variable(&mut arena, 2);
        let predecessor = variable(&mut arena, 1);
        let node = cons_node(&mut arena, element, predecessor);
        let domain = apply(&mut arena, family.children, node);
        let shifted_children = shift(&mut arena, children_fn, 0, 1);
        let body = {
            let element = variable(&mut arena, 3);
            let predecessor = variable(&mut arena, 2);
            let node = cons_node(&mut arena, element, predecessor);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, shifted_children, bound);
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
        let at_node = apply(&mut arena, step, node);
        let at_children = apply(&mut arena, at_node, children_fn);
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
    // computation across the packed `IW` pair costs 43,636 budgeted
    // steps over this description.
    assert_eq!(total - budget.remaining(), 43_636);
}

#[test]
fn induction_computes_on_nil_with_a_vacuous_child_function() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = vector_family(&mut arena);

    // Γ = Q, s, g' where `g' : Π(b : B nil). Vec (next nil b)` is the
    // vacuous — and therefore neutral — child function of a nil node.
    let nil_children_type = {
        let node = nil_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let codomain = {
            let node = nil_node(&mut arena);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            vec_at(&mut arena, &family, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let motive_type = {
        let index = variable(&mut arena, 0);
        let family_at = vec_at(&mut arena, &family, index);
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
                vec_at(&mut arena, &family, required)
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
        .with_signature(vector_signature(&mut arena))
        .extend(motive_type)
        .extend(step_type)
        .extend(nil_children_type);
    // In Γ: g' = 0, s = 1, Q = 2.

    let nil = {
        let node = nil_node(&mut arena);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, node, function)
    };
    let zero = nat_zero(&mut arena);
    let motive = variable(&mut arena, 2);
    let step = variable(&mut arena, 1);
    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, zero, nil);
    let shared = {
        let motive = variable(&mut arena, 2);
        let zero = nat_zero(&mut arena);
        let at_index = apply(&mut arena, motive, zero);
        apply(&mut arena, at_index, nil)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // `iindW Q s zeroN (isup nil g') ≡ s nil g' (b ↦ iindW Q s (next nil
    // b) (g' b))` — the constructor computation still lands; the dead
    // hypothesis function is formed but never callable.
    let node = nil_node(&mut arena);
    let hypothesis = {
        // Under b: g' = 1, s = 2, Q = 3.
        let node = nil_node(&mut arena);
        let domain = apply(&mut arena, family.children, node);
        let body = {
            let node = nil_node(&mut arena);
            let next_a = apply(&mut arena, family.next, node);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            let function = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, function, bound);
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
    let function = variable(&mut arena, 0);
    let expected = {
        let step = variable(&mut arena, 1);
        let at_node = apply(&mut arena, step, node);
        let at_children = apply(&mut arena, at_node, function);
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

    // Work receipt: the same eliminator run on the nil node — the
    // transport still goes through the `J` machinery even though every
    // child position is dead — costs 32,276 budgeted steps.
    assert_eq!(total - budget.remaining(), 32_276);
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
    let error = check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A `next` that grabs the payload's element rather than the
    // predecessor is a malformed description, not a different vector:
    // `fst p : Elem` never answers `Nat`.
    let mut captured = family.clone();
    captured.next = {
        let motive = {
            let tag = variable(&mut arena, 0);
            let payload_fn = payload(&mut arena);
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
        let zero_branch = {
            let payload_domain = two(&mut arena);
            let position_domain = empty_positions(&mut arena);
            let body = nat_zero(&mut arena);
            let inner = lambda(&mut arena, position_domain, body);
            lambda(&mut arena, payload_domain, inner)
        };
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
        let bound = variable(&mut arena, 0);
        let tag = fst(&mut arena, bound);
        let selected = case_two(&mut arena, motive, zero_branch, one_branch, tag);
        let bound = variable(&mut arena, 0);
        let payload_component = snd(&mut arena, bound);
        let body = apply(&mut arena, selected, payload_component);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let length = variable(&mut arena, 1);
    let malformed = captured.indexed_w(&mut arena, length);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A branching family into `Strict` is an illegal strict elimination,
    // rejected by `caseTwo`'s own motive rule before `W` formation ever
    // runs.
    let mut strict = family.clone();
    strict.children = {
        let motive = {
            let domain = two(&mut arena);
            let codomain = strict_sort(&mut arena, 0);
            lambda(&mut arena, domain, codomain)
        };
        let empty = empty_positions(&mut arena);
        let unit = unit_position(&mut arena);
        let bound = variable(&mut arena, 0);
        let tag = fst(&mut arena, bound);
        let body = case_two(&mut arena, motive, empty, unit, tag);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let length = variable(&mut arena, 1);
    let malformed = strict.indexed_w(&mut arena, length);
    let error = infer_type(&mut arena, &context, malformed, &mut budget).unwrap_err();
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
    let error = check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));
}

#[test]
fn a_vector_certificate_verifies_with_exact_closure_and_bounded_cost() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = vector_family(&mut arena);
    let declarations = vector_declarations(&mut arena);

    // Γ = e : Elem, n : Nat, tail : Vec n ⊢ cons e n tail : Vec (succN n)
    // — the whole judgment, signature included, re-decided from data.
    let tail_type = {
        let length = variable(&mut arena, 0);
        vec_at(&mut arena, &family, length)
    };
    let element = variable(&mut arena, 2);
    let predecessor = variable(&mut arena, 1);
    let node = cons_node(&mut arena, element, predecessor);
    let children_fn = {
        let domain = apply(&mut arena, family.children, node);
        let tail = variable(&mut arena, 1);
        lambda(&mut arena, domain, tail)
    };
    let cons = family.sup(&mut arena, node, children_fn);
    let expected = {
        let predecessor = variable(&mut arena, 1);
        let successor = nat_succ(&mut arena, predecessor);
        vec_at(&mut arena, &family, successor)
    };
    let element_type = elem(&mut arena);
    let index_type = nat(&mut arena);
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![element_type, index_type, tail_type],
        term: cons,
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

    // The certificate claiming `Vec n` — the tail's own index — is a
    // different, false judgment.
    let wrong = {
        let length = variable(&mut arena, 1);
        vec_at(&mut arena, &family, length)
    };
    let element_type = elem(&mut arena);
    let index_type = nat(&mut arena);
    let forged = MathematicalCertificate {
        signature: vector_declarations(&mut arena),
        level_arity: 0,
        context: vec![element_type, index_type, tail_type],
        term: cons,
        expected: wrong,
    };
    assert!(matches!(
        verify_mathematical_certificate(&mut arena, &forged, &mut budget),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Retained-storage and work receipts: checking the certificate —
    // the full nine-declaration signature plus the `isup` judgment —
    // re-materializes substituted instances of the encoding. Both
    // numbers are measured, not quotas.
    assert_eq!(arena.len(), 185_334);
    assert_eq!(spent, 2_937);
}

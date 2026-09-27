//! The profile's "mutual families" demonstration: a `Tree`/`Forest`
//! mutual recursion built on the derived [`indexed_scheme`] and checked
//! end-to-end by the kernel.
//!
//! Mutual inductives reduce to one indexed family whose index tags the
//! sort — the standard tag-index reduction, here over `I = Two` with
//! `zero` the Tree sort and `one` the Forest sort. The description is
//!
//! ```text
//! I    = Two                                    sort index
//! A    = Σ(tag : Two). Σ(e : Elem). Two         sort + element + ctor sub-tag
//! B a  = caseTwo(…, Id Two zero zero,                     node: one position
//!                caseTwo(…, Id Two zero one,              fnil: dead
//!                          Two,                           fcons: two positions
//!                          snd (snd a)),
//!                fst a)
//! out  = λa. fst a                              produced index = the sort
//! next = λa. caseTwo(Mn, λp. λ_. one,                     node's child ↦ Forest
//!                    λp. caseTwo(Mi, λ_. zero,            fnil: dead ↦ arbitrary
//!                                 λb. b,                 fcons: position ↦ sort
//!                                 snd p),
//!                    fst a) (snd a)
//! ```
//!
//! so `Tree := IW … zero` and `Forest := IW … one` form a mutual pair:
//! `node e f : Tree` for `f : Forest`, `fnil : Forest`, and
//! `fcons t f : Forest` for `t : Tree` and `f : Forest`. The payload is
//! uniform — `Σ(e : Elem). Two` under either tag — because a dependent
//! `Payload tag` cannot feed the second `caseTwo`: `snd a`'s type would
//! be the stuck `Payload (fst a)`, never `Two`. The first `Σ` field is
//! the node's element; the second selects `fnil`/`fcons` under the
//! forest tag and is a dummy under the tree tag.
//!
//! Where the length-indexed vector's `next` ignored the child position
//! — cons's one position always required the predecessor — this `next`
//! *is* the position at `fcons`: `next ⟨one, ⟨_, one⟩⟩ b ≡ b`, so the
//! head position `zero` requires a `Tree` and the tail `one` a
//! `Forest`. That is the mutual discipline, and it is definitional:
//! `iindW Q s one (fcons t f)` computes with induction hypothesis
//! `b ↦ iindW Q s b (g b)`, a Tree hypothesis at `zero` and a Forest
//! hypothesis at `one` inside the same step.
//!
//! `Elem` is a signature *assumption* — the demonstration exercises the
//! scheme against an arbitrary element type a producer would supply,
//! and the certificate's assumption closure records exactly that one
//! axiom. Every use is still re-decided by the checker.

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level,
    MathematicalCertificate, Signature, Sort, Term, TermArena, TermHandle,
    certificate_assumption_closure, check_signature, check_type, convertible, indexed_scheme,
    infer_sort, infer_type, verify_mathematical_certificate,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

/// The eliminator tests' ceiling: `iindW`'s typing and computation
/// unfold the whole `IndexedAt`/`IW` encoding — the `J` transport, the
/// repacked child functions, the pair-eta closures — for every
/// application, and this family's `next` runs a second `caseTwo` deeper
/// than the vector's. Measured spend is ~352–354 thousand steps per
/// test below; this bound leaves roughly half-again headroom so the run
/// stays budgeted rather than open-ended — `StepCeiling` is still the
/// decidability witness.
fn measure_budget() -> Budget {
    Budget::new(1 << 19)
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

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

// ── The producer signature ────────────────────────────────────────────
//
// Positions 0–4 are the derived scheme (`IndexedAt`, `IW`, `iwPack`,
// `isup`, `iindW`); the family's only assumption is the element type:

/// `Elem : Type 0` — the element type, declaration 5.
const ELEM: u32 = 5;

fn elem(arena: &mut TermArena) -> TermHandle {
    constant(arena, ELEM)
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

/// `Id Two zero zero` — the single live child position.
fn unit_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_zero(arena);
    id(arena, ty, left, right)
}

/// The uniform payload `Σ(e : Elem). Two` — element plus constructor
/// sub-tag. Written non-dependently because the `B`/`next` selections
/// must project the sub-tag from a neutral node: under a dependent
/// `Payload tag`, `snd a : Payload (fst a)` never reduces to `Two` for
/// the inner `caseTwo` scrutinee.
fn uniform_payload(arena: &mut TermArena) -> TermHandle {
    let element = elem(arena);
    let sub_tag = two(arena);
    sigma(arena, element, sub_tag)
}

/// The constructor carrier `A = Σ(tag : Two). Σ(e : Elem). Two`.
fn carrier(arena: &mut TermArena) -> TermHandle {
    let payload = uniform_payload(arena);
    let domain = two(arena);
    sigma(arena, domain, payload)
}

/// `B = λ(a : A). caseTwo(λ_.Type 0, Id Two zero zero, caseTwo(λ_.Type
/// 0, Id Two zero one, Two, snd (snd a)), fst a)` — child positions by
/// sort and sub-tag: one under `node`, none under `fnil`, two under
/// `fcons`.
fn branching(arena: &mut TermArena) -> TermHandle {
    let motive = type_motive(arena);
    let unit = unit_position(arena);
    let inner = {
        let motive = type_motive(arena);
        let empty = empty_positions(arena);
        let positions = two(arena);
        let bound = variable(arena, 0);
        let payload = snd(arena, bound);
        let sub_tag = snd(arena, payload);
        case_two(arena, motive, empty, positions, sub_tag)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let body = case_two(arena, motive, unit, inner, tag);
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// `out = λ(a : A). fst a` — the produced index is the node's sort tag:
/// `zero` for `node`, `one` for both forest constructors.
fn out_index(arena: &mut TermArena) -> TermHandle {
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let domain = carrier(arena);
    lambda(arena, domain, tag)
}

/// `B ⟨tag, p⟩` as an application of the `branching` lambda — the form
/// `next`'s motives quantify over, so the required position type under
/// an abstract `tag`/`p` still reduces once both are concrete.
fn branching_at(arena: &mut TermArena, tag: TermHandle, payload: TermHandle) -> TermHandle {
    let node = pair(arena, tag, payload);
    let family = branching(arena);
    apply(arena, family, node)
}

/// `next = λ(a : A). caseTwo(Mn, br0, br1, fst a) (snd a)` — the index
/// each child position requires:
///
/// - `Mn tag = Π(p : Σ(e : Elem). Two). Π(_ : B ⟨tag, p⟩). Two`
/// - `br0 = λp. λ(_ : Id Two zero zero). one` — a `node`'s single child
///   must be a `Forest`.
/// - `br1 = λp. caseTwo(Mi, λ(_ : Id Two zero one). zero, λb. b,
///   snd p)` — `fnil`'s dead position answers `zero` arbitrarily;
///   `fcons`'s position is its own required sort: head `zero` ↦ `Tree`,
///   tail `one` ↦ `Forest`.
/// - `Mi w = Π(_ : caseTwo(λ_.Type 0, Id Two zero one, Two, w)). Two` —
///   the position family the forest sub-tag selects.
fn next_index(arena: &mut TermArena) -> TermHandle {
    let inner_position_family = {
        // `λ(w : Two). caseTwo(λ_.Type 0, Id01, Two, w)`.
        let motive = type_motive(arena);
        let empty = empty_positions(arena);
        let positions = two(arena);
        let bound = variable(arena, 0);
        let body = case_two(arena, motive, empty, positions, bound);
        let domain = two(arena);
        lambda(arena, domain, body)
    };
    let motive = {
        // `Mn = λ(tag : Two). Π(p : Σ(e:Elem).Two). Π(_ : B ⟨tag,p⟩). Two`.
        // Under the `p` binder, `tag` is de Bruijn index 1 and `p` is 0.
        let tag = variable(arena, 1);
        let payload = variable(arena, 0);
        let position_type = branching_at(arena, tag, payload);
        let index = two(arena);
        let inner = pi(arena, position_type, index);
        let payload_domain = uniform_payload(arena);
        let body = pi(arena, payload_domain, inner);
        let domain = two(arena);
        lambda(arena, domain, body)
    };
    let zero_branch = {
        // `λ(p : Σ(e:Elem).Two). λ(_ : Id Two zero zero). one` — the
        // tree node's child position is a forest.
        let position_domain = unit_position(arena);
        let index = two_one(arena);
        let inner = lambda(arena, position_domain, index);
        let payload_domain = uniform_payload(arena);
        lambda(arena, payload_domain, inner)
    };
    let one_branch = {
        // `λ(p : Σ(e:Elem).Two). caseTwo(Mi, λ(_:Id01).zero, λb.b, snd p)`.
        let inner_motive = {
            // `Mi w = Π(_ : inner_position_family w). Two` under `w`.
            let bound = variable(arena, 0);
            let positions = apply(arena, inner_position_family, bound);
            let index = two(arena);
            let body = pi(arena, positions, index);
            let domain = two(arena);
            lambda(arena, domain, body)
        };
        let dead = {
            // `λ(_ : Id Two zero one). zero`.
            let position_domain = empty_positions(arena);
            let index = two_zero(arena);
            lambda(arena, position_domain, index)
        };
        let sorted = {
            // `λ(b : Two). b` — the fcons position requires its own sort.
            let domain = two(arena);
            let bound = variable(arena, 0);
            lambda(arena, domain, bound)
        };
        let bound = variable(arena, 0);
        let sub_tag = snd(arena, bound);
        let selected = case_two(arena, inner_motive, dead, sorted, sub_tag);
        let payload_domain = uniform_payload(arena);
        lambda(arena, payload_domain, selected)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload = snd(arena, bound);
    let body = apply(arena, selected, payload);
    let domain = carrier(arena);
    lambda(arena, domain, body)
}

/// The mutual `Tree`/`Forest` family: `IW Two A B out next` where index
/// `zero` names `Tree` and index `one` names `Forest`.
fn mutual_family(arena: &mut TermArena) -> IndexedFamily {
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: two(arena),
        carrier: carrier(arena),
        children: branching(arena),
        out: out_index(arena),
        next: next_index(arena),
    }
}

/// The producer signature: the five scheme declarations then the single
/// `Elem` assumption.
fn mutual_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Elem
    declarations
}

fn mutual_signature(arena: &mut TermArena) -> Signature {
    let declarations = mutual_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

/// `Tree` — the family's `zero` index.
fn tree_at(arena: &mut TermArena, family: &IndexedFamily) -> TermHandle {
    let index = two_zero(arena);
    family.indexed_w(arena, index)
}

/// `Forest` — the family's `one` index.
fn forest_at(arena: &mut TermArena, family: &IndexedFamily) -> TermHandle {
    let index = two_one(arena);
    family.indexed_w(arena, index)
}

/// `⟨zero, ⟨e, zero⟩⟩` — a `node` label: tree sort, element `e`, dummy
/// sub-tag.
fn node_label(arena: &mut TermArena, element: TermHandle) -> TermHandle {
    let sub_tag = two_zero(arena);
    let payload = pair(arena, element, sub_tag);
    let tag = two_zero(arena);
    pair(arena, tag, payload)
}

/// `⟨one, ⟨d, zero⟩⟩` — a `fnil` label: forest sort, dummy element,
/// `zero` sub-tag.
fn fnil_label(arena: &mut TermArena, dummy: TermHandle) -> TermHandle {
    let sub_tag = two_zero(arena);
    let payload = pair(arena, dummy, sub_tag);
    let tag = two_one(arena);
    pair(arena, tag, payload)
}

/// `⟨one, ⟨d, one⟩⟩` — an `fcons` label: forest sort, dummy element,
/// `one` sub-tag.
fn fcons_label(arena: &mut TermArena, dummy: TermHandle) -> TermHandle {
    let sub_tag = two_one(arena);
    let payload = pair(arena, dummy, sub_tag);
    let tag = two_one(arena);
    pair(arena, tag, payload)
}

/// `λ(w : Two). IW w` — the motive selecting a `Tree` at `zero` and a
/// `Forest` at `one`, for `fcons`'s sort-selected child function.
fn indexed_motive(arena: &mut TermArena, family: &IndexedFamily) -> TermHandle {
    let bound = variable(arena, 0);
    let body = family.indexed_w(arena, bound);
    let domain = two(arena);
    lambda(arena, domain, body)
}

/// `Π(i : Two). Π(_ : IW i). Type 0` — the `iindW` motive shape.
fn motive_type(arena: &mut TermArena, family: &IndexedFamily) -> TermHandle {
    let index = variable(arena, 0);
    let family_at = family.indexed_w(arena, index);
    let type_zero = type_sort(arena, 0);
    let inner = pi(arena, family_at, type_zero);
    let domain = two(arena);
    pi(arena, domain, inner)
}

/// `Π(a : A). Π(g : Π(b : B a). IW (next a b)). Π(_ : Π(b : B a). Q
/// (next a b) (g b)). Q (out a) (isup a g)` — the `iindW` step type,
/// written under the one-binding prefix `[Q]`.
fn step_type(arena: &mut TermArena, family: &IndexedFamily) -> TermHandle {
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
            family.indexed_w(arena, required)
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
}

#[test]
fn the_mutual_scheme_signature_checks_and_the_family_forms() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = mutual_family(&mut arena);
    let declarations = mutual_declarations(&mut arena);
    assert_eq!(declarations.len(), 6);

    let before = budget.remaining();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    assert_eq!(signature.len(), 6);
    assert!(before - budget.remaining() > 0);

    // Formation: under `i : Two`, `IW i` is a `Type 0` — the family's
    // `max(l, u, v)` lands on the closed level-0 description.
    let index_type = two(&mut arena);
    let context = Context::empty()
        .with_signature(signature)
        .extend(index_type);
    let index = variable(&mut arena, 0);
    let member = family.indexed_w(&mut arena, index);
    let sort = infer_sort(&mut arena, &context, member, &mut budget).unwrap();
    assert!(matches!(sort, Sort::Type(_)));
    let type_zero = type_sort(&mut arena, 0);
    check_type(&mut arena, &context, member, type_zero, &mut budget).unwrap();
}

#[test]
fn the_indexing_condition_assigns_each_child_position_its_sort() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = mutual_family(&mut arena);

    // At a `node`, `IndexedAt i (sup ⟨zero,⟨e,zero⟩⟩ k)` unfolds to
    //   `Σ(_ : Id Two zero i). Π(b : Id Two zero zero). IndexedAt one (k b)`
    // — produced index `zero` (the tree sort), and the one child
    // position requires the *other* sort `one`: trees carry forests.
    let node_k_type = {
        // Under [e]: `Π(b : B ⟨zero,⟨e,zero⟩⟩). W A B`.
        let element = variable(&mut arena, 0);
        let label = node_label(&mut arena, element);
        let domain = apply(&mut arena, family.children, label);
        let w = w_type(&mut arena, family.carrier, family.children);
        pi(&mut arena, domain, w)
    };
    let element_type = elem(&mut arena);
    let index_type = two(&mut arena);
    let node_context = Context::empty()
        .with_signature(mutual_signature(&mut arena))
        .extend(element_type)
        .extend(node_k_type)
        .extend(index_type);
    // In Γ: i = 0, k = 1, e = 2.
    let node = {
        let element = variable(&mut arena, 2);
        let label = node_label(&mut arena, element);
        let function = variable(&mut arena, 1);
        sup(&mut arena, family.carrier, family.children, label, function)
    };
    let index = variable(&mut arena, 0);
    let condition = family.indexed_at(&mut arena, index, node);
    let expected = {
        let domain = {
            let ty = two(&mut arena);
            let left = two_zero(&mut arena);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, left, index)
        };
        let codomain = {
            // Under the dead `_` binder: i = 1, k = 2, e = 3.
            let element = variable(&mut arena, 3);
            let label = node_label(&mut arena, element);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                // Under b: i = 2, k = 3. The required index is the
                // *reduced* `one` — `next ⟨zero,⟨e,zero⟩⟩ b ≡ one`
                // fires inside the comparison.
                let index = two_one(&mut arena);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, index, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    let type_zero = type_sort(&mut arena, 0);
    check_type(&mut arena, &node_context, condition, type_zero, &mut budget).unwrap();
    check_type(&mut arena, &node_context, expected, type_zero, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &node_context,
            condition,
            expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // At an `fcons`, `IndexedAt i (sup ⟨one,⟨d,one⟩⟩ k)` unfolds to
    //   `Σ(_ : Id Two one i). Π(b : Two). IndexedAt b (k b)`
    // — the required index is the position itself: `next`'s inner
    // `caseTwo` returns `b`, so the head position `zero` indexes a
    // `Tree` and the tail `one` a `Forest`.
    let fcons_k_type = {
        // Under [e]: `Π(b : B ⟨one,⟨e,one⟩⟩). W A B`.
        let dummy = variable(&mut arena, 0);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let w = w_type(&mut arena, family.carrier, family.children);
        pi(&mut arena, domain, w)
    };
    let element_type = elem(&mut arena);
    let index_type = two(&mut arena);
    let fcons_context = Context::empty()
        .with_signature(mutual_signature(&mut arena))
        .extend(element_type)
        .extend(fcons_k_type)
        .extend(index_type);
    // In Γ: i = 0, k = 1, e = 2.
    let fcons_node = {
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        let function = variable(&mut arena, 1);
        sup(&mut arena, family.carrier, family.children, label, function)
    };
    let index = variable(&mut arena, 0);
    let condition = family.indexed_at(&mut arena, index, fcons_node);
    let expected = {
        let domain = {
            let ty = two(&mut arena);
            let left = two_one(&mut arena);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, left, index)
        };
        let codomain = {
            // Under the dead `_` binder: i = 1, k = 2, e = 3.
            let dummy = variable(&mut arena, 3);
            let label = fcons_label(&mut arena, dummy);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                // Under b: i = 2, k = 3 — `IndexedAt b (k b)` with the
                // position `b` itself as the required index.
                let index = variable(&mut arena, 0);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, index, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(
        &mut arena,
        &fcons_context,
        condition,
        type_zero,
        &mut budget,
    )
    .unwrap();
    check_type(&mut arena, &fcons_context, expected, type_zero, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &fcons_context,
            condition,
            expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // The non-mutual misreading — every `fcons` child a `Forest` —
    // never converts: `IndexedAt b (k b)` under a neutral `b` is not
    // `IndexedAt one (k b)`.
    let uniform = {
        let domain = {
            let ty = two(&mut arena);
            let left = two_one(&mut arena);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, left, index)
        };
        let codomain = {
            let dummy = variable(&mut arena, 3);
            let label = fcons_label(&mut arena, dummy);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                let index = two_one(&mut arena);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, index, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(&mut arena, &fcons_context, uniform, type_zero, &mut budget).unwrap();
    assert!(
        !convertible(
            &mut arena,
            &fcons_context,
            condition,
            uniform,
            type_zero,
            &mut budget
        )
        .unwrap()
    );

    // At `fnil`, `IndexedAt i (sup ⟨one,⟨d,zero⟩⟩ k')` unfolds to
    //   `Σ(_ : Id Two one i). Π(b : Id Two zero one). IndexedAt zero (k' b)`
    // — produced index `one`, and the dead position's required index is
    // `br1`'s arbitrary `zero`.
    let fnil_k_type = {
        let dummy = variable(&mut arena, 0);
        let label = fnil_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let w = w_type(&mut arena, family.carrier, family.children);
        pi(&mut arena, domain, w)
    };
    let element_type = elem(&mut arena);
    let index_type = two(&mut arena);
    let fnil_context = Context::empty()
        .with_signature(mutual_signature(&mut arena))
        .extend(element_type)
        .extend(fnil_k_type)
        .extend(index_type);
    // In Γ: i = 0, k' = 1, e = 2.
    let fnil_node = {
        let dummy = variable(&mut arena, 2);
        let label = fnil_label(&mut arena, dummy);
        let function = variable(&mut arena, 1);
        sup(&mut arena, family.carrier, family.children, label, function)
    };
    let index = variable(&mut arena, 0);
    let condition = family.indexed_at(&mut arena, index, fnil_node);
    let expected = {
        let domain = {
            let ty = two(&mut arena);
            let left = two_one(&mut arena);
            let index = variable(&mut arena, 0);
            id(&mut arena, ty, left, index)
        };
        let codomain = {
            let dummy = variable(&mut arena, 3);
            let label = fnil_label(&mut arena, dummy);
            let b_domain = apply(&mut arena, family.children, label);
            let body = {
                let index = two_zero(&mut arena);
                let function = variable(&mut arena, 3);
                let bound = variable(&mut arena, 0);
                let child = apply(&mut arena, function, bound);
                family.indexed_at(&mut arena, index, child)
            };
            pi(&mut arena, b_domain, body)
        };
        sigma(&mut arena, domain, codomain)
    };
    check_type(&mut arena, &fnil_context, condition, type_zero, &mut budget).unwrap();
    check_type(&mut arena, &fnil_context, expected, type_zero, &mut budget).unwrap();
    assert!(
        convertible(
            &mut arena,
            &fnil_context,
            condition,
            expected,
            type_zero,
            &mut budget
        )
        .unwrap()
    );
}

/// The constructor-side context `e : Elem, t : Tree, f : Forest` —
/// indices f = 0, t = 1, e = 2.
fn constructor_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let tree = tree_at(arena, family);
    let forest = forest_at(arena, family);
    let element_type = elem(arena);
    Context::empty()
        .with_signature(mutual_signature(arena))
        .extend(element_type)
        .extend(tree)
        .extend(forest)
}

#[test]
fn constructors_build_at_their_sort_indices() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = mutual_family(&mut arena);
    let context = constructor_context(&mut arena, &family);
    // In Γ: f = 0, t = 1, e = 2.

    // `node e f := isup ⟨zero,⟨e,zero⟩⟩ (λ(_ : B (node e)). f)` — the
    // child function ignores the single `Id Two zero zero` position and
    // returns the forest, whose required index `next` computes to `one`.
    let element = variable(&mut arena, 2);
    let label = node_label(&mut arena, element);
    let children_fn = {
        let element = variable(&mut arena, 2);
        let label = node_label(&mut arena, element);
        let domain = apply(&mut arena, family.children, label);
        let forest = variable(&mut arena, 1);
        lambda(&mut arena, domain, forest)
    };
    let node = family.sup(&mut arena, label, children_fn);
    let expected = tree_at(&mut arena, &family);
    check_type(&mut arena, &context, node, expected, &mut budget).unwrap();

    // `fcons t f := isup ⟨one,⟨e,one⟩⟩ (λ(b : Two). caseTwo(λw. IW w, t,
    // f, b))` — the child function *selects* per position: `t` at head
    // `zero`, `f` at tail `one`. Checking the case against `Π(b : B
    // (fcons e)). IW (next (fcons e) b)` must reduce `next … b` to `b`.
    let dummy = variable(&mut arena, 2);
    let label = fcons_label(&mut arena, dummy);
    let children_fn = {
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            // Under b: f = 1, t = 2.
            let motive = indexed_motive(&mut arena, &family);
            let tree = variable(&mut arena, 2);
            let forest = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, tree, forest, bound)
        };
        lambda(&mut arena, domain, body)
    };
    let fcons = family.sup(&mut arena, label, children_fn);
    let expected = forest_at(&mut arena, &family);
    check_type(&mut arena, &context, fcons, expected, &mut budget).unwrap();

    // `fnil := isup ⟨one,⟨e,zero⟩⟩ g'` for a context-bound — hence
    // neutral — `g' : Π(b : B (fnil e)). IW (next (fnil e) b)`: every
    // child position is dead, so the function is vacuous and supplied
    // abstractly.
    let fnil_children_type = {
        let dummy = variable(&mut arena, 2);
        let label = fnil_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let codomain = {
            // Under b: e = 3.
            let dummy = variable(&mut arena, 3);
            let label = fnil_label(&mut arena, dummy);
            let next_a = apply(&mut arena, family.next, label);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            family.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = context.extend(fnil_children_type);
    // In Γ': g' = 0, f = 1, t = 2, e = 3.
    let fnil = {
        let dummy = variable(&mut arena, 3);
        let label = fnil_label(&mut arena, dummy);
        let function = variable(&mut arena, 0);
        family.sup(&mut arena, label, function)
    };
    let expected = forest_at(&mut arena, &family);
    check_type(&mut arena, &context, fnil, expected, &mut budget).unwrap();
}

/// The eliminator context `Q : Π(i : Two). Π(_ : IW i). Type 0`,
/// `s : <iindW step>`, `e : Elem`, then a caller-supplied child-function
/// binding.
fn eliminator_prefix(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let motive = motive_type(arena, family);
    let step = step_type(arena, family);
    let element_type = elem(arena);
    Context::empty()
        .with_signature(mutual_signature(arena))
        .extend(motive)
        .extend(step)
        .extend(element_type)
}

#[test]
fn induction_on_a_node_computes_a_forest_hypothesis() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = mutual_family(&mut arena);

    // Γ = Q, s, e, g where `g : Π(b : B (node e)). IW (next (node e) b)`
    // is the arbitrary — neutral — child function the profile requires.
    let g_type = {
        // Under [Q, s, e]: `Π(b : B (node e)). IW (next (node e) b)`.
        let element = variable(&mut arena, 0);
        let label = node_label(&mut arena, element);
        let domain = apply(&mut arena, family.children, label);
        let codomain = {
            // Under b: e = 1.
            let element = variable(&mut arena, 1);
            let label = node_label(&mut arena, element);
            let next_a = apply(&mut arena, family.next, label);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            family.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = eliminator_prefix(&mut arena, &family).extend(g_type);
    // In Γ: g = 0, e = 1, s = 2, Q = 3.

    // `node = isup ⟨zero,⟨e,zero⟩⟩ g` — a `Tree` by `out`.
    let element = variable(&mut arena, 1);
    let label = node_label(&mut arena, element);
    let function = variable(&mut arena, 0);
    let node = family.sup(&mut arena, label, function);

    // `iindW Q s zero node : Q zero node`.
    let motive = variable(&mut arena, 3);
    let step = variable(&mut arena, 2);
    let index = two_zero(&mut arena);
    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, index, node);
    let shared = {
        let motive = variable(&mut arena, 3);
        let index = two_zero(&mut arena);
        let at_index = apply(&mut arena, motive, index);
        apply(&mut arena, at_index, node)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    // The constructor computation judgment, with `g` still neutral:
    //   `iindW Q s zero (isup (node e) g)`
    //     `≡ s (node e) g (b ↦ iindW Q s one (g b))`
    // — the induction hypothesis is written at the *reduced* `one`, so
    // closing it must compute `next (node e) b ≡ one`: the tree's
    // induction hypothesis is a forest hypothesis.
    let hypothesis = {
        let element = variable(&mut arena, 1);
        let label = node_label(&mut arena, element);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            // Under b: g = 1, e = 2, s = 3, Q = 4.
            let motive = variable(&mut arena, 4);
            let step = variable(&mut arena, 3);
            let index = two_one(&mut arena);
            let function = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, function, bound);
            family.ind(&mut arena, Level::Constant(0), motive, step, index, child)
        };
        lambda(&mut arena, domain, body)
    };
    let expected = {
        let step = variable(&mut arena, 2);
        let element = variable(&mut arena, 1);
        let label = node_label(&mut arena, element);
        let at_node = apply(&mut arena, step, label);
        let function = variable(&mut arena, 0);
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

    // Work receipt: checking the `iindW` application and closing the
    // computation across the packed `IW` pair and the sort-tag
    // `next`/`out` selections costs 39_528 budgeted steps.
    assert_eq!(total - budget.remaining(), 39_528);
}

#[test]
fn induction_on_a_cons_computes_sort_selected_hypotheses() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let family = mutual_family(&mut arena);

    // Γ = Q, s, e, g where `g : Π(b : B (fcons e)). IW (next (fcons e) b)`
    // — `B (fcons e) ≡ Two`, `next (fcons e) b ≡ b` — is an arbitrary
    // sort-selected child function.
    let g_type = {
        let dummy = variable(&mut arena, 0);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let codomain = {
            // Under b: e = 1.
            let dummy = variable(&mut arena, 1);
            let label = fcons_label(&mut arena, dummy);
            let next_a = apply(&mut arena, family.next, label);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            family.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = eliminator_prefix(&mut arena, &family).extend(g_type);
    // In Γ: g = 0, e = 1, s = 2, Q = 3.

    let dummy = variable(&mut arena, 1);
    let label = fcons_label(&mut arena, dummy);
    let function = variable(&mut arena, 0);
    let fcons = family.sup(&mut arena, label, function);

    // `iindW Q s one fcons : Q one fcons`.
    let motive = variable(&mut arena, 3);
    let step = variable(&mut arena, 2);
    let index = two_one(&mut arena);
    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, index, fcons);
    let shared = {
        let motive = variable(&mut arena, 3);
        let index = two_one(&mut arena);
        let at_index = apply(&mut arena, motive, index);
        apply(&mut arena, at_index, fcons)
    };
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();

    //   `iindW Q s one (isup (fcons e) g)`
    //     `≡ s (fcons e) g (b ↦ iindW Q s b (g b))`
    // — the hypothesis at position `b` recurses into `IW b`: applied at
    // `zero` it is a `Q zero (g zero)` Tree hypothesis, at `one` a `Q
    // one (g one)` Forest hypothesis. The expected index is the bound
    // `b` itself, so the closure must compute `next (fcons e) b ≡ b`.
    let hypothesis = {
        let dummy = variable(&mut arena, 1);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            // Under b: g = 1, e = 2, s = 3, Q = 4.
            let motive = variable(&mut arena, 4);
            let step = variable(&mut arena, 3);
            let index = variable(&mut arena, 0);
            let function = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let child = apply(&mut arena, function, bound);
            family.ind(&mut arena, Level::Constant(0), motive, step, index, child)
        };
        lambda(&mut arena, domain, body)
    };
    let expected = {
        let step = variable(&mut arena, 2);
        let dummy = variable(&mut arena, 1);
        let label = fcons_label(&mut arena, dummy);
        let at_node = apply(&mut arena, step, label);
        let function = variable(&mut arena, 0);
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

    // Work receipt, measured against the same budgeted run.
    assert_eq!(total - budget.remaining(), 39_897);
}

#[test]
fn mutual_constructions_reject_wrong_sorts_and_malformed_descriptions() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = mutual_family(&mut arena);
    let context = constructor_context(&mut arena, &family);
    // In Γ: f = 0, t = 1, e = 2.

    let element = variable(&mut arena, 2);
    let label = node_label(&mut arena, element);
    let children_fn = {
        let element = variable(&mut arena, 2);
        let label = node_label(&mut arena, element);
        let domain = apply(&mut arena, family.children, label);
        let forest = variable(&mut arena, 1);
        lambda(&mut arena, domain, forest)
    };
    let node = family.sup(&mut arena, label, children_fn);

    // A `node` is a `Tree`, never a `Forest`: `out ⟨zero,·⟩ ≡ zero`
    // never lands at `one`.
    let forest = forest_at(&mut arena, &family);
    let error = check_type(&mut arena, &context, node, forest, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // …and a forest constructor is never a `Tree`.
    let dummy = variable(&mut arena, 2);
    let label = fcons_label(&mut arena, dummy);
    let children_fn = {
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            let motive = indexed_motive(&mut arena, &family);
            let tree = variable(&mut arena, 2);
            let forest = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, tree, forest, bound)
        };
        lambda(&mut arena, domain, body)
    };
    let fcons = family.sup(&mut arena, label, children_fn);
    let tree = tree_at(&mut arena, &family);
    let error = check_type(&mut arena, &context, fcons, tree, &mut budget).unwrap_err();
    assert!(matches!(error, CoreError::TypeMismatch { .. }));

    // A `node` whose child is a `Tree` rather than a `Forest`: `λ_. t`
    // at `Π(_ : B (node e)). IW one` never checks — the required index
    // `one` is the mutual discipline, not a suggestion.
    let wrong_child = {
        let element = variable(&mut arena, 2);
        let label = node_label(&mut arena, element);
        let domain = apply(&mut arena, family.children, label);
        let tree = variable(&mut arena, 2);
        let function = lambda(&mut arena, domain, tree);
        let element = variable(&mut arena, 2);
        let label = node_label(&mut arena, element);
        family.sup(&mut arena, label, function)
    };
    let expected = tree_at(&mut arena, &family);
    let error = check_type(&mut arena, &context, wrong_child, expected, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // An `fcons` child function returning a `Tree` at every position —
    // `λ(_ : Two). t` — never supplies `IW b` for neutral `b`.
    let uniform_tree = {
        let domain = two(&mut arena);
        let tree = variable(&mut arena, 2);
        let function = lambda(&mut arena, domain, tree);
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        family.sup(&mut arena, label, function)
    };
    let expected = forest_at(&mut arena, &family);
    let error = check_type(&mut arena, &context, uniform_tree, expected, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // The same with the `caseTwo` branches swapped — a `Forest` at the
    // head position, a `Tree` at the tail — fails the branch check:
    // `f : IW one` never lands at `IW zero`.
    let swapped = {
        let domain = two(&mut arena);
        let body = {
            let motive = indexed_motive(&mut arena, &family);
            let forest = variable(&mut arena, 1);
            let tree = variable(&mut arena, 2);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, forest, tree, bound)
        };
        let function = lambda(&mut arena, domain, body);
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        family.sup(&mut arena, label, function)
    };
    let expected = forest_at(&mut arena, &family);
    let error = check_type(&mut arena, &context, swapped, expected, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));

    // A `next` that flips the sorts at `fcons` — head ↦ `one`, tail ↦
    // `zero` — is a malformed description, not a different family: the
    // correctly-sorted `fcons` above never checks under it.
    let mut captured = family.clone();
    captured.next = {
        let inner_position_family = {
            let motive = type_motive(&mut arena);
            let empty = empty_positions(&mut arena);
            let positions = two(&mut arena);
            let bound = variable(&mut arena, 0);
            let body = case_two(&mut arena, motive, empty, positions, bound);
            let domain = two(&mut arena);
            lambda(&mut arena, domain, body)
        };
        let motive = {
            let tag = variable(&mut arena, 1);
            let payload = variable(&mut arena, 0);
            let position_type = branching_at(&mut arena, tag, payload);
            let index = two(&mut arena);
            let inner = pi(&mut arena, position_type, index);
            let payload_domain = uniform_payload(&mut arena);
            let body = pi(&mut arena, payload_domain, inner);
            let domain = two(&mut arena);
            lambda(&mut arena, domain, body)
        };
        let zero_branch = {
            let position_domain = unit_position(&mut arena);
            let index = two_one(&mut arena);
            let inner = lambda(&mut arena, position_domain, index);
            let payload_domain = uniform_payload(&mut arena);
            lambda(&mut arena, payload_domain, inner)
        };
        let one_branch = {
            let inner_motive = {
                let bound = variable(&mut arena, 0);
                let positions = apply(&mut arena, inner_position_family, bound);
                let index = two(&mut arena);
                let body = pi(&mut arena, positions, index);
                let domain = two(&mut arena);
                lambda(&mut arena, domain, body)
            };
            let dead = {
                let position_domain = empty_positions(&mut arena);
                let index = two_zero(&mut arena);
                lambda(&mut arena, position_domain, index)
            };
            let flipped = {
                // `λ(b : Two). caseTwo(λ_.Two, one, zero, b)`.
                let domain = two(&mut arena);
                let index = two(&mut arena);
                let motive = lambda(&mut arena, domain, index);
                let one = two_one(&mut arena);
                let zero = two_zero(&mut arena);
                let bound = variable(&mut arena, 0);
                let body = case_two(&mut arena, motive, one, zero, bound);
                let domain = two(&mut arena);
                lambda(&mut arena, domain, body)
            };
            let bound = variable(&mut arena, 0);
            let sub_tag = snd(&mut arena, bound);
            let selected = case_two(&mut arena, inner_motive, dead, flipped, sub_tag);
            let payload_domain = uniform_payload(&mut arena);
            lambda(&mut arena, payload_domain, selected)
        };
        let bound = variable(&mut arena, 0);
        let tag = fst(&mut arena, bound);
        let selected = case_two(&mut arena, motive, zero_branch, one_branch, tag);
        let bound = variable(&mut arena, 0);
        let payload = snd(&mut arena, bound);
        let body = apply(&mut arena, selected, payload);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let flipped_children = {
        let motive = indexed_motive(&mut arena, &captured);
        let tree = variable(&mut arena, 2);
        let forest = variable(&mut arena, 1);
        let bound = variable(&mut arena, 0);
        let body = case_two(&mut arena, motive, tree, forest, bound);
        let domain = two(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let captured_fcons = {
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        captured.sup(&mut arena, label, flipped_children)
    };
    let expected = forest_at(&mut arena, &captured);
    let error =
        check_type(&mut arena, &context, captured_fcons, expected, &mut budget).unwrap_err();
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
        let unit = unit_position(&mut arena);
        let positions = two(&mut arena);
        let bound = variable(&mut arena, 0);
        let tag = fst(&mut arena, bound);
        let body = case_two(&mut arena, motive, unit, positions, tag);
        let domain = carrier(&mut arena);
        lambda(&mut arena, domain, body)
    };
    let index = variable(&mut arena, 2);
    let member = strict.indexed_w(&mut arena, index);
    let error = infer_type(&mut arena, &context, member, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::StrictCaseMotiveCodomain { .. } | CoreError::TypeMismatch { .. }
    ));

    // The eliminator is universe-polymorphic over the motive level `w`:
    // claiming `w = 1` while `Q` lands at `Type 0` is a bad universe,
    // not a different judgment.
    let context = eliminator_prefix(&mut arena, &family).extend({
        let dummy = variable(&mut arena, 0);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let codomain = {
            let dummy = variable(&mut arena, 1);
            let label = fcons_label(&mut arena, dummy);
            let next_a = apply(&mut arena, family.next, label);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, next_a, bound);
            family.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    });
    // In Γ: g = 0, e = 1, s = 2, Q = 3.
    let dummy = variable(&mut arena, 1);
    let label = fcons_label(&mut arena, dummy);
    let function = variable(&mut arena, 0);
    let fcons = family.sup(&mut arena, label, function);
    let motive = variable(&mut arena, 3);
    let step = variable(&mut arena, 2);
    let index = two_one(&mut arena);
    let elimination = family.ind(&mut arena, Level::Constant(1), motive, step, index, fcons);
    let shared = {
        let motive = variable(&mut arena, 3);
        let index = two_one(&mut arena);
        let at_index = apply(&mut arena, motive, index);
        apply(&mut arena, at_index, fcons)
    };
    let error = check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap_err();
    assert!(matches!(
        error,
        CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. }
    ));
}

#[test]
fn a_mutual_family_certificate_verifies_with_exact_closure_and_bounded_cost() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let family = mutual_family(&mut arena);
    let declarations = mutual_declarations(&mut arena);

    // Γ = e : Elem, t : Tree, f : Forest proves
    //   `fcons t f := isup ⟨one,⟨e,one⟩⟩ (λ(b : Two). caseTwo(λw. IW w,
    //   t, f, b)) : Forest`
    // — the whole judgment, signature included, re-decided from data.
    let tree_type = tree_at(&mut arena, &family);
    let forest_type = forest_at(&mut arena, &family);
    let children_fn = {
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            // Under b: f = 1, t = 2.
            let motive = indexed_motive(&mut arena, &family);
            let tree = variable(&mut arena, 2);
            let forest = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, tree, forest, bound)
        };
        lambda(&mut arena, domain, body)
    };
    let dummy = variable(&mut arena, 2);
    let label = fcons_label(&mut arena, dummy);
    let fcons = family.sup(&mut arena, label, children_fn);
    let expected = forest_at(&mut arena, &family);
    let element_type = elem(&mut arena);
    let certificate = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![element_type, tree_type, forest_type],
        term: fcons,
        expected,
    };
    let before = budget.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    let spent = before - budget.remaining();
    assert!(spent > 0, "checking the certificate must do real work");

    // The judgment commits to exactly the one producer assumption —
    // `Elem` — and to none of the scheme's *definitions*, which carry no
    // assumption force.
    let closure = certificate_assumption_closure(&arena, &certificate);
    assert_eq!(closure, [ELEM].into_iter().collect());

    // The certificate claiming the `Tree` index — the head's sort, not
    // the produced forest's — is a different, false judgment.
    let tree_type = tree_at(&mut arena, &family);
    let forest_type = forest_at(&mut arena, &family);
    let children_fn = {
        let dummy = variable(&mut arena, 2);
        let label = fcons_label(&mut arena, dummy);
        let domain = apply(&mut arena, family.children, label);
        let body = {
            let motive = indexed_motive(&mut arena, &family);
            let tree = variable(&mut arena, 2);
            let forest = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            case_two(&mut arena, motive, tree, forest, bound)
        };
        lambda(&mut arena, domain, body)
    };
    let dummy = variable(&mut arena, 2);
    let label = fcons_label(&mut arena, dummy);
    let fcons = family.sup(&mut arena, label, children_fn);
    let forged = tree_at(&mut arena, &family);
    let element_type = elem(&mut arena);
    let forged = MathematicalCertificate {
        signature: mutual_declarations(&mut arena),
        level_arity: 0,
        context: vec![element_type, tree_type, forest_type],
        term: fcons,
        expected: forged,
    };
    assert!(matches!(
        verify_mathematical_certificate(&mut arena, &forged, &mut budget),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Retained-storage and work receipts: checking the certificate — the
    // six-declaration signature plus the `isup` judgment — re-materializes
    // substituted instances of the encoding. Both numbers are measured,
    // not quotas.
    assert_eq!(arena.len(), 182_042);
    assert_eq!(spent, 2_659);
}

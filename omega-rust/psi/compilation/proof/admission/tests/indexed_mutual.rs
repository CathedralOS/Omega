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
    Signature, Sort, Term, TermArena, TermHandle, check_signature, check_type, indexed_scheme,
    infer_type,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
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

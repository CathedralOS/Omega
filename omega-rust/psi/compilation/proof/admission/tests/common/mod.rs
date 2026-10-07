//! Shared term-building helpers for the integration suites: the small
//! constructor DSL every `indexed_*`/`existence`/`conversion` fixture used to
//! redefine verbatim.

#![allow(dead_code)]

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level, Sort,
    Term, TermArena, TermHandle, indexed_scheme, infer_type,
};
use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};

pub(crate) const NAT: u32 = 5;
pub(crate) const NAT_ZERO: u32 = 6;
pub(crate) const NAT_SUCC: u32 = 7;
pub(crate) const ELEM: u32 = 8;

pub(crate) fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

pub(crate) fn measure_budget() -> Budget {
    Budget::new(1 << 18)
}

pub(crate) fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

pub(crate) fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

pub(crate) fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

pub(crate) fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

pub(crate) fn pair(arena: &mut TermArena, first: TermHandle, second: TermHandle) -> TermHandle {
    arena.insert(Term::Pair { first, second })
}

pub(crate) fn fst(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair })
}

pub(crate) fn snd(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Snd { pair })
}

pub(crate) fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Sigma { domain, codomain })
}

pub(crate) fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
}

pub(crate) fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
}

pub(crate) fn two(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Two)
}

pub(crate) fn two_zero(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoZero)
}

pub(crate) fn two_one(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoOne)
}

pub(crate) fn case_two(
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

pub(crate) fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

pub(crate) fn empty_positions(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_one(arena);
    id(arena, ty, left, right)
}

pub(crate) fn unit_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_zero(arena);
    id(arena, ty, left, right)
}

pub(crate) fn type_motive(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let codomain = type_sort(arena, 0);
    lambda(arena, domain, codomain)
}

pub(crate) fn constant(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

pub(crate) fn nat(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT, Vec::new())
}

pub(crate) fn nat_zero(arena: &mut TermArena) -> TermHandle {
    constant(arena, NAT_ZERO, Vec::new())
}

pub(crate) fn nat_succ(arena: &mut TermArena, predecessor: TermHandle) -> TermHandle {
    let succ = constant(arena, NAT_SUCC, Vec::new());
    apply(arena, succ, predecessor)
}

pub(crate) fn elem(arena: &mut TermArena) -> TermHandle {
    constant(arena, ELEM, Vec::new())
}

pub(crate) fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
}

pub(crate) fn squash(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Squash { ty })
}

pub(crate) fn squash_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::SquashIntro { ty, value })
}

pub(crate) fn squash_elim(
    arena: &mut TermArena,
    proposition: TermHandle,
    function: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::SquashElim {
        proposition,
        function,
        scrutinee,
    })
}

pub(crate) fn boxed(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Box { ty })
}

pub(crate) fn box_elim(
    arena: &mut TermArena,
    motive: TermHandle,
    body: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::BoxElim {
        motive,
        body,
        scrutinee,
    })
}

pub(crate) fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean)
}

/// The step case's proposition `Π(g : Π(a:A).Π(b:children a).Q(next a b)).
/// Π(a:A).Π(b:children a).Q(next a b) → Q(out a).g a b` — `domain` is each
/// family's own carrier.
pub(crate) fn step_type(
    arena: &mut TermArena,
    family: &IndexedFamily,
    domain: TermHandle,
) -> TermHandle {
    let g_type = {
        // Under `a`: a = 0, Q = 1.
        let bound = variable(arena, 0);
        let b_domain = apply(arena, family.children, bound);
        let codomain = {
            // Under `b`: a = 1, Q = 2.
            let bound_a = variable(arena, 1);
            let next_a = apply(arena, family.next, bound_a);
            let bound = variable(arena, 0);
            let required = apply(arena, next_a, bound);
            family.indexed_w(arena, required)
        };
        pi(arena, b_domain, codomain)
    };
    let hypothesis_type = {
        // Under `g`, `a`: a = 1, g = 0, Q = 2.
        let bound_a = variable(arena, 1);
        let b_domain = apply(arena, family.children, bound_a);
        let codomain = {
            // Under `b`: a = 2, g = 1, Q = 3.
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
        // Under the hypothesis, `g`, `a`: a = 2, g = 1, Q = 3.
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
    pi(arena, domain, middle)
}

/// The constructor carrier `A = Σ(tag : Two). Payload tag` shared by the
/// tagged-family fixtures; `payload` is the family's own `Payload` lambda.
pub(crate) fn carrier(
    arena: &mut TermArena,
    payload: fn(&mut TermArena) -> TermHandle,
) -> TermHandle {
    let tag = variable(arena, 0);
    let payload_fn = payload(arena);
    let codomain = apply(arena, payload_fn, tag);
    let domain = two(arena);
    sigma(arena, domain, codomain)
}

/// `λ(a : A). caseTwo(λ_.Strict 0, <none>, <some>, fst a)` — a branching
/// family into `Strict`, the illegal strict elimination every malformed-W
/// test expects `caseTwo`'s motive rule to reject. `some` receives the
/// shared motive so nested selections reuse it.
pub(crate) fn strict_branching(
    arena: &mut TermArena,
    carrier: TermHandle,
    none: TermHandle,
    some: impl FnOnce(&mut TermArena, TermHandle) -> TermHandle,
) -> TermHandle {
    let motive = {
        let domain = two(arena);
        let codomain = strict_sort(arena, 0);
        lambda(arena, domain, codomain)
    };
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let some = some(arena, motive);
    let body = case_two(arena, motive, none, some, tag);
    lambda(arena, carrier, body)
}

/// `λ(a : A). caseTwo(M, <zero>, <one>, fst a) (snd a)` — the
/// tag-selected child function applied to the node's own payload.
pub(crate) fn tag_selected_children(
    arena: &mut TermArena,
    carrier: TermHandle,
    motive: TermHandle,
    zero_branch: TermHandle,
    one_branch: TermHandle,
) -> TermHandle {
    let bound = variable(arena, 0);
    let tag = fst(arena, bound);
    let selected = case_two(arena, motive, zero_branch, one_branch, tag);
    let bound = variable(arena, 0);
    let payload = snd(arena, bound);
    let body = apply(arena, selected, payload);
    lambda(arena, carrier, body)
}

/// `Payload = λ(tag : Two). caseTwo(λ_.Type 0, Two, Σ(e : Elem). Nat,
/// tag)` — the `⟨element, predecessor-length⟩` payload the vector family
/// and the nested family's element-vector share.
pub(crate) fn elem_nat_payload(arena: &mut TermArena) -> TermHandle {
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

/// `indexed_scheme` plus the `Nat`, `zeroN`, and `succN` assumptions every
/// Nat-indexed test family declares before its own element declarations.
pub(crate) fn nat_succ_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // Nat
    declarations.push(Declaration::assumption(0, nat(arena))); // zeroN
    let succ_domain = nat(arena);
    let succ_codomain = nat(arena);
    let succ_statement = pi(arena, succ_domain, succ_codomain);
    declarations.push(Declaration::assumption(0, succ_statement)); // succN
    declarations
}

/// `family` re-indexed with `children`, then `index`'s member type inferred —
/// the shared setup of every strict-children rejection assertion; the caller
/// still names its expected error class.
pub(crate) fn strict_children_error(
    arena: &mut TermArena,
    context: &Context,
    family: &IndexedFamily,
    children: TermHandle,
    index: TermHandle,
    budget: &mut Budget,
) -> CoreError {
    let mut strict = family.clone();
    strict.children = children;
    let malformed = strict.indexed_w(arena, index);
    infer_type(arena, context, malformed, budget).unwrap_err()
}

/// `λ(p : Two). λ(_ : Id Two zero one). zeroN` — the dead child position's
/// Nat answer every Two-indexed family reuses.
pub(crate) fn zero_nat_branch(arena: &mut TermArena) -> TermHandle {
    let payload_domain = two(arena);
    let position_domain = empty_positions(arena);
    let body = nat_zero(arena);
    let inner = lambda(arena, position_domain, body);
    lambda(arena, payload_domain, inner)
}

/// `family` indexed at `next label`, applied to the freshly bound index —
/// the codomain every successor-step type shares.
pub(crate) fn next_indexed_w(
    arena: &mut TermArena,
    family: &IndexedFamily,
    label: TermHandle,
) -> TermHandle {
    let next_a = apply(arena, family.next, label);
    let bound = variable(arena, 0);
    let required = apply(arena, next_a, bound);
    family.indexed_w(arena, required)
}

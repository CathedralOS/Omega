//! Fixtures shared by the mathematical core tests: sorts, lambdas,
//! projections, eliminators, budgets and level offsets.

mod identity_and_w_types;
mod levels_and_declarations;
mod schemes_and_quotients;
mod strict_layer;
mod type_checking;

use crate::mathematical_core::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, IndexedFamily, Level, QUOTIENT_IS_SET,
    QuotientFamily, Signature, Sort, Term, TermArena, TermHandle, check_signature, indexed_scheme,
    quotient_scheme,
};

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

fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
}

fn id_elim(
    arena: &mut TermArena,
    motive: TermHandle,
    base: TermHandle,
    endpoint: TermHandle,
    proof: TermHandle,
) -> TermHandle {
    arena.insert(Term::IdElim {
        motive,
        base,
        endpoint,
        proof,
    })
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

fn ind_w(
    arena: &mut TermArena,
    motive: TermHandle,
    step: TermHandle,
    tree: TermHandle,
) -> TermHandle {
    arena.insert(Term::IndW { motive, step, tree })
}

fn empty(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Empty)
}

fn empty_elim(arena: &mut TermArena, ty: TermHandle, scrutinee: TermHandle) -> TermHandle {
    arena.insert(Term::EmptyElim { ty, scrutinee })
}

fn squash(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Squash { ty })
}

fn squash_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::SquashIntro { ty, value })
}

fn squash_elim(
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

fn boxed(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Box { ty })
}

fn box_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::BoxIntro { ty, value })
}

fn box_elim(
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

fn default_budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

/// Builds `λ(A : Type 0). λ(x : A). x` and its Π type; returns the identity
/// term and the expected type for reuse by the storage test.
fn polymorphic_identity(arena: &mut TermArena) -> (TermHandle, TermHandle) {
    let type_zero = type_sort(arena, 0);
    let bound = variable(arena, 0);
    let inner = lambda(arena, bound, bound);
    let identity = lambda(arena, type_zero, inner);
    // Π(A : Type 0). Π(x : A). A
    let codomain_domain = variable(arena, 0);
    let codomain_body = variable(arena, 1);
    let codomain = pi(arena, codomain_domain, codomain_body);
    let expected_type = pi(arena, type_zero, codomain);
    (identity, expected_type)
}

/// `M := λ(_ : Two). Type 0` — the motive the large-elimination family
/// uses to return a universe per branch.
fn universe_motive(arena: &mut TermArena) -> TermHandle {
    let domain = two(arena);
    let body = type_sort(arena, 0);
    lambda(arena, domain, body)
}

/// `C := λ(t : Two). caseTwo(M, Π(_ : Two). Two, Two, t)` — a closed
/// family `Π(_ : Two). Type 0` landing at a function type on `zero` and
/// at `Two` on `one`, so the two branches of an outer elimination are
/// checked at definitionally different types.
fn branching_family(arena: &mut TermArena) -> TermHandle {
    let motive = universe_motive(arena);
    let domain = two(arena);
    let codomain = two(arena);
    let function_type = pi(arena, domain, codomain);
    let one_landing = two(arena);
    let bound = variable(arena, 0);
    let body = case_two(arena, motive, function_type, one_landing, bound);
    let domain = two(arena);
    lambda(arena, domain, body)
}

/// `C := λ(y : A). λ(_ : Id A x y). Id A y x` — the symmetry motive,
/// written at `depth` context entries where `a` is the carrier's index,
/// `x` the fixed endpoint's, and the bound `y` names index 0 under the
/// motive's own binder.
fn symmetry_motive(arena: &mut TermArena, a: u32, x: u32) -> TermHandle {
    let domain = variable(arena, a);
    let inner = {
        // Under the `y` binder the carrier is at `a + 1` and the fixed
        // endpoint at `x + 1`.
        let ty = variable(arena, a + 1);
        let fixed = variable(arena, x + 1);
        let bound = variable(arena, 0);
        let proof_domain = id(arena, ty, fixed, bound);
        // Under both binders: `Id A y x` — carrier `a + 2`, bound `y`
        // index 1, fixed `x + 2`.
        let ty = variable(arena, a + 2);
        let bound_y = variable(arena, 1);
        let fixed = variable(arena, x + 2);
        let body = id(arena, ty, bound_y, fixed);
        lambda(arena, proof_domain, body)
    };
    lambda(arena, domain, inner)
}

/// The `indW` step type `Π(a' : A). Π(k' : Π(b : B a'). W A B).
/// Π(_ : Π(b : B a'). P (k' b)). P (sup A B a' k')` over the prefix
/// `A : Type 0, B : Π(_:A). Type 0, a : A, k : Π(b:B a). W A B,
/// P : Π(_:W A B). Type 0` — written by hand so the kernel's own
/// step-type construction is cross-checked against an independent
/// encoding rather than against itself.
fn w_step_binding(arena: &mut TermArena) -> TermHandle {
    let a_domain = variable(arena, 4);
    // k' : Π(b : B a'). W A B — under `a'` B is index 4 and under
    // `a', b` the ambient A and B are indices 6 and 5.
    let b_at = variable(arena, 4);
    let a_bound = variable(arena, 0);
    let child_positions = apply(arena, b_at, a_bound);
    let carrier = variable(arena, 6);
    let children = variable(arena, 5);
    let w_at_two = w_type(arena, carrier, children);
    let function_type = pi(arena, child_positions, w_at_two);
    // ih : Π(b : B a'). P (k' b) — the domain at depth 2 has B = 5,
    // a' = 1; the codomain at depth 3 has P = 3, k' = 1, b = 0.
    let b_at = variable(arena, 5);
    let a_bound = variable(arena, 1);
    let hypothesis_domain = apply(arena, b_at, a_bound);
    let p_at = variable(arena, 3);
    let k_bound = variable(arena, 1);
    let b_bound = variable(arena, 0);
    let child = apply(arena, k_bound, b_bound);
    let hypothesis_codomain = apply(arena, p_at, child);
    let hypothesis = pi(arena, hypothesis_domain, hypothesis_codomain);
    // P (sup A B a' k') at depth 3: P = 3, a' = 2, k' = 1, and the
    // ambient A and B sit at 7 and 6.
    let p_at = variable(arena, 3);
    let carrier = variable(arena, 7);
    let children = variable(arena, 6);
    let label = variable(arena, 2);
    let function = variable(arena, 1);
    let node = sup(arena, carrier, children, label, function);
    let result = apply(arena, p_at, node);
    let inner = pi(arena, hypothesis, result);
    let middle = pi(arena, function_type, inner);
    pi(arena, a_domain, middle)
}

/// `A : Type 0, B : Π(_:A). Type 0, a : A, k : Π(b : B a). W A B,
/// P : Π(_ : W A B). Type 0, s : <step type>, t : W A B, u : W A B`.
/// At depth 0: u = 0, t = 1, s = 2, P = 3, k = 4, a = 5, B = 6, A = 7.
fn w_context(arena: &mut TermArena) -> Context {
    let type_zero = type_sort(arena, 0);
    let a_in_prefix = variable(arena, 0);
    let b_binding = pi(arena, a_in_prefix, type_zero);
    let a_binding = variable(arena, 1);
    // k : Π(b : B a). W A B over prefix [A, B, a].
    let b_at = variable(arena, 1);
    let a_at = variable(arena, 0);
    let child_positions = apply(arena, b_at, a_at);
    let carrier = variable(arena, 3);
    let children = variable(arena, 2);
    let w_under_b = w_type(arena, carrier, children);
    let k_binding = pi(arena, child_positions, w_under_b);
    // P : Π(_ : W A B). Type 0 over prefix [A, B, a, k].
    let carrier = variable(arena, 3);
    let children = variable(arena, 2);
    let w = w_type(arena, carrier, children);
    let p_binding = pi(arena, w, type_zero);
    let s_binding = w_step_binding(arena);
    // t over prefix [A, B, a, k, P, s]; u over one more binding.
    let carrier = variable(arena, 5);
    let children = variable(arena, 4);
    let t_binding = w_type(arena, carrier, children);
    let carrier = variable(arena, 6);
    let children = variable(arena, 5);
    let u_binding = w_type(arena, carrier, children);
    Context::empty()
        .extend(type_zero)
        .extend(b_binding)
        .extend(a_binding)
        .extend(k_binding)
        .extend(p_binding)
        .extend(s_binding)
        .extend(t_binding)
        .extend(u_binding)
}

/// `λ(a' : A). λ(k' : Π(b : B a'). W A B). λ(ih : Π(b : B a'). Two).
/// zero` — a concrete induction step for the constant motive
/// `λ(_ : W A B). Two`, checked against the kernel's built step type
/// through the motive's own reductions. Written in the 8-binding
/// `w_context` (A = 7, B = 6).
fn concrete_step(arena: &mut TermArena) -> TermHandle {
    let a_domain = variable(arena, 7);
    // Π(b : B a'). W A B under a': B = 7, a' = 0; codomain at depth 2:
    // A = 9, B = 8.
    let b_at = variable(arena, 7);
    let a_bound = variable(arena, 0);
    let child_positions = apply(arena, b_at, a_bound);
    let carrier = variable(arena, 9);
    let children = variable(arena, 8);
    let w_at_two = w_type(arena, carrier, children);
    let function_type = pi(arena, child_positions, w_at_two);
    // Π(b : B a'). Two at depth 2: B = 8, a' = 1.
    let b_at = variable(arena, 8);
    let a_bound = variable(arena, 1);
    let hypothesis_domain = apply(arena, b_at, a_bound);
    let hypothesis_codomain = two(arena);
    let hypothesis = pi(arena, hypothesis_domain, hypothesis_codomain);
    let body = two_zero(arena);
    let inner = lambda(arena, hypothesis, body);
    let middle = lambda(arena, function_type, inner);
    lambda(arena, a_domain, middle)
}

/// `Type` sorts holding arbitrary level expressions, for the
/// level-parameter tests below.
fn sort_level(arena: &mut TermArena, level: Level) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(level)))
}

/// `level + count` written with the successor constructor.
fn offset(mut level: Level, count: u32) -> Level {
    for _ in 0..count {
        level = level.successor().unwrap();
    }
    level
}

fn constant(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

/// `polyId : Π(A : Type u). Π(x : A). A := λ(A : Type u). λ(x : A). x` —
/// the canonical universe-polymorphic definition, one level parameter.
fn polymorphic_identity_declaration(arena: &mut TermArena) -> Declaration {
    let type_u = sort_level(arena, Level::Parameter(0));
    let bound_a = variable(arena, 0);
    let inner_a = variable(arena, 1);
    let inner_pi = pi(arena, bound_a, inner_a);
    let ty = pi(arena, type_u, inner_pi);
    let type_u = sort_level(arena, Level::Parameter(0));
    let bound_a = variable(arena, 0);
    let inner_x = variable(arena, 0);
    let inner = lambda(arena, bound_a, inner_x);
    let body = lambda(arena, type_u, inner);
    Declaration::definition(1, ty, body)
}

/// The `polyId` statement instantiated at `u`: `Π(A : Type u). Π(x : A). A`.
fn instantiated_identity_type(arena: &mut TermArena, u: Level) -> TermHandle {
    let type_u = sort_level(arena, u);
    let bound_a = variable(arena, 0);
    let inner_a = variable(arena, 1);
    let inner_pi = pi(arena, bound_a, inner_a);
    pi(arena, type_u, inner_pi)
}

/// A closed description for tests: indices `Two`, labels `Two`, child
/// positions `λ(_ : Two). Id Two one one` (a single relevant position),
/// `out = λt. t` (the node index is its label) and `next = λa. λ_. a`
/// (each child must carry its parent's label). Every level is `0`.
fn two_indexed_family(arena: &mut TermArena) -> IndexedFamily {
    let index = two(arena);
    let carrier = two(arena);
    let children = {
        let domain = two(arena);
        let ty = two(arena);
        let left = two_one(arena);
        let right = two_one(arena);
        let body = id(arena, ty, left, right);
        lambda(arena, domain, body)
    };
    let out = {
        let domain = two(arena);
        let body = variable(arena, 0);
        lambda(arena, domain, body)
    };
    let next = {
        let domain = two(arena);
        let inner_domain = {
            let ty = two(arena);
            let left = two_one(arena);
            let right = two_one(arena);
            id(arena, ty, left, right)
        };
        let body = variable(arena, 1);
        let inner = lambda(arena, inner_domain, body);
        lambda(arena, domain, inner)
    };
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index,
        carrier,
        children,
        out,
        next,
    }
}

/// The checked signature holding exactly the five scheme declarations.
fn scheme_signature(arena: &mut TermArena) -> Signature {
    let mut budget = default_budget();
    let declarations = indexed_scheme(arena);
    check_signature(arena, &declarations, &mut budget).unwrap()
}

/// The application-test context under the scheme signature:
/// `Q : Π(i : Two). Π(_ : IW i). Type 0`, `s : <the iindW step type>`,
/// `a : Two`, `g : Π(b : Id Two one one). IW (next a b)` — the family
/// description's `B`, `out` and `next` appear applied so the checker
/// must reduce them against `Id Two one one` and `a`. Indices:
/// g = 0, a = 1, s = 2, Q = 3.
fn indexed_context(arena: &mut TermArena, family: &IndexedFamily) -> Context {
    let motive_type = {
        let i_domain = two(arena);
        let i_bound = variable(arena, 0);
        let iw_at_i = family.indexed_w(arena, i_bound);
        let type_zero = type_sort(arena, 0);
        let inner = pi(arena, iw_at_i, type_zero);
        pi(arena, i_domain, inner)
    };
    let step_type = {
        // `Π(a : Two). Π(g : Π(b : B a). IW (next a b)).
        //  Π(_ : Π(b : B a). Q (next a b) (g b)). Q (out a) (isup a g)`
        let a_domain = two(arena);
        let g_domain = {
            // Under [a, Q]: `Π(b : B a). IW (next a b)`.
            let a_bound = variable(arena, 0);
            let b_domain = apply(arena, family.children, a_bound);
            let next_ab = {
                let a_bound = variable(arena, 1);
                let next_a = apply(arena, family.next, a_bound);
                let b_bound = variable(arena, 0);
                apply(arena, next_a, b_bound)
            };
            let codomain = family.indexed_w(arena, next_ab);
            pi(arena, b_domain, codomain)
        };
        let hypothesis_domain = {
            // Under [g, a, Q]: `Π(b : B a). Q (next a b) (g b)`.
            let a_bound = variable(arena, 1);
            let b_domain = apply(arena, family.children, a_bound);
            let codomain = {
                // Under [b, g, a, Q]: Q = 3, a = 2, g = 1, b = 0.
                let next_ab = {
                    let a_bound = variable(arena, 2);
                    let next_a = apply(arena, family.next, a_bound);
                    let b_bound = variable(arena, 0);
                    apply(arena, next_a, b_bound)
                };
                let q = variable(arena, 3);
                let q_at = apply(arena, q, next_ab);
                let g = variable(arena, 1);
                let b_bound = variable(arena, 0);
                let g_b = apply(arena, g, b_bound);
                apply(arena, q_at, g_b)
            };
            pi(arena, b_domain, codomain)
        };
        let step_result = {
            // Under [_, g, a, Q]: `Q (out a) (isup a g)` — `out a` left
            // unreduced so the check must compute `out = λt. t`.
            let q = variable(arena, 3);
            let a_bound = variable(arena, 2);
            let out_a = apply(arena, family.out, a_bound);
            let a_bound = variable(arena, 2);
            let g = variable(arena, 1);
            let node = family.sup(arena, a_bound, g);
            let q_out = apply(arena, q, out_a);
            apply(arena, q_out, node)
        };
        let inner = pi(arena, hypothesis_domain, step_result);
        let middle = pi(arena, g_domain, inner);
        pi(arena, a_domain, middle)
    };
    let a_binding = two(arena);
    let g_binding = {
        // Under [a, s, Q]: `Π(b : B a). IW (next a b)`.
        let a_bound = variable(arena, 0);
        let b_domain = apply(arena, family.children, a_bound);
        let next_ab = {
            let a_bound = variable(arena, 1);
            let next_a = apply(arena, family.next, a_bound);
            let b_bound = variable(arena, 0);
            apply(arena, next_a, b_bound)
        };
        let codomain = family.indexed_w(arena, next_ab);
        pi(arena, b_domain, codomain)
    };
    Context::empty()
        .with_signature(scheme_signature(arena))
        .extend(motive_type)
        .extend(step_type)
        .extend(a_binding)
        .extend(g_binding)
}

/// A closed description for tests: carrier `Two`, relation `λa. λb. Id
/// Two a b` — the equality relation — at levels `[0, 0]`.
fn two_quotient(arena: &mut TermArena) -> QuotientFamily {
    let carrier = two(arena);
    let relation = {
        let domain = two(arena);
        let inner_domain = two(arena);
        let ty = two(arena);
        let left = variable(arena, 1);
        let right = variable(arena, 0);
        let body = id(arena, ty, left, right);
        let inner = lambda(arena, inner_domain, body);
        lambda(arena, domain, inner)
    };
    QuotientFamily {
        levels: [Level::Constant(0), Level::Constant(0)],
        carrier,
        relation,
    }
}

/// The checked signature holding exactly the twenty scheme
/// declarations.
fn quotient_signature(arena: &mut TermArena) -> Signature {
    let mut budget = default_budget();
    let declarations = quotient_scheme(arena);
    check_signature(arena, &declarations, &mut budget).unwrap()
}

/// `isSet[0] B` — the setness law instantiated at `Two`-level.
fn is_set_zero(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    let is_set = constant(arena, QUOTIENT_IS_SET, vec![Level::Constant(0)]);
    apply(arena, is_set, ty)
}

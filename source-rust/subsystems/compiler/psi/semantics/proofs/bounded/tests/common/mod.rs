//! Shared term-building helpers for the integration suites: the small
//! constructor DSL every `indexed_*`/`existence`/`conversion` fixture shares.

#![allow(dead_code)]

use proof_admission::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Sort, Term, TermArena,
    TermHandle, argument_error_pair_pattern, check_type, declare_inductive_family, infer_type,
};
use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};

pub(crate) const NAT: u32 = 0;
pub(crate) const NAT_ZERO: u32 = 1;
pub(crate) const NAT_SUCC: u32 = 2;
pub(crate) const ELEM: u32 = 3;

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

pub(crate) fn apply(
    arena: &mut TermArena,
    function: TermHandle,
    argument: TermHandle,
) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

pub(crate) fn apps(arena: &mut TermArena, mut head: TermHandle, args: &[TermHandle]) -> TermHandle {
    for &arg in args {
        head = apply(arena, head, arg);
    }
    head
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

pub(crate) fn sort_level(arena: &mut TermArena, level: Level) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(level)))
}

pub(crate) fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    sort_level(arena, Level::Constant(level))
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

pub(crate) fn id(
    arena: &mut TermArena,
    ty: TermHandle,
    left: TermHandle,
    right: TermHandle,
) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
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

pub(crate) fn box_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::BoxIntro { ty, value })
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

/// The `Nat`, `zeroN`, and `succN` assumptions shared by Nat-indexed test
/// families at positions 0, 1, 2.
pub(crate) fn nat_succ_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // 0: Nat
    declarations.push(Declaration::assumption(0, nat(arena))); // 1: zeroN
    let succ_domain = nat(arena);
    let succ_codomain = nat(arena);
    let succ_statement = pi(arena, succ_domain, succ_codomain);
    declarations.push(Declaration::assumption(0, succ_statement)); // 2: succN
    declarations
}

/// Declare the native length-indexed vector family at `base`:
/// - `base + 0`: `Vec : Π(n : Nat). Type 0`
/// - `base + 1`: `nil : Vec zeroN`
/// - `base + 2`: `cons : Π(e : Elem). Π(n : Nat). Π(t : Vec n). Vec (succN n)`
/// - `base + 3`: `Vec.rec`
pub(crate) fn declare_vector_family(arena: &mut TermArena, base: u32) -> Vec<Declaration> {
    let nat_ty = nat(arena);
    let type0 = type_sort(arena, 0);
    let vec_ty = pi(arena, nat_ty, type0);

    let vec_c = constant(arena, base, Vec::new());
    let zero = nat_zero(arena);
    let nil_ty = apply(arena, vec_c, zero);

    // `Π(e : Elem). Π(n : Nat). Π(t : Vec n). Vec (succN n)`
    let elem_ty = elem(arena);
    let tail_ty = {
        let n = variable(arena, 0);
        apply(arena, vec_c, n)
    };
    let result_ty = {
        let n = variable(arena, 1);
        let succ_n = nat_succ(arena, n);
        apply(arena, vec_c, succ_n)
    };
    let over_tail = pi(arena, tail_ty, result_ty);
    let over_n = pi(arena, nat_ty, over_tail);
    let cons_ty = pi(arena, elem_ty, over_n);

    declare_inductive_family(arena, base, 0, 0, vec_ty, &[nil_ty, cons_ty])
}

/// Infer `term`'s type and require the type-mismatch family.
pub(crate) fn assert_type_mismatch(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) {
    let error = infer_type(arena, context, term, budget).unwrap_err();
    assert!(matches!(error, argument_error_pair_pattern!()));
}

/// The same rejection asserted through `check_type` against `expected`.
pub(crate) fn assert_check_type_mismatch(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    expected: TermHandle,
    budget: &mut Budget,
) {
    let error = check_type(arena, context, term, expected, budget).unwrap_err();
    assert!(matches!(error, argument_error_pair_pattern!()));
}

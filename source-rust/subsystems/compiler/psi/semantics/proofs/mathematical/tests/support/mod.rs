//! Term builders shared by the native and mutual inductive test suites.

use mathematical_kernel::{Declaration, Level, Sort, Term, TermArena, TermHandle, shift};

pub fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

pub fn pi_many(
    arena: &mut TermArena,
    domains: &[TermHandle],
    mut codomain: TermHandle,
) -> TermHandle {
    for &d in domains.iter().rev() {
        codomain = pi(arena, d, codomain);
    }
    codomain
}

pub fn lam(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

pub fn lam_many(arena: &mut TermArena, domains: &[TermHandle], mut body: TermHandle) -> TermHandle {
    for &d in domains.iter().rev() {
        body = lam(arena, d, body);
    }
    body
}

pub fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Sigma { domain, codomain })
}

pub fn prod(arena: &mut TermArena, left: TermHandle, right: TermHandle) -> TermHandle {
    let shifted_right = shift(arena, right, 0, 1);
    sigma(arena, left, shifted_right)
}

pub fn pair(arena: &mut TermArena, first: TermHandle, second: TermHandle) -> TermHandle {
    arena.insert(Term::Pair { first, second })
}

pub fn fst(arena: &mut TermArena, p: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair: p })
}

pub fn snd(arena: &mut TermArena, p: TermHandle) -> TermHandle {
    arena.insert(Term::Snd { pair: p })
}

pub fn var(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

pub fn c0(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

pub fn c_levels(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

pub fn type_n(arena: &mut TermArena, n: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(n))))
}

pub fn apps(arena: &mut TermArena, head: TermHandle, args: &[TermHandle]) -> TermHandle {
    arena.apply_arguments(head, args)
}

pub fn id_ty(
    arena: &mut TermArena,
    ty: TermHandle,
    left: TermHandle,
    right: TermHandle,
) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

pub fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
}

pub fn scalar_theory(arena: &mut TermArena) -> Vec<Declaration> {
    let t0 = type_n(arena, 0);
    let scalar = c0(arena, 0);
    let le = c0(arena, 1);
    let le_ty = pi_many(arena, &[scalar, scalar], t0);
    let v1 = var(arena, 1);
    let v2 = var(arena, 2);
    let v4 = var(arena, 4);
    let le_ab = apps(arena, le, &[v2, v1]);
    let le_bc = apps(arena, le, &[v2, v1]);
    let le_ac = apps(arena, le, &[v4, v2]);
    let le_trans_ty = pi_many(arena, &[scalar, scalar, scalar, le_ab, le_bc], le_ac);
    vec![
        Declaration::assumption(0, t0),
        Declaration::assumption(0, le_ty),
        Declaration::assumption(0, le_trans_ty),
    ]
}

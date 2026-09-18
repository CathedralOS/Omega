//! Term notation for authoring scheme declarations: binders are named
//! strings resolved to de Bruijn indices mechanically, so the emitted
//! terms stay free of hand-computed index arithmetic. The checker only
//! ever sees the built `Term`s; nothing name-based escapes construction.
//! `Variable` resolution is a build-time panic on a typo, never a
//! silently wrong index.
//!
//! Both derived schemes — [`super::indexed`] and [`super::quotient`] —
//! author their declarations through this one DSL so the only
//! construction-level difference between schemes is the declarations
//! themselves.

use super::term::{Level, Sort, Term, TermArena, TermHandle};

#[derive(Clone)]
pub(super) enum Syntax {
    Variable(&'static str),
    Sort(Sort),
    Pi(&'static str, Box<Syntax>, Box<Syntax>),
    Lambda(&'static str, Box<Syntax>, Box<Syntax>),
    Apply(Box<Syntax>, Box<Syntax>),
    Sigma(&'static str, Box<Syntax>, Box<Syntax>),
    Pair(Box<Syntax>, Box<Syntax>),
    Fst(Box<Syntax>),
    Snd(Box<Syntax>),
    Id(Box<Syntax>, Box<Syntax>, Box<Syntax>),
    Refl(Box<Syntax>, Box<Syntax>),
    IdElim(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
    W(Box<Syntax>, Box<Syntax>),
    Sup(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
    IndW(Box<Syntax>, Box<Syntax>, Box<Syntax>),
    Squash(Box<Syntax>),
    SquashIntro(Box<Syntax>, Box<Syntax>),
    /// `Box S` — named `Boxed` so `Box` stays the allocator inside this
    /// file's `Box<Syntax>` fields.
    Boxed(Box<Syntax>),
    BoxIntro(Box<Syntax>, Box<Syntax>),
    BoxElim(Box<Syntax>, Box<Syntax>, Box<Syntax>),
    /// A scheme declaration reference — `Constant { declaration,
    /// levels }` at the exact instantiation the referring declaration
    /// supplies. Every `Level::Parameter(i)` inside `levels` names the
    /// *referring* declaration's own `i`-th universe parameter, and the
    /// list's length must equal the *referenced* declaration's level
    /// arity; both rules are re-decided by typing, never trusted here.
    Scheme(u32, Vec<Level>),
}

pub(super) fn build(
    arena: &mut TermArena,
    scope: &mut Vec<&'static str>,
    syntax: &Syntax,
) -> TermHandle {
    match syntax {
        Syntax::Variable(name) => {
            let position = scope
                .iter()
                .rposition(|bound| bound == name)
                .unwrap_or_else(|| panic!("unbound scheme variable {name}"));
            // `scope` is outermost-first; the de Bruijn index counts
            // inward from the end.
            let index = scope.len() - 1 - position;
            arena.insert(Term::Variable(index as u32))
        }
        Syntax::Sort(sort) => arena.insert(Term::Sort(sort.clone())),
        Syntax::Pi(name, domain, codomain) | Syntax::Sigma(name, domain, codomain) => {
            let domain = build(arena, scope, domain);
            scope.push(name);
            let codomain = build(arena, scope, codomain);
            scope.pop();
            match syntax {
                Syntax::Pi(..) => arena.insert(Term::Pi { domain, codomain }),
                _ => arena.insert(Term::Sigma { domain, codomain }),
            }
        }
        Syntax::Lambda(name, domain, body) => {
            let domain = build(arena, scope, domain);
            scope.push(name);
            let body = build(arena, scope, body);
            scope.pop();
            arena.insert(Term::Lambda { domain, body })
        }
        Syntax::Apply(function, argument) => {
            let function = build(arena, scope, function);
            let argument = build(arena, scope, argument);
            arena.insert(Term::Apply { function, argument })
        }
        Syntax::Pair(first, second) => {
            let first = build(arena, scope, first);
            let second = build(arena, scope, second);
            arena.insert(Term::Pair { first, second })
        }
        Syntax::Fst(pair) => {
            let pair = build(arena, scope, pair);
            arena.insert(Term::Fst { pair })
        }
        Syntax::Snd(pair) => {
            let pair = build(arena, scope, pair);
            arena.insert(Term::Snd { pair })
        }
        Syntax::Id(ty, left, right) => {
            let ty = build(arena, scope, ty);
            let left = build(arena, scope, left);
            let right = build(arena, scope, right);
            arena.insert(Term::Id { ty, left, right })
        }
        Syntax::Refl(ty, value) => {
            let ty = build(arena, scope, ty);
            let value = build(arena, scope, value);
            arena.insert(Term::Refl { ty, value })
        }
        Syntax::IdElim(motive, base, endpoint, proof) => {
            let motive = build(arena, scope, motive);
            let base = build(arena, scope, base);
            let endpoint = build(arena, scope, endpoint);
            let proof = build(arena, scope, proof);
            arena.insert(Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            })
        }
        Syntax::W(carrier, children) => {
            let carrier = build(arena, scope, carrier);
            let children = build(arena, scope, children);
            arena.insert(Term::W { carrier, children })
        }
        Syntax::Sup(carrier, children, label, function) => {
            let carrier = build(arena, scope, carrier);
            let children = build(arena, scope, children);
            let label = build(arena, scope, label);
            let function = build(arena, scope, function);
            arena.insert(Term::Sup {
                carrier,
                children,
                label,
                function,
            })
        }
        Syntax::IndW(motive, step, tree) => {
            let motive = build(arena, scope, motive);
            let step = build(arena, scope, step);
            let tree = build(arena, scope, tree);
            arena.insert(Term::IndW { motive, step, tree })
        }
        Syntax::Squash(ty) => {
            let ty = build(arena, scope, ty);
            arena.insert(Term::Squash { ty })
        }
        Syntax::SquashIntro(ty, value) => {
            let ty = build(arena, scope, ty);
            let value = build(arena, scope, value);
            arena.insert(Term::SquashIntro { ty, value })
        }
        Syntax::Boxed(ty) => {
            let ty = build(arena, scope, ty);
            arena.insert(Term::Box { ty })
        }
        Syntax::BoxIntro(ty, value) => {
            let ty = build(arena, scope, ty);
            let value = build(arena, scope, value);
            arena.insert(Term::BoxIntro { ty, value })
        }
        Syntax::BoxElim(motive, body, scrutinee) => {
            let motive = build(arena, scope, motive);
            let body = build(arena, scope, body);
            let scrutinee = build(arena, scope, scrutinee);
            arena.insert(Term::BoxElim {
                motive,
                body,
                scrutinee,
            })
        }
        Syntax::Scheme(declaration, levels) => arena.insert(Term::Constant {
            declaration: *declaration,
            levels: levels.clone(),
        }),
    }
}

pub(super) fn v(name: &'static str) -> Syntax {
    Syntax::Variable(name)
}

pub(super) fn sort(level: Level) -> Syntax {
    Syntax::Sort(Sort::Type(level))
}

/// `Type u` at universe parameter `parameter` of the referring
/// declaration's own level scope.
pub(super) fn ty(parameter: u32) -> Syntax {
    sort(Level::Parameter(parameter))
}

/// `Strict u` at universe parameter `parameter` of the referring
/// declaration's own level scope.
pub(super) fn strict(parameter: u32) -> Syntax {
    Syntax::Sort(Sort::Strict(Level::Parameter(parameter)))
}

pub(super) fn app(function: Syntax, argument: Syntax) -> Syntax {
    Syntax::Apply(Box::new(function), Box::new(argument))
}

pub(super) fn apps(function: Syntax, arguments: impl IntoIterator<Item = Syntax>) -> Syntax {
    arguments.into_iter().fold(function, app)
}

pub(super) fn pi(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
    Syntax::Pi(name, Box::new(domain), Box::new(codomain))
}

pub(super) fn lam(name: &'static str, domain: Syntax, body: Syntax) -> Syntax {
    Syntax::Lambda(name, Box::new(domain), Box::new(body))
}

pub(super) fn sigma(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
    Syntax::Sigma(name, Box::new(domain), Box::new(codomain))
}

pub(super) fn pair(first: Syntax, second: Syntax) -> Syntax {
    Syntax::Pair(Box::new(first), Box::new(second))
}

pub(super) fn fst(pair: Syntax) -> Syntax {
    Syntax::Fst(Box::new(pair))
}

pub(super) fn snd(pair: Syntax) -> Syntax {
    Syntax::Snd(Box::new(pair))
}

pub(super) fn id(ty: Syntax, left: Syntax, right: Syntax) -> Syntax {
    Syntax::Id(Box::new(ty), Box::new(left), Box::new(right))
}

pub(super) fn refl(ty: Syntax, value: Syntax) -> Syntax {
    Syntax::Refl(Box::new(ty), Box::new(value))
}

pub(super) fn w(carrier: Syntax, children: Syntax) -> Syntax {
    Syntax::W(Box::new(carrier), Box::new(children))
}

pub(super) fn sup(carrier: Syntax, children: Syntax, label: Syntax, function: Syntax) -> Syntax {
    Syntax::Sup(
        Box::new(carrier),
        Box::new(children),
        Box::new(label),
        Box::new(function),
    )
}

/// `J(motive, base, endpoint, proof)` — the identity eliminator.
pub(super) fn jelim(motive: Syntax, base: Syntax, endpoint: Syntax, proof: Syntax) -> Syntax {
    Syntax::IdElim(
        Box::new(motive),
        Box::new(base),
        Box::new(endpoint),
        Box::new(proof),
    )
}

pub(super) fn indw(motive: Syntax, step: Syntax, tree: Syntax) -> Syntax {
    Syntax::IndW(Box::new(motive), Box::new(step), Box::new(tree))
}

/// `Squash A` — the strict squash former over a relevant `A`.
pub(super) fn squash(ty: Syntax) -> Syntax {
    Syntax::Squash(Box::new(ty))
}

/// `sq_A x` — squash introduction, carrying its carrier annotation.
pub(super) fn squash_intro(ty: Syntax, value: Syntax) -> Syntax {
    Syntax::SquashIntro(Box::new(ty), Box::new(value))
}

/// `Box S` — boxing the strict proposition `S` as relevant data.
pub(super) fn boxed(ty: Syntax) -> Syntax {
    Syntax::Boxed(Box::new(ty))
}

/// `box_S x` — box introduction, carrying its strict annotation.
pub(super) fn box_intro(ty: Syntax, value: Syntax) -> Syntax {
    Syntax::BoxIntro(Box::new(ty), Box::new(value))
}

/// `unbox P f x` — box elimination: `f : Π(a : S). P (box a)`.
pub(super) fn box_elim(motive: Syntax, body: Syntax, scrutinee: Syntax) -> Syntax {
    Syntax::BoxElim(Box::new(motive), Box::new(body), Box::new(scrutinee))
}

/// A scheme declaration reference at the explicit instantiation
/// `levels`: `Constant { declaration, levels }`. The referring
/// declaration supplies one level per referenced level parameter.
pub(super) fn scheme_at(declaration: u32, levels: Vec<Level>) -> Syntax {
    Syntax::Scheme(declaration, levels)
}

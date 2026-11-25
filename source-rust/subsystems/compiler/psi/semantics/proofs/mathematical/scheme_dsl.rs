use super::signature::Declaration;
use super::term::{Level, Sort, Term, TermArena, TermHandle};

#[derive(Clone)]
pub enum Syntax {
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
    Squash(Box<Syntax>),
    SquashIntro(Box<Syntax>, Box<Syntax>),
    Boxed(Box<Syntax>),
    BoxIntro(Box<Syntax>, Box<Syntax>),
    BoxElim(Box<Syntax>, Box<Syntax>, Box<Syntax>),
    Scheme(u32, Vec<Level>),
    Two,
    CaseTwo(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
}

pub(crate) fn define(
    arena: &mut TermArena,
    level_arity: u32,
    statement: &Syntax,
    body: &Syntax,
) -> Declaration {
    Declaration::definition(
        level_arity,
        build(arena, &mut Vec::new(), statement),
        build(arena, &mut Vec::new(), body),
    )
}

pub(crate) fn assume(arena: &mut TermArena, level_arity: u32, statement: &Syntax) -> Declaration {
    Declaration::assumption(level_arity, build(arena, &mut Vec::new(), statement))
}

pub fn build(arena: &mut TermArena, scope: &mut Vec<&'static str>, syntax: &Syntax) -> TermHandle {
    match syntax {
        Syntax::Variable(name) => {
            let position = scope
                .iter()
                .rposition(|bound| bound == name)
                .unwrap_or_else(|| panic!("unbound scheme variable {name}"));
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
        Syntax::Two => arena.insert(Term::Two),
        Syntax::CaseTwo(motive, zero_branch, one_branch, scrutinee) => {
            let motive = build(arena, scope, motive);
            let zero_branch = build(arena, scope, zero_branch);
            let one_branch = build(arena, scope, one_branch);
            let scrutinee = build(arena, scope, scrutinee);
            arena.insert(Term::CaseTwo {
                motive,
                zero_branch,
                one_branch,
                scrutinee,
            })
        }
    }
}

pub fn v(name: &'static str) -> Syntax {
    Syntax::Variable(name)
}

pub fn sort(level: Level) -> Syntax {
    Syntax::Sort(Sort::Type(level))
}

pub(crate) fn ty(parameter: u32) -> Syntax {
    sort(Level::Parameter(parameter))
}

pub(crate) fn strict(parameter: u32) -> Syntax {
    Syntax::Sort(Sort::Strict(Level::Parameter(parameter)))
}

pub(crate) fn app(function: Syntax, argument: Syntax) -> Syntax {
    Syntax::Apply(Box::new(function), Box::new(argument))
}

pub fn apps(function: Syntax, arguments: impl IntoIterator<Item = Syntax>) -> Syntax {
    arguments.into_iter().fold(function, app)
}

pub fn pi(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
    Syntax::Pi(name, Box::new(domain), Box::new(codomain))
}

pub fn pis(binders: impl IntoIterator<Item = (&'static str, Syntax)>, body: Syntax) -> Syntax {
    binders
        .into_iter()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .fold(body, |inner, (name, domain)| pi(name, domain, inner))
}

pub fn lam(name: &'static str, domain: Syntax, body: Syntax) -> Syntax {
    Syntax::Lambda(name, Box::new(domain), Box::new(body))
}

pub fn sigma(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
    Syntax::Sigma(name, Box::new(domain), Box::new(codomain))
}

pub(crate) fn pair(first: Syntax, second: Syntax) -> Syntax {
    Syntax::Pair(Box::new(first), Box::new(second))
}

pub fn id(ty: Syntax, left: Syntax, right: Syntax) -> Syntax {
    Syntax::Id(Box::new(ty), Box::new(left), Box::new(right))
}

pub(crate) fn refl(ty: Syntax, value: Syntax) -> Syntax {
    Syntax::Refl(Box::new(ty), Box::new(value))
}

pub(crate) fn jelim(motive: Syntax, base: Syntax, endpoint: Syntax, proof: Syntax) -> Syntax {
    Syntax::IdElim(
        Box::new(motive),
        Box::new(base),
        Box::new(endpoint),
        Box::new(proof),
    )
}

pub(crate) fn squash(ty: Syntax) -> Syntax {
    Syntax::Squash(Box::new(ty))
}

pub(crate) fn squash_intro(ty: Syntax, value: Syntax) -> Syntax {
    Syntax::SquashIntro(Box::new(ty), Box::new(value))
}

pub(crate) fn boxed(ty: Syntax) -> Syntax {
    Syntax::Boxed(Box::new(ty))
}

pub(crate) fn box_intro(ty: Syntax, value: Syntax) -> Syntax {
    Syntax::BoxIntro(Box::new(ty), Box::new(value))
}

pub(crate) fn box_elim(motive: Syntax, body: Syntax, scrutinee: Syntax) -> Syntax {
    Syntax::BoxElim(Box::new(motive), Box::new(body), Box::new(scrutinee))
}

pub fn scheme_at(declaration: u32, levels: Vec<Level>) -> Syntax {
    Syntax::Scheme(declaration, levels)
}

pub fn two() -> Syntax {
    Syntax::Two
}

pub fn case_two(
    motive: Syntax,
    zero_branch: Syntax,
    one_branch: Syntax,
    scrutinee: Syntax,
) -> Syntax {
    Syntax::CaseTwo(
        Box::new(motive),
        Box::new(zero_branch),
        Box::new(one_branch),
        Box::new(scrutinee),
    )
}

//! Capture-avoiding de Bruijn operations. Every walk here goes through
//! [`TermArena::rebuild_subterms`], which states the term layout — each
//! node's slots and the binders scoping them — once. A traversal supplies
//! only its leaf rule and its binder step. All three return the original
//! handle when no node changes, so unchanged subterms keep their storage.

use super::term::{Level, Sort, Term, TermArena, TermHandle, instantiate_level};

/// Increase every free variable at or above `cutoff` by `amount`. Returns
/// `term` unchanged when `amount` is zero or no variable qualifies.
pub fn shift(arena: &mut TermArena, term: TermHandle, cutoff: u32, amount: u32) -> TermHandle {
    if amount == 0 {
        return term;
    }
    if let Term::Variable(index) = arena.get(term) {
        return if index >= cutoff {
            arena.insert(Term::Variable(
                index.checked_add(amount).expect("variable shift overflow"),
            ))
        } else {
            term
        };
    }
    arena
        .rebuild_subterms(term, &mut |arena, child, binders| {
            Ok::<_, std::convert::Infallible>(shift(arena, child, cutoff + binders, amount))
        })
        .expect("a term shift never refuses")
        .unwrap_or(term)
}

/// Replace de Bruijn index 0 in `body` with `argument`, shifting `argument`
/// under each binder it passes and decrementing the remaining free variables
/// of `body`. Returns `body` unchanged when index 0 does not occur.
pub fn substitute(arena: &mut TermArena, body: TermHandle, argument: TermHandle) -> TermHandle {
    substitute_at(arena, body, argument, 0)
}

fn substitute_at(
    arena: &mut TermArena,
    term: TermHandle,
    argument: TermHandle,
    depth: u32,
) -> TermHandle {
    if let Term::Variable(index) = arena.get(term) {
        return match index.cmp(&depth) {
            std::cmp::Ordering::Equal => shift(arena, argument, 0, depth),
            std::cmp::Ordering::Greater => arena.insert(Term::Variable(index - 1)),
            std::cmp::Ordering::Less => term,
        };
    }
    arena
        .rebuild_subterms(term, &mut |arena, child, binders| {
            Ok::<_, std::convert::Infallible>(substitute_at(
                arena,
                child,
                argument,
                depth + binders,
            ))
        })
        .expect("a term substitution never refuses")
        .unwrap_or(term)
}

/// Instantiate the universe parameters of `term` at `arguments`: every
/// `Parameter(i)` inside a `Sort` payload or a nested `Constant`'s level
/// arguments becomes `arguments[i]`. This is the substitution half of the
/// declaration rule — a declaration checked at arity `n` is sound at
/// `arguments` of length `n` because the checking judgment was
/// parametric. Returns the same handle where nothing changed, so
/// instantiated subterms keep their storage.
///
/// `Err(index)` reports a parameter out of range of `arguments`: typing
/// scope-checks the declaration's own arity and every instantiation's
/// argument list before this runs, so reaching it means the signature or
/// term was malformed input rather than a checked judgment.
pub fn instantiate_levels(
    arena: &mut TermArena,
    term: TermHandle,
    arguments: &[Level],
) -> Result<TermHandle, u32> {
    // A level parameter is reachable only through a `Sort` payload or a
    // constant's level arguments: those two payloads are the whole leaf
    // rule, and every other node recurses through the shared walk. Level
    // arguments are not de Bruijn terms that a binder can scope, so no
    // slot here takes a binder step.
    match arena.get(term) {
        Term::Sort(sort) => {
            let level = instantiate_level(&sort.level(), arguments)?;
            let sort = match sort {
                Sort::Type(_) => Sort::Type(level),
                Sort::Strict(_) => Sort::Strict(level),
            };
            Ok(arena.insert(Term::Sort(sort)))
        }
        Term::Constant {
            declaration,
            levels,
        } => {
            let mut instantiated = Vec::with_capacity(levels.len());
            for level in &levels {
                instantiated.push(instantiate_level(level, arguments)?);
            }
            Ok(arena.insert(Term::Constant {
                declaration,
                levels: instantiated,
            }))
        }
        _ => Ok(arena
            .rebuild_subterms(term, &mut |arena, child, _binders| {
                instantiate_levels(arena, child, arguments)
            })?
            .unwrap_or(term)),
    }
}

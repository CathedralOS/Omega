//! Capture-avoiding de Bruijn operations. Both functions return the original
//! handle when no node changes, so unchanged subterms keep their storage.

use super::term::{Term, TermArena, TermHandle};

/// Increase every free variable at or above `cutoff` by `amount`. Returns
/// `term` unchanged when `amount` is zero or no variable qualifies.
pub fn shift(arena: &mut TermArena, term: TermHandle, cutoff: u32, amount: u32) -> TermHandle {
    if amount == 0 {
        return term;
    }
    match arena.get(term) {
        Term::Variable(index) => {
            if index >= cutoff {
                arena.insert(Term::Variable(
                    index.checked_add(amount).expect("variable shift overflow"),
                ))
            } else {
                term
            }
        }
        Term::Pi { domain, codomain } => {
            let shifted_domain = shift(arena, domain, cutoff, amount);
            let shifted_codomain = shift(arena, codomain, cutoff + 1, amount);
            if shifted_domain == domain && shifted_codomain == codomain {
                return term;
            }
            arena.insert(Term::Pi {
                domain: shifted_domain,
                codomain: shifted_codomain,
            })
        }
        Term::Lambda { domain, body } => {
            let shifted_domain = shift(arena, domain, cutoff, amount);
            let shifted_body = shift(arena, body, cutoff + 1, amount);
            if shifted_domain == domain && shifted_body == body {
                return term;
            }
            arena.insert(Term::Lambda {
                domain: shifted_domain,
                body: shifted_body,
            })
        }
        Term::Apply { function, argument } => {
            let shifted_function = shift(arena, function, cutoff, amount);
            let shifted_argument = shift(arena, argument, cutoff, amount);
            if shifted_function == function && shifted_argument == argument {
                return term;
            }
            arena.insert(Term::Apply {
                function: shifted_function,
                argument: shifted_argument,
            })
        }
        Term::Sigma { domain, codomain } => {
            let shifted_domain = shift(arena, domain, cutoff, amount);
            let shifted_codomain = shift(arena, codomain, cutoff + 1, amount);
            if shifted_domain == domain && shifted_codomain == codomain {
                return term;
            }
            arena.insert(Term::Sigma {
                domain: shifted_domain,
                codomain: shifted_codomain,
            })
        }
        Term::Pair { first, second } => {
            let shifted_first = shift(arena, first, cutoff, amount);
            let shifted_second = shift(arena, second, cutoff, amount);
            if shifted_first == first && shifted_second == second {
                return term;
            }
            arena.insert(Term::Pair {
                first: shifted_first,
                second: shifted_second,
            })
        }
        Term::Fst { pair } => {
            let shifted_pair = shift(arena, pair, cutoff, amount);
            if shifted_pair == pair {
                return term;
            }
            arena.insert(Term::Fst { pair: shifted_pair })
        }
        Term::Snd { pair } => {
            let shifted_pair = shift(arena, pair, cutoff, amount);
            if shifted_pair == pair {
                return term;
            }
            arena.insert(Term::Snd { pair: shifted_pair })
        }
        Term::Sort(_) | Term::Dummy => term,
    }
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
    match arena.get(term) {
        Term::Variable(index) => match index.cmp(&depth) {
            std::cmp::Ordering::Equal => shift(arena, argument, 0, depth),
            std::cmp::Ordering::Greater => arena.insert(Term::Variable(index - 1)),
            std::cmp::Ordering::Less => term,
        },
        Term::Pi { domain, codomain } => {
            let new_domain = substitute_at(arena, domain, argument, depth);
            let new_codomain = substitute_at(arena, codomain, argument, depth + 1);
            if new_domain == domain && new_codomain == codomain {
                return term;
            }
            arena.insert(Term::Pi {
                domain: new_domain,
                codomain: new_codomain,
            })
        }
        Term::Lambda { domain, body } => {
            let new_domain = substitute_at(arena, domain, argument, depth);
            let new_body = substitute_at(arena, body, argument, depth + 1);
            if new_domain == domain && new_body == body {
                return term;
            }
            arena.insert(Term::Lambda {
                domain: new_domain,
                body: new_body,
            })
        }
        Term::Apply {
            function,
            argument: operand,
        } => {
            let new_function = substitute_at(arena, function, argument, depth);
            let new_operand = substitute_at(arena, operand, argument, depth);
            if new_function == function && new_operand == operand {
                return term;
            }
            arena.insert(Term::Apply {
                function: new_function,
                argument: new_operand,
            })
        }
        Term::Sigma { domain, codomain } => {
            let new_domain = substitute_at(arena, domain, argument, depth);
            let new_codomain = substitute_at(arena, codomain, argument, depth + 1);
            if new_domain == domain && new_codomain == codomain {
                return term;
            }
            arena.insert(Term::Sigma {
                domain: new_domain,
                codomain: new_codomain,
            })
        }
        Term::Pair { first, second } => {
            let new_first = substitute_at(arena, first, argument, depth);
            let new_second = substitute_at(arena, second, argument, depth);
            if new_first == first && new_second == second {
                return term;
            }
            arena.insert(Term::Pair {
                first: new_first,
                second: new_second,
            })
        }
        Term::Fst { pair } => {
            let new_pair = substitute_at(arena, pair, argument, depth);
            if new_pair == pair {
                return term;
            }
            arena.insert(Term::Fst { pair: new_pair })
        }
        Term::Snd { pair } => {
            let new_pair = substitute_at(arena, pair, argument, depth);
            if new_pair == pair {
                return term;
            }
            arena.insert(Term::Snd { pair: new_pair })
        }
        Term::Sort(_) | Term::Dummy => term,
    }
}

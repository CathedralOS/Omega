//! Typed conversion for the Π/Σ fragment: β and pair-projection weak-head
//! normalization under a step ceiling, pair eta at a `Sigma` shared type,
//! and definitional proof irrelevance gated on the *shared type's* sort —
//! never on the shape of either side. Function eta is a separate step.

use super::substitution::substitute;
use super::term::{Term, TermArena, TermHandle};
use super::typing::{Context, CoreError, infer_sort, infer_type};

/// The default number of β steps a conversion attempt may take.
pub const DEFAULT_CONVERSION_STEPS: u32 = 65_536;

/// Bounded resource for normalization. Exhaustion is the typed error
/// `CoreError::StepCeiling`, never a false judgment and never a hang.
pub struct Budget {
    remaining_steps: u32,
}

impl Budget {
    pub fn new(remaining_steps: u32) -> Self {
        Self { remaining_steps }
    }

    pub fn remaining(&self) -> u32 {
        self.remaining_steps
    }

    fn consume(&mut self) -> Result<(), CoreError> {
        self.remaining_steps = self
            .remaining_steps
            .checked_sub(1)
            .ok_or(CoreError::StepCeiling)?;
        Ok(())
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new(DEFAULT_CONVERSION_STEPS)
    }
}

/// β and pair-projection weak-head normalization. Each reduction consumes
/// one step.
pub fn weak_head_normalize(
    arena: &mut TermArena,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    let mut current = term;
    loop {
        match arena.get(current) {
            Term::Apply { function, argument } => {
                let head = weak_head_normalize(arena, function, budget)?;
                match arena.get(head) {
                    Term::Lambda { body, .. } => {
                        budget.consume()?;
                        current = substitute(arena, body, argument);
                    }
                    _ => {
                        if head == function {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::Apply {
                            function: head,
                            argument,
                        }));
                    }
                }
            }
            Term::Fst { pair } => {
                let head = weak_head_normalize(arena, pair, budget)?;
                match arena.get(head) {
                    Term::Pair { first, .. } => {
                        budget.consume()?;
                        current = first;
                    }
                    _ => {
                        if head == pair {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::Fst { pair: head }));
                    }
                }
            }
            Term::Snd { pair } => {
                let head = weak_head_normalize(arena, pair, budget)?;
                match arena.get(head) {
                    Term::Pair { second, .. } => {
                        budget.consume()?;
                        current = second;
                    }
                    _ => {
                        if head == pair {
                            return Ok(current);
                        }
                        return Ok(arena.insert(Term::Snd { pair: head }));
                    }
                }
            }
            _ => return Ok(current),
        }
    }
}

/// Typed conversion: are `left` and `right` definitionally equal at
/// `shared_type` under `context`?
///
/// Precondition: the caller has already checked both sides at `shared_type`.
/// When the shared type's sort is `Strict(_)`, definitional proof irrelevance
/// makes every pair of inhabitants convertible and this returns `Ok(true)`
/// without inspecting the terms. That collapse is what makes the judgment
/// typed: it is decided by the sort of the shared type, never by the terms.
pub fn convertible(
    arena: &mut TermArena,
    context: &Context,
    left: TermHandle,
    right: TermHandle,
    shared_type: TermHandle,
    budget: &mut Budget,
) -> Result<bool, CoreError> {
    let shared_sort = infer_sort(arena, context, shared_type, budget)?;
    if shared_sort.is_strict() {
        return Ok(true);
    }

    let left = weak_head_normalize(arena, left, budget)?;
    let right = weak_head_normalize(arena, right, budget)?;

    match (arena.get(left), arena.get(right)) {
        (Term::Sort(left_sort), Term::Sort(right_sort)) => Ok(left_sort == right_sort),
        (
            Term::Pi {
                domain: left_domain,
                codomain: left_codomain,
            },
            Term::Pi {
                domain: right_domain,
                codomain: right_codomain,
            },
        ) => {
            let domain_sort = infer_sort(arena, context, left_domain, budget)?;
            let shared_domain = arena.insert(Term::Sort(domain_sort));
            if !convertible(
                arena,
                context,
                left_domain,
                right_domain,
                shared_domain,
                budget,
            )? {
                return Ok(false);
            }
            let extended = context.extend(left_domain);
            let codomain_sort = infer_sort(arena, &extended, left_codomain, budget)?;
            let shared_codomain = arena.insert(Term::Sort(codomain_sort));
            convertible(
                arena,
                &extended,
                left_codomain,
                right_codomain,
                shared_codomain,
                budget,
            )
        }
        (
            Term::Lambda {
                body: left_body, ..
            },
            Term::Lambda {
                body: right_body, ..
            },
        ) => {
            let type_head = weak_head_normalize(arena, shared_type, budget)?;
            match arena.get(type_head) {
                Term::Pi { domain, codomain } => {
                    let extended = context.extend(domain);
                    convertible(arena, &extended, left_body, right_body, codomain, budget)
                }
                _ => Ok(false),
            }
        }
        (
            Term::Sigma {
                domain: left_domain,
                codomain: left_codomain,
            },
            Term::Sigma {
                domain: right_domain,
                codomain: right_codomain,
            },
        ) => {
            let domain_sort = infer_sort(arena, context, left_domain, budget)?;
            let shared_domain = arena.insert(Term::Sort(domain_sort));
            if !convertible(
                arena,
                context,
                left_domain,
                right_domain,
                shared_domain,
                budget,
            )? {
                return Ok(false);
            }
            let extended = context.extend(left_domain);
            let codomain_sort = infer_sort(arena, &extended, left_codomain, budget)?;
            let shared_codomain = arena.insert(Term::Sort(codomain_sort));
            convertible(
                arena,
                &extended,
                left_codomain,
                right_codomain,
                shared_codomain,
                budget,
            )
        }
        (
            Term::Pair {
                first: left_first,
                second: left_second,
            },
            Term::Pair {
                first: right_first,
                second: right_second,
            },
        ) => {
            let type_head = weak_head_normalize(arena, shared_type, budget)?;
            match arena.get(type_head) {
                Term::Sigma { domain, codomain } => {
                    if !convertible(arena, context, left_first, right_first, domain, budget)? {
                        return Ok(false);
                    }
                    let second_type = substitute(arena, codomain, left_first);
                    convertible(
                        arena,
                        context,
                        left_second,
                        right_second,
                        second_type,
                        budget,
                    )
                }
                _ => Ok(false),
            }
        }
        (Term::Variable(left_index), Term::Variable(right_index)) => Ok(left_index == right_index),
        (
            Term::Apply {
                function: left_function,
                argument: left_argument,
            },
            Term::Apply {
                function: right_function,
                argument: right_argument,
            },
        ) => {
            // Neutral spines: the heads must convert and the arguments must
            // convert at the function's inferred Π domain.
            let function_type = infer_type(arena, context, left_function, budget)?;
            let type_head = weak_head_normalize(arena, function_type, budget)?;
            match arena.get(type_head) {
                Term::Pi { domain, .. } => {
                    if !convertible(
                        arena,
                        context,
                        left_argument,
                        right_argument,
                        domain,
                        budget,
                    )? {
                        return Ok(false);
                    }
                    convertible(
                        arena,
                        context,
                        left_function,
                        right_function,
                        function_type,
                        budget,
                    )
                }
                _ => Ok(false),
            }
        }
        (Term::Fst { pair: left_pair }, Term::Fst { pair: right_pair })
        | (Term::Snd { pair: left_pair }, Term::Snd { pair: right_pair }) => {
            // Neutral projections: the projected pairs must convert at
            // their inferred `Sigma`.
            let pair_type = infer_type(arena, context, left_pair, budget)?;
            let pair_head = weak_head_normalize(arena, pair_type, budget)?;
            match arena.get(pair_head) {
                Term::Sigma { .. } => {
                    convertible(arena, context, left_pair, right_pair, pair_head, budget)
                }
                _ => Ok(false),
            }
        }
        (Term::Pair { first, second }, _) => {
            // Pair eta: a literal pair converts to a non-pair only when the
            // non-pair's projections convert to its components.
            let type_head = weak_head_normalize(arena, shared_type, budget)?;
            match arena.get(type_head) {
                Term::Sigma { domain, codomain } => {
                    let right_first = arena.insert(Term::Fst { pair: right });
                    if !convertible(arena, context, first, right_first, domain, budget)? {
                        return Ok(false);
                    }
                    let right_second = arena.insert(Term::Snd { pair: right });
                    let second_type = substitute(arena, codomain, right_first);
                    convertible(arena, context, second, right_second, second_type, budget)
                }
                _ => Ok(false),
            }
        }
        (_, Term::Pair { first, second }) => {
            let type_head = weak_head_normalize(arena, shared_type, budget)?;
            match arena.get(type_head) {
                Term::Sigma { domain, codomain } => {
                    let left_first = arena.insert(Term::Fst { pair: left });
                    if !convertible(arena, context, left_first, first, domain, budget)? {
                        return Ok(false);
                    }
                    let left_second = arena.insert(Term::Snd { pair: left });
                    let second_type = substitute(arena, codomain, left_first);
                    convertible(arena, context, left_second, second, second_type, budget)
                }
                _ => Ok(false),
            }
        }
        _ => Ok(false),
    }
}

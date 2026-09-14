//! Typing and checking judgments for the Π/Σ fragment with stratified
//! relevant/strict universes — predicative formation, explicit closed
//! levels, no cumulativity, no self-typing universe — plus the inductive
//! profile's two-element type and its dependent eliminator.

use super::conversion::{Budget, convertible, weak_head_normalize};
use super::substitution::{shift, substitute};
use super::term::{Level, Sort, Term, TermArena, TermHandle};

/// Types of the bound variables, innermost last. Each stored type is
/// well-scoped for its own prefix; `lookup` shifts it into the full context.
#[derive(Clone, Debug, Default)]
pub struct Context {
    bindings: Vec<TermHandle>,
}

impl Context {
    pub fn empty() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// A new context with one more innermost binding of type `domain`.
    pub fn extend(&self, domain: TermHandle) -> Context {
        let mut bindings = self.bindings.clone();
        bindings.push(domain);
        Context { bindings }
    }

    /// The type of the variable at de Bruijn `index`, shifted by `index + 1`
    /// so it is well-scoped in the full context.
    pub fn lookup(&self, arena: &mut TermArena, index: u32) -> Option<TermHandle> {
        let position = self
            .bindings
            .len()
            .checked_sub(usize::try_from(index).ok()?.checked_add(1)?)?;
        let stored = self.bindings[position];
        Some(shift(arena, stored, 0, index + 1))
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

/// Every refusal of the mathematical core is a typed error; resource
/// exhaustion (`StepCeiling`) is an error, never a false judgment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreError {
    DummyTerm,
    UnboundVariable {
        index: u32,
        context_depth: usize,
    },
    LevelOverflow,
    NotASort {
        term: TermHandle,
        actual_type: TermHandle,
    },
    NotAFunction {
        function: TermHandle,
        actual_type: TermHandle,
    },
    NotAPair {
        pair: TermHandle,
        actual_type: TermHandle,
    },
    /// A `caseTwo` motive must be a family into a relevant universe
    /// `Π(_ : Two). Type w`. A strict codomain is not an admitted
    /// elimination target: strict motives belong to the reference core's
    /// boxing rules, not this eliminator.
    StrictCaseMotiveCodomain {
        codomain: TermHandle,
    },
    /// A `caseTwo` motive whose codomain is not a universe at all leaves
    /// `C t` without a type to check branches or results against.
    CaseMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    ArgumentTypeMismatch {
        expected: TermHandle,
        actual: TermHandle,
    },
    TypeMismatch {
        expected: TermHandle,
        actual: TermHandle,
    },
    StepCeiling,
}

/// The sort at which `term` is a type: infer its type, weak-head normalize
/// it, and require a `Term::Sort`.
pub fn infer_sort(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<Sort, CoreError> {
    let inferred = infer_type(arena, context, term, budget)?;
    let head = weak_head_normalize(arena, inferred, budget)?;
    match arena.get(head) {
        Term::Sort(sort) => Ok(sort),
        _ => Err(CoreError::NotASort {
            term,
            actual_type: head,
        }),
    }
}

/// The type of `term` under `context`, computed without search.
pub fn infer_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    match arena.get(term) {
        Term::Dummy => Err(CoreError::DummyTerm),
        Term::Variable(index) => context
            .lookup(arena, index)
            .ok_or(CoreError::UnboundVariable {
                index,
                context_depth: context.len(),
            }),
        Term::Sort(sort) => {
            // `Type u : Type (u+1)` and `Strict v : Type (v+1)`.
            let level = sort.level().successor().ok_or(CoreError::LevelOverflow)?;
            Ok(arena.insert(Term::Sort(Sort::Type(level))))
        }
        Term::Pi { domain, codomain } => {
            let domain_sort = infer_sort(arena, context, domain, budget)?;
            let extended = context.extend(domain);
            let codomain_sort = infer_sort(arena, &extended, codomain, budget)?;
            let level = domain_sort.level().max(codomain_sort.level());
            // The codomain's sort selects the layer; the level is the maximum.
            let result_sort = match codomain_sort {
                Sort::Strict(_) => Sort::Strict(level),
                Sort::Type(_) => Sort::Type(level),
            };
            Ok(arena.insert(Term::Sort(result_sort)))
        }
        Term::Lambda { domain, body } => {
            infer_sort(arena, context, domain, budget)?;
            let extended = context.extend(domain);
            let body_type = infer_type(arena, &extended, body, budget)?;
            Ok(arena.insert(Term::Pi {
                domain,
                codomain: body_type,
            }))
        }
        Term::Apply { function, argument } => {
            let function_type = infer_type(arena, context, function, budget)?;
            let function_head = weak_head_normalize(arena, function_type, budget)?;
            match arena.get(function_head) {
                Term::Pi { domain, codomain } => {
                    if let Term::Pair { .. } = arena.get(argument) {
                        // A dependent pair checks componentwise against a
                        // `Sigma` domain; its non-dependent inference could
                        // never convert there.
                        check_type(arena, context, argument, domain, budget)?;
                    } else {
                        let argument_type = infer_type(arena, context, argument, budget)?;
                        let domain_sort = infer_sort(arena, context, domain, budget)?;
                        let shared_type = arena.insert(Term::Sort(domain_sort));
                        if !convertible(arena, context, argument_type, domain, shared_type, budget)?
                        {
                            return Err(CoreError::ArgumentTypeMismatch {
                                expected: domain,
                                actual: argument_type,
                            });
                        }
                    }
                    Ok(substitute(arena, codomain, argument))
                }
                _ => Err(CoreError::NotAFunction {
                    function,
                    actual_type: function_head,
                }),
            }
        }
        Term::Sigma { domain, codomain } => {
            let domain_sort = infer_sort(arena, context, domain, budget)?;
            let extended = context.extend(domain);
            let codomain_sort = infer_sort(arena, &extended, codomain, budget)?;
            let level = domain_sort.level().max(codomain_sort.level());
            // A pair type is a strict proposition only when both components
            // are; any relevant component carries data and keeps the whole
            // type relevant.
            let result_sort = match (domain_sort, codomain_sort) {
                (Sort::Strict(_), Sort::Strict(_)) => Sort::Strict(level),
                _ => Sort::Type(level),
            };
            Ok(arena.insert(Term::Sort(result_sort)))
        }
        Term::Pair { first, second } => {
            // Inference is non-dependent: nothing records how the codomain
            // should mention the first component. `check_type` admits a
            // dependent pair componentwise against an expected `Sigma`.
            let first_type = infer_type(arena, context, first, budget)?;
            let second_type = infer_type(arena, context, second, budget)?;
            let codomain = shift(arena, second_type, 0, 1);
            Ok(arena.insert(Term::Sigma {
                domain: first_type,
                codomain,
            }))
        }
        Term::Fst { pair } => {
            let pair_type = infer_type(arena, context, pair, budget)?;
            let head = weak_head_normalize(arena, pair_type, budget)?;
            match arena.get(head) {
                Term::Sigma { domain, .. } => Ok(domain),
                _ => Err(CoreError::NotAPair {
                    pair,
                    actual_type: head,
                }),
            }
        }
        Term::Snd { pair } => {
            let pair_type = infer_type(arena, context, pair, budget)?;
            let head = weak_head_normalize(arena, pair_type, budget)?;
            match arena.get(head) {
                Term::Sigma { codomain, .. } => {
                    // `snd p : B[fst p]` — the dependent result keeps the
                    // projected first component.
                    let projected = arena.insert(Term::Fst { pair });
                    Ok(substitute(arena, codomain, projected))
                }
                _ => Err(CoreError::NotAPair {
                    pair,
                    actual_type: head,
                }),
            }
        }
        Term::Two => Ok(arena.insert(Term::Sort(Sort::Type(Level(0))))),
        Term::TwoZero | Term::TwoOne => Ok(arena.insert(Term::Two)),
        Term::CaseTwo {
            motive,
            zero_branch,
            one_branch,
            scrutinee,
        } => {
            // `caseTwo(C, d0, d1, t) : C t` for `C : Π(_ : Two). Type w`,
            // `d0 : C zero`, `d1 : C one` and `t : Two`. The motive's
            // codomain must normalize to a relevant universe: the
            // profile's eliminators target `Type`, and a `Strict`
            // codomain is a strict target owned by the reference core's
            // boxing rules, not this eliminator. The level `w` is read
            // off the checked codomain — elimination is not confined to
            // the scrutinee's level.
            let motive_type = infer_type(arena, context, motive, budget)?;
            let motive_head = weak_head_normalize(arena, motive_type, budget)?;
            let codomain = match arena.get(motive_head) {
                Term::Pi { domain, codomain } => {
                    let two = arena.insert(Term::Two);
                    let domain_sort = infer_sort(arena, context, domain, budget)?;
                    let shared_domain = arena.insert(Term::Sort(domain_sort));
                    if !convertible(arena, context, domain, two, shared_domain, budget)? {
                        return Err(CoreError::TypeMismatch {
                            expected: two,
                            actual: domain,
                        });
                    }
                    codomain
                }
                _ => {
                    return Err(CoreError::NotAFunction {
                        function: motive,
                        actual_type: motive_head,
                    });
                }
            };
            let codomain_head = weak_head_normalize(arena, codomain, budget)?;
            match arena.get(codomain_head) {
                Term::Sort(Sort::Type(_)) => {}
                Term::Sort(Sort::Strict(_)) => {
                    return Err(CoreError::StrictCaseMotiveCodomain { codomain });
                }
                _ => {
                    return Err(CoreError::CaseMotiveCodomainNotAUniverse { codomain });
                }
            }
            let two = arena.insert(Term::Two);
            check_type(arena, context, scrutinee, two, budget)?;
            let zero = arena.insert(Term::TwoZero);
            let zero_type = arena.insert(Term::Apply {
                function: motive,
                argument: zero,
            });
            check_type(arena, context, zero_branch, zero_type, budget)?;
            let one = arena.insert(Term::TwoOne);
            let one_type = arena.insert(Term::Apply {
                function: motive,
                argument: one,
            });
            check_type(arena, context, one_branch, one_type, budget)?;
            Ok(arena.insert(Term::Apply {
                function: motive,
                argument: scrutinee,
            }))
        }
    }
}

/// Check `term` against `expected`. Both types live at the sort of
/// `expected`, which is always relevant (`Type`), so strict collapse can
/// never fire for the types themselves.
pub fn check_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    expected: TermHandle,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    let expected_sort = infer_sort(arena, context, expected, budget)?;
    // A pair against a `Sigma` checks componentwise, so the second
    // component sees the dependent codomain instantiated by the first.
    if let Term::Pair { first, second } = arena.get(term) {
        let head = weak_head_normalize(arena, expected, budget)?;
        if let Term::Sigma { domain, codomain } = arena.get(head) {
            check_type(arena, context, first, domain, budget)?;
            let second_type = substitute(arena, codomain, first);
            return check_type(arena, context, second, second_type, budget);
        }
    }
    let shared_type = arena.insert(Term::Sort(expected_sort));
    let actual = infer_type(arena, context, term, budget)?;
    if convertible(arena, context, actual, expected, shared_type, budget)? {
        Ok(())
    } else {
        Err(CoreError::TypeMismatch { expected, actual })
    }
}

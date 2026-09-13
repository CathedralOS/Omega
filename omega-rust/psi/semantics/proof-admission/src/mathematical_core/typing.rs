//! Typing and checking judgments for the Π fragment with stratified
//! relevant/strict universes: predicative formation, explicit closed levels,
//! no cumulativity, no self-typing universe.

use super::conversion::{Budget, convertible, weak_head_normalize};
use super::substitution::{shift, substitute};
use super::term::{Sort, Term, TermArena, TermHandle};

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
                    let argument_type = infer_type(arena, context, argument, budget)?;
                    let domain_sort = infer_sort(arena, context, domain, budget)?;
                    let shared_type = arena.insert(Term::Sort(domain_sort));
                    if !convertible(arena, context, argument_type, domain, shared_type, budget)? {
                        return Err(CoreError::ArgumentTypeMismatch {
                            expected: domain,
                            actual: argument_type,
                        });
                    }
                    Ok(substitute(arena, codomain, argument))
                }
                _ => Err(CoreError::NotAFunction {
                    function,
                    actual_type: function_head,
                }),
            }
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
    let shared_type = arena.insert(Term::Sort(expected_sort));
    let actual = infer_type(arena, context, term, budget)?;
    if convertible(arena, context, actual, expected, shared_type, budget)? {
        Ok(())
    } else {
        Err(CoreError::TypeMismatch { expected, actual })
    }
}

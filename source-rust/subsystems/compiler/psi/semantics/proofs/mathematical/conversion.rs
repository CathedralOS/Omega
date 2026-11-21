use super::signature::Signature;
use super::substitution::{instantiate_levels, shift, substitute};
use super::term::{Term, TermArena, TermHandle};
use super::typing::{Context, CoreError};

macro_rules! row {
    (Apply => $function:expr => $argument:expr) => {
        Term::Apply {
            function: $function,
            argument: $argument,
        }
    };
}

pub const DEFAULT_CONVERSION_STEPS: u32 = 65_536;

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

pub fn weak_head_normalize(
    arena: &mut TermArena,
    signature: &Signature,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    weak_head_normalize_retaining(arena, signature, term, budget, &|_| false)
}

pub fn weak_head_normalize_retaining(
    arena: &mut TermArena,
    signature: &Signature,
    term: TermHandle,
    budget: &mut Budget,
    retained: &(impl Fn(u32) -> bool + ?Sized),
) -> Result<TermHandle, CoreError> {
    let mut current = term;
    let mut frames = Vec::new();
    'normalize: loop {
        loop {
            match arena.get(current) {
                Term::Constant {
                    declaration,
                    levels,
                } => {
                    let index = declaration;
                    let Some(declaration) = signature.get(index) else {
                        return Err(CoreError::UnknownDeclaration {
                            declaration: index,
                            signature_len: signature.len(),
                        });
                    };
                    if retained(index) {
                        break;
                    }
                    let Some(body) = declaration.body else {
                        break;
                    };
                    budget.consume()?;
                    current = instantiate_levels(arena, body, &levels).map_err(|index| {
                        CoreError::UnboundLevelParameter {
                            index,
                            arity: declaration.level_arity,
                        }
                    })?;
                }
                _ => match weak_head_focus(arena.node(current)) {
                    Some((child, slot)) => {
                        frames.push((current, slot));
                        current = child;
                    }
                    None => break,
                },
            }
        }
        while let Some((parent, slot)) = frames.pop() {
            match (arena.get(parent), arena.get(current)) {
                (Term::Apply { argument, .. }, Term::Lambda { body, .. }) => {
                    budget.consume()?;
                    current = substitute(arena, body, argument);
                    continue 'normalize;
                }
                (Term::Fst { .. }, Term::Pair { first, .. }) => {
                    budget.consume()?;
                    current = first;
                    continue 'normalize;
                }
                (Term::Snd { .. }, Term::Pair { second, .. }) => {
                    budget.consume()?;
                    current = second;
                    continue 'normalize;
                }
                (Term::CaseTwo { zero_branch, .. }, Term::TwoZero) => {
                    budget.consume()?;
                    current = zero_branch;
                    continue 'normalize;
                }
                (Term::CaseTwo { one_branch, .. }, Term::TwoOne) => {
                    budget.consume()?;
                    current = one_branch;
                    continue 'normalize;
                }
                (Term::IdElim { base, .. }, Term::Refl { .. }) => {
                    budget.consume()?;
                    current = base;
                    continue 'normalize;
                }
                (
                    Term::IndW { motive, step, .. },
                    Term::Sup {
                        children,
                        label,
                        function,
                        ..
                    },
                ) => {
                    budget.consume()?;
                    let bound_domain = arena.insert(row!(Apply => children => label));
                    let shifted_motive = shift(arena, motive, 0, 1);
                    let shifted_step = shift(arena, step, 0, 1);
                    let shifted_function = shift(arena, function, 0, 1);
                    let bound = arena.insert(Term::Variable(0));
                    let child = arena.insert(row!(Apply => shifted_function => bound));
                    let body = arena.insert(Term::IndW {
                        motive: shifted_motive,
                        step: shifted_step,
                        tree: child,
                    });
                    let hypothesis = arena.insert(Term::Lambda {
                        domain: bound_domain,
                        body,
                    });
                    let applied = arena.insert(row!(Apply => step => label));
                    let applied = arena.insert(row!(Apply => applied => function));
                    current = arena.insert(row!(Apply => applied => hypothesis));
                    continue 'normalize;
                }
                (Term::SquashElim { function, .. }, Term::SquashIntro { value, .. }) => {
                    budget.consume()?;
                    current = arena.insert(row!(Apply => function => value));
                    continue 'normalize;
                }
                (Term::BoxElim { body, .. }, Term::BoxIntro { value, .. }) => {
                    budget.consume()?;
                    current = arena.insert(row!(Apply => body => value));
                    continue 'normalize;
                }
                _ => {}
            }
            current = replace_head_focus(arena, parent, slot, current);
        }
        return Ok(current);
    }
}

fn weak_head_focus(node: &Term) -> Option<(TermHandle, usize)> {
    match node {
        Term::Apply { function, .. } => Some((*function, 0)),
        Term::Fst { pair } | Term::Snd { pair } => Some((*pair, 0)),
        Term::CaseTwo { scrutinee, .. } => Some((*scrutinee, 3)),
        Term::IdElim { proof, .. } => Some((*proof, 3)),
        Term::IndW { tree, .. } => Some((*tree, 2)),
        Term::EmptyElim { scrutinee, .. } => Some((*scrutinee, 1)),
        Term::SquashElim { scrutinee, .. } | Term::BoxElim { scrutinee, .. } => {
            Some((*scrutinee, 2))
        }
        _ => None,
    }
}

fn replace_head_focus(
    arena: &mut TermArena,
    parent: TermHandle,
    slot: usize,
    head: TermHandle,
) -> TermHandle {
    let node = arena.node(parent);
    let slots = node.subterms();
    let mut children = [parent; super::term::MAX_SUBTERMS];
    for (index, (child, _)) in slots.iter().enumerate() {
        children[index] = child;
    }
    if children[slot] == head {
        return parent;
    }
    children[slot] = head;
    let node = node.with_children(&children[..slots.len()]);
    arena.insert(node)
}

/// Checks both operands at `shared_type`, then decides typed conversion.
/// The context has the same well-formed signature and binding premises as
/// `infer_type` and `check_type`. Formation and comparison share `budget`.
pub fn convertible(
    arena: &mut TermArena,
    context: &Context,
    left: TermHandle,
    right: TermHandle,
    shared_type: TermHandle,
    budget: &mut Budget,
) -> Result<bool, CoreError> {
    convertible_retaining(arena, context, left, right, shared_type, budget, &|_| false)
}

/// Checks the operands before comparing with selected definitions opaque.
/// Retention restricts comparison; operand formation uses ordinary typing.
pub fn convertible_retaining(
    arena: &mut TermArena,
    context: &Context,
    left: TermHandle,
    right: TermHandle,
    shared_type: TermHandle,
    budget: &mut Budget,
    retained: &impl Fn(u32) -> bool,
) -> Result<bool, CoreError> {
    super::typing::convert_retaining(arena, context, left, right, shared_type, budget, retained)
}

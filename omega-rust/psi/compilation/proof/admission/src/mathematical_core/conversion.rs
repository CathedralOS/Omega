//! Typed conversion for the Π/Σ fragment, the `Two`/`Id`/`W` primitives
//! and the strict layer: β, pair-projection and constructor-scrutinee
//! `caseTwo`/`J`/`indW`/`unsq`/`unbox` weak-head normalization under a
//! step ceiling, pair eta at a `Sigma` shared type, typed function eta
//! at a `Pi` shared type, and definitional proof irrelevance gated on
//! the *shared type's* sort — never on the shape of either side.
//!
//! Function eta is the profile's selected extension (`inductive_profile.md`
//! §typed-function-eta): a lambda and a non-lambda convert only when the
//! shared type weak-head normalizes to `Pi`, and only through the typed
//! rule `x ↦ f x ≡ f`. The judgment never deletes a wrapper shape at a
//! non-function shared type and grants no pointwise-equality collapse:
//! `x ↦ g x` and `f` convert only when `g` and `f` already do.

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

/// β, pair-projection, constructor-scrutinee and constant-unfolding
/// weak-head normalization. Each reduction consumes one step.
///
/// Reduction is untyped — it takes the declaration `signature` it unfolds
/// constants against, never a context. A definition constant unfolds to
/// its instantiated body as a budgeted step (δ); an assumption constant
/// is a neutral atom and stays stuck. Unfolding terminates because
/// signatures are prefix-checked: every constant references a strictly
/// earlier declaration, so each step lowers the greatest reachable index.
pub fn weak_head_normalize(
    arena: &mut TermArena,
    signature: &Signature,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    weak_head_normalize_retaining(arena, signature, term, budget, &|_| false)
}

/// Use the same reduction rules while retaining selected declaration heads.
/// This projects a checked source view before transparent carrier aliases
/// disappear. Retention changes no kernel conversion or proof judgment; the
/// ordinary entrance above unfolds every definition.
pub fn weak_head_normalize_retaining(
    arena: &mut TermArena,
    signature: &Signature,
    term: TermHandle,
    budget: &mut Budget,
    retained: &(impl Fn(u32) -> bool + ?Sized),
) -> Result<TermHandle, CoreError> {
    let mut current = term;
    // Frames own only arena handles and the focused child slot. Reductions
    // resume the suspended context; a stuck head rebuilds precisely that slot.
    // No result cache can skip an owed reduction or its budget charge.
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
                    // Typing equates the endpoint and reflexivity annotation.
                    // A neutral proof stays stuck; there is no identity eta.
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
                    // Preserve the constructor's checked B annotation and the
                    // exact dependent induction hypothesis. A neutral child
                    // function never blocks the constructor reduction.
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
                // Empty has no constructors. All other unmatched elimination
                // heads stay neutral; normalization supplies no eta law.
                _ => {}
            }
            current = replace_head_focus(arena, parent, slot, current);
        }
        return Ok(current);
    }
}

/// The reduction context of each eliminator, in the canonical term layout.
/// Motives, annotations, branches and arguments are never eagerly normalized.
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

/// Typed conversion at one already-checked shared type. Its sort gates strict
/// irrelevance; malformed shared types refuse before either value is compared.
/// The caller has already checked both sides at this same complete application.
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

/// Selected transparent heads remain explicit only for this source comparison.
/// Their retention changes no binder, eta, strictness or universe rule and
/// supplies no extra value equality. Child typing judgments use ordinary heads.
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

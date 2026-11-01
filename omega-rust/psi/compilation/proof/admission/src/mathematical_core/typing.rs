//! Typing and checking judgments for the Π/Σ fragment with stratified
//! relevant/strict universes — predicative formation, explicit closed
//! levels, no cumulativity, no self-typing universe — plus the inductive
//! profile's two-element type, relevant identity type, and W-type with
//! their dependent eliminators.

use super::conversion::{Budget, weak_head_normalize};
use super::signature::Signature;
use super::substitution::{instantiate_levels, shift};
use super::term::{Level, Sort, Term, TermArena, TermHandle};

macro_rules! row {
    (Apply => $function:expr => $argument:expr) => {
        Term::Apply {
            function: $function,
            argument: $argument,
        }
    };
    (Pi => $domain:expr => $codomain:expr) => {
        Term::Pi {
            domain: $domain,
            codomain: $codomain,
        }
    };
    (TypeMismatch => $expected:expr => $actual:expr) => {
        CoreError::TypeMismatch {
            expected: $expected,
            actual: $actual,
        }
    };
}

/// The full ambient judgment scope `Σ; Δ; Γ`: the signature `Σ` of
/// declarations every `Constant` resolves through, the level arity `Δ` —
/// the count of universe parameters every `Level::Parameter` must stay
/// under — and the types of the bound variables `Γ`, innermost last.
/// Each stored type is well-scoped for its own prefix; `lookup` shifts it
/// into the full context. Extending a binder never touches the signature
/// or the level arity: constants and level parameters are judgment scope,
/// not de Bruijn-bound.
#[derive(Clone, Debug, Default)]
pub struct Context {
    bindings: Vec<TermHandle>,
    level_arity: u32,
    signature: Signature,
}

impl Context {
    /// The closed context: no term bindings, no universe parameters and
    /// no declarations, so every level must be a closed constant and no
    /// `Constant` resolves.
    pub fn empty() -> Self {
        Self {
            bindings: Vec::new(),
            level_arity: 0,
            signature: Signature::new(),
        }
    }

    /// The context of a judgment parametric over `level_arity` universe
    /// parameters: `Level::Parameter(i)` is in scope exactly when
    /// `i < level_arity`. This is the substrate of a universe-polymorphic
    /// declaration — the judgment holds for every level instantiation.
    /// The signature starts empty; `with_signature` installs one.
    pub fn with_level_arity(level_arity: u32) -> Self {
        Self {
            bindings: Vec::new(),
            level_arity,
            signature: Signature::new(),
        }
    }

    /// The number of universe parameters in scope for this judgment.
    pub fn level_arity(&self) -> u32 {
        self.level_arity
    }

    /// The declaration signature this judgment resolves `Constant`
    /// references against — the ambient `Σ`, unchanged by binders.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// The same judgment scope under `signature`. The signature is
    /// ambient, not a binder, so the local bindings and level arity carry
    /// over unchanged.
    pub fn with_signature(&self, signature: Signature) -> Context {
        Context {
            bindings: self.bindings.clone(),
            level_arity: self.level_arity,
            signature,
        }
    }

    /// A new context with one more innermost binding of type `domain`.
    /// Term binders never touch the level arity: level parameters are not
    /// de Bruijn-bound and do not shift under binders.
    pub fn extend(&self, domain: TermHandle) -> Context {
        let mut bindings = self.bindings.clone();
        bindings.push(domain);
        Context {
            bindings,
            level_arity: self.level_arity,
            signature: self.signature.clone(),
        }
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
    /// The largest representable constant level cannot be succeeded — a
    /// malformed universe, not a judgment failure.
    LevelOverflow,
    /// `Level::Parameter(index)` must name one of the judgment's
    /// `level_arity` universe parameters. An out-of-scope parameter is a
    /// malformed universe: the certificate claims a judgment under a level
    /// scope the level never references.
    UnboundLevelParameter {
        index: u32,
        arity: u32,
    },
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
    /// `Id A x y` requires `A : Type u`: the profile's identity type is
    /// relevant data over a relevant carrier, and a strict domain is a
    /// proposition where proof-relevant distinction has no meaning.
    StrictIdentityDomain {
        domain: TermHandle,
    },
    /// The scrutinee of an identity elimination must inhabit an identity
    /// type `Id A x y`; anything else leaves `J` without endpoints or a
    /// fixed side to take `refl` on.
    NotAnIdentity {
        proof: TermHandle,
        actual_type: TermHandle,
    },
    /// A `J` motive must take both the endpoint and the identity proof:
    /// after the endpoint domain, its codomain must still be a `Π` whose
    /// domain is `Id A x y`.
    IdentityMotiveCodomainNotAFunction {
        codomain: TermHandle,
    },
    /// A `J` motive must be a family into a relevant universe
    /// `Π(y : A). Π(_ : Id A x y). Type w`. A strict codomain is not an
    /// admitted elimination target: strict motives belong to the
    /// reference core's boxing rules, not this eliminator.
    StrictIdentityMotiveCodomain {
        codomain: TermHandle,
    },
    /// A `J` motive whose codomain is not a universe at all leaves
    /// `C y p` without a type to check the base or the result against.
    IdentityMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// `W A B` requires `A : Type u`: over a strict proposition a
    /// well-founded tree has no relevant data to carry.
    StrictWCarrier {
        carrier: TermHandle,
    },
    /// A `W` branching family must land in a relevant universe
    /// `Π(_ : A). Type v`. A strict codomain makes child positions
    /// propositions, which the reference core's boxing rules own.
    StrictWChildrenCodomain {
        codomain: TermHandle,
    },
    /// A `W` branching family whose codomain is not a universe leaves
    /// `B a` without a type for child positions.
    WChildrenCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// An `indW` tree must inhabit a `W` type; anything else leaves
    /// the induction without a carrier or branching family.
    NotAW {
        tree: TermHandle,
        actual_type: TermHandle,
    },
    /// An `indW` motive must be a family into a relevant universe
    /// `Π(_ : W A B). Type w`. A strict codomain is not an admitted
    /// elimination target: strict motives belong to the reference
    /// core's boxing rules, not this eliminator.
    StrictInductionMotiveCodomain {
        codomain: TermHandle,
    },
    /// An `indW` motive whose codomain is not a universe at all leaves
    /// `P t` without a type to check the step or the result against.
    InductionMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// `Squash A` and `sq` require `A : Type u`: squashing is the map
    /// from relevant types into strict propositions. A strict domain is
    /// already a proposition — there is nothing left to forget.
    StrictSquashDomain {
        domain: TermHandle,
    },
    /// An `unsq` scrutinee must inhabit a squash type `Squash A`;
    /// anything else leaves the elimination without a carrier.
    NotASquash {
        scrutinee: TermHandle,
        actual_type: TermHandle,
    },
    /// `unsq P f x` requires `P : Strict v`: squashed existence may
    /// flow into a strict proposition, never back into relevant data.
    SquashTargetNotStrict {
        proposition: TermHandle,
    },
    /// `Box A` and `box` require `A : Strict v`: boxing wraps a strict
    /// proposition as a relevant type. A relevant domain was never
    /// squashed, and boxing it would only re-wrap live data.
    NonStrictBoxDomain {
        domain: TermHandle,
    },
    /// An `unbox` scrutinee must inhabit a boxed type `Box A`;
    /// anything else leaves the elimination without a proposition to
    /// open.
    NotABox {
        scrutinee: TermHandle,
        actual_type: TermHandle,
    },
    /// An `unbox` motive whose codomain is not a universe leaves
    /// `P x` without a type to check the body or the result against.
    /// The reference core admits motive codomains at either sort —
    /// `Ty_j` or `P_j` — so only a non-sort rejects.
    BoxMotiveCodomainNotAUniverse {
        codomain: TermHandle,
    },
    /// A `Constant` names a declaration position the ambient signature
    /// does not have — a forward reference, a self-reference (the
    /// signature only holds the checked prefix), or a reference into an
    /// empty signature. Malformed, never a valid judgment.
    UnknownDeclaration {
        declaration: u32,
        signature_len: usize,
    },
    /// A `Constant` instantiation must supply exactly the declaration's
    /// own level arity — the judgment claims the declaration at precisely
    /// those universe parameters.
    DeclarationArityMismatch {
        declaration: u32,
        expected: u32,
        supplied: usize,
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
    inferred_sort(arena, context, term, inferred, budget)
}

fn inferred_sort(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    inferred: TermHandle,
    budget: &mut Budget,
) -> Result<Sort, CoreError> {
    let head = weak_head_normalize(arena, context.signature(), inferred, budget)?;
    match arena.get(head) {
        Term::Sort(sort) => Ok(sort),
        _ => Err(CoreError::NotASort {
            term,
            actual_type: head,
        }),
    }
}

/// The type of `term` under `context`, computed without search.
///
/// Ordinary constructor trees share one handle continuation owner, including
/// binder domains, application arguments, pair components and projections.
/// Exact binder prefixes return with their completed child judgments. Inductive
/// and strict-layer typing premises use the same continuation owner. Typed
/// conversion still owns its separate recursive comparison rules, so the
/// verification worker remains until that route is converted too.
pub fn infer_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    budget: &mut Budget,
) -> Result<TermHandle, CoreError> {
    let node = arena.get(term);
    match node {
        Term::Pi { .. }
        | Term::Sigma { .. }
        | Term::Squash { .. }
        | Term::Box { .. }
        | Term::Lambda { .. }
        | Term::Apply { .. }
        | Term::Pair { .. }
        | Term::Fst { .. }
        | Term::Snd { .. }
        | Term::Id { .. }
        | Term::Refl { .. }
        | Term::SquashIntro { .. }
        | Term::BoxIntro { .. }
        | Term::EmptyElim { .. }
        | Term::W { .. }
        | Term::Sup { .. }
        | Term::CaseTwo { .. }
        | Term::IndW { .. }
        | Term::BoxElim { .. }
        | Term::SquashElim { .. }
        | Term::IdElim { .. } => judgments::infer(arena, context, term, budget),
        node => infer_leaf_type(arena, context, node),
    }
}

fn infer_leaf_type(
    arena: &mut TermArena,
    context: &Context,
    node: Term,
) -> Result<TermHandle, CoreError> {
    match node {
        Term::Dummy => Err(CoreError::DummyTerm),
        Term::Variable(index) => context
            .lookup(arena, index)
            .ok_or(CoreError::UnboundVariable {
                index,
                context_depth: context.len(),
            }),
        Term::Sort(sort) => {
            // `Type u : Type (u+1)` and `Strict v : Type (v+1)`. The sort's
            // level must be well-formed under the judgment's level arity —
            // every `Sort` node reachable from a checked judgment passes
            // through this rule, so an out-of-scope parameter anywhere in
            // the term, the claimed type or a context binding rejects.
            check_level(context.level_arity(), &sort.level())?;
            let level = sort.level().successor().ok_or(CoreError::LevelOverflow)?;
            Ok(arena.insert(Term::Sort(Sort::Type(level))))
        }
        Term::Pi { .. }
        | Term::Sigma { .. }
        | Term::Squash { .. }
        | Term::Box { .. }
        | Term::Lambda { .. }
        | Term::Apply { .. }
        | Term::Pair { .. }
        | Term::Fst { .. }
        | Term::Snd { .. }
        | Term::Id { .. }
        | Term::Refl { .. }
        | Term::SquashIntro { .. }
        | Term::BoxIntro { .. }
        | Term::EmptyElim { .. }
        | Term::W { .. }
        | Term::Sup { .. }
        | Term::CaseTwo { .. }
        | Term::IndW { .. }
        | Term::BoxElim { .. }
        | Term::SquashElim { .. }
        | Term::IdElim { .. } => unreachable!("constructor inference owns these nodes"),
        Term::Two => Ok(arena.insert(Term::Sort(Sort::Type(Level::Constant(0))))),
        Term::TwoZero | Term::TwoOne => Ok(arena.insert(Term::Two)),
        Term::Empty => {
            // `sEmpty : Strict 0` — the strict empty proposition sits at
            // level 0 like `Two` sits at `Type 0`: it carries no data,
            // and its eliminator reaches into every universe.
            Ok(arena.insert(Term::Sort(Sort::Strict(Level::Constant(0)))))
        }
        Term::Constant {
            declaration: index,
            levels,
        } => {
            // `d(ls) : instantiate(decl[d].ty, ls)` — the declaration's
            // statement instantiated at the supplied level arguments. The
            // reference is checked, never trusted: the declaration must
            // exist in the ambient signature (which for a declaration
            // under check holds only its prefix, so self- and forward
            // references never resolve), the instantiation must supply
            // exactly its level arity, and every level argument must be
            // in scope under this judgment's own arity.
            let signature = context.signature();
            let Some(declaration) = signature.get(index) else {
                return Err(CoreError::UnknownDeclaration {
                    declaration: index,
                    signature_len: signature.len(),
                });
            };
            if levels.len() != declaration.level_arity as usize {
                return Err(CoreError::DeclarationArityMismatch {
                    declaration: index,
                    expected: declaration.level_arity,
                    supplied: levels.len(),
                });
            }
            for level in &levels {
                check_level(context.level_arity(), level)?;
            }
            instantiate_levels(arena, declaration.ty, &levels).map_err(|index| {
                CoreError::UnboundLevelParameter {
                    index,
                    arity: declaration.level_arity,
                }
            })
        }
    }
}

pub(super) fn convert_retaining(
    arena: &mut TermArena,
    context: &Context,
    left: TermHandle,
    right: TermHandle,
    shared_type: TermHandle,
    budget: &mut Budget,
    retained: &impl Fn(u32) -> bool,
) -> Result<bool, CoreError> {
    judgments::convert(arena, context, left, right, shared_type, budget, retained)
}

/// The closed-level scope rule: a level is well-formed under `arity` when
/// every `Parameter(i)` names one of the judgment's `arity` universe
/// parameters. Constant, successor and maximum levels introduce no scope
/// of their own, so the check is a structural walk; out-of-scope
/// parameters are malformed universes, never valid judgments.
fn check_level(arity: u32, level: &Level) -> Result<(), CoreError> {
    let mut pending: Vec<&Level> = Vec::new();
    let mut current = level;
    loop {
        match current {
            Level::Constant(_) => {}
            Level::Parameter(index) if *index < arity => {}
            Level::Parameter(index) => {
                return Err(CoreError::UnboundLevelParameter {
                    index: *index,
                    arity,
                });
            }
            Level::Successor(inner) => {
                current = inner;
                continue;
            }
            Level::Maximum(left, right) => {
                pending.push(right);
                current = left;
                continue;
            }
        }
        match pending.pop() {
            Some(next) => current = next,
            None => return Ok(()),
        }
    }
}

/// The checked type of an `indW` induction step:
/// `Π(a : A). Π(k : Π(b : B a). W A B). Π(_ : Π(b : B a). motive (k b)).
/// motive (sup A B a k)` — an induction hypothesis for every child.
///
/// `carrier`, `children` and `motive` are terms in the ambient context;
/// the builder shifts each under the binders it introduces. Depth 1
/// lives under `a`; depth 2 under `a, k`, which is also where `k`'s own
/// `b` binder leaves the `W A B` codomain; depth 3 under `a, k, ih` for
/// the result, and under `a, k` plus the hypothesis's `b` for its
/// codomain `motive (k b)`.
pub(super) fn w_step_type(
    arena: &mut TermArena,
    carrier: TermHandle,
    children: TermHandle,
    motive: TermHandle,
) -> TermHandle {
    let children_at_one = shift(arena, children, 0, 1);
    let bound_a = arena.insert(Term::Variable(0));
    let children_domain = arena.insert(row!(Apply => children_at_one => bound_a));
    let carrier_at_two = shift(arena, carrier, 0, 2);
    let children_at_two = shift(arena, children, 0, 2);
    let w_at_two = arena.insert(Term::W {
        carrier: carrier_at_two,
        children: children_at_two,
    });
    let function_type = arena.insert(row!(Pi => children_domain => w_at_two));
    let bound_a = arena.insert(Term::Variable(1));
    let hypothesis_domain = arena.insert(row!(Apply => children_at_two => bound_a));
    let motive_at_three = shift(arena, motive, 0, 3);
    let bound_k = arena.insert(Term::Variable(1));
    let bound_b = arena.insert(Term::Variable(0));
    let child = arena.insert(row!(Apply => bound_k => bound_b));
    let hypothesis_codomain = arena.insert(row!(Apply => motive_at_three => child));
    let hypothesis = arena.insert(row!(Pi => hypothesis_domain => hypothesis_codomain));
    let carrier_at_three = shift(arena, carrier, 0, 3);
    let children_at_three = shift(arena, children, 0, 3);
    let bound_a = arena.insert(Term::Variable(2));
    let bound_k = arena.insert(Term::Variable(1));
    let sup = arena.insert(Term::Sup {
        carrier: carrier_at_three,
        children: children_at_three,
        label: bound_a,
        function: bound_k,
    });
    let result = arena.insert(row!(Apply => motive_at_three => sup));
    let inner = arena.insert(row!(Pi => hypothesis => result));
    let middle = arena.insert(row!(Pi => function_type => inner));
    arena.insert(row!(Pi => carrier => middle))
}

/// The checked type of an `unbox` body:
/// `Π(a : A). motive (box_A a)` — the motive instantiated at the boxed
/// canonical inhabitant of the bound payload.
///
/// `payload` and `motive` are terms in the ambient context; the builder
/// shifts each under the `a` binder it introduces.
pub(super) fn box_body_type(
    arena: &mut TermArena,
    payload: TermHandle,
    motive: TermHandle,
) -> TermHandle {
    let shifted_motive = shift(arena, motive, 0, 1);
    let shifted_payload = shift(arena, payload, 0, 1);
    let bound = arena.insert(Term::Variable(0));
    let boxed = arena.insert(Term::BoxIntro {
        ty: shifted_payload,
        value: bound,
    });
    let body_codomain = arena.insert(row!(Apply => shifted_motive => boxed));
    arena.insert(row!(Pi => payload => body_codomain))
}

mod judgments;

/// Check `term` against `expected`. The two types are compared at
/// `Sort(s)` where `s` is `expected`'s own sort — a universe, whose own
/// sort is always `Type`, so strict collapse can never fire for the
/// *types* themselves even when `expected` is a strict proposition such
/// as `sEmpty` or a squash's payload.
///
/// Componentwise descents use one mutable context and handle continuations.
/// Both pair branches and lambda bodies stay off the Rust call stack. A pair
/// continuation restores its exact ambient prefix before substituting the
/// successfully checked first component into the second component's type.
pub fn check_type(
    arena: &mut TermArena,
    context: &Context,
    term: TermHandle,
    expected: TermHandle,
    budget: &mut Budget,
) -> Result<(), CoreError> {
    judgments::check(arena, context, term, expected, budget)
}

/// The two argument-count error spellings consumers reject together.
#[macro_export]
macro_rules! argument_error_pair_pattern {
    () => {
        $crate::CoreError::TypeMismatch { .. } | $crate::CoreError::ArgumentTypeMismatch { .. }
    };
}

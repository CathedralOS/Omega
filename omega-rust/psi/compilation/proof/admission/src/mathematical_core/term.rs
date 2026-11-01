//! Term model for the common mathematical core: universe level expressions
//! over the judgment's level parameters, relevant and strict sorts,
//! arena-held de Bruijn terms, and the selected inductive profile's
//! two-element type, relevant identity type, and W-type of well-founded
//! trees.

use std::collections::HashSet;

use arena::Arena;

/// A universe level expression.
///
/// `Constant` is a closed natural; `Parameter(i)` names the `i`-th universe
/// parameter of the enclosing judgment — the level arity is fixed for the
/// whole judgment, so parameters are positional indices, never bound or
/// shifted by term binders. `Successor` and `Maximum` are the two
/// constructors the formation rules need: `Type u : Type (u+1)` and
/// Π/Σ/`W` formation at `max(u, v)`. There is no `imax`: the selected core
/// is predicative without cumulativity, so no rule needs a level that
/// depends on which side is strict.
///
/// `Level` is syntax, not a normal form: `Maximum(v, u)` and `Maximum(u, v)`
/// are different levels that convert. Semantic equality is decided by
/// `levels_equal` through the `max`-normal form, never by `==`.
#[derive(compiler_storage::Record)]
pub enum Level {
    /// A closed natural level `k`.
    Constant(u32),
    /// Universe parameter `i` of the enclosing judgment's level arity.
    Parameter(u32),
    /// `u + 1`.
    Successor(Box<Level>),
    /// `max(u, v)`.
    Maximum(Box<Level>, Box<Level>),
}

mod levels;
pub(crate) use levels::{instantiate_level, levels_equal};

/// Semantic sort equality: the layers agree (`Type` never converts to
/// `Strict` — there is no cumulativity and no layer collapse) and the
/// levels convert.
pub(crate) fn sorts_equal(left: &Sort, right: &Sort) -> bool {
    match (left, right) {
        (Sort::Type(left), Sort::Type(right)) | (Sort::Strict(left), Sort::Strict(right)) => {
            levels_equal(left, right)
        }
        _ => false,
    }
}

/// A mathematical universe sort: `Type u` is relevant, `Strict v` is the
/// definitionally irrelevant logical layer. Derived `PartialEq` is
/// syntactic on the level syntax; conversion uses `sorts_equal`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, compiler_storage::Record)]
pub enum Sort {
    Type(Level),
    Strict(Level),
}

impl Sort {
    pub fn level(&self) -> Level {
        match self {
            Sort::Type(level) | Sort::Strict(level) => level.clone(),
        }
    }

    pub fn is_strict(&self) -> bool {
        matches!(self, Sort::Strict(_))
    }
}

/// Handle into a `TermArena`. The zero handle resolves to the arena dummy.
pub type TermHandle = arena::Handle<Term>;

/// One term node. Children and binders are handles into the same arena.
/// `Variable` indices are de Bruijn indices; 0 names the innermost binder.
/// `Sort` payloads clone their level expressions out of the arena — terms
/// are `Clone`, not `Copy`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, compiler_storage::Record)]
pub enum Term {
    /// ZII arena dummy; never well-typed. Typing rejects it with
    /// `CoreError::DummyTerm`.
    #[default]
    Dummy,
    Variable(u32),
    Sort(Sort),
    /// The codomain lives under the binder.
    Pi {
        domain: TermHandle,
        codomain: TermHandle,
    },
    Lambda {
        domain: TermHandle,
        body: TermHandle,
    },
    Apply {
        function: TermHandle,
        argument: TermHandle,
    },
    /// A dependent pair type. The codomain lives under the binder.
    Sigma {
        domain: TermHandle,
        codomain: TermHandle,
    },
    Pair {
        first: TermHandle,
        second: TermHandle,
    },
    Fst {
        pair: TermHandle,
    },
    Snd {
        pair: TermHandle,
    },
    /// The profile's two-element type `Two : Type 0` with constructors
    /// `zero` and `one`. `Two` is a type constant, not a sort, so it
    /// inhabits `Type 0` rather than a universe above itself.
    Two,
    TwoZero,
    TwoOne,
    /// Dependent `Two` elimination `caseTwo(motive, zero_branch,
    /// one_branch, scrutinee)`. No child lives under a binder; the motive
    /// is an ordinary `Π(_ : Two). Type w` term.
    CaseTwo {
        motive: TermHandle,
        zero_branch: TermHandle,
        one_branch: TermHandle,
        scrutinee: TermHandle,
    },
    /// The profile's relevant identity type `Id A x y : Type u` for
    /// `A : Type u` and `x, y : A` — proof-relevant and intensional, so
    /// no K/UIP or identity-proof irrelevance applies.
    Id {
        ty: TermHandle,
        left: TermHandle,
        right: TermHandle,
    },
    /// `refl A x : Id A x x`. The type is an annotation like `Lambda`'s
    /// domain: checking the value against it directly is what lets a
    /// dependent-pair endpoint type-check without searching.
    Refl {
        ty: TermHandle,
        value: TermHandle,
    },
    /// Dependent identity elimination `J(motive, base, endpoint, proof)`:
    /// for `proof : Id A x endpoint` and `motive : Π(y : A). Π(_ : Id A
    /// x y). Type w` with `base : motive x (refl A x)`, the elimination
    /// has type `motive endpoint proof` and computes to `base` on
    /// `refl`. No child lives under a binder.
    IdElim {
        motive: TermHandle,
        base: TermHandle,
        endpoint: TermHandle,
        proof: TermHandle,
    },
    /// The profile's W-type `W A B : Type max(u, v)` for `A : Type u`
    /// and `B : A → Type v` — well-founded trees whose nodes carry an
    /// `A` label and one child per `B a` position. `children` is the
    /// branching family `B`, not a child term. Both sides must be
    /// relevant: strict labels or strict child positions belong to the
    /// reference core's boxing rules, not this type former.
    W {
        carrier: TermHandle,
        children: TermHandle,
    },
    /// `sup A B a k : W A B` — the W constructor bundling a label
    /// `a : A` with a child function `k : Π(b : B a). W A B`. The
    /// `carrier`/`children` fields are annotations like `Refl`'s `ty`:
    /// checked against the formation rule, never trusted, and kept on
    /// the node so constructor computation can rebuild the induction
    /// hypothesis's domain `B a` even when the surrounding `W` type is
    /// neutral. No child lives under a binder.
    Sup {
        carrier: TermHandle,
        children: TermHandle,
        label: TermHandle,
        function: TermHandle,
    },
    /// Dependent W-induction `indW(motive, step, tree) : motive tree`
    /// for `tree : W A B`, `motive : Π(_ : W A B). Type w`, and
    /// `step : Π(a : A). Π(k : Π(b : B a). W A B). Π(_ : Π(b : B a).
    /// motive (k b)). motive (sup A B a k)` — an induction hypothesis
    /// for every child. Computes on `sup` to `step a k (λ(b : B a).
    /// indW(motive, step, k b))` as a budgeted step; a neutral tree
    /// stays stuck. No child lives under a binder.
    IndW {
        motive: TermHandle,
        step: TermHandle,
        tree: TermHandle,
    },
    /// The reference core's strict empty proposition `sEmpty : Strict 0`
    /// (foundation §3.3). Fixed at level 0 like `Two`: it carries no
    /// data, so a single level suffices for ex falso into every
    /// universe. The strict unit is derived, not primitive: `sUnit :=
    /// Π(_ : sEmpty). sEmpty` with `tt := λx. x`, unique up to strict
    /// irrelevance.
    Empty,
    /// Strict empty elimination `sEmpty_rect A e : A` for `e : sEmpty`.
    /// `ty` is the target type `A` itself, checked to be a type at
    /// *either* sort — the reference core eliminates the empty
    /// proposition into any universe, relevant or strict, since no
    /// closed `e` exists for the result to depend on. The eliminator
    /// never computes: `sEmpty` has no constructors, so a well-typed
    /// `e` is always neutral. No child lives under a binder.
    EmptyElim {
        ty: TermHandle,
        scrutinee: TermHandle,
    },
    /// The squash (bracket) type `Squash A : Strict u` for `A : Type u`
    /// (foundation §3.4) — existence without witness access. The
    /// carrier must be a *relevant* type: squashing is the map from
    /// `Type` into `Strict`, and a strict `A` is already a proposition.
    Squash {
        ty: TermHandle,
    },
    /// Squash introduction `sq_A x : Squash A` for `x : A`. The `ty`
    /// field is a checked annotation like `Refl`'s — it must be a
    /// relevant type and `value` must check at it, so a dependent-pair
    /// witness still checks componentwise.
    SquashIntro {
        ty: TermHandle,
        value: TermHandle,
    },
    /// Squash elimination `unsq P f x : P` for `P : Strict v`,
    /// `f : Π(_ : A). P` and `x : Squash A` — the paper's non-dependent
    /// eliminator into a strict proposition only: squashed existence
    /// never extracts a witness into relevant data. The dependent
    /// eliminator is derivable through irrelevance (its motive
    /// `Π(_ : Squash A). Strict v` applied pointwise is a strict
    /// function type, so `unsq` at that Π yields the dependent
    /// landing). Computes `unsq P f (sq a) → f a` as a budgeted step;
    /// a neutral `x` stays stuck. No child lives under a binder.
    SquashElim {
        proposition: TermHandle,
        function: TermHandle,
        scrutinee: TermHandle,
    },
    /// Boxing `Box A : Type v` for `A : Strict v` (foundation §3.5) —
    /// the converse embedding: a strict proposition wrapped as a
    /// *relevant* type, so its neutral inhabitants do not collapse
    /// under irrelevance. `A` must be a strict proposition; boxing a
    /// relevant type would re-wrap data that was never squashed.
    Box {
        ty: TermHandle,
    },
    /// Box introduction `box_A x : Box A` for `x : A`. The `ty` field is
    /// a checked annotation: it must be a strict proposition and `value`
    /// must check at it. Two boxed proofs `box a`, `box b` convert
    /// componentwise — `a ≡ b : A` already holds by strictness — but a
    /// *neutral* `x : Box A` is not forced to be a `box`, which is
    /// exactly how boxing keeps irrelevance from escaping.
    BoxIntro {
        ty: TermHandle,
        value: TermHandle,
    },
    /// Box elimination `unbox P f x : P x` for `x : Box A`,
    /// `P : Π(_ : Box A). s_v` at *either* sort (the reference core's
    /// eliminator targets `Ty_j` or `P_j` alike), and
    /// `f : Π(a : A). P (box a)`. Computes `unbox P f (box a) → f a` as
    /// a budgeted step; a neutral `x` stays stuck. The plain projection
    /// `Box A → A` is the strict-motive non-dependent instance
    /// `P := λ_. A`. No child lives under a binder.
    BoxElim {
        motive: TermHandle,
        body: TermHandle,
        scrutinee: TermHandle,
    },
    /// A reference to declaration `declaration` of the ambient
    /// signature, instantiated at `levels` — one level argument per
    /// universe parameter the declaration is polymorphic over. This is
    /// how a named definition or assumption enters a judgment: a
    /// definition constant unfolds to its instantiated body as a
    /// budgeted step, while an assumption constant is a neutral atom.
    /// The level arguments are judgment-scope level expressions, not
    /// terms: no binder binds them and `shift`/`substitute` never
    /// reach them.
    Constant {
        declaration: u32,
        levels: Vec<Level>,
    },
}

/// The widest node in [`Term`]: `caseTwo`, `J` and `sup` each carry four
/// subterms.
pub(super) const MAX_SUBTERMS: usize = 4;

/// The immediate subterms of one node in slot order, each with the number of
/// binders scoping it inside that node. Payload-only nodes carry none, which
/// is how a walk learns that its work is in the payload.
pub(super) struct Subterms {
    slots: [Option<(TermHandle, u32)>; MAX_SUBTERMS],
    count: usize,
}

impl Subterms {
    fn new() -> Self {
        Self {
            slots: [None; MAX_SUBTERMS],
            count: 0,
        }
    }

    fn extend(&mut self, children: impl IntoIterator<Item = (TermHandle, u32)>) {
        for (handle, binders) in children {
            self.slots[self.count] = Some((handle, binders));
            self.count += 1;
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub(super) fn len(&self) -> usize {
        self.count
    }

    pub(super) fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (TermHandle, u32)> + ExactSizeIterator + '_ {
        self.slots[..self.count]
            .iter()
            .map(|slot| slot.expect("one initialized term slot"))
    }
}

/// One statement of the term layout, beside the variants it describes.
///
/// Each row names a variant's fields in declaration order with the number of
/// binders scoping that slot — `Pi`'s codomain, `Lambda`'s body and `Sigma`'s
/// codomain sit under one more binder, every other slot under none.
/// [`Term::subterms`] reads a row and [`Term::with_children`] writes it back in
/// the same order, so the two can never disagree about a slot, and a new
/// variant fails to compile here instead of being missed by one of the walks.
macro_rules! term_layout {
    ($($variant:ident { $($field:ident : $binders:expr),* $(,)? })+) => {
        impl Term {
            /// This node's subterms in slot order, each with the number of
            /// binders scoping it. Payload-only nodes carry none, which is how
            /// a walk learns that its work is in the payload.
            pub(super) fn subterms(&self) -> Subterms {
                let mut slots = Subterms::new();
                match self {
                    $(Term::$variant { $($field),* } => slots.extend([$((*$field, $binders)),*]),)+
                    // `Variable` and `Constant` carry their own data (de Bruijn
                    // indices, level arguments); the rest carry nothing. No
                    // walk descends from any of them.
                    Term::Dummy
                    | Term::Variable(_)
                    | Term::Sort(_)
                    | Term::Two
                    | Term::TwoZero
                    | Term::TwoOne
                    | Term::Empty
                    | Term::Constant { .. } => {}
                }
                slots
            }

            /// Structural payload equality, separate from the child walk.
            /// This deliberately keeps universe syntax equality distinct from
            /// semantic level conversion and never treats Dummy as evidence.
            fn payloads_equal(&self, other: &Term) -> bool {
                match (self, other) {
                    $((Term::$variant { .. }, Term::$variant { .. }) => true,)+
                    (Term::Variable(left), Term::Variable(right)) => left == right,
                    (Term::Sort(left), Term::Sort(right)) => left == right,
                    (Term::Constant { declaration: left, levels: left_levels },
                     Term::Constant { declaration: right, levels: right_levels }) => {
                        left == right && left_levels == right_levels
                    }
                    (Term::Two, Term::Two)
                    | (Term::TwoZero, Term::TwoZero)
                    | (Term::TwoOne, Term::TwoOne)
                    | (Term::Empty, Term::Empty) => true,
                    _ => false,
                }
            }

            /// The inverse of [`Term::subterms`]: this node with `children` in
            /// its slot order. A payload-only node keeps its payload.
            pub(super) fn with_children(&self, children: &[TermHandle]) -> Term {
                debug_assert_eq!(
                    children.len(),
                    self.subterms().len(),
                    "every slot of a term takes exactly one child"
                );
                let mut next = children.iter().copied();
                match self {
                    $(Term::$variant { .. } => Term::$variant {
                        $($field: next.next().expect("one child per term slot")),*
                    },)+
                    Term::Dummy
                    | Term::Variable(_)
                    | Term::Sort(_)
                    | Term::Two
                    | Term::TwoZero
                    | Term::TwoOne
                    | Term::Empty
                    | Term::Constant { .. } => self.clone(),
                }
            }
        }
    };
}

term_layout! {
    Pi { domain: 0, codomain: 1 }
    Lambda { domain: 0, body: 1 }
    Apply { function: 0, argument: 0 }
    Sigma { domain: 0, codomain: 1 }
    Pair { first: 0, second: 0 }
    Fst { pair: 0 }
    Snd { pair: 0 }
    CaseTwo { motive: 0, zero_branch: 0, one_branch: 0, scrutinee: 0 }
    Id { ty: 0, left: 0, right: 0 }
    Refl { ty: 0, value: 0 }
    IdElim { motive: 0, base: 0, endpoint: 0, proof: 0 }
    W { carrier: 0, children: 0 }
    Sup { carrier: 0, children: 0, label: 0, function: 0 }
    IndW { motive: 0, step: 0, tree: 0 }
    EmptyElim { ty: 0, scrutinee: 0 }
    Squash { ty: 0 }
    SquashIntro { ty: 0, value: 0 }
    SquashElim { proposition: 0, function: 0, scrutinee: 0 }
    Box { ty: 0 }
    BoxIntro { ty: 0, value: 0 }
    BoxElim { motive: 0, body: 0, scrutinee: 0 }
}
/// Contiguous arena storage for terms. Null handles resolve to `Term::Dummy`.
#[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct TermArena {
    terms: Arena<Term>,
}

impl TermArena {
    pub fn new() -> Self {
        Self {
            terms: Arena::new(),
        }
    }

    pub fn insert(&mut self, term: Term) -> TermHandle {
        self.terms.insert(term)
    }

    /// `function a₁ … aₙ` — the left-nested application spine every
    /// constant reference, law instance and relation elaboration ends on:
    /// the arguments bind left to right, so slot order is statement order.
    pub fn apply_arguments(
        &mut self,
        function: TermHandle,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let mut spine = function;
        for &argument in arguments {
            spine = self.insert(Term::Apply {
                function: spine,
                argument,
            });
        }
        spine
    }

    /// `relation left right`: every binary-relation elaboration ends on
    /// this pair of applications.
    pub fn apply_binary(
        &mut self,
        relation: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        self.apply_arguments(relation, &[left, right])
    }

    pub fn get(&self, handle: TermHandle) -> Term {
        self.node(handle).clone()
    }

    /// Read-only handle walks borrow payloads instead of cloning level syntax.
    pub(super) fn node(&self, handle: TermHandle) -> &Term {
        self.terms.get(handle)
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Structural equality by content. Alpha-equivalence is free with de
    /// Bruijn indices, so this is also the neutral-head comparison used by
    /// conversion and the tests.
    pub fn structurally_equal(&self, left: TermHandle, right: TermHandle) -> bool {
        let mut pending = vec![(false, (left, right))];
        let mut active = HashSet::new();
        let mut complete = HashSet::new();
        while let Some((finish, pair)) = pending.pop() {
            if finish {
                active.remove(&pair);
                complete.insert(pair);
                continue;
            }
            if complete.contains(&pair) {
                continue;
            }
            // A finite term DAG shares completed subgraphs. A back edge is
            // not a completed equality judgment and supplies no evidence.
            if !active.insert(pair) {
                return false;
            }
            let left = self.node(pair.0);
            let right = self.node(pair.1);
            if !left.payloads_equal(right) {
                return false;
            }
            pending.push((true, pair));
            pending.extend(
                left.subterms()
                    .iter()
                    .zip(right.subterms().iter())
                    .rev()
                    .map(|((left, _), (right, _))| (false, (left, right))),
            );
        }
        true
    }
}

impl Default for TermArena {
    fn default() -> Self {
        Self::new()
    }
}

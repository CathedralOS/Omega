//! Term model for the common mathematical core: universe level expressions
//! over the judgment's level parameters, relevant and strict sorts,
//! arena-held de Bruijn terms, and the selected inductive profile's
//! two-element type, relevant identity type, and W-type of well-founded
//! trees.

use std::collections::BTreeMap;

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
/// [`levels_equal`] through the `max`-normal form, never by `==`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

impl Level {
    /// `u + 1`, or `None` when `u` is the largest representable constant —
    /// typing maps `None` to `CoreError::LevelOverflow`. Constants fold at
    /// construction so closed levels keep their collapsed syntax.
    pub fn successor(self) -> Option<Level> {
        match self {
            Level::Constant(value) => value.checked_add(1).map(Level::Constant),
            _ => Some(Level::Successor(Box::new(self))),
        }
    }

    /// `max(u, v)`. Constants fold at construction so closed levels keep
    /// their collapsed syntax; anything involving a parameter stays
    /// `Maximum` syntax and is normalized only for comparison.
    pub fn maximum(self, other: Level) -> Level {
        match (self, other) {
            (Level::Constant(left), Level::Constant(right)) => Level::Constant(left.max(right)),
            (left, right) => Level::Maximum(Box::new(left), Box::new(right)),
        }
    }
}

/// The `max`-normal form of a level: `max(k, p₀+o₀, p₁+o₁, …)` kept as a
/// constant floor plus the maximum successor offset per parameter. This is
/// the free algebra of `max` (associative, commutative, idempotent) with
/// `succ` distributing over `max`, plus the one absorption the semantics
/// forces: a constant floor never exceeds some variable's offset is
/// redundant, since `max(k, p+o) = p+o` for all `p` whenever `o >= k`.
/// Equality of normal forms is then exactly equality of the induced
/// functions over the naturals, so conversion is decidable and complete —
/// no level solving: distinct parameters never convert.
#[derive(Debug, PartialEq, Eq)]
struct NormalLevel {
    constant: u64,
    offsets: BTreeMap<u32, u64>,
}

impl NormalLevel {
    /// Absorb the constant floor when a variable offset already reaches it:
    /// at the all-zero instantiation `max(k, pᵢ+oᵢ)` contributes `oᵢ >= k`
    /// anyway, so `k` can never decide the value.
    fn absorb_constant(&mut self) {
        if self
            .offsets
            .values()
            .max()
            .is_some_and(|offset| *offset >= self.constant)
        {
            self.constant = 0;
        }
    }
}

fn normal_form(level: &Level) -> NormalLevel {
    let mut form = raw_normal_form(level);
    form.absorb_constant();
    form
}

fn raw_normal_form(level: &Level) -> NormalLevel {
    match level {
        Level::Constant(value) => NormalLevel {
            constant: u64::from(*value),
            offsets: BTreeMap::new(),
        },
        Level::Parameter(index) => {
            let mut offsets = BTreeMap::new();
            offsets.insert(*index, 0);
            NormalLevel {
                constant: 0,
                offsets,
            }
        }
        Level::Successor(inner) => {
            let mut form = raw_normal_form(inner);
            form.constant += 1;
            for offset in form.offsets.values_mut() {
                *offset += 1;
            }
            form
        }
        Level::Maximum(left, right) => {
            let mut form = raw_normal_form(left);
            let other = raw_normal_form(right);
            form.constant = form.constant.max(other.constant);
            for (parameter, offset) in other.offsets {
                form.offsets
                    .entry(parameter)
                    .and_modify(|existing| *existing = (*existing).max(offset))
                    .or_insert(offset);
            }
            form
        }
    }
}

/// Semantic level equality: the `max`-normal forms agree. `==` on `Level`
/// is syntactic and only answers whether the two expressions are the same
/// syntax; conversion must use this instead so `max(u, v)` converts with
/// `max(v, u)` and `succ` distributes over `max`.
pub(crate) fn levels_equal(left: &Level, right: &Level) -> bool {
    normal_form(left) == normal_form(right)
}

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
/// syntactic on the level syntax; conversion uses [`sorts_equal`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
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
}

/// Contiguous arena storage for terms. Null handles resolve to `Term::Dummy`.
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

    pub fn get(&self, handle: TermHandle) -> Term {
        self.terms.get(handle).clone()
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
        match (self.get(left), self.get(right)) {
            (Term::Variable(left_index), Term::Variable(right_index)) => left_index == right_index,
            (Term::Sort(left_sort), Term::Sort(right_sort)) => left_sort == right_sort,
            (
                Term::Pi {
                    domain: left_domain,
                    codomain: left_codomain,
                },
                Term::Pi {
                    domain: right_domain,
                    codomain: right_codomain,
                },
            )
            | (
                Term::Lambda {
                    domain: left_domain,
                    body: left_codomain,
                },
                Term::Lambda {
                    domain: right_domain,
                    body: right_codomain,
                },
            )
            | (
                Term::Apply {
                    function: left_domain,
                    argument: left_codomain,
                },
                Term::Apply {
                    function: right_domain,
                    argument: right_codomain,
                },
            )
            | (
                Term::Sigma {
                    domain: left_domain,
                    codomain: left_codomain,
                },
                Term::Sigma {
                    domain: right_domain,
                    codomain: right_codomain,
                },
            ) => {
                self.structurally_equal(left_domain, right_domain)
                    && self.structurally_equal(left_codomain, right_codomain)
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
                self.structurally_equal(left_first, right_first)
                    && self.structurally_equal(left_second, right_second)
            }
            (Term::Fst { pair: left }, Term::Fst { pair: right })
            | (Term::Snd { pair: left }, Term::Snd { pair: right }) => {
                self.structurally_equal(left, right)
            }
            (Term::Two, Term::Two)
            | (Term::TwoZero, Term::TwoZero)
            | (Term::TwoOne, Term::TwoOne) => true,
            (
                Term::CaseTwo {
                    motive: left_motive,
                    zero_branch: left_zero_branch,
                    one_branch: left_one_branch,
                    scrutinee: left_scrutinee,
                },
                Term::CaseTwo {
                    motive: right_motive,
                    zero_branch: right_zero_branch,
                    one_branch: right_one_branch,
                    scrutinee: right_scrutinee,
                },
            ) => {
                self.structurally_equal(left_motive, right_motive)
                    && self.structurally_equal(left_zero_branch, right_zero_branch)
                    && self.structurally_equal(left_one_branch, right_one_branch)
                    && self.structurally_equal(left_scrutinee, right_scrutinee)
            }
            (
                Term::Id {
                    ty: left_ty,
                    left: left_left,
                    right: left_right,
                },
                Term::Id {
                    ty: right_ty,
                    left: right_left,
                    right: right_right,
                },
            ) => {
                self.structurally_equal(left_ty, right_ty)
                    && self.structurally_equal(left_left, right_left)
                    && self.structurally_equal(left_right, right_right)
            }
            (
                Term::Refl {
                    ty: left_ty,
                    value: left_value,
                },
                Term::Refl {
                    ty: right_ty,
                    value: right_value,
                },
            ) => {
                self.structurally_equal(left_ty, right_ty)
                    && self.structurally_equal(left_value, right_value)
            }
            (
                Term::IdElim {
                    motive: left_motive,
                    base: left_base,
                    endpoint: left_endpoint,
                    proof: left_proof,
                },
                Term::IdElim {
                    motive: right_motive,
                    base: right_base,
                    endpoint: right_endpoint,
                    proof: right_proof,
                },
            ) => {
                self.structurally_equal(left_motive, right_motive)
                    && self.structurally_equal(left_base, right_base)
                    && self.structurally_equal(left_endpoint, right_endpoint)
                    && self.structurally_equal(left_proof, right_proof)
            }
            (
                Term::W {
                    carrier: left_carrier,
                    children: left_children,
                },
                Term::W {
                    carrier: right_carrier,
                    children: right_children,
                },
            ) => {
                self.structurally_equal(left_carrier, right_carrier)
                    && self.structurally_equal(left_children, right_children)
            }
            (
                Term::Sup {
                    carrier: left_carrier,
                    children: left_children,
                    label: left_label,
                    function: left_function,
                },
                Term::Sup {
                    carrier: right_carrier,
                    children: right_children,
                    label: right_label,
                    function: right_function,
                },
            ) => {
                self.structurally_equal(left_carrier, right_carrier)
                    && self.structurally_equal(left_children, right_children)
                    && self.structurally_equal(left_label, right_label)
                    && self.structurally_equal(left_function, right_function)
            }
            (
                Term::IndW {
                    motive: left_motive,
                    step: left_step,
                    tree: left_tree,
                },
                Term::IndW {
                    motive: right_motive,
                    step: right_step,
                    tree: right_tree,
                },
            ) => {
                self.structurally_equal(left_motive, right_motive)
                    && self.structurally_equal(left_step, right_step)
                    && self.structurally_equal(left_tree, right_tree)
            }
            _ => false,
        }
    }
}

impl Default for TermArena {
    fn default() -> Self {
        Self::new()
    }
}

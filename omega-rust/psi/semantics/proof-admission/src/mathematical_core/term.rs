//! Term model for the common mathematical core: closed universe levels,
//! relevant and strict sorts, arena-held de Bruijn terms, and the selected
//! inductive profile's two-element type, relevant identity type, and
//! W-type of well-founded trees.

use arena::Arena;

/// A closed universe level. Level variables are not part of this slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Level(pub u32);

impl Level {
    /// The next universe level, or `None` when the level overflows. Typing
    /// maps `None` to `CoreError::LevelOverflow`.
    pub fn successor(self) -> Option<Level> {
        self.0.checked_add(1).map(Level)
    }

    /// The larger of the two levels, used by dependent function formation.
    pub fn max(self, other: Level) -> Level {
        Level(self.0.max(other.0))
    }
}

/// A mathematical universe sort: `Type u` is relevant, `Strict v` is the
/// definitionally irrelevant logical layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sort {
    Type(Level),
    Strict(Level),
}

impl Sort {
    pub fn level(self) -> Level {
        match self {
            Sort::Type(level) | Sort::Strict(level) => level,
        }
    }

    pub fn is_strict(self) -> bool {
        matches!(self, Sort::Strict(_))
    }
}

/// Handle into a `TermArena`. The zero handle resolves to the arena dummy.
pub type TermHandle = arena::Handle<Term>;

/// One term node. Children and binders are handles into the same arena.
/// `Variable` indices are de Bruijn indices; 0 names the innermost binder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
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
        *self.terms.get(handle)
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

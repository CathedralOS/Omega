//! Term model for the common mathematical core: closed universe levels,
//! relevant and strict sorts, arena-held de Bruijn terms, and the selected
//! inductive profile's two-element type.

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
            _ => false,
        }
    }
}

impl Default for TermArena {
    fn default() -> Self {
        Self::new()
    }
}

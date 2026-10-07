//! The set-quotient scheme: the quotient interface of the selected
//! [quotient specification](../../../../../../wiki/spec/proofs/quotients.md)'s
//! "Set-quotient foundation" section, authored as ordinary
//! universe-polymorphic declarations the kernel re-decides.
//!
//! Given a producer's description —
//!
//! ```text
//! A : Type u                    representative carrier (need not be a set)
//! R : A → A → Type v            the selected relation
//! ```
//!
//! the carrier is `Q(A,R) : Type max(u,v)` — opaque, never resized to a
//! lower universe, and carrying no destructuring, recursor or executable
//! provider by declaration. The interface the specification lists as
//! explicitly admitted named mathematical assumptions is seven
//! declarations:
//!
//! ```text
//! Q         : Π(A : Type u). Π(R : A → A → Type v). Type max(u,v)
//! project   : Π A R. A → Q
//! setQ      : Π A R. isSet(Q)                        — propositional setness
//! sound     : Π A R. Π(a b : A). R a b → Id Q (project a) (project b)
//! effective : Π A R. Π(a b : A). Id Q (project a) (project b) → R a b
//! elim      : Π A R. Π(P : Q → Type w). Π(_ : (z : Q) → isSet(P z)).
//!             Π(d : (a : A) → P (project a)).
//!             Π(c : (a b : A) → (r : R a b)
//!                  → Id (P (project b)) (transport P a b r (d a)) (d b)).
//!             Π(z : Q). P z
//! beta      : Π A R P sets d c. Π(a : A).
//!             Id (P (project a)) (elim P sets d c (project a)) (d a)
//! ```
//!
//! `beta` is point computation as a *relevant identity*, exactly as the
//! specification states: no new conversion rule, so `lift`/`elim` applied
//! to `project a` stays a neutral term and never converts to `d a`.
//!
//! Eleven declarations are *derived with checked terms*, as the
//! specification requires rather than adding separate default axioms:
//!
//! - `transport` — forward premise transport `P (project a) → P
//!   (project b)` across `r : R a b`, defined through `sound` and `J`.
//!   This is the transport `elim`'s coherence law `c` names, and the
//!   optional precondition-transport surface a producer selects when its
//!   operation carries premise evidence.
//! - `idTrans`, `transportConst` — the J-derived identity lemmas the
//!   derived operations are built from (transitivity; transport across a
//!   constant family).
//! - `lift` — ordinary quotient-owned lift: for `B : Type w` with
//!   `isSet B`, a representative operation `f : A → B` and an explicit
//!   congruence theorem `respect : Π(a b : A). R a b → Id B (f a) (f b)`,
//!   `lift` is the checked section `Π(z : Q). B` — `elim` on the
//!   constant family `λ_. B`, whose coherence obligation reduces to
//!   `transportConst` composed with `respect` by `idTrans`. The
//!   congruence theorem is an explicit argument: no theorem search, no
//!   structural inference, no implicit witness selection.
//! - `idSym`, `idCancel`, `propIsSet`, `boxProp` — the remaining
//!   derivable infrastructure: `Id` symmetry and the inverse law, the
//!   standard `J` proof that a mere proposition is a set, and the
//!   `unbox`-backed proof that a boxed strict proposition is a mere
//!   proposition.
//! - `indProp`, `coverage`, `unique` — the rest of the specification's
//!   derive-list: proposition-valued induction through set-valued
//!   elimination and boxing, coverage `Squash(Σ a. Id (project a) z)`,
//!   and pointwise uniqueness of sections agreeing on projections.
//!
//! `liftPre` is the optional forward-precondition-transport shape: the
//! public operation's premise `Pub : Q → Type p` is transported to the
//! representative operation's premise `Pre : A → Type q` by an explicit
//! `t : (a : A) → Pub (project a) → Pre a`, and the congruence theorem
//! covers `f` under the transported premises. Because `Π(z : Q). Pub z
//! → B` is a function-valued motive, no `elim`-derivation exists without
//! extensionality — so `liftPre` is admitted as one more named
//! assumption with its exact statement, like the rest of the interface,
//! and a checked-only consumer sees it in every assumption closure that
//! references it.
//!
//! No `Term` variant, typing rule, conversion rule or wire tag is added:
//! a malformed description or a missing law is an ordinary typing
//! rejection, not a new judgment. Every assumption lands in
//! [`super::signature::assumption_closure`], so a receiver refusing
//! quotient assumptions rejects any judgment that commits to one.
//!
//! Producers apply the scheme through [`QuotientFamily`], which bundles
//! the description and builds the `Constant` spines; the kernel
//! re-decides every application through ordinary typing and conversion.
//! The scheme declarations are signature prefix `0..20` — they reference
//! only each other, so a producer's own declarations append after them.

use super::scheme_dsl::{
    Syntax, app, apps, box_elim, box_intro, boxed, build, id, jelim, lam, pair, pi, pis, refl,
    scheme_at, sigma, sort, squash, squash_intro, strict, ty, v,
};
use super::signature::Declaration;
use super::term::{Level, Term, TermArena, TermHandle};

mod declarations;
pub use declarations::quotient_scheme;

/// `isSet X = Π(x y : X). Π(p q : Id X x y). Id (Id X x y) p q`,
/// declaration 0 of [`quotient_scheme`] — a definition, not an
/// assumption: setness is ordinary mathematics here, so it never enters
/// an assumption closure.
pub const QUOTIENT_IS_SET: u32 = 0;
/// `Q`, declaration 1 — the opaque quotient carrier, an assumption.
pub const QUOTIENT: u32 = 1;
/// `project`, declaration 2 — the quotient image of a representative,
/// an assumption.
pub const QUOTIENT_PROJECT: u32 = 2;
/// `setQ`, declaration 3 — `isSet(Q)`, an assumption: propositional
/// setness, not definitional proof irrelevance.
pub const QUOTIENT_SET: u32 = 3;
/// `sound`, declaration 4 — `R a b → Id Q (project a) (project b)`, an
/// assumption.
pub const QUOTIENT_SOUND: u32 = 4;
/// `effective`, declaration 5 — `Id Q (project a) (project b) → R a b`,
/// an assumption supplying evidence of exactly the selected relation.
pub const QUOTIENT_EFFECTIVE: u32 = 5;
/// `transport`, declaration 6 — forward premise transport
/// `P (project a) → P (project b)` across `r : R a b`, *derived* through
/// `sound` and `J`.
pub const QUOTIENT_TRANSPORT: u32 = 6;
/// `elim`, declaration 7 — dependent elimination into set-valued
/// families, an assumption.
pub const QUOTIENT_ELIM: u32 = 7;
/// `beta`, declaration 8 — the point computation *identity*, an
/// assumption. It is a relevant `Id`, never a conversion rule.
pub const QUOTIENT_BETA: u32 = 8;
/// `idTrans`, declaration 9 — `Id` transitivity, *derived* through `J`.
pub const QUOTIENT_ID_TRANS: u32 = 9;
/// `transportConst`, declaration 10 — transport across a constant
/// family is propositionally the identity, *derived* through `J`.
pub const QUOTIENT_TRANSPORT_CONST: u32 = 10;
/// `lift`, declaration 11 — the quotient-owned operation shape:
/// explicit representative operation plus explicit congruence theorem,
/// *derived* through `elim`.
pub const QUOTIENT_LIFT: u32 = 11;
/// `liftPre`, declaration 12 — the optional forward-precondition-
/// transport operation shape, an admitted named assumption: it is not
/// `elim`-derivable without extensionality, which this calculus
/// deliberately lacks.
pub const QUOTIENT_LIFT_PRECONDITION: u32 = 12;
/// `idSym`, declaration 13 — symmetry of `Id` through `J`, a
/// *definition*: the first of the identity lemmas the remaining derived
/// operations are built from.
pub const QUOTIENT_ID_SYM: u32 = 13;
/// `idCancel`, declaration 14 — a *definition*: the inverse law
/// `trans (sym p) p = refl`, proved by `J` on `p` — the base computes
/// because `refl` is canonical.
pub const QUOTIENT_ID_CANCEL: u32 = 14;
/// `propIsSet`, declaration 15 — a *definition*: a mere proposition is
/// a set, `isProp X → isSet X`. The standard `J` development: every
/// `p : Id X x y` equals `trans (sym (f x x)) (f x y)`, so parallel
/// proofs share that normal form.
pub const QUOTIENT_PROP_IS_SET: u32 = 15;
/// `boxProp`, declaration 16 — a *definition*: a boxed strict
/// proposition is a mere proposition, `Π(S : Strict s). Π(x y : Box S).
/// Id (Box S) x y`. Two `unbox` rounds reduce the goal to `box a =
/// box b`, which holds definitionally because `S` is strict.
pub const QUOTIENT_BOX_PROP: u32 = 16;
/// `indProp`, declaration 17 — a *definition*: proposition-valued
/// induction, `Π A R. Π(P : Π(_ : Q A R). Strict s). Π(_ : Π(a : A).
/// P (project a)). Π(z : Q A R). P z`, derived from `elim` through the
/// boxed motive `λz. Box (P z)` and `unbox` — exactly the "boxing when
/// needed" route the specification names.
pub const QUOTIENT_IND_PROP: u32 = 17;
/// `coverage`, declaration 18 — a *definition*: every quotient element
/// is squashed-covered by a projection, `Π A R. Π(z : Q A R).
/// Squash (Σ(a : A). Id (Q A R) (project a) z)` — an `indProp`
/// application; it supplies no representative-extraction function.
pub const QUOTIENT_COVERAGE: u32 = 18;
/// `unique`, declaration 19 — a *definition*: pointwise uniqueness of
/// sections agreeing on projections, `Π A R. Π(P : Π(_ : Q A R). Type
/// w). Π(_ : Π(z : Q A R). isSet (P z)). Π(s₁ s₂ : Π(z : Q A R). P z).
/// Π(_ : Π(a : A). Id (P (project a)) (s₁ (project a)) (s₂ (project
/// a))). Π(z : Q A R). Id (P z) (s₁ z) (s₂ z)`. Derived by `elim` at
/// the identity family; its coherence and setness collapse through the
/// propositional setness of `P`. Equality of the whole functions would
/// need the extensionality this calculus lacks — only the pointwise
/// statement is derived.
pub const QUOTIENT_UNIQUE: u32 = 19;

/// The level of the carrier `Q A R`: `max(u, v)` — the specification
/// forbids resizing the quotient to a lower universe.
fn carrier_level() -> Level {
    Level::Maximum(Box::new(Level::Parameter(0)), Box::new(Level::Parameter(1)))
}

/// `[u, v]` — the instantiation every carrier-side declaration shares.
fn uv() -> Vec<Level> {
    vec![Level::Parameter(0), Level::Parameter(1)]
}

/// `[u, v, w]` — adds the motive level `w` as parameter 2.
fn uvw() -> Vec<Level> {
    let mut levels = uv();
    levels.push(Level::Parameter(2));
    levels
}

/// A reference to an arity-2 scheme declaration at `[u, v]`.
fn carrier_scheme(declaration: u32) -> Syntax {
    scheme_at(declaration, uv())
}

/// A reference to an arity-3 scheme declaration at `[u, v, w]`.
fn motive_scheme(declaration: u32) -> Syntax {
    scheme_at(declaration, uvw())
}

/// `Q A R` — the applied quotient carrier.
fn quotient_of() -> Syntax {
    apps(carrier_scheme(QUOTIENT), [v("A"), v("R")])
}

/// `project A R a` — the quotient image of representative `a`.
fn project_of(representative: Syntax) -> Syntax {
    apps(
        carrier_scheme(QUOTIENT_PROJECT),
        [v("A"), v("R"), representative],
    )
}

/// `R a b` — the selected relation applied to two representatives.
fn related(left: Syntax, right: Syntax) -> Syntax {
    app(app(v("R"), left), right)
}

/// `isSet[s] X` — `X` is a set: parallel identities in `X` are
/// propositionally equal.
fn is_set(level: Level, ty: Syntax) -> Syntax {
    app(scheme_at(QUOTIENT_IS_SET, vec![level]), ty)
}

/// `sound A R a b r : Id (Q A R) (project a) (project b)`.
fn sound_of(left: Syntax, right: Syntax, evidence: Syntax) -> Syntax {
    apps(
        carrier_scheme(QUOTIENT_SOUND),
        [v("A"), v("R"), left, right, evidence],
    )
}

/// The shared description telescope `Π(A : Type u). Π(R : A → A → Type
/// v). …` — parameters are de Bruijn-resolved by name inside `rest`.
fn pi_description(rest: Syntax) -> Syntax {
    pi(
        "A",
        ty(0),
        pi("R", pi("_", v("A"), pi("_", v("A"), ty(1))), rest),
    )
}

/// The λ-side of the same telescope for declaration bodies.
fn lam_description(rest: Syntax) -> Syntax {
    lam(
        "A",
        ty(0),
        lam("R", pi("_", v("A"), pi("_", v("A"), ty(1))), rest),
    )
}

/// The motive telescope `Π(P : Q A R → Type w). …` used by `elim`,
/// `beta` and `transport`.
fn motive_type() -> Syntax {
    pi("_", quotient_of(), ty(2))
}

/// A producer's description of a set quotient — the `A, R` of the
/// profile plus the levels `u, v` they live at. The bundled spines
/// build `Constant` applications; every argument is still re-decided by
/// typing at use.
#[derive(Clone, Debug)]
pub struct QuotientFamily {
    /// The description's levels `[u, v]`, as level expressions valid in
    /// the calling judgment's scope — `Parameter` indices inside a
    /// polymorphic declaration, `Constant` levels for a closed one.
    pub levels: [Level; 2],
    /// `A : Type u` — the representative carrier.
    pub carrier: TermHandle,
    /// `R : Π(_ _ : A). Type v` — the selected relation.
    pub relation: TermHandle,
}

impl QuotientFamily {
    /// `DECL level args…` — a lemma constant applied under its own level
    /// rather than the description's `[u, v]` instantiation spine.
    fn unspined(
        &self,
        arena: &mut TermArena,
        declaration: u32,
        level: Level,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let mut term = arena.insert(Term::Constant {
            declaration,
            levels: vec![level],
        });
        for &argument in arguments {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
    }

    fn spine(
        &self,
        arena: &mut TermArena,
        declaration: u32,
        mut levels: Vec<Level>,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let mut term = arena.insert(Term::Constant {
            declaration,
            levels: {
                let mut instantiation = self.levels.to_vec();
                instantiation.append(&mut levels);
                instantiation
            },
        });
        term = arena.insert(Term::Apply {
            function: term,
            argument: self.carrier,
        });
        term = arena.insert(Term::Apply {
            function: term,
            argument: self.relation,
        });
        for &argument in arguments {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
    }

    /// `Q A R` — the opaque carrier, a `Type max(u,v)`.
    pub fn quotient(&self, arena: &mut TermArena) -> TermHandle {
        self.spine(arena, QUOTIENT, Vec::new(), &[])
    }

    /// `project A R a : Q A R` — the quotient image of `a`.
    pub fn project(&self, arena: &mut TermArena, representative: TermHandle) -> TermHandle {
        self.spine(arena, QUOTIENT_PROJECT, Vec::new(), &[representative])
    }

    /// `sound A R a b r : Id (Q A R) (project a) (project b)` — the
    /// congruence law of the selected relation.
    pub fn sound(
        &self,
        arena: &mut TermArena,
        left: TermHandle,
        right: TermHandle,
        evidence: TermHandle,
    ) -> TermHandle {
        self.spine(arena, QUOTIENT_SOUND, Vec::new(), &[left, right, evidence])
    }

    /// `effective A R a b p : R a b` — the effectivity law: an identity
    /// of projections supplies evidence of exactly the selected
    /// relation, never the representative.
    pub fn effective(
        &self,
        arena: &mut TermArena,
        left: TermHandle,
        right: TermHandle,
        proof: TermHandle,
    ) -> TermHandle {
        self.spine(arena, QUOTIENT_EFFECTIVE, Vec::new(), &[left, right, proof])
    }

    /// `transport A R P a b r h : P (project b)` — forward premise
    /// transport across `r : R a b`, for `P : Q A R → Type w` and
    /// `h : P (project a)`. `motive_level` is the family's `w`.
    pub fn transport(
        &self,
        arena: &mut TermArena,
        motive_level: Level,
        family: TermHandle,
        left: TermHandle,
        right: TermHandle,
        evidence: TermHandle,
        premise: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            QUOTIENT_TRANSPORT,
            vec![motive_level],
            &[family, left, right, evidence, premise],
        )
    }

    /// `elim A R P sets d c z : P z` — dependent elimination into the
    /// set-valued family `P`, with `sets : Π(z : Q). isSet (P z)`, the
    /// representative case `d : Π(a : A). P (project a)` and the
    /// transport compatibility `c`.
    pub fn elim(
        &self,
        arena: &mut TermArena,
        motive_level: Level,
        family: TermHandle,
        sets: TermHandle,
        case: TermHandle,
        compatibility: TermHandle,
        element: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            QUOTIENT_ELIM,
            vec![motive_level],
            &[family, sets, case, compatibility, element],
        )
    }

    /// `beta A R P sets d c a : Id (P (project a)) (elim P sets d c
    /// (project a)) (d a)` — the point computation identity.
    pub fn beta(
        &self,
        arena: &mut TermArena,
        motive_level: Level,
        family: TermHandle,
        sets: TermHandle,
        case: TermHandle,
        compatibility: TermHandle,
        representative: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            QUOTIENT_BETA,
            vec![motive_level],
            &[family, sets, case, compatibility, representative],
        )
    }

    /// `idTrans B x y z p q : Id B x z` — transitivity of `Id`, where
    /// `p : Id B x y` and `q : Id B y z`. `level` is `B`'s level.
    pub fn id_trans(
        &self,
        arena: &mut TermArena,
        level: Level,
        ty: TermHandle,
        left: TermHandle,
        middle: TermHandle,
        right: TermHandle,
        first: TermHandle,
        second: TermHandle,
    ) -> TermHandle {
        self.unspined(
            arena,
            QUOTIENT_ID_TRANS,
            level,
            &[ty, left, middle, right, first, second],
        )
    }

    /// `lift A R B setB f respect z : B` — the quotient-owned operation:
    /// representative operation `f : Π(_ : A). B` plus its explicit
    /// congruence theorem `respect`, applied at `z : Q A R`.
    /// `motive_level` is `B`'s `w`; `set_proof : isSet B`.
    pub fn lift(
        &self,
        arena: &mut TermArena,
        motive_level: Level,
        result: TermHandle,
        set_proof: TermHandle,
        operation: TermHandle,
        respect: TermHandle,
        element: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            QUOTIENT_LIFT,
            vec![motive_level],
            &[result, set_proof, operation, respect, element],
        )
    }

    /// `liftPre A R B setB Pub Pre t f respect z h : B` — the optional
    /// forward-precondition-transport shape: `t : Π(a : A). Pub (project
    /// a) → Pre a` transports the public premise to the representative
    /// operation's premise, `h : Pub z`. The three appended levels are
    /// `[w, p, q]` — `B`'s level, `Pub`'s family level and `Pre`'s
    /// family level.
    pub fn lift_precondition(
        &self,
        arena: &mut TermArena,
        result_level: Level,
        public_level: Level,
        representative_level: Level,
        result: TermHandle,
        set_proof: TermHandle,
        public: TermHandle,
        representative: TermHandle,
        transport: TermHandle,
        operation: TermHandle,
        respect: TermHandle,
        element: TermHandle,
        premise: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            QUOTIENT_LIFT_PRECONDITION,
            vec![result_level, public_level, representative_level],
            &[
                result,
                set_proof,
                public,
                representative,
                transport,
                operation,
                respect,
                element,
                premise,
            ],
        )
    }

    /// `idSym B x y p : Id B y x` — symmetry of `Id`. `level` is `B`'s
    /// level. Like `idTrans`, this lemma is not `A`-/`R`-spined.
    pub fn id_sym(
        &self,
        arena: &mut TermArena,
        level: Level,
        ty: TermHandle,
        left: TermHandle,
        right: TermHandle,
        proof: TermHandle,
    ) -> TermHandle {
        self.unspined(arena, QUOTIENT_ID_SYM, level, &[ty, left, right, proof])
    }

    /// `idCancel B x y p : Id (Id B y y) (trans (sym p) p) (refl B y)`
    /// — the inverse law. `level` is `B`'s level.
    pub fn id_cancel(
        &self,
        arena: &mut TermArena,
        level: Level,
        ty: TermHandle,
        left: TermHandle,
        right: TermHandle,
        proof: TermHandle,
    ) -> TermHandle {
        self.unspined(arena, QUOTIENT_ID_CANCEL, level, &[ty, left, right, proof])
    }

    /// `propIsSet X f : isSet X` — a mere proposition is a set, where
    /// `evidence : Π(x y : X). Id X x y`. `level` is `X`'s level.
    pub fn prop_is_set(
        &self,
        arena: &mut TermArena,
        level: Level,
        ty: TermHandle,
        evidence: TermHandle,
    ) -> TermHandle {
        self.unspined(arena, QUOTIENT_PROP_IS_SET, level, &[ty, evidence])
    }

    /// `boxProp S x y : Id (Box S) x y` — a boxed strict proposition is
    /// a mere proposition. `level` is the strict `S`'s level.
    pub fn box_prop(
        &self,
        arena: &mut TermArena,
        level: Level,
        strict: TermHandle,
        left: TermHandle,
        right: TermHandle,
    ) -> TermHandle {
        self.unspined(arena, QUOTIENT_BOX_PROP, level, &[strict, left, right])
    }

    /// `indProp A R P d z : P z` — proposition-valued induction for
    /// `P : Π(_ : Q A R). Strict s` and `case : Π(a : A). P (project
    /// a)`. `strict_level` is the motive's `s`; it is appended after
    /// the description's `[u, v]` in the constant's instantiation.
    pub fn ind_prop(
        &self,
        arena: &mut TermArena,
        strict_level: Level,
        family: TermHandle,
        case: TermHandle,
        element: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            QUOTIENT_IND_PROP,
            vec![strict_level],
            &[family, case, element],
        )
    }

    /// `coverage A R z : Squash (Σ(a : A). Id (Q A R) (project a) z)` —
    /// every quotient element is squashed-covered by a projection.
    pub fn coverage(&self, arena: &mut TermArena, element: TermHandle) -> TermHandle {
        self.spine(arena, QUOTIENT_COVERAGE, Vec::new(), &[element])
    }

    /// `unique A R P sets s₁ s₂ h z : Id (P z) (s₁ z) (s₂ z)` —
    /// pointwise uniqueness of sections `s₁, s₂ : Π(z : Q A R). P z`
    /// agreeing on projections through `agreement`.
    /// `motive_level` is the family's `w`.
    pub fn unique(
        &self,
        arena: &mut TermArena,
        motive_level: Level,
        family: TermHandle,
        sets: TermHandle,
        first: TermHandle,
        second: TermHandle,
        agreement: TermHandle,
        element: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            QUOTIENT_UNIQUE,
            vec![motive_level],
            &[family, sets, first, second, agreement, element],
        )
    }
}

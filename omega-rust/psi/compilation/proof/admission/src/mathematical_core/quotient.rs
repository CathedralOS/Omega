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
    Syntax, app, apps, box_elim, box_intro, boxed, build, id, jelim, lam, pair, pi, refl,
    scheme_at, sigma, sort, squash, squash_intro, strict, ty, v,
};
use super::signature::Declaration;
use super::term::{Level, Term, TermArena, TermHandle};

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

/// `isSet` — declaration 0, a definition:
/// `λ(X : Type s). Π(x y : X). Π(p q : Id X x y). Id (Id X x y) p q`.
fn is_set_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi("X", ty(0), ty(0));
    let body = lam(
        "X",
        ty(0),
        pi(
            "x",
            v("X"),
            pi(
                "y",
                v("X"),
                pi(
                    "p",
                    id(v("X"), v("x"), v("y")),
                    pi(
                        "q",
                        id(v("X"), v("x"), v("y")),
                        id(id(v("X"), v("x"), v("y")), v("p"), v("q")),
                    ),
                ),
            ),
        ),
    );
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(1, statement, body)
}

/// `Q` — declaration 1, an assumption:
/// `Π(A : Type u). Π(R : A → A → Type v). Type max(u,v)`. The opaque
/// carrier carries no destructuring or recursor by declaration.
fn quotient_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(sort(carrier_level()));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(2, statement)
}

/// `project` — declaration 2, an assumption:
/// `Π A R. Π(_ : A). Q A R`.
fn project_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi("_", v("A"), quotient_of()));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(2, statement)
}

/// `setQ` — declaration 3, an assumption: `Π A R. isSet (Q A R)` —
/// the quotient is a set, propositionally.
fn set_quotient_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(is_set(carrier_level(), quotient_of()));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(2, statement)
}

/// `sound` — declaration 4, an assumption:
/// `Π A R. Π(a b : A). Π(_ : R a b). Id (Q A R) (project a) (project b)`.
fn sound_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "a",
        v("A"),
        pi(
            "b",
            v("A"),
            pi(
                "_",
                related(v("a"), v("b")),
                id(quotient_of(), project_of(v("a")), project_of(v("b"))),
            ),
        ),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(2, statement)
}

/// `effective` — declaration 5, an assumption:
/// `Π A R. Π(a b : A). Π(_ : Id (Q A R) (project a) (project b)). R a b`.
/// Effectivity supplies evidence of exactly the selected relation —
/// never an extraction of the representative.
fn effective_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "a",
        v("A"),
        pi(
            "b",
            v("A"),
            pi(
                "_",
                id(quotient_of(), project_of(v("a")), project_of(v("b"))),
                related(v("a"), v("b")),
            ),
        ),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(2, statement)
}

/// `transport` — declaration 6, a *definition*: forward premise
/// transport across the selected relation,
/// `Π A R. Π(P : Q → Type w). Π(a b : A). Π(_ : R a b).
///  Π(_ : P (project a)). P (project b)`,
/// defined as `J(λy. λ(_ : Id Q (project a) y). P y, h, project b,
/// sound a b r)` — the transport `elim`'s coherence law names, and the
/// optional precondition-transport surface for premise evidence.
fn transport_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "P",
        motive_type(),
        pi(
            "a",
            v("A"),
            pi(
                "b",
                v("A"),
                pi(
                    "_",
                    related(v("a"), v("b")),
                    pi(
                        "_",
                        app(v("P"), project_of(v("a"))),
                        app(v("P"), project_of(v("b"))),
                    ),
                ),
            ),
        ),
    ));
    // `C y p = P y` over `p : Id Q (project a) y`.
    let motive = lam(
        "y",
        quotient_of(),
        lam(
            "_",
            id(quotient_of(), project_of(v("a")), v("y")),
            app(v("P"), v("y")),
        ),
    );
    let body = lam_description(lam(
        "P",
        motive_type(),
        lam(
            "a",
            v("A"),
            lam(
                "b",
                v("A"),
                lam(
                    "r",
                    related(v("a"), v("b")),
                    lam(
                        "h",
                        app(v("P"), project_of(v("a"))),
                        jelim(
                            motive,
                            v("h"),
                            project_of(v("b")),
                            sound_of(v("a"), v("b"), v("r")),
                        ),
                    ),
                ),
            ),
        ),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// The `sets`/`d`/`c` telescope `elim` and `beta` share, under the
/// bound description and `P`:
/// `Π(sets : Π(z : Q). isSet (P z)). Π(d : Π(a : A). P (project a)).
///  Π(c : Π(a b : A). Π(r : R a b).
///      Id (P (project b)) (transport P a b r (d a)) (d b)). rest`.
fn pi_cases(rest: Syntax) -> Syntax {
    pi(
        "sets",
        pi(
            "z",
            quotient_of(),
            is_set(Level::Parameter(2), app(v("P"), v("z"))),
        ),
        pi(
            "d",
            pi("a", v("A"), app(v("P"), project_of(v("a")))),
            pi(
                "c",
                pi(
                    "a",
                    v("A"),
                    pi(
                        "b",
                        v("A"),
                        pi(
                            "r",
                            related(v("a"), v("b")),
                            id(
                                app(v("P"), project_of(v("b"))),
                                apps(
                                    motive_scheme(QUOTIENT_TRANSPORT),
                                    [
                                        v("A"),
                                        v("R"),
                                        v("P"),
                                        v("a"),
                                        v("b"),
                                        v("r"),
                                        app(v("d"), v("a")),
                                    ],
                                ),
                                app(v("d"), v("b")),
                            ),
                        ),
                    ),
                ),
                rest,
            ),
        ),
    )
}

/// `elim` — declaration 7, an assumption: dependent elimination into a
/// set-valued family, `Π A R. Π(P : Q → Type w). Π sets d c. Π(z : Q).
/// P z`. The eliminator does not target arbitrary higher types; its
/// coherence is `c`, and with set-valued fibers comparisons between
/// parallel transport proofs are propositionally unique.
fn elim_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "P",
        motive_type(),
        pi_cases(pi("z", quotient_of(), app(v("P"), v("z")))),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(3, statement)
}

/// `beta` — declaration 8, an assumption: the point computation
/// identity `Π A R P sets d c. Π(a : A). Id (P (project a)) (elim P sets
/// d c (project a)) (d a)`. A relevant `Id`, never a conversion rule —
/// `elim` at `project a` is a neutral term that does not reduce.
fn beta_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "P",
        motive_type(),
        pi_cases(pi(
            "a",
            v("A"),
            id(
                app(v("P"), project_of(v("a"))),
                apps(
                    motive_scheme(QUOTIENT_ELIM),
                    [
                        v("A"),
                        v("R"),
                        v("P"),
                        v("sets"),
                        v("d"),
                        v("c"),
                        project_of(v("a")),
                    ],
                ),
                app(v("d"), v("a")),
            ),
        )),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(3, statement)
}

/// `idTrans` — declaration 9, a *definition*: transitivity of `Id`,
/// `Π(B : Type w). Π(x y z : B). Id B x y → Id B y z → Id B x z`,
/// defined as `J(λz'. λ(_ : Id B y z'). Id B x z', p, z, q)` — eliminate
/// `q`; the fixed endpoint is `y` and the base is `p`.
fn id_trans_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi(
        "B",
        ty(0),
        pi(
            "x",
            v("B"),
            pi(
                "y",
                v("B"),
                pi(
                    "z",
                    v("B"),
                    pi(
                        "_",
                        id(v("B"), v("x"), v("y")),
                        pi("_", id(v("B"), v("y"), v("z")), id(v("B"), v("x"), v("z"))),
                    ),
                ),
            ),
        ),
    );
    // `C z' q = Id B x z'` over `q : Id B y z'`.
    let motive = lam(
        "z2",
        v("B"),
        lam(
            "_",
            id(v("B"), v("y"), v("z2")),
            id(v("B"), v("x"), v("z2")),
        ),
    );
    let body = lam(
        "B",
        ty(0),
        lam(
            "x",
            v("B"),
            lam(
                "y",
                v("B"),
                lam(
                    "z",
                    v("B"),
                    lam(
                        "p",
                        id(v("B"), v("x"), v("y")),
                        lam(
                            "q",
                            id(v("B"), v("y"), v("z")),
                            jelim(motive, v("p"), v("z"), v("q")),
                        ),
                    ),
                ),
            ),
        ),
    );
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(1, statement, body)
}

/// `transportConst` — declaration 10, a *definition*: transport across a
/// constant family is propositionally the identity,
/// `Π(A : Type u). Π(B : Type w). Π(x y : A). Π(p : Id A x y). Π(b : B).
///  Id B (J(λy'. λ(_ : Id A x y'). B, b, y, p)) b`,
/// defined by `J` on `p` whose base at `refl` reduces the inner `J` to
/// `b`, closing with `refl B b`.
fn transport_const_declaration(arena: &mut TermArena) -> Declaration {
    // `C₀ y' q = B` — the constant motive of the inner transport.
    let constant_motive = || lam("y2", v("A"), lam("_", id(v("A"), v("x"), v("y2")), v("B")));
    let inner = jelim(constant_motive(), v("b"), v("y"), v("p"));
    let statement = pi(
        "A",
        ty(0),
        pi(
            "B",
            ty(1),
            pi(
                "x",
                v("A"),
                pi(
                    "y",
                    v("A"),
                    pi(
                        "p",
                        id(v("A"), v("x"), v("y")),
                        pi("b", v("B"), id(v("B"), inner, v("b"))),
                    ),
                ),
            ),
        ),
    );
    // `C' y' p' = Id B (J(C₀, b, y', p')) b` — at `refl` the inner
    // elimination reduces to `b`, so `refl B b` closes the base.
    let outer_motive = lam(
        "y2",
        v("A"),
        lam(
            "p2",
            id(v("A"), v("x"), v("y2")),
            id(
                v("B"),
                jelim(constant_motive(), v("b"), v("y2"), v("p2")),
                v("b"),
            ),
        ),
    );
    let body = lam(
        "A",
        ty(0),
        lam(
            "B",
            ty(1),
            lam(
                "x",
                v("A"),
                lam(
                    "y",
                    v("A"),
                    lam(
                        "p",
                        id(v("A"), v("x"), v("y")),
                        lam(
                            "b",
                            v("B"),
                            jelim(outer_motive, refl(v("B"), v("b")), v("y"), v("p")),
                        ),
                    ),
                ),
            ),
        ),
    );
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(2, statement, body)
}

/// `lift` — declaration 11, a *definition*: the quotient-owned
/// operation shape. For `B : Type w` a set, an explicit representative
/// operation `f : A → B` and an explicit congruence theorem `respect :
/// Π(a b : A). R a b → Id B (f a) (f b)`, `lift` is the checked section
/// `Π(z : Q). B`:
///
/// ```text
/// lift A R B setB f respect
///   := elim A R (λ_. B) (λ_. setB) f
///        (λa. λb. λr. idTrans B (transport (λ_. B) a b r (f a)) (f a) (f b)
///           (transportConst (Q A R) B (project a) (project b) (sound a b r) (f a))
///           (respect a b r))
/// ```
///
/// The coherence obligation of `elim`'s `c` at the constant family asks
/// `Id B (transport (λ_. B) a b r (f a)) (f b)`; `transportConst`
/// reduces the transported value to `f a` propositionally and
/// `respect` is the explicit congruence step — composed by `idTrans`.
/// The theorem is an argument: no search, no implicit selection.
fn lift_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "B",
        ty(2),
        pi(
            "_",
            is_set(Level::Parameter(2), v("B")),
            pi(
                "f",
                pi("_", v("A"), v("B")),
                pi(
                    "respect",
                    pi(
                        "a",
                        v("A"),
                        pi(
                            "b",
                            v("A"),
                            pi(
                                "_",
                                related(v("a"), v("b")),
                                id(v("B"), app(v("f"), v("a")), app(v("f"), v("b"))),
                            ),
                        ),
                    ),
                    pi("_", quotient_of(), v("B")),
                ),
            ),
        ),
    ));
    // `P' = λ_. B` — the constant set-valued family.
    let constant_family = || lam("_", quotient_of(), v("B"));
    // `sound A R a b r` under the `a b r` binders.
    let soundness = sound_of(v("a"), v("b"), v("r"));
    // `transportConst (Q A R) B (project a) (project b) (sound a b r)
    //  (f a)` — `transportConst`'s two levels are the ambient
    // `max(u, v)` of `Q A R` and the target `w` of `B`.
    let transported_case = apps(
        scheme_at(
            QUOTIENT_TRANSPORT_CONST,
            vec![carrier_level(), Level::Parameter(2)],
        ),
        [
            quotient_of(),
            v("B"),
            project_of(v("a")),
            project_of(v("b")),
            soundness,
            app(v("f"), v("a")),
        ],
    );
    // `transport (λ_. B) a b r (f a)` — the transported left endpoint,
    // spelled exactly as `elim`'s `c` obligation instantiates it.
    let transported = apps(
        motive_scheme(QUOTIENT_TRANSPORT),
        [
            v("A"),
            v("R"),
            constant_family(),
            v("a"),
            v("b"),
            v("r"),
            app(v("f"), v("a")),
        ],
    );
    // `idTrans B transported (f a) (f b) (transportConst …) (respect a b r)`.
    let coherence = lam(
        "a",
        v("A"),
        lam(
            "b",
            v("A"),
            lam(
                "r",
                related(v("a"), v("b")),
                apps(
                    scheme_at(QUOTIENT_ID_TRANS, vec![Level::Parameter(2)]),
                    [
                        v("B"),
                        transported,
                        app(v("f"), v("a")),
                        app(v("f"), v("b")),
                        transported_case,
                        apps(v("respect"), [v("a"), v("b"), v("r")]),
                    ],
                ),
            ),
        ),
    );
    let body = lam_description(lam(
        "B",
        ty(2),
        lam(
            "setB",
            is_set(Level::Parameter(2), v("B")),
            lam(
                "f",
                pi("_", v("A"), v("B")),
                lam(
                    "respect",
                    pi(
                        "a",
                        v("A"),
                        pi(
                            "b",
                            v("A"),
                            pi(
                                "_",
                                related(v("a"), v("b")),
                                id(v("B"), app(v("f"), v("a")), app(v("f"), v("b"))),
                            ),
                        ),
                    ),
                    lam(
                        "z",
                        quotient_of(),
                        apps(
                            motive_scheme(QUOTIENT_ELIM),
                            [
                                v("A"),
                                v("R"),
                                constant_family(),
                                lam("_", quotient_of(), v("setB")),
                                v("f"),
                                coherence,
                                v("z"),
                            ],
                        ),
                    ),
                ),
            ),
        ),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// `liftPre` — declaration 12, an assumption: the optional
/// forward-precondition-transport operation shape,
///
/// ```text
/// Π A R. Π(B : Type w). Π(_ : isSet B).
/// Π(Pub : Q → Type p). Π(Pre : A → Type q).
/// Π(t : Π(a : A). Pub (project a) → Pre a).            — forward transport
/// Π(f : Π(a : A). Pre a → B).                          — representative operation
/// Π(respect : Π(a b : A). R a b →
///     Π(ha : Pub (project a)). Π(hb : Pub (project b)).
///     Id B (f a (t a ha)) (f b (t b hb))).             — congruence under
///                                                        transported premises
/// Π(z : Q). Π(_ : Pub z). B
/// ```
///
/// The result's function-valued motive `λz. Pub z → B` is not `elim`-
/// derivable: its setness would need function extensionality, which this
/// calculus deliberately lacks. So the shape is admitted as a named
/// assumption with its exact statement — visible in every assumption
/// closure that references it — rather than silently postulated through
/// an extensionality law.
fn lift_precondition_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "B",
        ty(2),
        pi(
            "_",
            is_set(Level::Parameter(2), v("B")),
            pi(
                "Pub",
                pi("_", quotient_of(), ty(3)),
                pi(
                    "Pre",
                    pi("_", v("A"), ty(4)),
                    pi(
                        "t",
                        pi(
                            "a",
                            v("A"),
                            pi(
                                "_",
                                app(v("Pub"), project_of(v("a"))),
                                app(v("Pre"), v("a")),
                            ),
                        ),
                        pi(
                            "f",
                            pi("a", v("A"), pi("_", app(v("Pre"), v("a")), v("B"))),
                            pi(
                                "respect",
                                pi(
                                    "a",
                                    v("A"),
                                    pi(
                                        "b",
                                        v("A"),
                                        pi(
                                            "_",
                                            related(v("a"), v("b")),
                                            pi(
                                                "ha",
                                                app(v("Pub"), project_of(v("a"))),
                                                pi(
                                                    "hb",
                                                    app(v("Pub"), project_of(v("b"))),
                                                    id(
                                                        v("B"),
                                                        app(
                                                            app(v("f"), v("a")),
                                                            apps(v("t"), [v("a"), v("ha")]),
                                                        ),
                                                        app(
                                                            app(v("f"), v("b")),
                                                            apps(v("t"), [v("b"), v("hb")]),
                                                        ),
                                                    ),
                                                ),
                                            ),
                                        ),
                                    ),
                                ),
                                pi("z", quotient_of(), pi("_", app(v("Pub"), v("z")), v("B"))),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    Declaration::assumption(5, statement)
}

/// `idSym` — declaration 13, a *definition*: symmetry of `Id`,
/// `Π(X : Type s). Π(x y : X). Π(_ : Id X x y). Id X y x`,
/// defined as `J(λy'. λ(_ : Id X x y'). Id X y' x, refl X x, y, p)`.
fn id_sym_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi(
        "X",
        ty(0),
        pi(
            "x",
            v("X"),
            pi(
                "y",
                v("X"),
                pi("_", id(v("X"), v("x"), v("y")), id(v("X"), v("y"), v("x"))),
            ),
        ),
    );
    // `C y' p' = Id X y' x` over `p' : Id X x y'`.
    let motive = lam(
        "y2",
        v("X"),
        lam(
            "_",
            id(v("X"), v("x"), v("y2")),
            id(v("X"), v("y2"), v("x")),
        ),
    );
    let body = lam(
        "X",
        ty(0),
        lam(
            "x",
            v("X"),
            lam(
                "y",
                v("X"),
                lam(
                    "p",
                    id(v("X"), v("x"), v("y")),
                    jelim(motive, refl(v("X"), v("x")), v("y"), v("p")),
                ),
            ),
        ),
    );
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(1, statement, body)
}

/// `trans (sym p) p` over `p : Id X x e` — the inverse-law left side,
/// spelled `idTrans X e x e (idSym X x e p) p`, parameterized by the
/// endpoint/proof binder names so the statement and motive share it.
fn cancelled(endpoint: &'static str, proof: &'static str) -> Syntax {
    let level = vec![Level::Parameter(0)];
    apps(
        scheme_at(QUOTIENT_ID_TRANS, level.clone()),
        [
            v("X"),
            v(endpoint),
            v("x"),
            v(endpoint),
            apps(
                scheme_at(QUOTIENT_ID_SYM, level),
                [v("X"), v("x"), v(endpoint), v(proof)],
            ),
            v(proof),
        ],
    )
}

/// `idCancel` — declaration 14, a *definition*: the inverse law
/// `Π(X : Type s). Π(x y : X). Π(p : Id X x y).
///  Id (Id X y y) (trans (sym p) p) (refl X y)`, proved by `J` on `p`:
/// at the `refl` base `trans (sym refl) refl` reduces to `refl`, so
/// `refl (Id X x x) (refl X x)` closes it.
fn id_cancel_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi(
        "X",
        ty(0),
        pi(
            "x",
            v("X"),
            pi(
                "y",
                v("X"),
                pi(
                    "p",
                    id(v("X"), v("x"), v("y")),
                    id(
                        id(v("X"), v("y"), v("y")),
                        cancelled("y", "p"),
                        refl(v("X"), v("y")),
                    ),
                ),
            ),
        ),
    );
    // `C y' p' = Id (Id X y' y') (trans (sym p') p') (refl X y')`.
    let motive = lam(
        "y2",
        v("X"),
        lam(
            "p2",
            id(v("X"), v("x"), v("y2")),
            id(
                id(v("X"), v("y2"), v("y2")),
                cancelled("y2", "p2"),
                refl(v("X"), v("y2")),
            ),
        ),
    );
    let body = lam(
        "X",
        ty(0),
        lam(
            "x",
            v("X"),
            lam(
                "y",
                v("X"),
                lam(
                    "p",
                    id(v("X"), v("x"), v("y")),
                    jelim(
                        motive,
                        refl(id(v("X"), v("x"), v("x")), refl(v("X"), v("x"))),
                        v("y"),
                        v("p"),
                    ),
                ),
            ),
        ),
    );
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(1, statement, body)
}

/// `propIsSet` — declaration 15, a *definition*: a mere proposition is
/// a set,
/// `Π(X : Type s). Π(_ : Π(x y : X). Id X x y). isSet[s] X`.
///
/// The proof is the standard `J` development: with
/// `f : Π(x y : X). Id X x y`, every `p : Id X x y` equals the pinned
/// normal form `M(y) := trans (sym (f x x)) (f x y)` — shown by `J` on
/// `p`, whose base `refl = M(x)` is `idCancel` at `f x x` read backward.
/// Parallel `p q : Id X x y` then share `M(y)`, so `trans` of the two
/// normal forms (one symmetrized) closes `Id p q`.
fn prop_is_set_declaration(arena: &mut TermArena) -> Declaration {
    let level = || vec![Level::Parameter(0)];
    // `f x e` under the `f` hypothesis — the mere-proposition evidence
    // applied at the fixed `x` and the endpoint named by `endpoint`.
    let prop = |endpoint: &'static str| apps(v("f"), [v("x"), v(endpoint)]);
    // `M(e) = trans (sym (f x x)) (f x e)` — the normal form every
    // proof of `Id X x e` is propositionally equal to.
    let normal = |endpoint: &'static str| {
        apps(
            scheme_at(QUOTIENT_ID_TRANS, level()),
            [
                v("X"),
                v("x"),
                v("x"),
                v(endpoint),
                apps(
                    scheme_at(QUOTIENT_ID_SYM, level()),
                    [v("X"), v("x"), v("x"), prop("x")],
                ),
                prop(endpoint),
            ],
        )
    };
    // `C y' p' = Id (Id X x y') p' (M y')` over `p' : Id X x y'`.
    let path_motive = lam(
        "y2",
        v("X"),
        lam(
            "p2",
            id(v("X"), v("x"), v("y2")),
            id(id(v("X"), v("x"), v("y2")), v("p2"), normal("y2")),
        ),
    );
    // `refl = M(x)` — `idCancel (f x x) : M(x) = refl`, symmetrized at
    // the identity-type carrier `Id X x x`.
    let path_base = apps(
        scheme_at(QUOTIENT_ID_SYM, level()),
        [
            id(v("X"), v("x"), v("x")),
            normal("x"),
            refl(v("X"), v("x")),
            apps(
                scheme_at(QUOTIENT_ID_CANCEL, level()),
                [v("X"), v("x"), v("x"), prop("x")],
            ),
        ],
    );
    // `path t = J(C, base, y, t) : Id t (M y)` for `t : Id X x y`.
    let path =
        |proof: &'static str| jelim(path_motive.clone(), path_base.clone(), v("y"), v(proof));
    let is_prop = pi("x2", v("X"), pi("y2", v("X"), id(v("X"), v("x2"), v("y2"))));
    let statement = pi(
        "X",
        ty(0),
        pi("f", is_prop.clone(), is_set(Level::Parameter(0), v("X"))),
    );
    let identity_xy = id(v("X"), v("x"), v("y"));
    let body = lam(
        "X",
        ty(0),
        lam(
            "f",
            is_prop,
            lam(
                "x",
                v("X"),
                lam(
                    "y",
                    v("X"),
                    lam(
                        "p",
                        identity_xy.clone(),
                        lam(
                            "q",
                            identity_xy.clone(),
                            apps(
                                scheme_at(QUOTIENT_ID_TRANS, level()),
                                [
                                    identity_xy.clone(),
                                    v("p"),
                                    normal("y"),
                                    v("q"),
                                    path("p"),
                                    apps(
                                        scheme_at(QUOTIENT_ID_SYM, level()),
                                        [identity_xy.clone(), v("q"), normal("y"), path("q")],
                                    ),
                                ],
                            ),
                        ),
                    ),
                ),
            ),
        ),
    );
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(1, statement, body)
}

/// `boxProp` — declaration 16, a *definition*: a boxed strict
/// proposition is a mere proposition,
/// `Π(S : Strict s). Π(x y : Box S). Id (Box S) x y`.
///
/// Two `unbox` rounds expose the payloads: `x`'s case fixes `a : S`,
/// then `y`'s case fixes `b : S`, and `refl (Box S) (box a)` closes
/// `Id (box a) (box b)` because `a ≡ b : S` holds definitionally by
/// strict proof irrelevance.
fn box_prop_declaration(arena: &mut TermArena) -> Declaration {
    let boxed_s = || boxed(v("S"));
    // `P' y'' = Id (Box S) (box a) y''` — the inner unbox motive under
    // the `a : S` case.
    let inner_motive = lam(
        "y3",
        boxed_s(),
        id(boxed_s(), box_intro(v("S"), v("a")), v("y3")),
    );
    // `P'' x' = Π(y' : Box S). Id (Box S) x' y'` — the outer unbox
    // motive: each boxed `x` proves every boxed `y` identical to it.
    let outer_motive = lam(
        "x2",
        boxed_s(),
        pi("y2", boxed_s(), id(boxed_s(), v("x2"), v("y2"))),
    );
    // `g a y' = unbox P' (λb. refl (box a)) y' : Id (box a) y'`.
    let case = lam(
        "a",
        v("S"),
        lam(
            "y2",
            boxed_s(),
            box_elim(
                inner_motive,
                lam("b", v("S"), refl(boxed_s(), box_intro(v("S"), v("a")))),
                v("y2"),
            ),
        ),
    );
    let statement = pi(
        "S",
        strict(0),
        pi(
            "x",
            boxed_s(),
            pi("y", boxed_s(), id(boxed_s(), v("x"), v("y"))),
        ),
    );
    let body = lam(
        "S",
        strict(0),
        lam(
            "x",
            boxed_s(),
            lam(
                "y",
                boxed_s(),
                app(box_elim(outer_motive, case, v("x")), v("y")),
            ),
        ),
    );
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(1, statement, body)
}

/// `indProp` — declaration 17, a *definition*: proposition-valued
/// induction,
/// `Π A R. Π(P : Π(_ : Q A R). Strict s). Π(_ : Π(a : A). P (project
/// a)). Π(z : Q A R). P z`.
///
/// Derived through `elim` at the boxed motive `P' = λz. Box (P z)`:
/// `P'` is a set because `boxProp` makes it a mere proposition and
/// `propIsSet` lifts that to setness; the representative case boxes `d
/// a`; the coherence obligation between two inhabitants of `Box (P
/// (project b))` is `boxProp` again; and `unbox` turns the resulting
/// `Box (P z)` back into the strict proposition `P z`. This is exactly
/// the specification's "set-valued elimination and boxing when needed"
/// route — no new elimination rule.
fn ind_prop_declaration(arena: &mut TermArena) -> Declaration {
    let strict_level = || vec![Level::Parameter(2)];
    // `P' = λ(z' : Q). Box (P z')` — the boxed relevant family.
    let boxed_family = lam("z2", quotient_of(), boxed(app(v("P"), v("z2"))));
    // `sets' = λz'. propIsSet (Box (P z')) (boxProp (P z'))`.
    let sets = lam(
        "z3",
        quotient_of(),
        apps(
            scheme_at(QUOTIENT_PROP_IS_SET, strict_level()),
            [
                boxed(app(v("P"), v("z3"))),
                apps(
                    scheme_at(QUOTIENT_BOX_PROP, strict_level()),
                    [app(v("P"), v("z3"))],
                ),
            ],
        ),
    );
    // `d' = λa. box (d a)`.
    let case = lam(
        "a",
        v("A"),
        box_intro(app(v("P"), project_of(v("a"))), app(v("d"), v("a"))),
    );
    // `c' a b r = boxProp (P (project b)) (transport P' a b r (d' a))
    //  (d' b)` — the coherence collapses because `Box (P _)` is a mere
    // proposition.
    let coherence = lam(
        "a",
        v("A"),
        lam(
            "b",
            v("A"),
            lam(
                "r",
                related(v("a"), v("b")),
                apps(
                    scheme_at(QUOTIENT_BOX_PROP, strict_level()),
                    [
                        app(v("P"), project_of(v("b"))),
                        apps(
                            motive_scheme(QUOTIENT_TRANSPORT),
                            [
                                v("A"),
                                v("R"),
                                boxed_family.clone(),
                                v("a"),
                                v("b"),
                                v("r"),
                                app(case.clone(), v("a")),
                            ],
                        ),
                        app(case.clone(), v("b")),
                    ],
                ),
            ),
        ),
    );
    // `unbox (λ(_ : Box (P z)). P z) (λ(t : P z). t) (elim P' sets' d'
    // c' z)` — the boxed section delivered by `elim` is unboxed into
    // the strict target.
    let section = apps(
        motive_scheme(QUOTIENT_ELIM),
        [v("A"), v("R"), boxed_family, sets, case, coherence, v("z")],
    );
    let unboxed = box_elim(
        lam("_", boxed(app(v("P"), v("z"))), app(v("P"), v("z"))),
        lam("t", app(v("P"), v("z")), v("t")),
        section,
    );
    let family_type = pi("_", quotient_of(), strict(2));
    let case_type = pi("a", v("A"), app(v("P"), project_of(v("a"))));
    let statement = pi_description(pi(
        "P",
        family_type.clone(),
        pi(
            "d",
            case_type.clone(),
            pi("z", quotient_of(), app(v("P"), v("z"))),
        ),
    ));
    let body = lam_description(lam(
        "P",
        family_type,
        lam("d", case_type, lam("z", quotient_of(), unboxed)),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// `coverage` — declaration 18, a *definition*: the specification's
/// coverage law, `Π A R. Π(z : Q A R). Squash (Σ(a : A). Id (Q A R)
/// (project a) z)` — every quotient element is squashed-covered by a
/// projection. One `indProp` application at the squashed family; it
/// supplies no representative-extraction function.
fn coverage_declaration(arena: &mut TermArena) -> Declaration {
    // `Cov z' = Squash (Σ(a : A). Id Q (project a) z')`.
    let covered = |anchor: Syntax| {
        squash(sigma(
            "a2",
            v("A"),
            id(quotient_of(), project_of(v("a2")), anchor),
        ))
    };
    let statement = pi_description(pi("z", quotient_of(), covered(v("z"))));
    // `d_cov a = sq ⟨a, refl (project a)⟩`.
    let case = lam(
        "a",
        v("A"),
        squash_intro(
            sigma(
                "a2",
                v("A"),
                id(quotient_of(), project_of(v("a2")), project_of(v("a"))),
            ),
            pair(v("a"), refl(quotient_of(), project_of(v("a")))),
        ),
    );
    // `P_cov = λ(z' : Q). Cov z'` — the strict motive `indProp`
    // instantiates at `s := max(u, v)`, the Σ's level.
    let motive = lam("z2", quotient_of(), covered(v("z2")));
    let body = lam_description(lam(
        "z",
        quotient_of(),
        apps(
            scheme_at(
                QUOTIENT_IND_PROP,
                vec![Level::Parameter(0), Level::Parameter(1), carrier_level()],
            ),
            [v("A"), v("R"), motive, case, v("z")],
        ),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(2, statement, body)
}

/// `unique` — declaration 19, a *definition*: pointwise uniqueness of
/// sections agreeing on projections,
///
/// ```text
/// Π A R. Π(P : Π(_ : Q A R). Type w). Π(sets : Π(z : Q A R). isSet (P z)).
/// Π(s₁ s₂ : Π(z : Q A R). P z).
/// Π(_ : Π(a : A). Id (P (project a)) (s₁ (project a)) (s₂ (project a))).
/// Π(z : Q A R). Id (P z) (s₁ z) (s₂ z)
/// ```
///
/// Derived by `elim` at the identity family `M = λz. Id (P z) (s₁ z)
/// (s₂ z)`: `M z` is a proposition because `sets z` supplies exactly
/// that, and `propIsSet` lifts it to the setness `elim` requires; the
/// coherence `c` between two proofs of `M (project b)` collapses by the
/// same propositional setness — no transport lemma is needed, since
/// both endpoints already inhabit a proposition.
fn unique_declaration(arena: &mut TermArena) -> Declaration {
    // `M = λ(z' : Q). Id (P z') (s₁ z') (s₂ z')`.
    let identity_family = lam(
        "z2",
        quotient_of(),
        id(
            app(v("P"), v("z2")),
            app(v("s1"), v("z2")),
            app(v("s2"), v("z2")),
        ),
    );
    // `sets' = λz'. propIsSet (M z') (sets z' (s₁ z') (s₂ z'))` — the
    // family's own setness, since `sets z'` makes `M z'` a proposition.
    let sets = lam(
        "z3",
        quotient_of(),
        apps(
            scheme_at(QUOTIENT_PROP_IS_SET, vec![Level::Parameter(2)]),
            [
                id(
                    app(v("P"), v("z3")),
                    app(v("s1"), v("z3")),
                    app(v("s2"), v("z3")),
                ),
                apps(
                    v("sets"),
                    [v("z3"), app(v("s1"), v("z3")), app(v("s2"), v("z3"))],
                ),
            ],
        ),
    );
    // `c a b r = sets (project b) (s₁ _) (s₂ _) (transport M a b r (h
    // a)) (h b)` — coherence by propositional collapse of `M (project
    // b)`.
    let coherence = lam(
        "a",
        v("A"),
        lam(
            "b",
            v("A"),
            lam(
                "r",
                related(v("a"), v("b")),
                apps(
                    v("sets"),
                    [
                        project_of(v("b")),
                        app(v("s1"), project_of(v("b"))),
                        app(v("s2"), project_of(v("b"))),
                        apps(
                            motive_scheme(QUOTIENT_TRANSPORT),
                            [
                                v("A"),
                                v("R"),
                                identity_family.clone(),
                                v("a"),
                                v("b"),
                                v("r"),
                                app(v("h"), v("a")),
                            ],
                        ),
                        app(v("h"), v("b")),
                    ],
                ),
            ),
        ),
    );
    let family_type = motive_type();
    let sets_type = pi(
        "z",
        quotient_of(),
        is_set(Level::Parameter(2), app(v("P"), v("z"))),
    );
    let section_type = pi("z", quotient_of(), app(v("P"), v("z")));
    let agreement_type = pi(
        "a",
        v("A"),
        id(
            app(v("P"), project_of(v("a"))),
            app(v("s1"), project_of(v("a"))),
            app(v("s2"), project_of(v("a"))),
        ),
    );
    let statement = pi_description(pi(
        "P",
        family_type.clone(),
        pi(
            "sets",
            sets_type.clone(),
            pi(
                "s1",
                section_type.clone(),
                pi(
                    "s2",
                    section_type.clone(),
                    pi(
                        "h",
                        agreement_type.clone(),
                        pi(
                            "z",
                            quotient_of(),
                            id(
                                app(v("P"), v("z")),
                                app(v("s1"), v("z")),
                                app(v("s2"), v("z")),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    ));
    let body = lam_description(lam(
        "P",
        family_type,
        lam(
            "sets",
            sets_type,
            lam(
                "s1",
                section_type.clone(),
                lam(
                    "s2",
                    section_type,
                    lam(
                        "h",
                        agreement_type,
                        lam(
                            "z",
                            quotient_of(),
                            apps(
                                motive_scheme(QUOTIENT_ELIM),
                                [
                                    v("A"),
                                    v("R"),
                                    identity_family,
                                    sets,
                                    v("h"),
                                    coherence,
                                    v("z"),
                                ],
                            ),
                        ),
                    ),
                ),
            ),
        ),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// The twenty declarations of the set-quotient scheme, in signature
/// order: `isSet`, `Q`, `project`, `setQ`, `sound`, `effective`,
/// `transport`, `elim`, `beta`, `idTrans`, `transportConst`, `lift`,
/// `liftPre`, `idSym`, `idCancel`, `propIsSet`, `boxProp`, `indProp`,
/// `coverage` and `unique` occupy positions 0–19 and reference only
/// each other. Append them to the front of a producer signature (or
/// after any prefix the producer checks first — the contained
/// `Constant` positions are absolute), then re-decide the whole
/// signature with `check_signature`.
///
/// The carrier/operation/law declarations of the specification's
/// interface — `Q`, `project`, `setQ`, `sound`, `effective`, `elim`,
/// `beta` — plus the extensionality-blocked `liftPre` are *assumptions*:
/// admitted named axioms whose exact statements the kernel re-decides
/// and whose dependencies `assumption_closure` reports exactly.
/// `transport`, `idTrans`, `transportConst`, `lift`, `idSym`,
/// `idCancel`, `propIsSet`, `boxProp`, `indProp`, `coverage` and
/// `unique` are *definitions* whose bodies the same check re-decides —
/// every derivable item the specification's "Derive the following"
/// list names is a checked term, not a default axiom.
pub fn quotient_scheme(arena: &mut TermArena) -> Vec<Declaration> {
    vec![
        is_set_declaration(arena),
        quotient_declaration(arena),
        project_declaration(arena),
        set_quotient_declaration(arena),
        sound_declaration(arena),
        effective_declaration(arena),
        transport_declaration(arena),
        elim_declaration(arena),
        beta_declaration(arena),
        id_trans_declaration(arena),
        transport_const_declaration(arena),
        lift_declaration(arena),
        lift_precondition_declaration(arena),
        id_sym_declaration(arena),
        id_cancel_declaration(arena),
        prop_is_set_declaration(arena),
        box_prop_declaration(arena),
        ind_prop_declaration(arena),
        coverage_declaration(arena),
        unique_declaration(arena),
    ]
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

    /// `setQ A R : isSet (Q A R)` — the propositional setness law.
    pub fn set_quotient(&self, arena: &mut TermArena) -> TermHandle {
        self.spine(arena, QUOTIENT_SET, Vec::new(), &[])
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
        let mut term = arena.insert(Term::Constant {
            declaration: QUOTIENT_ID_TRANS,
            levels: vec![level],
        });
        for argument in [ty, left, middle, right, first, second] {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
    }

    /// `transportConst A B x y p b : Id B (J(λy'. λ_. B, b, y, p)) b` —
    /// transport across the constant family is propositionally the
    /// identity. `levels` are `[u, w]` — the ambient level of `A` and
    /// the target level of `B`.
    pub fn transport_const(
        &self,
        arena: &mut TermArena,
        levels: [Level; 2],
        ambient: TermHandle,
        target: TermHandle,
        left: TermHandle,
        right: TermHandle,
        proof: TermHandle,
        value: TermHandle,
    ) -> TermHandle {
        let mut term = arena.insert(Term::Constant {
            declaration: QUOTIENT_TRANSPORT_CONST,
            levels: levels.to_vec(),
        });
        for argument in [ambient, target, left, right, proof, value] {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
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
        let mut term = arena.insert(Term::Constant {
            declaration: QUOTIENT_ID_SYM,
            levels: vec![level],
        });
        for argument in [ty, left, right, proof] {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
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
        let mut term = arena.insert(Term::Constant {
            declaration: QUOTIENT_ID_CANCEL,
            levels: vec![level],
        });
        for argument in [ty, left, right, proof] {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
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
        let mut term = arena.insert(Term::Constant {
            declaration: QUOTIENT_PROP_IS_SET,
            levels: vec![level],
        });
        for argument in [ty, evidence] {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
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
        let mut term = arena.insert(Term::Constant {
            declaration: QUOTIENT_BOX_PROP,
            levels: vec![level],
        });
        for argument in [strict, left, right] {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
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

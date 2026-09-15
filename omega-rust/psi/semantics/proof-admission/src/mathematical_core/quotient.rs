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
//! Three declarations are *derived with checked terms*, as the
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
//! The scheme declarations are signature prefix `0..13` — they reference
//! only each other, so a producer's own declarations append after them.

use super::scheme_dsl::{
    Syntax, app, apps, build, id, jelim, lam, pi, refl, scheme_at, sort, ty, v,
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

/// The thirteen declarations of the set-quotient scheme, in signature
/// order: `isSet`, `Q`, `project`, `setQ`, `sound`, `effective`,
/// `transport`, `elim`, `beta`, `idTrans`, `transportConst`, `lift`,
/// `liftPre` occupy positions 0–12 and reference only each other.
/// Append them to the front of a producer signature (or after any
/// prefix the producer checks first — the contained `Constant`
/// positions are absolute), then re-decide the whole signature with
/// `check_signature`.
///
/// The carrier/operation/law declarations of the specification's
/// interface — `Q`, `project`, `setQ`, `sound`, `effective`, `elim`,
/// `beta` — plus the extensionality-blocked `liftPre` are *assumptions*:
/// admitted named axioms whose exact statements the kernel re-decides
/// and whose dependencies `assumption_closure` reports exactly.
/// `transport`, `idTrans`, `transportConst` and `lift` are *definitions*
/// whose bodies the same check re-decides — derived, not postulated.
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
}

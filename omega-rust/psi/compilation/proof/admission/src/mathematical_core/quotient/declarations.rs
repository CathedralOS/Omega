use super::*;

/// `isSet` — declaration 0, a definition:
/// `λ(X : Type s). Π(x y : X). Π(p q : Id X x y). Id (Id X x y) p q`.
fn is_set_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi("X", ty(0), ty(0));
    let body = lam(
        "X",
        ty(0),
        pis(
            [
                ("x", v("X")),
                ("y", v("X")),
                ("p", id(v("X"), v("x"), v("y"))),
                ("q", id(v("X"), v("x"), v("y"))),
            ],
            id(id(v("X"), v("x"), v("y")), v("p"), v("q")),
        ),
    );
    define(arena, 1, &statement, &body)
}

/// `Q` — declaration 1, an assumption:
/// `Π(A : Type u). Π(R : A → A → Type v). Type max(u,v)`. The opaque
/// carrier carries no destructuring or recursor by declaration.
fn quotient_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(sort(carrier_level()));
    assume(arena, 2, &statement)
}

/// `project` — declaration 2, an assumption:
/// `Π A R. Π(_ : A). Q A R`.
fn project_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi("_", v("A"), quotient_of()));
    assume(arena, 2, &statement)
}

/// `setQ` — declaration 3, an assumption: `Π A R. isSet (Q A R)` —
/// the quotient is a set, propositionally.
fn set_quotient_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(is_set(carrier_level(), quotient_of()));
    assume(arena, 2, &statement)
}

/// `sound` — declaration 4, an assumption:
/// `Π A R. Π(a b : A). Π(_ : R a b). Id (Q A R) (project a) (project b)`.
fn sound_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(related_pis(
        "_",
        id(quotient_of(), project_of(v("a")), project_of(v("b"))),
    ));
    assume(arena, 2, &statement)
}

/// `effective` — declaration 5, an assumption:
/// `Π A R. Π(a b : A). Π(_ : Id (Q A R) (project a) (project b)). R a b`.
/// Effectivity supplies evidence of exactly the selected relation —
/// never an extraction of the representative.
fn effective_declaration(arena: &mut TermArena) -> Declaration {
    // Effectivity's third binder is the projection identity `Id (Q A R)
    // (project a) (project b)`, not the relation evidence: it spells the
    // description independently of `related_pis`.
    let statement = pi_description(pis(
        [
            ("a", v("A")),
            ("b", v("A")),
            (
                "_",
                id(quotient_of(), project_of(v("a")), project_of(v("b"))),
            ),
        ],
        related(v("a"), v("b")),
    ));
    assume(arena, 2, &statement)
}

/// `transport` — declaration 6, a *definition*: forward premise
/// transport across the selected relation,
/// `Π A R. Π(P : Q → Type w). Π(a b : A). Π(_ : R a b).
///  Π(_ : P (project a)). P (project b)`,
/// defined as `J(λy. λ(_ : Id Q (project a) y). P y, h, project b,
/// sound a b r)` — the transport `elim`'s coherence law names, and the
/// optional precondition-transport surface for premise evidence.
fn transport_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pis(
        [("P", motive_type())],
        related_pis(
            "_",
            pi(
                "_",
                app(v("P"), project_of(v("a"))),
                app(v("P"), project_of(v("b"))),
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
        related_lams(lam(
            "h",
            app(v("P"), project_of(v("a"))),
            jelim(
                motive,
                v("h"),
                project_of(v("b")),
                sound_of(v("a"), v("b"), v("r")),
            ),
        )),
    ));
    define(arena, 3, &statement, &body)
}

/// The `sets`/`d`/`c` telescope `elim` and `beta` share, under the
/// bound description and `P`:
/// `Π(sets : Π(z : Q). isSet (P z)). Π(d : Π(a : A). P (project a)).
///  Π(c : Π(a b : A). Π(r : R a b).
///      Id (P (project b)) (transport P a b r (d a)) (d b)). rest`.
fn pi_cases(rest: Syntax) -> Syntax {
    pis(
        [
            ("P", motive_type()),
            (
                "sets",
                pi(
                    "z",
                    quotient_of(),
                    is_set(Level::Parameter(2), app(v("P"), v("z"))),
                ),
            ),
            ("d", pi("a", v("A"), app(v("P"), project_of(v("a"))))),
            (
                "c",
                related_pis(
                    "r",
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
        ],
        rest,
    )
}

/// `Π(a b : A). Π(evidence : R a b). rest` — the shape of a representative
/// congruence theorem every relation-stated declaration shares. `evidence`
/// is `"_"` when `rest` never names the relation proof.
fn related_pis(evidence: &'static str, rest: Syntax) -> Syntax {
    pis(
        [
            ("a", v("A")),
            ("b", v("A")),
            (evidence, related(v("a"), v("b"))),
        ],
        rest,
    )
}

/// `λ(a b : A). λ(r : R a b). rest` — the clause form every coherence a
/// derived declaration supplies is stated under.
fn related_lams(rest: Syntax) -> Syntax {
    lam(
        "a",
        v("A"),
        lam("b", v("A"), lam("r", related(v("a"), v("b")), rest)),
    )
}

/// `elim` — declaration 7, an assumption: dependent elimination into a
/// set-valued family, `Π A R. Π(P : Q → Type w). Π sets d c. Π(z : Q).
/// P z`. The eliminator does not target arbitrary higher types; its
/// coherence is `c`, and with set-valued fibers comparisons between
/// parallel transport proofs are propositionally unique.
fn elim_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi_cases(pi("z", quotient_of(), app(v("P"), v("z")))));
    assume(arena, 3, &statement)
}

/// `beta` — declaration 8, an assumption: the point computation
/// identity `Π A R P sets d c. Π(a : A). Id (P (project a)) (elim P sets
/// d c (project a)) (d a)`. A relevant `Id`, never a conversion rule —
/// `elim` at `project a` is a neutral term that does not reduce.
fn beta_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi_cases(pi(
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
    )));
    assume(arena, 3, &statement)
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
    define(arena, 1, &statement, &body)
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
    define(arena, 2, &statement, &body)
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
                    related_pis("_", id(v("B"), app(v("f"), v("a")), app(v("f"), v("b")))),
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
    let coherence = related_lams(apps(
        scheme_at(QUOTIENT_ID_TRANS, vec![Level::Parameter(2)]),
        [
            v("B"),
            transported,
            app(v("f"), v("a")),
            app(v("f"), v("b")),
            transported_case,
            apps(v("respect"), [v("a"), v("b"), v("r")]),
        ],
    ));
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
                    related_pis("_", id(v("B"), app(v("f"), v("a")), app(v("f"), v("b")))),
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
    define(arena, 3, &statement, &body)
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
                                related_pis(
                                    "_",
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
                                pi("z", quotient_of(), pi("_", app(v("Pub"), v("z")), v("B"))),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    ));
    assume(arena, 5, &statement)
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
    define(arena, 1, &statement, &body)
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
    define(arena, 1, &statement, &body)
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
    define(arena, 1, &statement, &body)
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
    define(arena, 1, &statement, &body)
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
    let coherence = related_lams(apps(
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
    ));
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
    define(arena, 3, &statement, &body)
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
    define(arena, 2, &statement, &body)
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
    let coherence = related_lams(apps(
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
    ));
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
    define(arena, 3, &statement, &body)
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

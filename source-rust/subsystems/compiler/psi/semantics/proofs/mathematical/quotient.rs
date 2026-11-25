use super::scheme_dsl::{
    Syntax, app, apps, assume, box_elim, box_intro, boxed, define, id, jelim, lam, pair, pi, pis,
    refl, scheme_at, sigma, sort, squash, squash_intro, strict, ty, v,
};
use super::signature::Declaration;
use super::term::{Level, Term, TermArena, TermHandle};

mod declarations {
    use super::*;

    macro_rules! pi {
        ($name:expr => $domain:expr => $codomain:expr) => {
            pi($name, $domain, $codomain)
        };
    }
    macro_rules! lam {
        ($name:expr => $domain:expr => $body:expr) => {
            lam($name, $domain, $body)
        };
    }
    macro_rules! id {
        ($ty:expr => $left:expr => $right:expr) => {
            id($ty, $left, $right)
        };
    }

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

    fn quotient_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(sort(carrier_level()));
        assume(arena, 2, &statement)
    }

    fn project_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(pi("_", v("A"), quotient_of()));
        assume(arena, 2, &statement)
    }

    fn set_quotient_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(is_set(carrier_level(), quotient_of()));
        assume(arena, 2, &statement)
    }

    fn sound_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(related_pis(
            "_",
            id(quotient_of(), project_of(v("a")), project_of(v("b"))),
        ));
        assume(arena, 2, &statement)
    }

    /// Effectivity is consistent only for a propositional equivalence
    /// relation (wiki/spec/proofs/quotients.md#set-quotient-foundation), so
    /// the law takes those proofs as explicit hypotheses. Without them, the
    /// constant relation `λ_ _. Id Two zero one` would let
    /// `effective(x, x, refl)` prove `Id Two zero one`. The quotient type
    /// itself stays indexed only by `A` and `R`, as the spec requires.
    fn effective_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(pis(
            [
                (
                    "propositional",
                    pis(
                        [
                            ("a", v("A")),
                            ("b", v("A")),
                            ("r", related(v("a"), v("b"))),
                            ("s", related(v("a"), v("b"))),
                        ],
                        id(related(v("a"), v("b")), v("r"), v("s")),
                    ),
                ),
                ("reflexive", pi("a", v("A"), related(v("a"), v("a")))),
                ("symmetric", related_pis("_", related(v("b"), v("a")))),
                (
                    "transitive",
                    pis(
                        [
                            ("a", v("A")),
                            ("b", v("A")),
                            ("c", v("A")),
                            ("_", related(v("a"), v("b"))),
                            ("_", related(v("b"), v("c"))),
                        ],
                        related(v("a"), v("c")),
                    ),
                ),
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

    fn transport_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(pis(
            [("P", motive_type())],
            related_pis(
                "_",
                pi!("_" => app(v("P"), project_of(v("a"))) => app(v("P"), project_of(v("b")))),
            ),
        ));
        let motive = lam(
            "y",
            quotient_of(),
            lam!("_" => id(quotient_of(), project_of(v("a")), v("y")) => app(v("P"), v("y"))),
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

    fn pi_cases(rest: Syntax) -> Syntax {
        pis(
            [
                ("P", motive_type()),
                (
                    "sets",
                    pi!("z" => quotient_of() => is_set(Level::Parameter(2), app(v("P"), v("z")))),
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

    fn related_lams(rest: Syntax) -> Syntax {
        lam!("a" => v("A") => lam("b", v("A"), lam("r", related(v("a"), v("b")), rest)))
    }

    fn elim_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi_description(pi_cases(pi("z", quotient_of(), app(v("P"), v("z")))));
        assume(arena, 3, &statement)
    }

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
        let motive = lam(
            "z2",
            v("B"),
            lam!("_" => id(v("B"), v("y"), v("z2")) => id(v("B"), v("x"), v("z2"))),
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

    fn transport_const_declaration(arena: &mut TermArena) -> Declaration {
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
        let outer_motive = lam(
            "y2",
            v("A"),
            lam(
                "p2",
                id(v("A"), v("x"), v("y2")),
                id!(v("B") => jelim(constant_motive(), v("b"), v("y2"), v("p2")) => v("b")),
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
        let constant_family = || lam("_", quotient_of(), v("B"));
        let soundness = sound_of(v("a"), v("b"), v("r"));
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

    fn id_sym_declaration(arena: &mut TermArena) -> Declaration {
        let statement = pi(
            "X",
            ty(0),
            pi(
                "x",
                v("X"),
                pi!("y" => v("X") => pi("_", id(v("X"), v("x"), v("y")), id(v("X"), v("y"), v("x")))),
            ),
        );
        let motive = lam(
            "y2",
            v("X"),
            lam!("_" => id(v("X"), v("x"), v("y2")) => id(v("X"), v("y2"), v("x"))),
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
                        id!(id(v("X"), v("y"), v("y")) => cancelled("y", "p") => refl(v("X"), v("y"))),
                    ),
                ),
            ),
        );
        let motive = lam(
            "y2",
            v("X"),
            lam(
                "p2",
                id(v("X"), v("x"), v("y2")),
                id!(id(v("X"), v("y2"), v("y2")) => cancelled("y2", "p2") => refl(v("X"), v("y2"))),
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

    fn prop_is_set_declaration(arena: &mut TermArena) -> Declaration {
        let level = || vec![Level::Parameter(0)];
        let prop = |endpoint: &'static str| apps(v("f"), [v("x"), v(endpoint)]);
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
        let path_motive = lam(
            "y2",
            v("X"),
            lam(
                "p2",
                id(v("X"), v("x"), v("y2")),
                id(id(v("X"), v("x"), v("y2")), v("p2"), normal("y2")),
            ),
        );
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

    fn box_prop_declaration(arena: &mut TermArena) -> Declaration {
        let boxed_s = || boxed(v("S"));
        let inner_motive = lam(
            "y3",
            boxed_s(),
            id(boxed_s(), box_intro(v("S"), v("a")), v("y3")),
        );
        let outer_motive = lam(
            "x2",
            boxed_s(),
            pi("y2", boxed_s(), id(boxed_s(), v("x2"), v("y2"))),
        );
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
            pi!("x" => boxed_s() => pi("y", boxed_s(), id(boxed_s(), v("x"), v("y")))),
        );
        let body = lam(
            "S",
            strict(0),
            lam(
                "x",
                boxed_s(),
                lam!("y" => boxed_s() => app(box_elim(outer_motive, case, v("x")), v("y"))),
            ),
        );
        define(arena, 1, &statement, &body)
    }

    fn ind_prop_declaration(arena: &mut TermArena) -> Declaration {
        let strict_level = || vec![Level::Parameter(2)];
        let boxed_family = lam("z2", quotient_of(), boxed(app(v("P"), v("z2"))));
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
        let case = lam(
            "a",
            v("A"),
            box_intro(app(v("P"), project_of(v("a"))), app(v("d"), v("a"))),
        );
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
            pi!("d" => case_type.clone() => pi("z", quotient_of(), app(v("P"), v("z")))),
        ));
        let body = lam_description(lam(
            "P",
            family_type,
            lam("d", case_type, lam("z", quotient_of(), unboxed)),
        ));
        define(arena, 3, &statement, &body)
    }

    fn coverage_declaration(arena: &mut TermArena) -> Declaration {
        let covered = |anchor: Syntax| {
            squash(sigma(
                "a2",
                v("A"),
                id(quotient_of(), project_of(v("a2")), anchor),
            ))
        };
        let statement = pi_description(pi("z", quotient_of(), covered(v("z"))));
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

    fn unique_declaration(arena: &mut TermArena) -> Declaration {
        let identity_family = lam(
            "z2",
            quotient_of(),
            id!(app(v("P"), v("z2")) => app(v("s1"), v("z2")) => app(v("s2"), v("z2"))),
        );
        let sets = lam(
            "z3",
            quotient_of(),
            apps(
                scheme_at(QUOTIENT_PROP_IS_SET, vec![Level::Parameter(2)]),
                [
                    id!(app(v("P"), v("z3")) => app(v("s1"), v("z3")) => app(v("s2"), v("z3"))),
                    apps(
                        v("sets"),
                        [v("z3"), app(v("s1"), v("z3")), app(v("s2"), v("z3"))],
                    ),
                ],
            ),
        );
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
            id_sym_declaration(arena),
            id_cancel_declaration(arena),
            prop_is_set_declaration(arena),
            box_prop_declaration(arena),
            ind_prop_declaration(arena),
            coverage_declaration(arena),
            unique_declaration(arena),
        ]
    }
}
pub use declarations::quotient_scheme;

pub const QUOTIENT_IS_SET: u32 = 0;
pub const QUOTIENT: u32 = 1;
pub const QUOTIENT_PROJECT: u32 = 2;
pub const QUOTIENT_SET: u32 = 3;
pub const QUOTIENT_SOUND: u32 = 4;
pub const QUOTIENT_EFFECTIVE: u32 = 5;
pub const QUOTIENT_TRANSPORT: u32 = 6;
pub const QUOTIENT_ELIM: u32 = 7;
pub const QUOTIENT_BETA: u32 = 8;
pub const QUOTIENT_ID_TRANS: u32 = 9;
pub const QUOTIENT_TRANSPORT_CONST: u32 = 10;
pub const QUOTIENT_LIFT: u32 = 11;
pub const QUOTIENT_ID_SYM: u32 = 12;
pub const QUOTIENT_ID_CANCEL: u32 = 13;
pub const QUOTIENT_PROP_IS_SET: u32 = 14;
pub const QUOTIENT_BOX_PROP: u32 = 15;
pub const QUOTIENT_IND_PROP: u32 = 16;
pub const QUOTIENT_COVERAGE: u32 = 17;
pub const QUOTIENT_UNIQUE: u32 = 18;

fn carrier_level() -> Level {
    Level::Maximum(Box::new(Level::Parameter(0)), Box::new(Level::Parameter(1)))
}

fn uv() -> Vec<Level> {
    vec![Level::Parameter(0), Level::Parameter(1)]
}

fn uvw() -> Vec<Level> {
    let mut levels = uv();
    levels.push(Level::Parameter(2));
    levels
}

fn carrier_scheme(declaration: u32) -> Syntax {
    scheme_at(declaration, uv())
}

fn motive_scheme(declaration: u32) -> Syntax {
    scheme_at(declaration, uvw())
}

fn quotient_of() -> Syntax {
    apps(carrier_scheme(QUOTIENT), [v("A"), v("R")])
}

fn project_of(representative: Syntax) -> Syntax {
    apps(
        carrier_scheme(QUOTIENT_PROJECT),
        [v("A"), v("R"), representative],
    )
}

fn related(left: Syntax, right: Syntax) -> Syntax {
    app(app(v("R"), left), right)
}

fn is_set(level: Level, ty: Syntax) -> Syntax {
    app(scheme_at(QUOTIENT_IS_SET, vec![level]), ty)
}

fn sound_of(left: Syntax, right: Syntax, evidence: Syntax) -> Syntax {
    apps(
        carrier_scheme(QUOTIENT_SOUND),
        [v("A"), v("R"), left, right, evidence],
    )
}

fn pi_description(rest: Syntax) -> Syntax {
    pi(
        "A",
        ty(0),
        pi("R", pi("_", v("A"), pi("_", v("A"), ty(1))), rest),
    )
}

fn lam_description(rest: Syntax) -> Syntax {
    lam(
        "A",
        ty(0),
        lam("R", pi("_", v("A"), pi("_", v("A"), ty(1))), rest),
    )
}

fn motive_type() -> Syntax {
    pi("_", quotient_of(), ty(2))
}

/// Proofs that a quotient's relation `R` is a mere proposition at every
/// pair and an equivalence relation, in the order `effective` binds them.
#[derive(Clone, Copy, Debug)]
pub struct EquivalenceProofs {
    /// `(a b : A) → (r s : R a b) → Id (R a b) r s`
    pub propositional: TermHandle,
    /// `(a : A) → R a a`
    pub reflexive: TermHandle,
    /// `(a b : A) → R a b → R b a`
    pub symmetric: TermHandle,
    /// `(a b c : A) → R a b → R b c → R a c`
    pub transitive: TermHandle,
}

#[derive(Clone, Debug)]
pub struct QuotientFamily {
    pub levels: [Level; 2],
    pub carrier: TermHandle,
    pub relation: TermHandle,
}

impl QuotientFamily {
    fn unspined(
        &self,
        arena: &mut TermArena,
        declaration: u32,
        level: Level,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let constant = arena.insert(Term::Constant {
            declaration,
            levels: vec![level],
        });
        arena.apply_arguments(constant, arguments)
    }

    fn spine(
        &self,
        arena: &mut TermArena,
        declaration: u32,
        mut levels: Vec<Level>,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let constant = arena.insert(Term::Constant {
            declaration,
            levels: {
                let mut instantiation = self.levels.to_vec();
                instantiation.append(&mut levels);
                instantiation
            },
        });
        let related = arena.apply_arguments(constant, &[self.carrier, self.relation]);
        arena.apply_arguments(related, arguments)
    }

    pub fn quotient(&self, arena: &mut TermArena) -> TermHandle {
        self.spine(arena, QUOTIENT, Vec::new(), &[])
    }

    pub fn project(&self, arena: &mut TermArena, representative: TermHandle) -> TermHandle {
        self.spine(arena, QUOTIENT_PROJECT, Vec::new(), &[representative])
    }

    pub fn sound(
        &self,
        arena: &mut TermArena,
        left: TermHandle,
        right: TermHandle,
        evidence: TermHandle,
    ) -> TermHandle {
        self.spine(arena, QUOTIENT_SOUND, Vec::new(), &[left, right, evidence])
    }

    /// Effectivity cites the relation's equivalence and proposition proofs
    /// before the two representatives and their projection identity.
    pub fn effective(
        &self,
        arena: &mut TermArena,
        relation_laws: &EquivalenceProofs,
        left: TermHandle,
        right: TermHandle,
        proof: TermHandle,
    ) -> TermHandle {
        let EquivalenceProofs {
            propositional,
            reflexive,
            symmetric,
            transitive,
        } = *relation_laws;
        self.spine(
            arena,
            QUOTIENT_EFFECTIVE,
            Vec::new(),
            &[
                propositional,
                reflexive,
                symmetric,
                transitive,
                left,
                right,
                proof,
            ],
        )
    }

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

    pub fn prop_is_set(
        &self,
        arena: &mut TermArena,
        level: Level,
        ty: TermHandle,
        evidence: TermHandle,
    ) -> TermHandle {
        self.unspined(arena, QUOTIENT_PROP_IS_SET, level, &[ty, evidence])
    }

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

    pub fn coverage(&self, arena: &mut TermArena, element: TermHandle) -> TermHandle {
        self.spine(arena, QUOTIENT_COVERAGE, Vec::new(), &[element])
    }

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

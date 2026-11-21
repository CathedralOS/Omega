use super::scheme_dsl::{
    Syntax, app, apps, define, fst, id, indw, lam, pair, pi, refl, scheme_at, sigma, snd, sort,
    sup, ty, v, w,
};
use super::signature::Declaration;
use super::term::{Level, Term, TermArena, TermHandle};

macro_rules! pi {
    ($name:expr => $domain:expr => $codomain:expr) => {
        pi($name, $domain, $codomain)
    };
}

pub const INDEXED_AT: u32 = 0;
pub const INDEXED_W: u32 = 1;
pub const INDEXED_PACK: u32 = 2;
pub const INDEXED_SUP: u32 = 3;
pub const INDEXED_IND: u32 = 4;

fn scheme(declaration: u32) -> Syntax {
    scheme_at(
        declaration,
        vec![
            Level::Parameter(0),
            Level::Parameter(1),
            Level::Parameter(2),
        ],
    )
}

fn description_telescope(
    connective: fn(&'static str, Syntax, Syntax) -> Syntax,
    rest: Syntax,
) -> Syntax {
    connective(
        "I",
        ty(0),
        connective(
            "A",
            ty(1),
            connective(
                "B",
                pi("_", v("A"), ty(2)),
                connective(
                    "out",
                    pi("_", v("A"), v("I")),
                    connective(
                        "next",
                        pi("a", v("A"), pi("_", app(v("B"), v("a")), v("I"))),
                        rest,
                    ),
                ),
            ),
        ),
    )
}

fn pi_description(rest: Syntax) -> Syntax {
    description_telescope(pi, rest)
}

fn lam_description(rest: Syntax) -> Syntax {
    description_telescope(lam, rest)
}

fn at(index: Syntax, tree: Syntax) -> Syntax {
    apps(
        scheme(INDEXED_AT),
        [v("I"), v("A"), v("B"), v("out"), v("next"), index, tree],
    )
}

fn iw(index: Syntax) -> Syntax {
    apps(
        scheme(INDEXED_W),
        [v("I"), v("A"), v("B"), v("out"), v("next"), index],
    )
}

fn family_level() -> Level {
    Level::Maximum(
        Box::new(Level::Maximum(
            Box::new(Level::Parameter(0)),
            Box::new(Level::Parameter(1)),
        )),
        Box::new(Level::Parameter(2)),
    )
}

fn indexed_at_declaration(arena: &mut TermArena) -> Declaration {
    let level = family_level();
    let statement = pi_description(pi(
        "i",
        v("I"),
        pi("t", w(v("A"), v("B")), sort(level.clone())),
    ));
    let motive = lam("_", w(v("A"), v("B")), pi("i2", v("I"), sort(level)));
    let step = lam(
        "a",
        v("A"),
        lam(
            "k",
            pi("b", app(v("B"), v("a")), w(v("A"), v("B"))),
            lam(
                "ih",
                pi!("b" => app(v("B"), v("a")) => app(motive.clone(), app(v("k"), v("b")))),
                lam(
                    "i2",
                    v("I"),
                    sigma(
                        "_",
                        id(v("I"), app(v("out"), v("a")), v("i2")),
                        pi(
                            "b",
                            app(v("B"), v("a")),
                            app(app(v("ih"), v("b")), app(app(v("next"), v("a")), v("b"))),
                        ),
                    ),
                ),
            ),
        ),
    );
    let body = lam_description(lam(
        "i",
        v("I"),
        lam(
            "t",
            w(v("A"), v("B")),
            app(indw(motive, step, v("t")), v("i")),
        ),
    ));
    define(arena, 3, &statement, &body)
}

fn indexed_w_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi("i", v("I"), sort(family_level())));
    let body = lam_description(lam(
        "i",
        v("I"),
        sigma("t", w(v("A"), v("B")), at(v("i"), v("t"))),
    ));
    define(arena, 3, &statement, &body)
}

fn indexed_pack_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "i",
        v("I"),
        pi!("t" => w(v("A"), v("B")) => pi("e", at(v("i"), v("t")), iw(v("i")))),
    ));
    let body = lam_description(lam(
        "i",
        v("I"),
        lam(
            "t",
            w(v("A"), v("B")),
            lam(
                "e",
                at(v("i"), v("t")),
                app(lam("p", iw(v("i")), v("p")), pair(v("t"), v("e"))),
            ),
        ),
    ));
    define(arena, 3, &statement, &body)
}

fn indexed_sup_declaration(arena: &mut TermArena) -> Declaration {
    let children_type = pi(
        "b",
        app(v("B"), v("a")),
        iw(app(app(v("next"), v("a")), v("b"))),
    );
    let statement = pi_description(pi(
        "a",
        v("A"),
        pi("g", children_type.clone(), iw(app(v("out"), v("a")))),
    ));
    let body = lam_description(lam(
        "a",
        v("A"),
        lam(
            "g",
            children_type,
            app(
                lam("p", iw(app(v("out"), v("a"))), v("p")),
                pair(
                    sup(
                        v("A"),
                        v("B"),
                        v("a"),
                        lam("b", app(v("B"), v("a")), fst(app(v("g"), v("b")))),
                    ),
                    pair(
                        refl(v("I"), app(v("out"), v("a"))),
                        lam("b", app(v("B"), v("a")), snd(app(v("g"), v("b")))),
                    ),
                ),
            ),
        ),
    ));
    define(arena, 3, &statement, &body)
}

fn indexed_ind_declaration(arena: &mut TermArena) -> Declaration {
    let motive_level = Level::Parameter(3);
    let motive_type = pi("i", v("I"), pi("_", iw(v("i")), sort(motive_level.clone())));
    let step_type = pi(
        "a",
        v("A"),
        pi(
            "g",
            pi!("b" => app(v("B"), v("a")) => iw(app(app(v("next"), v("a")), v("b")))),
            pi(
                "_",
                pi(
                    "b",
                    app(v("B"), v("a")),
                    app(
                        app(v("Q"), app(app(v("next"), v("a")), v("b"))),
                        app(v("g"), v("b")),
                    ),
                ),
                app(
                    app(v("Q"), app(v("out"), v("a"))),
                    apps(
                        scheme(INDEXED_SUP),
                        [v("I"), v("A"), v("B"), v("out"), v("next"), v("a"), v("g")],
                    ),
                ),
            ),
        ),
    );
    let statement = pi_description(pi(
        "Q",
        motive_type.clone(),
        pi(
            "s",
            step_type.clone(),
            pi!("i" => v("I") => pi("t", iw(v("i")), app(app(v("Q"), v("i")), v("t")))),
        ),
    ));

    let inner_motive = lam(
        "u",
        w(v("A"), v("B")),
        pi(
            "i2",
            v("I"),
            pi!("e2" => at(v("i2"), v("u")) => app(app(v("Q"), v("i2")), pair(v("u"), v("e2")))),
        ),
    );
    let packed_children = lam(
        "b",
        app(v("B"), v("a")),
        apps(
            scheme(INDEXED_PACK),
            [
                v("I"),
                v("A"),
                v("B"),
                v("out"),
                v("next"),
                app(app(v("next"), v("a")), v("b")),
                app(v("k"), v("b")),
                app(snd(v("e2")), v("b")),
            ],
        ),
    );
    let hypotheses = lam(
        "b",
        app(v("B"), v("a")),
        app(
            app(app(v("ih"), v("b")), app(app(v("next"), v("a")), v("b"))),
            app(snd(v("e2")), v("b")),
        ),
    );
    let base = app(app(app(v("s"), v("a")), packed_children), hypotheses);
    let transport_motive = lam(
        "y",
        v("I"),
        lam(
            "p",
            id(v("I"), app(v("out"), v("a")), v("y")),
            app(
                app(v("Q"), v("y")),
                pair(
                    sup(v("A"), v("B"), v("a"), v("k")),
                    pair(v("p"), snd(v("e2"))),
                ),
            ),
        ),
    );
    let step = lam(
        "a",
        v("A"),
        lam(
            "k",
            pi("b", app(v("B"), v("a")), w(v("A"), v("B"))),
            lam(
                "ih",
                pi!("b" => app(v("B"), v("a")) => app(inner_motive.clone(), app(v("k"), v("b")))),
                lam(
                    "i2",
                    v("I"),
                    lam(
                        "e2",
                        at(v("i2"), sup(v("A"), v("B"), v("a"), v("k"))),
                        Syntax::IdElim(
                            Box::new(transport_motive),
                            Box::new(base),
                            Box::new(v("i2")),
                            Box::new(fst(v("e2"))),
                        ),
                    ),
                ),
            ),
        ),
    );
    let body = lam_description(lam(
        "Q",
        motive_type,
        lam(
            "s",
            step_type,
            lam(
                "i",
                v("I"),
                lam(
                    "t",
                    iw(v("i")),
                    app(
                        app(indw(inner_motive, step, fst(v("t"))), v("i")),
                        snd(v("t")),
                    ),
                ),
            ),
        ),
    ));
    define(arena, 4, &statement, &body)
}

fn indexed_correctness_declaration(arena: &mut TermArena) -> Declaration {
    let root_step = lam(
        "a",
        v("A"),
        lam(
            "k",
            pi("b", app(v("B"), v("a")), w(v("A"), v("B"))),
            lam("_", pi("_", app(v("B"), v("a")), v("A")), v("a")),
        ),
    );
    let root_of = |tree: Syntax| {
        app(
            v("out"),
            indw(
                lam("_", w(v("A"), v("B")), v("A")),
                root_step.clone(),
                fst(tree),
            ),
        )
    };
    let motive = lam(
        "i2",
        v("I"),
        lam("t2", iw(v("i2")), id(v("I"), root_of(v("t2")), v("i2"))),
    );
    let step = lam(
        "a",
        v("A"),
        lam(
            "g",
            pi!("b" => app(v("B"), v("a")) => iw(app(app(v("next"), v("a")), v("b")))),
            lam(
                "ih",
                pi(
                    "b",
                    app(v("B"), v("a")),
                    app(
                        app(motive.clone(), app(app(v("next"), v("a")), v("b"))),
                        app(v("g"), v("b")),
                    ),
                ),
                refl(v("I"), app(v("out"), v("a"))),
            ),
        ),
    );
    let statement = pi_description(pi(
        "i",
        v("I"),
        pi("t", iw(v("i")), id(v("I"), root_of(v("t")), v("i"))),
    ));
    let body = lam_description(lam(
        "i",
        v("I"),
        lam(
            "t",
            iw(v("i")),
            apps(
                scheme_at(
                    INDEXED_IND,
                    vec![
                        Level::Parameter(0),
                        Level::Parameter(1),
                        Level::Parameter(2),
                        Level::Parameter(0),
                    ],
                ),
                [
                    v("I"),
                    v("A"),
                    v("B"),
                    v("out"),
                    v("next"),
                    motive,
                    step,
                    v("i"),
                    v("t"),
                ],
            ),
        ),
    ));
    define(arena, 3, &statement, &body)
}

pub fn indexed_correctness(arena: &mut TermArena) -> Declaration {
    indexed_correctness_declaration(arena)
}

pub fn indexed_scheme(arena: &mut TermArena) -> Vec<Declaration> {
    vec![
        indexed_at_declaration(arena),
        indexed_w_declaration(arena),
        indexed_pack_declaration(arena),
        indexed_sup_declaration(arena),
        indexed_ind_declaration(arena),
    ]
}

#[derive(Clone, Debug)]
pub struct IndexedFamily {
    pub levels: [Level; 3],
    pub index: TermHandle,
    pub carrier: TermHandle,
    pub children: TermHandle,
    pub out: TermHandle,
    pub next: TermHandle,
}

impl IndexedFamily {
    fn spine(
        &self,
        arena: &mut TermArena,
        declaration: u32,
        levels: Vec<Level>,
        arguments: &[TermHandle],
    ) -> TermHandle {
        let constant = arena.insert(Term::Constant {
            declaration,
            levels,
        });
        let instantiated = arena.apply_arguments(
            constant,
            &[self.index, self.carrier, self.children, self.out, self.next],
        );
        arena.apply_arguments(instantiated, arguments)
    }

    pub fn indexed_at(
        &self,
        arena: &mut TermArena,
        index: TermHandle,
        tree: TermHandle,
    ) -> TermHandle {
        self.spine(arena, INDEXED_AT, self.levels.to_vec(), &[index, tree])
    }

    pub fn indexed_w(&self, arena: &mut TermArena, index: TermHandle) -> TermHandle {
        self.spine(arena, INDEXED_W, self.levels.to_vec(), &[index])
    }

    pub fn pack(
        &self,
        arena: &mut TermArena,
        index: TermHandle,
        tree: TermHandle,
        evidence: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            INDEXED_PACK,
            self.levels.to_vec(),
            &[index, tree, evidence],
        )
    }

    pub fn sup(
        &self,
        arena: &mut TermArena,
        label: TermHandle,
        children_fn: TermHandle,
    ) -> TermHandle {
        self.spine(
            arena,
            INDEXED_SUP,
            self.levels.to_vec(),
            &[label, children_fn],
        )
    }

    pub fn ind(
        &self,
        arena: &mut TermArena,
        motive_level: Level,
        motive: TermHandle,
        step: TermHandle,
        index: TermHandle,
        tree: TermHandle,
    ) -> TermHandle {
        let mut levels = self.levels.to_vec();
        levels.push(motive_level);
        self.spine(arena, INDEXED_IND, levels, &[motive, step, index, tree])
    }
}

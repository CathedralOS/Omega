use super::scheme_dsl::{app, define, id, jelim, lam, pi, ty, v};
use super::signature::Declaration;
use super::term::TermArena;

pub fn identity_substitution(arena: &mut TermArena) -> Declaration {
    let motive = lam(
        "y2",
        v("A"),
        lam("_", id(v("A"), v("x"), v("y2")), app(v("P"), v("y2"))),
    );
    let statement = pi(
        "A",
        ty(0),
        pi(
            "P",
            pi("_", v("A"), ty(1)),
            pi(
                "x",
                v("A"),
                pi(
                    "y",
                    v("A"),
                    pi(
                        "_",
                        id(v("A"), v("x"), v("y")),
                        pi("_", app(v("P"), v("x")), app(v("P"), v("y"))),
                    ),
                ),
            ),
        ),
    );
    let body = lam(
        "A",
        ty(0),
        lam(
            "P",
            pi("_", v("A"), ty(1)),
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
                            "h",
                            app(v("P"), v("x")),
                            jelim(motive, v("h"), v("y"), v("p")),
                        ),
                    ),
                ),
            ),
        ),
    );
    define(arena, 2, &statement, &body)
}

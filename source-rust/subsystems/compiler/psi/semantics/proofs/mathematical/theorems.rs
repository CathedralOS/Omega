use super::scheme_dsl::{app, id, jelim, lam, pi, theorem, ty, v};
use super::signature::Declaration;
use super::term::TermArena;

pub fn identity_substitution(arena: &mut TermArena) -> Declaration {
    let motive = lam(
        "y2",
        v("A"),
        lam("_", id(v("A"), v("x"), v("y2")), app(v("P"), v("y2"))),
    );
    let binders = vec![
        ("A", ty(0)),
        ("P", pi("_", v("A"), ty(1))),
        ("x", v("A")),
        ("y", v("A")),
        ("p", id(v("A"), v("x"), v("y"))),
    ];
    let statement = pi("_", app(v("P"), v("x")), app(v("P"), v("y")));
    let body = lam(
        "h",
        app(v("P"), v("x")),
        jelim(motive, v("h"), v("y"), v("p")),
    );
    theorem(arena, 2, binders, statement, body)
}

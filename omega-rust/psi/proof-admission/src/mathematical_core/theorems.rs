//! Named theorems of the common mathematical core, authored as ordinary
//! checked declarations: a theorem is a `Declaration::definition` whose
//! statement is a proposition-shaped `Π` telescope and whose body is its
//! checked proof. Nothing about a theorem is trusted — `check_signature`
//! re-decides the body against the statement exactly as for any other
//! definition, and a certificate that cites the theorem through
//! `Term::Constant` re-decides the citation's instantiation on the
//! receiving side.
//!
//! These are the "library theorems" the board item names: results about
//! arbitrary types and predicates, stated once polymorphically and cited
//! by consumers, rather than a producer-local proof term re-elaborated at
//! every use.

use super::scheme_dsl::{app, build, id, jelim, lam, pi, ty, v};
use super::signature::Declaration;
use super::term::TermArena;

/// `subst` — identity substitution, the transport theorem:
///
/// ```text
/// subst : Π(A : Type u). Π(P : Π(_ : A). Type v). Π(x : A). Π(y : A).
///         Π(_ : Id A x y). Π(_ : P x). P y
/// subst := λA. λP. λx. λy. λp. λh. J(λy'. λ(_ : Id A x y'). P y', h, y, p)
/// ```
///
/// `P` is an arbitrary predicate at its own universe `v`: the theorem
/// quantifies over it rather than fixing a family, which is what makes
/// transport usable by a consumer the producer never saw. The proof is
/// `J` with the motive `λy'. λ(_ : Id A x y'). P y'` — the base `h`
/// checks at `C x (refl A x)`, which converts to `P x`, and the result
/// `C y p` converts to `P y`.
///
/// Universe-polymorphic over `u` (the carrier's level) and `v` (the
/// predicate's level): `Declaration::definition(2, …)`, so a `Constant`
/// reference supplies exactly two level arguments.
pub fn identity_substitution(arena: &mut TermArena) -> Declaration {
    // `C = λ(y' : A). λ(_ : Id A x y'). P y'`.
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
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(2, statement, body)
}

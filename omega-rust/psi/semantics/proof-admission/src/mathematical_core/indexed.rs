//! The derived indexed-family scheme: indexed inductives built from the
//! kernel's W-type rather than a second primitive inductive checker.
//!
//! This module implements the selected
//! [W-based profile](../../../../../../wiki/spec/proofs/inductive_profile.md)'s
//! "derived indexed families" section. Given a producer's description of
//! an indexed family —
//!
//! ```text
//! I : Type l                    indices
//! A : Type u                    constructor labels and payloads
//! B : A → Type v              child positions
//! out : A → I                 index at a node
//! next : (a : A) → B(a) → I   required index of each child
//! ```
//!
//! the indexing condition is defined by W-induction over the unindexed
//! tree, with the motive quantifying over the index so the recursion can
//! move to `next a b` at each child:
//!
//! ```text
//! IndexedAt(i, sup(a,k)) = Id I (out a) i × ((b : B(a)) → IndexedAt(next a b, k b))
//! IW(i)                  = Σ (t : W(A,B)). IndexedAt(i, t)
//! ```
//!
//! Both are ordinary universe-polymorphic *definitions*: the scheme is
//! five declarations in dependency order — `IndexedAt`, `IW`, `iwPack`,
//! `isup`, `iindW` — which [`check_signature`] re-decides parametrically
//! over the levels `l, u, v` (and `w` for the eliminator's motive). No
//! `Term` variant, typing rule, conversion rule or wire tag is added; a
//! wrong description is a malformed application, not a new judgment.
//!
//! The derived constructor `isup` and dependent eliminator `iindW` are
//! defined terms whose computation judgments are definitional, not
//! postulated:
//!
//! ```text
//! isup a g : IW(out a)        for g : (b : B(a)) → IW(next a b)
//! iindW Q s i (isup a g) ≡ s a g (b ↦ iindW Q s (next a b) (g b))
//! ```
//!
//! The constructor's child function stays arbitrary: with `g` a neutral
//! variable, `isup a g` unfolds to `⟨sup a (b ↦ fst (g b)), ⟨refl, b ↦
//! snd (g b)⟩⟩`, `iindW` reconstructs `b ↦ iwPack (next a b) (fst (g b))
//! (snd (g b))`, and pair eta plus the profile's typed function eta close
//! `b ↦ ⟨fst (g b), snd (g b)⟩` to `g` — the conversion dependency the
//! profile names explicitly. `iwPack` exists because that dependent pair
//! only *checks* at `IW`'s Σ type; a bound lambda's body never infers it,
//! so the packed child function is routed through the declared type of
//! the `iwPack` constant. Inside `iindW`'s step, the J-eliminator
//! transports `Q (out a) (isup a g)` across `fst e : Id I (out a) i` to
//! reach `Q i ⟨sup a k, e⟩`, where pair eta identifies `⟨fst e, snd e⟩`
//! with `e`.
//!
//! Producers apply the scheme through [`IndexedFamily`], which bundles
//! the description and builds the `Constant` spines; the kernel
//! re-decides every application through ordinary typing and conversion.
//! The scheme declarations are signature prefix `0..5` — they reference
//! only each other, so a producer's own declarations append after them.

use super::signature::Declaration;
use super::term::{Level, Sort, Term, TermArena, TermHandle};

/// `IndexedAt` — the indexing condition — is declaration 0 of
/// [`indexed_scheme`].
pub const INDEXED_AT: u32 = 0;
/// `IW i = Σ (t : W A B). IndexedAt i t`, declaration 1.
pub const INDEXED_W: u32 = 1;
/// `iwPack i t e = ⟨t, e⟩`, declaration 2 — the dependent pair packaged
/// under its declared `IW` type so call sites infer it (see the module
/// documentation for why a literal `λb. ⟨k b, f b⟩` cannot).
pub const INDEXED_PACK: u32 = 2;
/// `isup`, declaration 3: the derived indexed constructor.
pub const INDEXED_SUP: u32 = 3;
/// `iindW`, declaration 4: the derived dependent eliminator,
/// universe-polymorphic over `l, u, v` and the motive level `w`.
pub const INDEXED_IND: u32 = 4;

/// Term notation for authoring the scheme declarations: binders are
/// named strings resolved to de Bruijn indices mechanically, so the
/// emitted terms stay free of hand-computed index arithmetic. The
/// checker only ever sees the built `Term`s; nothing name-based escapes
/// construction. `Variable` resolution is a build-time panic on a typo,
/// never a silently wrong index.
#[derive(Clone)]
enum Syntax {
    Variable(&'static str),
    Sort(Sort),
    Pi(&'static str, Box<Syntax>, Box<Syntax>),
    Lambda(&'static str, Box<Syntax>, Box<Syntax>),
    Apply(Box<Syntax>, Box<Syntax>),
    Sigma(&'static str, Box<Syntax>, Box<Syntax>),
    Pair(Box<Syntax>, Box<Syntax>),
    Fst(Box<Syntax>),
    Snd(Box<Syntax>),
    Id(Box<Syntax>, Box<Syntax>, Box<Syntax>),
    Refl(Box<Syntax>, Box<Syntax>),
    IdElim(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
    W(Box<Syntax>, Box<Syntax>),
    Sup(Box<Syntax>, Box<Syntax>, Box<Syntax>, Box<Syntax>),
    IndW(Box<Syntax>, Box<Syntax>, Box<Syntax>),
    /// A scheme declaration reference at parameters `l, u, v` —
    /// `Constant { declaration, levels: [Parameter 0, 1, 2] }`. Every
    /// cross-declaration reference inside the scheme instantiates this
    /// way; `iindW`'s extra parameter `w` never reaches the others.
    Scheme(u32),
}

fn build(arena: &mut TermArena, scope: &mut Vec<&'static str>, syntax: &Syntax) -> TermHandle {
    match syntax {
        Syntax::Variable(name) => {
            let position = scope
                .iter()
                .rposition(|bound| bound == name)
                .unwrap_or_else(|| panic!("unbound scheme variable {name}"));
            // `scope` is outermost-first; the de Bruijn index counts
            // inward from the end.
            let index = scope.len() - 1 - position;
            arena.insert(Term::Variable(index as u32))
        }
        Syntax::Sort(sort) => arena.insert(Term::Sort(sort.clone())),
        Syntax::Pi(name, domain, codomain) | Syntax::Sigma(name, domain, codomain) => {
            let domain = build(arena, scope, domain);
            scope.push(name);
            let codomain = build(arena, scope, codomain);
            scope.pop();
            match syntax {
                Syntax::Pi(..) => arena.insert(Term::Pi { domain, codomain }),
                _ => arena.insert(Term::Sigma { domain, codomain }),
            }
        }
        Syntax::Lambda(name, domain, body) => {
            let domain = build(arena, scope, domain);
            scope.push(name);
            let body = build(arena, scope, body);
            scope.pop();
            arena.insert(Term::Lambda { domain, body })
        }
        Syntax::Apply(function, argument) => {
            let function = build(arena, scope, function);
            let argument = build(arena, scope, argument);
            arena.insert(Term::Apply { function, argument })
        }
        Syntax::Pair(first, second) => {
            let first = build(arena, scope, first);
            let second = build(arena, scope, second);
            arena.insert(Term::Pair { first, second })
        }
        Syntax::Fst(pair) => {
            let pair = build(arena, scope, pair);
            arena.insert(Term::Fst { pair })
        }
        Syntax::Snd(pair) => {
            let pair = build(arena, scope, pair);
            arena.insert(Term::Snd { pair })
        }
        Syntax::Id(ty, left, right) => {
            let ty = build(arena, scope, ty);
            let left = build(arena, scope, left);
            let right = build(arena, scope, right);
            arena.insert(Term::Id { ty, left, right })
        }
        Syntax::Refl(ty, value) => {
            let ty = build(arena, scope, ty);
            let value = build(arena, scope, value);
            arena.insert(Term::Refl { ty, value })
        }
        Syntax::IdElim(motive, base, endpoint, proof) => {
            let motive = build(arena, scope, motive);
            let base = build(arena, scope, base);
            let endpoint = build(arena, scope, endpoint);
            let proof = build(arena, scope, proof);
            arena.insert(Term::IdElim {
                motive,
                base,
                endpoint,
                proof,
            })
        }
        Syntax::W(carrier, children) => {
            let carrier = build(arena, scope, carrier);
            let children = build(arena, scope, children);
            arena.insert(Term::W { carrier, children })
        }
        Syntax::Sup(carrier, children, label, function) => {
            let carrier = build(arena, scope, carrier);
            let children = build(arena, scope, children);
            let label = build(arena, scope, label);
            let function = build(arena, scope, function);
            arena.insert(Term::Sup {
                carrier,
                children,
                label,
                function,
            })
        }
        Syntax::IndW(motive, step, tree) => {
            let motive = build(arena, scope, motive);
            let step = build(arena, scope, step);
            let tree = build(arena, scope, tree);
            arena.insert(Term::IndW { motive, step, tree })
        }
        Syntax::Scheme(declaration) => arena.insert(Term::Constant {
            declaration: *declaration,
            levels: vec![
                Level::Parameter(0),
                Level::Parameter(1),
                Level::Parameter(2),
            ],
        }),
    }
}

fn v(name: &'static str) -> Syntax {
    Syntax::Variable(name)
}

fn sort(level: Level) -> Syntax {
    Syntax::Sort(Sort::Type(level))
}

fn ty(level: u32) -> Syntax {
    sort(Level::Parameter(level))
}

fn app(function: Syntax, argument: Syntax) -> Syntax {
    Syntax::Apply(Box::new(function), Box::new(argument))
}

fn apps(function: Syntax, arguments: impl IntoIterator<Item = Syntax>) -> Syntax {
    arguments.into_iter().fold(function, app)
}

fn pi(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
    Syntax::Pi(name, Box::new(domain), Box::new(codomain))
}

fn lam(name: &'static str, domain: Syntax, body: Syntax) -> Syntax {
    Syntax::Lambda(name, Box::new(domain), Box::new(body))
}

fn sigma(name: &'static str, domain: Syntax, codomain: Syntax) -> Syntax {
    Syntax::Sigma(name, Box::new(domain), Box::new(codomain))
}

fn pair(first: Syntax, second: Syntax) -> Syntax {
    Syntax::Pair(Box::new(first), Box::new(second))
}

fn fst(pair: Syntax) -> Syntax {
    Syntax::Fst(Box::new(pair))
}

fn snd(pair: Syntax) -> Syntax {
    Syntax::Snd(Box::new(pair))
}

fn id(ty: Syntax, left: Syntax, right: Syntax) -> Syntax {
    Syntax::Id(Box::new(ty), Box::new(left), Box::new(right))
}

fn refl(ty: Syntax, value: Syntax) -> Syntax {
    Syntax::Refl(Box::new(ty), Box::new(value))
}

fn w(carrier: Syntax, children: Syntax) -> Syntax {
    Syntax::W(Box::new(carrier), Box::new(children))
}

fn sup(carrier: Syntax, children: Syntax, label: Syntax, function: Syntax) -> Syntax {
    Syntax::Sup(
        Box::new(carrier),
        Box::new(children),
        Box::new(label),
        Box::new(function),
    )
}

fn indw(motive: Syntax, step: Syntax, tree: Syntax) -> Syntax {
    Syntax::IndW(Box::new(motive), Box::new(step), Box::new(tree))
}

fn scheme(declaration: u32) -> Syntax {
    Syntax::Scheme(declaration)
}

/// The shared description telescope
/// `Π(I : Type l). Π(A : Type u). Π(B : Π(_ : A). Type v).
/// Π(out : Π(_ : A). I). Π(next : Π(a : A). Π(_ : B a). I). …` —
/// parameters are de Bruijn-resolved by name inside `rest`.
fn pi_description(rest: Syntax) -> Syntax {
    pi(
        "I",
        ty(0),
        pi(
            "A",
            ty(1),
            pi(
                "B",
                pi("_", v("A"), ty(2)),
                pi(
                    "out",
                    pi("_", v("A"), v("I")),
                    pi(
                        "next",
                        pi("a", v("A"), pi("_", app(v("B"), v("a")), v("I"))),
                        rest,
                    ),
                ),
            ),
        ),
    )
}

/// The λ-side of the same telescope for declaration bodies.
fn lam_description(rest: Syntax) -> Syntax {
    lam(
        "I",
        ty(0),
        lam(
            "A",
            ty(1),
            lam(
                "B",
                pi("_", v("A"), ty(2)),
                lam(
                    "out",
                    pi("_", v("A"), v("I")),
                    lam(
                        "next",
                        pi("a", v("A"), pi("_", app(v("B"), v("a")), v("I"))),
                        rest,
                    ),
                ),
            ),
        ),
    )
}

/// `IndexedAt · I · A · B · out · next · index · tree` — the spine the
/// spec writes `IndexedAt(i, t)`.
fn at(index: Syntax, tree: Syntax) -> Syntax {
    apps(
        scheme(INDEXED_AT),
        [v("I"), v("A"), v("B"), v("out"), v("next"), index, tree],
    )
}

/// `IW · I · A · B · out · next · index`.
fn iw(index: Syntax) -> Syntax {
    apps(
        scheme(INDEXED_W),
        [v("I"), v("A"), v("B"), v("out"), v("next"), index],
    )
}

/// The level of `IndexedAt`/`IW` values: `max(l, u, v)`.
fn family_level() -> Level {
    Level::Maximum(
        Box::new(Level::Maximum(
            Box::new(Level::Parameter(0)),
            Box::new(Level::Parameter(1)),
        )),
        Box::new(Level::Parameter(2)),
    )
}

/// `IndexedAt : Π I A B out next. Π(i : I). Π(t : W A B). Type s` where
/// `s = max(l, u, v)`, defined by W-induction whose motive quantifies
/// over the index: `indW(λ_. Π(i' : I). Type s, step0, t) i` with
/// `step0 a k ih i' = Σ(_ : Id I (out a) i'). Π(b : B a). ih b (next a
/// b)`. Unfolding once gives exactly the profile's equation
/// `IndexedAt(i, sup a k) ≡ Id I (out a) i × Π(b : B a). IndexedAt(next
/// a b, k b)` — the recursion lands on `next a b` because `ih b` is the
/// `Π(i' : I). Type s`-valued hypothesis applied there.
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
                pi(
                    "b",
                    app(v("B"), v("a")),
                    app(motive.clone(), app(v("k"), v("b"))),
                ),
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
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// `IW : Π I A B out next. Π(i : I). Type s` defined as
/// `λ…λi. Σ(t : W A B). IndexedAt i t`.
fn indexed_w_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi("i", v("I"), sort(family_level())));
    let body = lam_description(lam(
        "i",
        v("I"),
        sigma("t", w(v("A"), v("B")), at(v("i"), v("t"))),
    ));
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// `iwPack : Π I A B out next. Π(i : I). Π(t : W A B). Π(e : IndexedAt i
/// t). IW i` defined as `λ…λi λt λe. (λ(p : IW i). p) ⟨t, e⟩`. The
/// identity application forces the literal pair to *check* against the
/// unfolded `IW` Σ type componentwise — bare-pair inference would only
/// produce the non-dependent `Σ (_ : W A B). IndexedAt i t`, which never
/// converts to `Σ (t' : W A B). IndexedAt i t'`. The *declared type* is
/// then what `iindW`'s reconstructed child function `b ↦ iwPack (next a
/// b) (k b) (snd e b)` infers, which a literal `b ↦ ⟨k b, snd e b⟩`
/// could never do: the same non-dependent inference loses the
/// `IW (next a b)` reading.
fn indexed_pack_declaration(arena: &mut TermArena) -> Declaration {
    let statement = pi_description(pi(
        "i",
        v("I"),
        pi(
            "t",
            w(v("A"), v("B")),
            pi("e", at(v("i"), v("t")), iw(v("i"))),
        ),
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
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// `isup : Π I A B out next. Π(a : A). Π(g : Π(b : B a). IW (next a
/// b)). IW (out a)` defined as `λ…λa λg. (λ(p : IW (out a)). p)
/// ⟨sup A B a (λb. fst (g b)), ⟨refl I (out a), λb. snd (g b)⟩⟩` — the
/// identity application puts the pair in checking position exactly as in
/// `iwPack`. The underlying tree unwraps the packed children; the
/// indexing evidence is reflexivity at `out a` plus the children proofs
/// — checked, never trusted, when `IndexedAt (out a) (sup …)` unfolds
/// to its Σ shape.
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
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(3, statement, body)
}

/// `iindW` — the derived dependent eliminator, universe-polymorphic
/// over `l, u, v` and the motive level `w`:
///
/// ```text
/// iindW : Π I A B out next.
///         Π(Q : Π(i : I). Π(_ : IW i). Type w).
///         Π(s : Π(a : A). Π(g : Π(b : B a). IW (next a b)).
///               Π(_ : Π(b : B a). Q (next a b) (g b)).
///               Q (out a) (isup a g)).
///         Π(i : I). Π(t : IW i). Q i t
/// iindW Q s i t := indW(P', step', fst t) i (snd t)
/// ```
///
/// with `P' u = Π(i : I). Π(e : IndexedAt i u). Q i ⟨u, e⟩` and the step
/// `step' a k ih i e = J(C, d, i, fst e)` where `e`'s unfolded `IndexedAt
/// i (sup a k)` supplies `fst e : Id I (out a) i` and `snd e : Π(b : B
/// a). IndexedAt (next a b) (k b)`. The transported base
/// `d = s a g' ih'` repacks children through `g' b = iwPack (next a b)
/// (k b) (snd e b)` and hypotheses through `ih' b = ih b (next a b)
/// (snd e b)`; `C y p = Q y ⟨sup a k, ⟨p, snd e⟩⟩` moves the index and
/// the identity proof together. `d`'s inferred `Q (out a) (isup a g')`
/// converts to `C (out a) (refl (out a))` through pair eta and typed
/// function eta — `g' ≡ b ↦ ⟨k b, snd e b⟩`, `sup`'s stored child
/// function `b ↦ fst (g' b) ≡ b ↦ k b ≡ k` — and the result `Q i ⟨sup a
/// k, ⟨fst e, snd e⟩⟩` converts to `Q i ⟨sup a k, e⟩` by pair eta on
/// `e`.
fn indexed_ind_declaration(arena: &mut TermArena) -> Declaration {
    let motive_level = Level::Parameter(3);
    let motive_type = pi("i", v("I"), pi("_", iw(v("i")), sort(motive_level.clone())));
    let step_type = pi(
        "a",
        v("A"),
        pi(
            "g",
            pi(
                "b",
                app(v("B"), v("a")),
                iw(app(app(v("next"), v("a")), v("b"))),
            ),
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
            pi(
                "i",
                v("I"),
                pi("t", iw(v("i")), app(app(v("Q"), v("i")), v("t"))),
            ),
        ),
    ));

    // `P' = λu. Π(i' : I). Π(e' : IndexedAt i' u). Q i' ⟨u, e'⟩`.
    let inner_motive = lam(
        "u",
        w(v("A"), v("B")),
        pi(
            "i2",
            v("I"),
            pi(
                "e2",
                at(v("i2"), v("u")),
                app(app(v("Q"), v("i2")), pair(v("u"), v("e2"))),
            ),
        ),
    );
    // `g' = λb. iwPack (next a b) (k b) (snd e b)`.
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
    // `ih' = λb. ih b (next a b) (snd e b)`.
    let hypotheses = lam(
        "b",
        app(v("B"), v("a")),
        app(
            app(app(v("ih"), v("b")), app(app(v("next"), v("a")), v("b"))),
            app(snd(v("e2")), v("b")),
        ),
    );
    // `d = s a g' ih'`.
    let base = app(app(app(v("s"), v("a")), packed_children), hypotheses);
    // `C = λy. λ(p : Id I (out a) y). Q y ⟨sup a k, ⟨p, snd e⟩⟩`.
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
    // `step' = λa. λk. λih. λi'. λe. J(C, d, i', fst e)`.
    let step = lam(
        "a",
        v("A"),
        lam(
            "k",
            pi("b", app(v("B"), v("a")), w(v("A"), v("B"))),
            lam(
                "ih",
                pi(
                    "b",
                    app(v("B"), v("a")),
                    app(inner_motive.clone(), app(v("k"), v("b"))),
                ),
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
    let statement = build(arena, &mut Vec::new(), &statement);
    let body = build(arena, &mut Vec::new(), &body);
    Declaration::definition(4, statement, body)
}

/// The five declarations of the derived indexed-family scheme, in
/// signature order: `IndexedAt`, `IW`, `iwPack`, `isup`, `iindW` occupy
/// positions 0–4 and reference only each other. Append them to the front
/// of a producer signature (or after any prefix the producer checks
/// first — the contained `Constant` positions are absolute), then
/// re-decide the whole signature with `check_signature`.
pub fn indexed_scheme(arena: &mut TermArena) -> Vec<Declaration> {
    vec![
        indexed_at_declaration(arena),
        indexed_w_declaration(arena),
        indexed_pack_declaration(arena),
        indexed_sup_declaration(arena),
        indexed_ind_declaration(arena),
    ]
}

/// A producer's description of an indexed family — the `I, A, B, out,
/// next` of the profile's encoding plus the levels `l, u, v` they live
/// at. The bundled spines build `Constant` applications; every argument
/// is still re-decided by typing at use.
#[derive(Clone, Debug)]
pub struct IndexedFamily {
    /// The description's levels `[l, u, v]`, as level expressions valid
    /// in the calling judgment's scope — `Parameter` indices inside a
    /// polymorphic declaration, `Constant` levels for a closed one.
    pub levels: [Level; 3],
    /// `I : Type l` — the index type.
    pub index: TermHandle,
    /// `A : Type u` — constructor labels and payloads.
    pub carrier: TermHandle,
    /// `B : A → Type v` — child positions.
    pub children: TermHandle,
    /// `out : A → I` — the index at a node.
    pub out: TermHandle,
    /// `next : Π(a : A). Π(_ : B a). I` — the required index of each
    /// child.
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
        let mut term = arena.insert(Term::Constant {
            declaration,
            levels,
        });
        for parameter in [self.index, self.carrier, self.children, self.out, self.next] {
            term = arena.insert(Term::Apply {
                function: term,
                argument: parameter,
            });
        }
        for &argument in arguments {
            term = arena.insert(Term::Apply {
                function: term,
                argument,
            });
        }
        term
    }

    /// `IndexedAt i t` — the indexing predicate, a `Type max(l,u,v)`.
    pub fn indexed_at(
        &self,
        arena: &mut TermArena,
        index: TermHandle,
        tree: TermHandle,
    ) -> TermHandle {
        self.spine(arena, INDEXED_AT, self.levels.to_vec(), &[index, tree])
    }

    /// `IW i` — the indexed family itself, a `Type max(l,u,v)`.
    pub fn indexed_w(&self, arena: &mut TermArena, index: TermHandle) -> TermHandle {
        self.spine(arena, INDEXED_W, self.levels.to_vec(), &[index])
    }

    /// `iwPack i t e : IW i` — package a tree with its indexing
    /// evidence.
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

    /// `isup a g : IW (out a)` — the derived indexed constructor, where
    /// `g : Π(b : B a). IW (next a b)` supplies each child already
    /// indexed at its required position.
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

    /// `iindW Q s i t : Q i t` — the derived dependent eliminator, with
    /// `motive_level` the `w` of `Q`'s codomain `Type w`. The scheme
    /// declaration is polymorphic over `l, u, v, w`; `motive_level` is
    /// appended to the family's three levels for the instantiation.
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

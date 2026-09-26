//! The kernel legs of the foundation's nonconstructive-existence
//! discriminating control (`wiki/spec/proofs/foundation.md`'s migration
//! acceptance and `wiki/spec/proofs/mathematical_bindings.md`'s
//! `boundary let choose`): squashed existence composes through a logical
//! hypothesis with *no* axiom, while reaching a relevant witness requires
//! an admitted choice assumption with an exact statement — and the
//! admission is recorded in the judgment's assumption closure so a
//! refusing receiver sees exactly what the proof committed to.
//!
//! * `transfer` is a *proved* declaration — the spec's squash-
//!   elimination example: from `Squash (Σ(x:A). Box (P x))` ("present")
//!   and a hypothesis `Π(x:A). Π(_ : P x). Squash (Σ(y:A). Box (Q y))`
//!   ("step") it derives `Squash (Σ(y:A). Box (Q y))`. `unsq` accepts
//!   because the target is itself a squash — a strict proposition — and
//!   the boxed premise is opened by `unbox` at a strict motive, the
//!   reference core's plain projection `Box A → A`. The witness never
//!   escapes; choice is unnecessary, and the closure is empty.
//! * `choice` is an *assumption* — the pinned
//!   `choose<u, A : Type u>(inhabited : Squash A) : A` — and `chosen`
//!   applies it at `Subset P := Σ(x : U32). Box (P x)`: a chosen member
//!   of a nonempty subset of `u32`, with `fst`/`snd` projecting the
//!   member and its boxed membership evidence. The member is a neutral
//!   `fst (choice …)` — an exact mathematical witness with no algorithm.
//! * The refusal cells: `unsq` into the relevant `Subset P` is
//!   unauthorized strict elimination (`SquashTargetNotStrict`), `fst`
//!   on a squash is `NotAPair`, and the derivable projection lands only
//!   in `Squash U32` — squashed relations yield only squashed evidence.
//!
//! Everything is re-decided as `MathematicalCertificate` judgments with
//! exact closures; the canonical wire already carries the strict layer
//! and assumption-bearing signatures (terminal-codec's
//! `mathematical_certificate`/`theorem_certificate` tests), so the wire
//! leg adds nothing new here.

use std::collections::BTreeSet;

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, Level,
    MathematicalCertificate, Sort, Term, TermArena, TermHandle, certificate_assumption_closure,
    check_signature, check_type, convertible, infer_type, judgment_assumption_closure,
    verify_mathematical_certificate, weak_head_normalize,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

fn type_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(level))))
}

fn strict_sort(arena: &mut TermArena, level: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level::Constant(level))))
}

fn variable(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

fn lambda(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

fn apply(arena: &mut TermArena, function: TermHandle, argument: TermHandle) -> TermHandle {
    arena.insert(Term::Apply { function, argument })
}

fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Sigma { domain, codomain })
}

fn fst(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair })
}

fn snd(arena: &mut TermArena, pair: TermHandle) -> TermHandle {
    arena.insert(Term::Snd { pair })
}

fn squash(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Squash { ty })
}

fn squash_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::SquashIntro { ty, value })
}

fn squash_elim(
    arena: &mut TermArena,
    proposition: TermHandle,
    function: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::SquashElim {
        proposition,
        function,
        scrutinee,
    })
}

fn boxed(arena: &mut TermArena, ty: TermHandle) -> TermHandle {
    arena.insert(Term::Box { ty })
}

fn box_elim(
    arena: &mut TermArena,
    motive: TermHandle,
    body: TermHandle,
    scrutinee: TermHandle,
) -> TermHandle {
    arena.insert(Term::BoxElim {
        motive,
        body,
        scrutinee,
    })
}

fn constant(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

// ── The declaration positions ───────────────────────────────────────
//
// The signature orders assumptions first, then the definitions that
// cite them; `transfer` is last because it is self-contained.

/// `U32 : Type 0` — declaration 0, the carrier a nonempty subset
/// ranges over. The discriminating control names `u32`; the kernel
/// models the machine scalar as an opaque relevant carrier exactly as
/// the bounded denotation does.
const U32: u32 = 0;
/// `choice : Π(A : Type u). Π(_ : Squash A). A` — declaration 1, the
/// admitted indefinite-description axiom (arity 1).
const CHOICE: u32 = 1;
/// `subset : Π(P : Π(_:U32). Strict v). Type v := λP. Σ(x:U32). Box (P
/// x)` — declaration 2, a *definition*: it never enters a closure.
const SUBSET: u32 = 2;
/// `chosen : Π(P : Π(_:U32). Strict v). Π(_ : Squash (subset P)).
/// subset P := λP. λh. choice[v] (subset[v] P) h` — declaration 3.
const CHOSEN: u32 = 3;
/// `transfer` — declaration 4, the proved squash-composition (arity 3:
/// the carrier `u`, the two predicates' strict levels `v` and `w`).
const TRANSFER: u32 = 4;

/// `U32` as a term reference (arity 0).
fn u32(arena: &mut TermArena) -> TermHandle {
    constant(arena, U32, Vec::new())
}

/// `Π(_ : U32). Strict p0` — the subset-predicate type inside an
/// arity-1 declaration (closed scope: no term binders above it).
fn predicate_domain(arena: &mut TermArena) -> TermHandle {
    let domain = u32(arena);
    let codomain = arena.insert(Term::Sort(Sort::Strict(Level::Parameter(0))));
    pi(arena, domain, codomain)
}

/// `subset[l] P` — the subset type `Σ(x:U32). Box (P x)` instantiated at
/// level `l` and applied to a `P` term. Declarations at arity 1 pass
/// `Parameter(0)`; arity-0 judgments pass `Constant(0)`.
fn subset_applied(arena: &mut TermArena, level: Level, p: TermHandle) -> TermHandle {
    let subset = constant(arena, SUBSET, vec![level]);
    apply(arena, subset, p)
}

fn u32_declaration(arena: &mut TermArena) -> Declaration {
    let statement = type_sort(arena, 0);
    Declaration::assumption(0, statement)
}

/// `choice : Π(A : Type u). Π(_ : Squash A). A` at level arity 1.
fn choice_declaration(arena: &mut TermArena) -> Declaration {
    let domain = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
    let squashed = {
        let a = variable(arena, 0);
        squash(arena, a)
    };
    let a = variable(arena, 1);
    let inhabited = pi(arena, squashed, a);
    let statement = pi(arena, domain, inhabited);
    Declaration::assumption(1, statement)
}

/// `subset : Π(P : Π(_:U32). Strict p0). Type p0` with body
/// `λP. Σ(x:U32). Box (P x)` — a relevant carrier pair: the member and
/// its boxed membership proof.
fn subset_declaration(arena: &mut TermArena) -> Declaration {
    let statement = {
        let domain = predicate_domain(arena);
        let codomain = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        pi(arena, domain, codomain)
    };
    let body = {
        let domain = predicate_domain(arena);
        // Under P: `Σ(x:U32). Box (P x)` — U32 is a constant, never a
        // de Bruijn index; under the Σ binder P is 1, x is 0.
        let pair = {
            let p = variable(arena, 1);
            let x = variable(arena, 0);
            let membership = apply(arena, p, x);
            boxed(arena, membership)
        };
        let member = u32(arena);
        let bundle = sigma(arena, member, pair);
        lambda(arena, domain, bundle)
    };
    Declaration::definition(1, statement, body)
}

/// `chosen : Π(P : Π(_:U32). Strict p0). Π(_ : Squash (subset P)).
/// subset P := λP. λh. choice[p0] (subset[p0] P) h` — the admitted
/// axiom applied at the subset type.
fn chosen_declaration(arena: &mut TermArena) -> Declaration {
    let statement = {
        let domain = predicate_domain(arena);
        let codomain = {
            // Under P: `Π(_ : Squash (subset P)). subset P` — the
            // domain's binder shifts `P` to 1 for the codomain.
            let p = variable(arena, 0);
            let subset_p = subset_applied(arena, Level::Parameter(0), p);
            let inhabited = squash(arena, subset_p);
            let p = variable(arena, 1);
            let landing = subset_applied(arena, Level::Parameter(0), p);
            pi(arena, inhabited, landing)
        };
        pi(arena, domain, codomain)
    };
    let body = {
        let domain = predicate_domain(arena);
        let inner = {
            // Under P: `λ(h : Squash (subset P)). choice (subset P) h`.
            let p = variable(arena, 0);
            let subset_p = subset_applied(arena, Level::Parameter(0), p);
            let inhabited = squash(arena, subset_p);
            let applied = {
                // Under P and h: P is 1, h is 0.
                let choice = constant(arena, CHOICE, vec![Level::Parameter(0)]);
                let p = variable(arena, 1);
                let subset_p = subset_applied(arena, Level::Parameter(0), p);
                let choose_at = apply(arena, choice, subset_p);
                let h = variable(arena, 0);
                apply(arena, choose_at, h)
            };
            lambda(arena, inhabited, applied)
        };
        lambda(arena, domain, inner)
    };
    Declaration::definition(1, statement, body)
}

/// `Σ(x:A). Box (P x)` written at a scope where `A` and `P` sit at de
/// Bruijn indices `a`/`p` — the codomain's binder shifts them once.
fn witness_bundle(arena: &mut TermArena, a: u32, p: u32) -> TermHandle {
    let domain = variable(arena, a);
    let codomain = {
        let p = variable(arena, p + 1);
        let x = variable(arena, 0);
        let membership = apply(arena, p, x);
        boxed(arena, membership)
    };
    sigma(arena, domain, codomain)
}

/// `Squash (Σ(y:A). Box (Q y))` — the strict squash of a witness
/// bundle, written where `A`/`Q` sit at `a`/`q`.
fn squashed_bundle(arena: &mut TermArena, a: u32, q: u32) -> TermHandle {
    let bundle = witness_bundle(arena, a, q);
    squash(arena, bundle)
}

/// `Π(x:A). Π(_ : P x). Squash (Σ(y:A). Box (Q y))` — the step
/// hypothesis's type, written where `A`, `P`, `Q` sit at `a`, `p`,
/// `q`. The premise is the strict `P x` itself: opening its box is the
/// eliminator's job.
fn step_type(arena: &mut TermArena, a: u32, p: u32, q: u32) -> TermHandle {
    let domain = variable(arena, a);
    let codomain = {
        // Under x: the premise `P x`, then the squashed `Q` bundle
        // where A is a+2 and Q is q+2 inside the Σ binder.
        let p = variable(arena, p + 1);
        let x = variable(arena, 0);
        let premise = apply(arena, p, x);
        let target = squashed_bundle(arena, a + 2, q + 2);
        pi(arena, premise, target)
    };
    pi(arena, domain, codomain)
}

/// `transfer : Π(A : Type u). Π(P : Π(_:A). Strict v). Π(Q : Π(_:A).
/// Strict w). Π(_ : Squash (Σ(x:A). Box (P x))). Π(_ : Π(x:A). Π(_ : P
/// x). Squash (Σ(y:A). Box (Q y))). Squash (Σ(y:A). Box (Q y))` at
/// level arity 3 — proved, not admitted:
///
/// ```text
/// λA. λP. λQ. λpresent. λstep.
///   unsq (Squash (Σ(y:A). Box (Q y)))
///        (λ(w : Σ(x:A). Box (P x)).
///           step (fst w)
///                (unbox (λ(_ : Box (P (fst w))). P (fst w))
///                       (λ(p : P (fst w)). p)
///                       (snd w)))
///        present
/// ```
///
/// The strict target is what licenses `unsq` — existence propagates to
/// existence — and `unbox` at the constant strict motive is the plain
/// projection `Box (P (fst w)) → P (fst w)`. The bundle's member `fst
/// w` is *used* (it feeds `step`) but never escapes: the result stays
/// inside `Squash`.
fn transfer_declaration(arena: &mut TermArena) -> Declaration {
    let statement = {
        let a_sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let p_domain = {
            let a = variable(arena, 0);
            let codomain = arena.insert(Term::Sort(Sort::Strict(Level::Parameter(1))));
            pi(arena, a, codomain)
        };
        let q_domain = {
            let a = variable(arena, 1);
            let codomain = arena.insert(Term::Sort(Sort::Strict(Level::Parameter(2))));
            pi(arena, a, codomain)
        };
        let present_domain = {
            // Under A, P, Q: A is 2, P is 1.
            squashed_bundle(arena, 2, 1)
        };
        let step_domain = {
            // Under A, P, Q, present: A is 3, P is 2, Q is 1.
            step_type(arena, 3, 2, 1)
        };
        let conclusion = {
            // Under all five: A is 4, Q is 2.
            squashed_bundle(arena, 4, 2)
        };
        let telescope = pi(arena, step_domain, conclusion);
        let telescope = pi(arena, present_domain, telescope);
        let telescope = pi(arena, q_domain, telescope);
        let telescope = pi(arena, p_domain, telescope);
        pi(arena, a_sort, telescope)
    };
    let body = {
        // Inside the λ telescope: step is 0, present is 1, Q is 2,
        // P is 3, A is 4.
        let a_sort = arena.insert(Term::Sort(Sort::Type(Level::Parameter(0))));
        let p_domain = {
            let a = variable(arena, 0);
            let codomain = arena.insert(Term::Sort(Sort::Strict(Level::Parameter(1))));
            pi(arena, a, codomain)
        };
        let q_domain = {
            let a = variable(arena, 1);
            let codomain = arena.insert(Term::Sort(Sort::Strict(Level::Parameter(2))));
            pi(arena, a, codomain)
        };
        let present_domain = squashed_bundle(arena, 2, 1);
        let step_domain = step_type(arena, 3, 2, 1);
        let elimination = {
            let proposition = squashed_bundle(arena, 4, 2);
            let function = {
                // Under w (the bundle binder): w is 0, step is 1,
                // present is 2, Q is 3, P is 4, A is 5.
                let domain = witness_bundle(arena, 4, 3);
                let projected = {
                    // `unbox (λ(_:Box (P (fst w))). P (fst w)) (λp. p)
                    // (snd w)`. The motive's domain field is ambient
                    // to its binder — P is 4, w is 0 there; inside the
                    // motive body they are 5 and 1.
                    let p_fst_w_ambient = {
                        let p = variable(arena, 4);
                        let w = variable(arena, 0);
                        let first = fst(arena, w);
                        apply(arena, p, first)
                    };
                    let motive = {
                        let domain = boxed(arena, p_fst_w_ambient);
                        let body = {
                            let p = variable(arena, 5);
                            let w = variable(arena, 1);
                            let first = fst(arena, w);
                            apply(arena, p, first)
                        };
                        lambda(arena, domain, body)
                    };
                    let unbox_body = {
                        let body = variable(arena, 0);
                        lambda(arena, p_fst_w_ambient, body)
                    };
                    let w = variable(arena, 0);
                    let second = snd(arena, w);
                    box_elim(arena, motive, unbox_body, second)
                };
                let w = variable(arena, 0);
                let first = fst(arena, w);
                let step = variable(arena, 1);
                let step_at = apply(arena, step, first);
                let body = apply(arena, step_at, projected);
                lambda(arena, domain, body)
            };
            let present = variable(arena, 1);
            squash_elim(arena, proposition, function, present)
        };
        let inner = lambda(arena, step_domain, elimination);
        let inner = lambda(arena, present_domain, inner);
        let inner = lambda(arena, q_domain, inner);
        let inner = lambda(arena, p_domain, inner);
        lambda(arena, a_sort, inner)
    };
    Declaration::definition(3, statement, body)
}

/// The five-declaration producer signature: two assumptions, three
/// checked definitions.
fn existence_signature(arena: &mut TermArena) -> Vec<Declaration> {
    vec![
        u32_declaration(arena),
        choice_declaration(arena),
        subset_declaration(arena),
        chosen_declaration(arena),
        transfer_declaration(arena),
    ]
}

/// The arity-0 context `P : Π(_:U32). Strict 0, h : Squash (subset[0]
/// P)` — a subset predicate and the squashed evidence that its subset
/// is inhabited — over the checked signature.
fn subset_context(arena: &mut TermArena) -> (Context, Vec<TermHandle>) {
    let p_binding = {
        let domain = u32(arena);
        let codomain = strict_sort(arena, 0);
        pi(arena, domain, codomain)
    };
    let h_binding = {
        // Under P alone: P is 0.
        let p = variable(arena, 0);
        let subset_p = subset_applied(arena, Level::Constant(0), p);
        squash(arena, subset_p)
    };
    let declarations = existence_signature(arena);
    let signature = check_signature(arena, &declarations, &mut budget()).unwrap();
    let context = Context::empty()
        .with_signature(signature)
        .extend(p_binding)
        .extend(h_binding);
    (context, vec![p_binding, h_binding])
}

/// `chosen[0] P h` written inside `subset_context` (P is 1, h is 0) —
/// the declaration takes the predicate and the squash directly.
fn chosen_member(arena: &mut TermArena) -> TermHandle {
    let chosen = constant(arena, CHOSEN, vec![Level::Constant(0)]);
    let p = variable(arena, 1);
    let chosen_at = apply(arena, chosen, p);
    let h = variable(arena, 0);
    apply(arena, chosen_at, h)
}

#[test]
fn squashed_existence_composes_through_a_hypothesis_without_choice() {
    let mut arena = TermArena::new();
    let declarations = existence_signature(&mut arena);
    let signature = check_signature(&mut arena, &declarations, &mut budget()).unwrap();

    // `transfer`'s statement and body name no constants at all — it is
    // proved from `unsq`/`unbox` alone, so its assumption closure is
    // exactly empty: squashed existence composes with no choice axiom.
    let transfer_ty = signature.declarations()[TRANSFER as usize].ty;
    let transfer_body = signature.declarations()[TRANSFER as usize].body.unwrap();
    let closure = judgment_assumption_closure(&arena, &signature, &[transfer_ty, transfer_body]);
    assert!(
        closure.is_empty(),
        "the proved composition commits to no assumption — choice is unnecessary"
    );

    // In Γ = A, P, Q, w : Σ(x:A). Box (P x), step, the elimination
    // `transfer[0,0,0] A P Q (sq w) step` computes: `unsq` fires on the
    // canonical `sq`, `unbox` opens the boxed premise, and the result
    // is the neutral spine `step (fst w) (unbox …)`.
    let context = {
        let a_binding = type_sort(&mut arena, 0);
        let p_binding = {
            let a = variable(&mut arena, 0);
            let strict = strict_sort(&mut arena, 0);
            pi(&mut arena, a, strict)
        };
        let q_binding = {
            let a = variable(&mut arena, 1);
            let strict = strict_sort(&mut arena, 0);
            pi(&mut arena, a, strict)
        };
        let w_binding = witness_bundle(&mut arena, 2, 1);
        let step_binding = step_type(&mut arena, 3, 2, 1);
        Context::empty()
            .with_signature(signature.clone())
            .extend(a_binding)
            .extend(p_binding)
            .extend(q_binding)
            .extend(w_binding)
            .extend(step_binding)
    };
    let application = {
        let transfer = constant(
            &mut arena,
            TRANSFER,
            vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        );
        let a = variable(&mut arena, 4);
        let applied = apply(&mut arena, transfer, a);
        let p = variable(&mut arena, 3);
        let applied = apply(&mut arena, applied, p);
        let q = variable(&mut arena, 2);
        let applied = apply(&mut arena, applied, q);
        let present = {
            let bundle = witness_bundle(&mut arena, 4, 3);
            let w = variable(&mut arena, 1);
            squash_intro(&mut arena, bundle, w)
        };
        let applied = apply(&mut arena, applied, present);
        let step = variable(&mut arena, 0);
        apply(&mut arena, applied, step)
    };
    // The elimination lands at the strict `Squash (Σ(y:A). Box (Q
    // y))` — under Γ, A is 4 and Q is 2.
    let target = squashed_bundle(&mut arena, 4, 2);
    check_type(&mut arena, &context, application, target, &mut budget()).unwrap();

    let mut spent = budget();
    let before = spent.remaining();
    let normal =
        weak_head_normalize(&mut arena, context.signature(), application, &mut spent).unwrap();
    // The result is the neutral spine `step (fst w) (unbox …)` — under
    // Γ, step is 0 and w is 1.
    let (function, argument) = match arena.get(normal) {
        Term::Apply { function, argument } => (function, argument),
        other => panic!("the elimination reduces to an application spine, got {other:?}"),
    };
    let (head, first_argument) = match arena.get(function) {
        Term::Apply { function, argument } => (function, argument),
        other => panic!("the spine applies the step hypothesis, got {other:?}"),
    };
    assert!(
        matches!(arena.get(head), Term::Variable(0))
            && matches!(arena.get(first_argument), Term::Fst { pair } if matches!(arena.get(pair), Term::Variable(1))),
        "the member is projected and fed to the step hypothesis"
    );
    let scrutinee = match arena.get(argument) {
        Term::BoxElim {
            motive,
            body,
            scrutinee,
        } => {
            assert!(
                matches!(arena.get(motive), Term::Lambda { .. })
                    && matches!(arena.get(body), Term::Lambda { .. }),
                "the boxed premise opens at a constant strict motive"
            );
            scrutinee
        }
        other => panic!("the premise is opened by unbox, got {other:?}"),
    };
    assert!(
        matches!(arena.get(scrutinee), Term::Snd { pair } if matches!(arena.get(pair), Term::Variable(1))),
        "the membership proof is the bundle's second projection"
    );
    assert!(
        before - spent.remaining() > 0,
        "the computation is budgeted work — measured, not assumed"
    );

    // The result is squashed evidence: its sort is `Strict`, so the
    // member inside can never be projected into relevant data.
    let inferred = infer_type(&mut arena, &context, application, &mut budget()).unwrap();
    let head =
        weak_head_normalize(&mut arena, context.signature(), inferred, &mut budget()).unwrap();
    assert!(
        matches!(arena.get(head), Term::Squash { .. }),
        "squashed witness existence stays in the strict layer"
    );
}

#[test]
fn an_admitted_choice_axiom_supplies_a_mathematical_member() {
    let mut arena = TermArena::new();
    let (context, _bindings) = subset_context(&mut arena);

    // `fst (chosen P h) : U32` — the member of a nonempty subset of
    // `u32`, supplied mathematically by the admitted axiom.
    let member = {
        let chosen = chosen_member(&mut arena);
        fst(&mut arena, chosen)
    };
    let u32_ty = u32(&mut arena);
    check_type(&mut arena, &context, member, u32_ty, &mut budget()).unwrap();

    // `snd (chosen P h) : Box (P (fst (chosen P h)))` — the membership
    // evidence is dependent: it speaks of the very member `fst`
    // projects.
    let membership = {
        let chosen = chosen_member(&mut arena);
        snd(&mut arena, chosen)
    };
    let membership_ty = {
        let p = variable(&mut arena, 1);
        let chosen = chosen_member(&mut arena);
        let member = fst(&mut arena, chosen);
        let at = apply(&mut arena, p, member);
        boxed(&mut arena, at)
    };
    check_type(
        &mut arena,
        &context,
        membership,
        membership_ty,
        &mut budget(),
    )
    .unwrap();

    // The member is *noncomputable*: `chosen` unfolds to `choice`, an
    // assumption constant, which stays neutral — `fst` cannot project
    // a concrete `u32` because the axiom supplies no algorithm.
    let chosen = chosen_member(&mut arena);
    let normal =
        weak_head_normalize(&mut arena, context.signature(), chosen, &mut budget()).unwrap();
    let mut spine = normal;
    while let Term::Apply { function, .. } = arena.get(spine) {
        spine = function;
    }
    assert!(
        matches!(
            arena.get(spine),
            Term::Constant {
                declaration: CHOICE,
                ..
            }
        ),
        "the member's evidence spine is headed by the neutral axiom"
    );

    // The exact assumption closure names `U32` and `choice` — and only
    // them: `subset`/`chosen` are definitions, and a closure computed
    // over stored statements and bodies records the axiom even though
    // this judgment's evidence never unfolds it.
    let closure =
        judgment_assumption_closure(&arena, context.signature(), &[member, membership_ty]);
    assert_eq!(
        closure,
        BTreeSet::from([U32, CHOICE]),
        "the judgment commits to exactly the carrier and the admitted axiom"
    );
}

#[test]
fn extraction_without_the_axiom_is_refused() {
    let mut arena = TermArena::new();
    let (context, _bindings) = subset_context(&mut arena);

    // `unsq` cannot land in the relevant `Subset P`: squashed existence
    // proves propositions, never data — unauthorized strict
    // elimination, refused by name.
    let subset_p = {
        let p = variable(&mut arena, 1);
        subset_applied(&mut arena, Level::Constant(0), p)
    };
    let refused = {
        let function = {
            let body = variable(&mut arena, 0);
            lambda(&mut arena, subset_p, body)
        };
        let h = variable(&mut arena, 0);
        squash_elim(&mut arena, subset_p, function, h)
    };
    assert!(matches!(
        infer_type(&mut arena, &context, refused, &mut budget()),
        Err(CoreError::SquashTargetNotStrict { .. })
    ));

    // `fst` on the squash itself is not a pair — the bundle inside is
    // inaccessible without the axiom.
    let projected = {
        let h = variable(&mut arena, 0);
        fst(&mut arena, h)
    };
    assert!(matches!(
        infer_type(&mut arena, &context, projected, &mut budget()),
        Err(CoreError::NotAPair { .. })
    ));

    // What *is* derivable stays squashed: `Squash (Subset P) → Squash
    // U32` by `unsq` at the strict target, composing the member out
    // with `sq` — existence of a member, never the member. Its closure
    // is {U32}, not {U32, choice}.
    let member_exists = {
        let statement = {
            let p_domain = {
                let domain = u32(&mut arena);
                let codomain = strict_sort(&mut arena, 0);
                pi(&mut arena, domain, codomain)
            };
            let codomain = {
                // Under P: `Squash (subset P) → Squash U32`.
                let p = variable(&mut arena, 0);
                let subset_p = subset_applied(&mut arena, Level::Constant(0), p);
                let inhabited = squash(&mut arena, subset_p);
                let target = {
                    let carrier = u32(&mut arena);
                    squash(&mut arena, carrier)
                };
                pi(&mut arena, inhabited, target)
            };
            pi(&mut arena, p_domain, codomain)
        };
        let body = {
            let p_domain = {
                let domain = u32(&mut arena);
                let codomain = strict_sort(&mut arena, 0);
                pi(&mut arena, domain, codomain)
            };
            let inner = {
                // Under P: `λ(h : Squash (subset P)). unsq (Squash
                // U32) (λ(w : subset P). sq (fst w)) h`.
                let p = variable(&mut arena, 0);
                let subset_p = subset_applied(&mut arena, Level::Constant(0), p);
                let inhabited = squash(&mut arena, subset_p);
                let elimination = {
                    let target = {
                        let carrier = u32(&mut arena);
                        squash(&mut arena, carrier)
                    };
                    let function = {
                        // The domain annotation is ambient to the `w`
                        // binder — under P and h, P is 1.
                        let p = variable(&mut arena, 1);
                        let domain = subset_applied(&mut arena, Level::Constant(0), p);
                        let member = {
                            let carrier = u32(&mut arena);
                            let w = variable(&mut arena, 0);
                            let first = fst(&mut arena, w);
                            squash_intro(&mut arena, carrier, first)
                        };
                        lambda(&mut arena, domain, member)
                    };
                    let scrutinee = variable(&mut arena, 0);
                    squash_elim(&mut arena, target, function, scrutinee)
                };
                lambda(&mut arena, inhabited, elimination)
            };
            lambda(&mut arena, p_domain, inner)
        };
        (statement, body)
    };
    check_type(
        &mut arena,
        &Context::empty().with_signature(context.signature().clone()),
        member_exists.1,
        member_exists.0,
        &mut budget(),
    )
    .expect("the squashed weakening is derivable without choice");
    let closure = judgment_assumption_closure(
        &arena,
        context.signature(),
        &[member_exists.0, member_exists.1],
    );
    assert_eq!(
        closure,
        BTreeSet::from([U32]),
        "squashed existence of a member needs only the carrier, never the axiom"
    );
}

#[test]
fn the_choice_certificate_redecides_with_exact_closure() {
    let mut arena = TermArena::new();
    let (_context, bindings) = subset_context(&mut arena);

    // The certificate claims `Γ ⊢ snd (chosen P h) : Box (P (fst
    // (chosen P h)))` — dependent membership evidence for the chosen
    // member — under the producer signature.
    let term = {
        let chosen = chosen_member(&mut arena);
        snd(&mut arena, chosen)
    };
    let expected = {
        let p = variable(&mut arena, 1);
        let chosen = chosen_member(&mut arena);
        let member = fst(&mut arena, chosen);
        let at = apply(&mut arena, p, member);
        boxed(&mut arena, at)
    };
    let certificate = MathematicalCertificate {
        signature: existence_signature(&mut arena),
        level_arity: 0,
        context: bindings.clone(),
        term,
        expected,
    };
    let mut spent = budget();
    let before = spent.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut spent).unwrap();
    assert!(
        before - spent.remaining() > 0,
        "verification is measured work, not a flag"
    );
    assert_eq!(
        certificate_assumption_closure(&arena, &certificate),
        BTreeSet::from([U32, CHOICE]),
        "the certificate's closure retains the admitted axiom through the judgment"
    );

    // The same judgment under a signature that drops `choice` is not
    // the same evidence: `chosen`'s body now resolves its constants
    // against a shorter prefix — `Constant 2` names `chosen` itself
    // under a prefix that ends at `subset`, so the declaration fails
    // before the judgment runs. The assumption is load-bearing, not
    // decoration.
    let mut forged = certificate.clone();
    forged.signature = vec![
        u32_declaration(&mut arena),
        subset_declaration(&mut arena),
        chosen_declaration(&mut arena),
        transfer_declaration(&mut arena),
    ];
    assert!(
        verify_mathematical_certificate(&mut arena, &forged, &mut budget()).is_err(),
        "a signature without the axiom cannot carry the judgment"
    );

    // And the axiom-free *derivation* of the member rejects inside
    // checking, not just in closure accounting: `unsq` into the
    // relevant subset is refused in `check_type`.
    let axiom_free = {
        let subset_p = {
            let p = variable(&mut arena, 1);
            subset_applied(&mut arena, Level::Constant(0), p)
        };
        let function = {
            let body = variable(&mut arena, 0);
            lambda(&mut arena, subset_p, body)
        };
        let h = variable(&mut arena, 0);
        squash_elim(&mut arena, subset_p, function, h)
    };
    let mut axiom_free_certificate = certificate.clone();
    axiom_free_certificate.term = axiom_free;
    assert!(matches!(
        verify_mathematical_certificate(&mut arena, &axiom_free_certificate, &mut budget()),
        Err(CoreError::SquashTargetNotStrict { .. })
    ));

    // Conversion is honest at the boundary: the chosen member is not
    // convertible to another term at the relevant `U32` — the strict
    // collapse never fires at a relevant type.
    let context = {
        let p_binding = bindings[0];
        let h_binding = bindings[1];
        let declarations = existence_signature(&mut arena);
        let signature = check_signature(&mut arena, &declarations, &mut budget()).unwrap();
        Context::empty()
            .with_signature(signature)
            .extend(p_binding)
            .extend(h_binding)
    };
    let member = {
        let chosen = chosen_member(&mut arena);
        fst(&mut arena, chosen)
    };
    let other = variable(&mut arena, 0);
    let member_ty = u32(&mut arena);
    assert!(
        !convertible(
            &mut arena,
            &context,
            member,
            other,
            member_ty,
            &mut budget()
        )
        .unwrap(),
        "the chosen member and the hypothesis stay distinct at the relevant carrier"
    );
}

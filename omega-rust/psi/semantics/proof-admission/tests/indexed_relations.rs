//! The derived indexed scheme at the strict layer: a squashed indexed
//! family is the profile's *logical inductive relation* — existence of
//! a derivation without witness access — and the file closes out the
//! encoding's refusal boundary that the vector/derivation/mutual/nested
//! demonstrations do not already pin:
//!
//! * `Squash (IW …)` is `Strict`-sorted existence over the family.
//!   `unsq` eliminates it only into a strict proposition; asking it to
//!   land in relevant data (`Two`), feeding it the un-squashed
//!   derivation, treating the squashed witness as a derivation, or
//!   squashing an already-strict carrier are each refused by name.
//! * `iindW`'s motive must land in `Type w`. A `Strict`-codomain motive
//!   — unauthorized strict elimination through the relevant eliminator —
//!   is refused; the reference core's route is `Box`: eliminate into
//!   `Box (Squash P)` and `unbox` to the strict target. Both halves are
//!   witnessed computing on a neutral child function.
//! * A description cannot name the family being defined. The encoding's
//!   only name for a family is the `Constant` spine `IW I A B out next
//!   i`, and `check_signature` resolves constants against the strict
//!   prefix — a `B` or `next` argument that references the declaration
//!   under check (self) or a later one (forward) fails scope resolution
//!   as `UnknownDeclaration` before any typing rule sees it. Negative
//!   recursion is therefore not a positivity check the producer can
//!   evade: it is the absence of a name. The control signature shows a
//!   *completed* family embedded negatively inside a later description
//!   (`B' a = IW[done] zero → Two`) is checked and constructs.

use proof_admission::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, INDEXED_W, IndexedFamily,
    Level, MathematicalCertificate, Signature, Sort, Term, TermArena, TermHandle,
    certificate_assumption_closure, check_signature, check_type, indexed_scheme, infer_sort,
    infer_type, verify_mathematical_certificate, weak_head_normalize,
};

fn budget() -> Budget {
    Budget::new(DEFAULT_CONVERSION_STEPS)
}

/// The eliminator ceiling: `iindW`'s typing and computation unfold the
/// whole `IndexedAt`/`IW` encoding — the `J` transport, the repacked
/// child functions, the pair-eta closures — for every application, even
/// when the description is this file's one-label leaf. Measured spend
/// on this host is 41 steps for the head computation; checking the
/// `iindW` application (which re-decides the step type against the
/// instantiated statement) dominates and stays far below the bound —
/// `StepCeiling` remains the decidability witness.
fn measure_budget() -> Budget {
    Budget::new(1 << 18)
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

fn two(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::Two)
}

fn two_zero(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoZero)
}

fn two_one(arena: &mut TermArena) -> TermHandle {
    arena.insert(Term::TwoOne)
}

fn id(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
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

fn box_intro(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::BoxIntro { ty, value })
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

fn constant(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

/// `Id Two one one` — the dead child position: no closed inhabitant, so
/// every node of the leaf family is a leaf.
fn dead_positions(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_one(arena);
    let right = two_one(arena);
    id(arena, ty, left, right)
}

/// `Id Two zero zero` — a live strict target for `unsq` landings.
fn zero_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let left = two_zero(arena);
    let right = two_zero(arena);
    id(arena, ty, left, right)
}

/// The leaf description as plain terms:
///
/// ```text
/// I    = Two                       level index
/// A    = Two                       one meaningful label, `zero`
/// B a  = Id Two one one            dead — nodes carry no children
/// out  = λa. a                     every node lands at its label
/// next = λa. λ_. a                 children would land at `a` (vacuous)
/// ```
///
/// `IW zero` is then a one-constructor inductive family — an `isup zero
/// g` exists whenever `g` supplies the (uninhabited) child positions.
fn leaf_family(arena: &mut TermArena) -> IndexedFamily {
    let children = {
        let domain = two(arena);
        let body = dead_positions(arena);
        lambda(arena, domain, body)
    };
    let out = {
        let domain = two(arena);
        let body = variable(arena, 0);
        lambda(arena, domain, body)
    };
    let next = {
        let domain = two(arena);
        let inner = {
            let domain = dead_positions(arena);
            let body = variable(arena, 1);
            lambda(arena, domain, body)
        };
        lambda(arena, domain, inner)
    };
    IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: two(arena),
        carrier: two(arena),
        children,
        out,
        next,
    }
}

/// The checked five-declaration scheme.
fn scheme_signature(arena: &mut TermArena) -> Signature {
    let declarations = indexed_scheme(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

#[test]
fn a_squashed_family_is_existence_without_witness_access() {
    let mut arena = TermArena::new();
    let family = leaf_family(&mut arena);
    let signature = scheme_signature(&mut arena);

    // Γ = g : Π(_ : Id Two one one). IW zero — a neutral child
    // function, so `isup zero g` is a derivation with no closed
    // children and nothing to extract.
    let g_binding = {
        let domain = dead_positions(&mut arena);
        let index = two_zero(&mut arena);
        let codomain = family.indexed_w(&mut arena, index);
        pi(&mut arena, domain, codomain)
    };
    let context = Context::empty()
        .with_signature(signature.clone())
        .extend(g_binding);

    let index = two_zero(&mut arena);
    let iw_zero = family.indexed_w(&mut arena, index);
    assert!(matches!(
        infer_sort(&mut arena, &context, iw_zero, &mut budget()).unwrap(),
        Sort::Type(_)
    ));
    let squashed = squash(&mut arena, iw_zero);
    assert!(
        matches!(
            infer_sort(&mut arena, &context, squashed, &mut budget()).unwrap(),
            Sort::Strict(_)
        ),
        "Squash moves the relevant family into the strict layer"
    );

    // `isup zero g : IW zero` — `out zero` computes to `zero`.
    let derivation = {
        let label = two_zero(&mut arena);
        let children_fn = variable(&mut arena, 0);
        family.sup(&mut arena, label, children_fn)
    };
    check_type(&mut arena, &context, derivation, iw_zero, &mut budget()).unwrap();

    // `sq (isup zero g) : Squash (IW zero)` — existence, boxed at the
    // strict layer.
    let witness = squash_intro(&mut arena, iw_zero, derivation);
    let inferred = infer_type(&mut arena, &context, witness, &mut budget()).unwrap();
    assert!(arena.structurally_equal(inferred, squashed));

    // `unsq` reaches a strict proposition: `P := Squash (Id Two zero
    // zero)`, `f := λ(_ : IW zero). sq (refl Two zero)`, and the
    // elimination computes on the canonical witness.
    let proposition = {
        let target = zero_position(&mut arena);
        squash(&mut arena, target)
    };
    let function = {
        let body = {
            let ty = two(&mut arena);
            let value = two_zero(&mut arena);
            let proof = refl(&mut arena, ty, value);
            let target = zero_position(&mut arena);
            squash_intro(&mut arena, target, proof)
        };
        lambda(&mut arena, iw_zero, body)
    };
    let elimination = squash_elim(&mut arena, proposition, function, witness);
    check_type(
        &mut arena,
        &context,
        elimination,
        proposition,
        &mut budget(),
    )
    .unwrap();
    let mut spent = measure_budget();
    let before = spent.remaining();
    let normal =
        weak_head_normalize(&mut arena, context.signature(), elimination, &mut spent).unwrap();
    let expected_normal = {
        let ty = two(&mut arena);
        let value = two_zero(&mut arena);
        let proof = refl(&mut arena, ty, value);
        let target = zero_position(&mut arena);
        squash_intro(&mut arena, target, proof)
    };
    assert!(
        arena.structurally_equal(normal, expected_normal),
        "unsq P f (sq d) computes to f d — here the boxed `refl`"
    );
    assert!(
        before - spent.remaining() > 0,
        "the computation is budgeted work, not an axiom"
    );

    // A zero budget refuses the same computation — decidability is
    // witnessed, not assumed.
    assert!(matches!(
        weak_head_normalize(
            &mut arena,
            context.signature(),
            elimination,
            &mut Budget::new(0)
        ),
        Err(CoreError::StepCeiling)
    ));

    // ── The refusal cells ──────────────────────────────────────────
    //
    // `unsq` into a relevant target — asking squashed existence to
    // produce data — is unauthorized strict elimination.
    let relevant_proposition = two(&mut arena);
    let refused = squash_elim(&mut arena, relevant_proposition, function, witness);
    assert!(matches!(
        infer_type(&mut arena, &context, refused, &mut budget()),
        Err(CoreError::SquashTargetNotStrict { .. })
    ));

    // `unsq` over the un-squashed derivation: `IW (out zero)` whnf's to
    // the packed `Sigma`, never to a `Squash`.
    let refused = squash_elim(&mut arena, proposition, function, derivation);
    assert!(matches!(
        infer_type(&mut arena, &context, refused, &mut budget()),
        Err(CoreError::NotASquash { .. })
    ));

    // The squashed witness is not itself a derivation: `Squash (IW
    // zero)` does not convert to `IW zero` — existence never supplies
    // the witness.
    assert!(matches!(
        check_type(&mut arena, &context, witness, iw_zero, &mut budget()),
        Err(CoreError::TypeMismatch { .. })
    ));

    // Squashing an already-strict carrier, and boxing a relevant one,
    // are malformed encodings — each refused by its own rule.
    let double_squash = squash(&mut arena, squashed);
    assert!(matches!(
        infer_sort(&mut arena, &context, double_squash, &mut budget()),
        Err(CoreError::StrictSquashDomain { .. })
    ));
    let bad_box = boxed(&mut arena, iw_zero);
    assert!(matches!(
        infer_sort(&mut arena, &context, bad_box, &mut budget()),
        Err(CoreError::NonStrictBoxDomain { .. })
    ));

    // The squashed judgment re-decides as a certificate. `isup`, `IW`
    // and the rest of the scheme are *definitions*, so the assumption
    // closure is exactly empty — the judgment commits to no axioms.
    let certificate = MathematicalCertificate {
        signature: indexed_scheme(&mut arena),
        level_arity: 0,
        context: vec![g_binding],
        term: witness,
        expected: squashed,
    };
    let mut spent = measure_budget();
    let before = spent.remaining();
    verify_mathematical_certificate(&mut arena, &certificate, &mut spent).unwrap();
    assert!(before - spent.remaining() > 0);
    let closure = certificate_assumption_closure(&arena, &certificate);
    assert!(
        closure.is_empty(),
        "a judgment over scheme definitions commits to no assumptions"
    );
}

#[test]
fn indexed_elimination_reaches_a_strict_target_through_boxing() {
    let mut arena = TermArena::new();
    let family = leaf_family(&mut arena);
    let signature = scheme_signature(&mut arena);

    // Γ = a : Two, g : Π(b : B a). IW (next a b) — a neutral label and
    // a neutral child function.
    let a_binding = two(&mut arena);
    let g_binding = {
        // Ambient under [a]: a is 0; under the `b` binder a is 1.
        let a_ambient = variable(&mut arena, 0);
        let domain = apply(&mut arena, family.children, a_ambient);
        let codomain = {
            let a = variable(&mut arena, 1);
            let bound = variable(&mut arena, 0);
            let next_a = apply(&mut arena, family.next, a);
            let required = apply(&mut arena, next_a, bound);
            family.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = Context::empty()
        .with_signature(signature.clone())
        .extend(a_binding)
        .extend(g_binding);
    // Under Γ: g is 0, a is 1.
    let a = variable(&mut arena, 1);
    let children_fn = variable(&mut arena, 0);
    let derivation = family.sup(&mut arena, a, children_fn);
    let at_a = family.indexed_w(&mut arena, a);
    check_type(&mut arena, &context, derivation, at_a, &mut budget()).unwrap();

    // `Q := λ(i : Two). λ(_ : IW i). Box (Squash (Id Two i i))` — a
    // *relevant* `Type 0` codomain carrying a strict payload, the only
    // channel the reference core gives `iindW` for strict targets.
    let motive = {
        let i_domain = two(&mut arena);
        let i = variable(&mut arena, 0);
        let t_domain = family.indexed_w(&mut arena, i);
        let body = {
            // Under λi.λt: i is 1.
            let i = variable(&mut arena, 1);
            let endpoints = {
                let ty = two(&mut arena);
                id(&mut arena, ty, i, i)
            };
            let squashed = squash(&mut arena, endpoints);
            boxed(&mut arena, squashed)
        };
        let inner = lambda(&mut arena, t_domain, body);
        lambda(&mut arena, i_domain, inner)
    };

    // `s := λa. λg. λih. box_{Squash (Id Two (out a) (out a))}
    //       (sq_{Id Two (out a) (out a)} (refl Two (out a)))`
    // — the step ignores the (vacuous) children and the hypothesis.
    let step = {
        let g_domain = {
            // Under λa: a is 0; under the `b` binder a is 1.
            let a_ambient = variable(&mut arena, 0);
            let domain = apply(&mut arena, family.children, a_ambient);
            let codomain = {
                let a = variable(&mut arena, 1);
                let bound = variable(&mut arena, 0);
                let next_a = apply(&mut arena, family.next, a);
                let required = apply(&mut arena, next_a, bound);
                family.indexed_w(&mut arena, required)
            };
            pi(&mut arena, domain, codomain)
        };
        let ih_domain = {
            // Under λa.λg: a is 1, g is 0; under the `b` binder a is 2,
            // g is 1, b is 0. The hypothesis at `b` is
            // `Q (next a b) (g b)`.
            let a_ambient = variable(&mut arena, 1);
            let domain = apply(&mut arena, family.children, a_ambient);
            let codomain = {
                let a = variable(&mut arena, 2);
                let g = variable(&mut arena, 1);
                let bound = variable(&mut arena, 0);
                let next_a = apply(&mut arena, family.next, a);
                let next_ab = apply(&mut arena, next_a, bound);
                let gb = apply(&mut arena, g, bound);
                let at_index = apply(&mut arena, motive, next_ab);
                apply(&mut arena, at_index, gb)
            };
            pi(&mut arena, domain, codomain)
        };
        let body = {
            // Under λa.λg.λih: a is 2.
            let a_bound = variable(&mut arena, 2);
            let out_a = apply(&mut arena, family.out, a_bound);
            let endpoints = {
                let ty = two(&mut arena);
                id(&mut arena, ty, out_a, out_a)
            };
            let proof = {
                let ty = two(&mut arena);
                refl(&mut arena, ty, out_a)
            };
            let squashed_proof = squash_intro(&mut arena, endpoints, proof);
            let payload = {
                let a_bound = variable(&mut arena, 2);
                let out_a = apply(&mut arena, family.out, a_bound);
                let endpoints = {
                    let ty = two(&mut arena);
                    id(&mut arena, ty, out_a, out_a)
                };
                squash(&mut arena, endpoints)
            };
            box_intro(&mut arena, payload, squashed_proof)
        };
        let inner = lambda(&mut arena, ih_domain, body);
        let middle = lambda(&mut arena, g_domain, inner);
        let a_domain = two(&mut arena);
        lambda(&mut arena, a_domain, middle)
    };

    let elimination = family.ind(&mut arena, Level::Constant(0), motive, step, a, derivation);
    let expected_type = {
        let at_index = apply(&mut arena, motive, a);
        apply(&mut arena, at_index, derivation)
    };
    check_type(
        &mut arena,
        &context,
        elimination,
        expected_type,
        &mut measure_budget(),
    )
    .unwrap();

    // `iindW` computes through the `isup` node to the boxed squashed
    // `refl` — a strict proposition travelling as relevant data.
    let mut spent = measure_budget();
    let before = spent.remaining();
    let normal =
        weak_head_normalize(&mut arena, context.signature(), elimination, &mut spent).unwrap();
    let expected_normal = {
        let out_a = apply(&mut arena, family.out, a);
        let endpoints = {
            let ty = two(&mut arena);
            id(&mut arena, ty, out_a, out_a)
        };
        let proof = {
            let ty = two(&mut arena);
            refl(&mut arena, ty, out_a)
        };
        let squashed_proof = squash_intro(&mut arena, endpoints, proof);
        let payload = {
            let out_a = apply(&mut arena, family.out, a);
            let endpoints = {
                let ty = two(&mut arena);
                id(&mut arena, ty, out_a, out_a)
            };
            squash(&mut arena, endpoints)
        };
        box_intro(&mut arena, payload, squashed_proof)
    };
    assert!(
        arena.structurally_equal(normal, expected_normal),
        "iindW computes to `s a g ih` = the boxed squashed reflexivity"
    );
    let spent_on_elimination = before - spent.remaining();
    assert!(spent_on_elimination > 0);

    // `unbox` lands at the strict target: `P' := λ(_ : Box (Squash (Id
    // Two a a))). Squash (Id Two a a)`, `f' := λp. p`.
    let unbox_motive = {
        // Domain in ambient Γ: a is 1; body under the binder: a is 2.
        let endpoints = {
            let ty = two(&mut arena);
            id(&mut arena, ty, a, a)
        };
        let domain = {
            let squashed = squash(&mut arena, endpoints);
            boxed(&mut arena, squashed)
        };
        let shifted = variable(&mut arena, 2);
        let body = {
            let ty = two(&mut arena);
            let endpoints = id(&mut arena, ty, shifted, shifted);
            squash(&mut arena, endpoints)
        };
        lambda(&mut arena, domain, body)
    };
    let unbox_body = {
        // λ(p : Squash (Id Two a a)). p — domain in ambient Γ.
        let endpoints = {
            let ty = two(&mut arena);
            id(&mut arena, ty, a, a)
        };
        let domain = squash(&mut arena, endpoints);
        let bound = variable(&mut arena, 0);
        lambda(&mut arena, domain, bound)
    };
    let unboxed = box_elim(&mut arena, unbox_motive, unbox_body, elimination);
    let landing = {
        let endpoints = {
            let ty = two(&mut arena);
            id(&mut arena, ty, a, a)
        };
        squash(&mut arena, endpoints)
    };
    check_type(
        &mut arena,
        &context,
        unboxed,
        landing,
        &mut measure_budget(),
    )
    .unwrap();
    let normal_unboxed = weak_head_normalize(
        &mut arena,
        context.signature(),
        unboxed,
        &mut measure_budget(),
    )
    .unwrap();
    let expected_unboxed = {
        let out_a = apply(&mut arena, family.out, a);
        let endpoints = {
            let ty = two(&mut arena);
            id(&mut arena, ty, out_a, out_a)
        };
        let proof = {
            let ty = two(&mut arena);
            refl(&mut arena, ty, out_a)
        };
        squash_intro(&mut arena, endpoints, proof)
    };
    assert!(
        arena.structurally_equal(normal_unboxed, expected_unboxed),
        "unbox P' f' (iindW …) computes through the box to `sq (refl (out a))`"
    );

    // A `Strict`-codomain motive is unauthorized strict elimination:
    // `Q_s : Π(i : Two). Π(_ : IW i). Strict 0` fails `iindW`'s motive
    // domain `Π i. Π(_ : IW i). Type w` — strict targets travel through
    // `Box`, never through the eliminator's own sort.
    let strict_motive_binding = {
        let i_domain = two(&mut arena);
        let i = variable(&mut arena, 0);
        let t_domain = family.indexed_w(&mut arena, i);
        let codomain = strict_sort(&mut arena, 0);
        let inner = pi(&mut arena, t_domain, codomain);
        pi(&mut arena, i_domain, inner)
    };
    let strict_context = context.extend(strict_motive_binding);
    // Under [a, g, Q_s]: Q_s is 0, g is 1, a is 2.
    let refused = {
        let strict_motive = variable(&mut arena, 0);
        let a = variable(&mut arena, 2);
        let g = variable(&mut arena, 1);
        let derivation = family.sup(&mut arena, a, g);
        family.ind(
            &mut arena,
            Level::Constant(0),
            strict_motive,
            step,
            a,
            derivation,
        )
    };
    assert!(
        matches!(
            infer_type(&mut arena, &strict_context, refused, &mut measure_budget()),
            Err(CoreError::TypeMismatch { .. } | CoreError::ArgumentTypeMismatch { .. })
        ),
        "the eliminator's motive must land at a relevant sort"
    );
}

// ── The description constants ───────────────────────────────────────
//
// Positions 0–4 are the derived scheme; positions 5–8 are the shared
// head of a producer signature spelling the leaf description as
// definitions — the only machinery the encoding offers for *naming* a
// family inside a later description.

/// `dI : Type 0 := Two` — declaration 5.
const D_INDEX: u32 = 5;
/// `dA : Type 0 := Two` — declaration 6.
const D_CARRIER: u32 = 6;
/// `dOut : Π(_ : Two). Two := λa. a` — declaration 7.
const D_OUT: u32 = 7;
/// `dNext : Π(a : Two). Π(_ : Id Two one one). Two := λa. λ_. a` —
/// declaration 8.
const D_NEXT: u32 = 8;
/// `dB0 : Π(_ : Two). Type 0 := λ_. Id Two one one` — declaration 9 in
/// the completed control signature.
const D_CHILDREN: u32 = 9;
/// `dBNeg : Π(_ : Two). Type 0` — declaration 10 in the control.
const D_CHILDREN_NEG: u32 = 10;
/// `dNextNeg : Π(a : Two). Π(_ : Π(_ : IW[done] zero). Two). Two` —
/// declaration 11 in the control.
const D_NEXT_NEG: u32 = 11;

/// `IW·dI·dA·dB·dOut·dNext·i` — the encoding's only name for a family:
/// a `Constant` spine over earlier description declarations.
fn iw_spine(arena: &mut TermArena, children: u32, next: u32, index: TermHandle) -> TermHandle {
    let head = arena.insert(Term::Constant {
        declaration: INDEXED_W,
        levels: vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
    });
    let parameters = [
        constant(arena, D_INDEX),
        constant(arena, D_CARRIER),
        constant(arena, children),
        constant(arena, D_OUT),
        constant(arena, next),
        index,
    ];
    let mut spine = head;
    for parameter in parameters {
        spine = apply(arena, spine, parameter);
    }
    spine
}

/// The scheme plus `dI`, `dA`, `dOut`, `dNext` — positions 0–8.
fn description_prefix(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = indexed_scheme(arena);
    // [5] dI := Two.
    declarations.push(Declaration::definition(0, type_sort(arena, 0), two(arena)));
    // [6] dA := Two.
    declarations.push(Declaration::definition(0, type_sort(arena, 0), two(arena)));
    // [7] dOut := λa. a.
    let statement = {
        let domain = two(arena);
        let codomain = two(arena);
        pi(arena, domain, codomain)
    };
    let body = {
        let domain = two(arena);
        let body = variable(arena, 0);
        lambda(arena, domain, body)
    };
    declarations.push(Declaration::definition(0, statement, body));
    // [8] dNext := λa. λ_. a.
    let statement = {
        let domain = two(arena);
        let inner = {
            let domain = dead_positions(arena);
            let codomain = two(arena);
            pi(arena, domain, codomain)
        };
        pi(arena, domain, inner)
    };
    let body = {
        let domain = two(arena);
        let inner = {
            let domain = dead_positions(arena);
            let body = variable(arena, 1);
            lambda(arena, domain, body)
        };
        lambda(arena, domain, inner)
    };
    declarations.push(Declaration::definition(0, statement, body));
    declarations
}

/// `dB0 : Π(_ : Two). Type 0 := λ_. Id Two one one` — the leaf family's
/// child-position family, completed at declaration 9.
fn dead_children_declaration(arena: &mut TermArena) -> Declaration {
    let statement = {
        let domain = two(arena);
        let codomain = type_sort(arena, 0);
        pi(arena, domain, codomain)
    };
    let body = {
        let domain = two(arena);
        let codomain = dead_positions(arena);
        lambda(arena, domain, codomain)
    };
    Declaration::definition(0, statement, body)
}

#[test]
fn a_family_description_cannot_name_itself() {
    let mut arena = TermArena::new();

    // Self-reference: `dBSelf : Π(_ : Two). Type 0 := λ_. Π(_ : IW[with
    // dBSelf as B] zero). Two` — the `T → Two` negative-recursion
    // shape, where `T` is spelled through the very declaration under
    // check. The spine's third argument `Constant(9)` resolves against
    // a prefix that ends at 8.
    let self_signature = {
        let mut declarations = description_prefix(&mut arena);
        let statement = {
            let domain = two(&mut arena);
            let codomain = type_sort(&mut arena, 0);
            pi(&mut arena, domain, codomain)
        };
        let body = {
            let domain = two(&mut arena);
            let codomain = {
                let index = two_zero(&mut arena);
                let domain = iw_spine(&mut arena, D_CHILDREN, D_NEXT, index);
                let codomain = two(&mut arena);
                pi(&mut arena, domain, codomain)
            };
            lambda(&mut arena, domain, codomain)
        };
        // Position 9: the declaration names itself inside its own body.
        declarations.push(Declaration::definition(0, statement, body));
        declarations
    };
    assert!(
        matches!(
            check_signature(&mut arena, &self_signature, &mut budget()),
            Err(CoreError::UnknownDeclaration {
                declaration: 9,
                signature_len: 9,
            })
        ),
        "the family's only name is a constant that does not exist yet"
    );

    // Forward reference: the same move one slot later — `dBFwd`'s
    // `next` argument names `dNextLater`, which is only declared at
    // position 11. The check of declaration 10 sees a prefix of 10.
    let forward_signature = {
        let mut declarations = description_prefix(&mut arena);
        // Position 9: the completed leaf `B`.
        declarations.push(dead_children_declaration(&mut arena));
        // Position 10: `dBFwd := λ_. Π(_ : IW·dI·dA·dB0·dOut·dNextLater·
        // zero). Two`.
        let statement = {
            let domain = two(&mut arena);
            let codomain = type_sort(&mut arena, 0);
            pi(&mut arena, domain, codomain)
        };
        let body = {
            let domain = two(&mut arena);
            let codomain = {
                let index = two_zero(&mut arena);
                let domain = iw_spine(&mut arena, D_CHILDREN, D_NEXT_NEG, index);
                let codomain = two(&mut arena);
                pi(&mut arena, domain, codomain)
            };
            lambda(&mut arena, domain, codomain)
        };
        declarations.push(Declaration::definition(0, statement, body));
        // Position 11: `dNextLater := λa. λ_. a` — the intended target,
        // unreachable from the description that needed it.
        let statement = {
            let domain = two(&mut arena);
            let inner = {
                let domain = dead_positions(&mut arena);
                let codomain = two(&mut arena);
                pi(&mut arena, domain, codomain)
            };
            pi(&mut arena, domain, inner)
        };
        let body = {
            let domain = two(&mut arena);
            let inner = {
                let domain = dead_positions(&mut arena);
                let body = variable(&mut arena, 1);
                lambda(&mut arena, domain, body)
            };
            lambda(&mut arena, domain, inner)
        };
        declarations.push(Declaration::definition(0, statement, body));
        declarations
    };
    assert!(
        matches!(
            check_signature(&mut arena, &forward_signature, &mut budget()),
            Err(CoreError::UnknownDeclaration {
                declaration: 11,
                signature_len: 10,
            })
        ),
        "a forward name is refused identically — the prefix is the whole scope"
    );

    // The control: a *completed* family embedded negatively inside a
    // later description is legal. `dBNeg a = Π(_ : IW[leaf] zero).
    // Two` positions each child as a function out of the finished
    // leaf family — negative-shaped, but over a declaration that
    // already checked.
    let control_signature = {
        let mut declarations = description_prefix(&mut arena);
        declarations.push(dead_children_declaration(&mut arena));
        // Position 10: `dBNeg := λ_. Π(_ : IW·dI·dA·dB0·dOut·dNext·zero).
        // Two`.
        let statement = {
            let domain = two(&mut arena);
            let codomain = type_sort(&mut arena, 0);
            pi(&mut arena, domain, codomain)
        };
        let body = {
            let domain = two(&mut arena);
            let codomain = {
                let index = two_zero(&mut arena);
                let domain = iw_spine(&mut arena, D_CHILDREN, D_NEXT, index);
                let codomain = two(&mut arena);
                pi(&mut arena, domain, codomain)
            };
            lambda(&mut arena, domain, codomain)
        };
        declarations.push(Declaration::definition(0, statement, body));
        // Position 11: `dNextNeg : Π(a : Two). Π(_ : dBNeg a). Two :=
        // λa. λ_. a`, with `dBNeg a` spelled out as its unfolding.
        let statement = {
            let domain = two(&mut arena);
            let inner = {
                let index = two_zero(&mut arena);
                let source = iw_spine(&mut arena, D_CHILDREN, D_NEXT, index);
                let domain = {
                    let codomain = two(&mut arena);
                    pi(&mut arena, source, codomain)
                };
                let codomain = two(&mut arena);
                pi(&mut arena, domain, codomain)
            };
            pi(&mut arena, domain, inner)
        };
        let body = {
            let domain = two(&mut arena);
            let inner = {
                let index = two_zero(&mut arena);
                let source = iw_spine(&mut arena, D_CHILDREN, D_NEXT, index);
                let domain = {
                    let codomain = two(&mut arena);
                    pi(&mut arena, source, codomain)
                };
                let body = variable(&mut arena, 1);
                lambda(&mut arena, domain, body)
            };
            lambda(&mut arena, domain, inner)
        };
        declarations.push(Declaration::definition(0, statement, body));
        declarations
    };
    let signature = check_signature(&mut arena, &control_signature, &mut budget())
        .expect("a completed family may be embedded negatively inside a later description");
    assert_eq!(signature.len(), 12);

    // And the negatively-embedding family forms and constructs: `IW'`
    // at `zero`, then `isup zero g'` for a neutral
    // `g' : Π(_ : dBNeg zero). IW' (dNextNeg zero _)`.
    let embedded = IndexedFamily {
        levels: [Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        index: constant(&mut arena, D_INDEX),
        carrier: constant(&mut arena, D_CARRIER),
        children: constant(&mut arena, D_CHILDREN_NEG),
        out: constant(&mut arena, D_OUT),
        next: constant(&mut arena, D_NEXT_NEG),
    };
    let context = Context::empty().with_signature(signature);
    let index = two_zero(&mut arena);
    let family_at_zero = embedded.indexed_w(&mut arena, index);
    assert!(matches!(
        infer_sort(&mut arena, &context, family_at_zero, &mut budget()).unwrap(),
        Sort::Type(_)
    ));
    let g_binding = {
        // `Π(_ : dBNeg zero). IW' (dNextNeg zero _)` — under the binder
        // `dNextNeg zero b` computes to `zero`; spelling it unreduced
        // keeps the expected shape visible.
        let domain = {
            let children = constant(&mut arena, D_CHILDREN_NEG);
            let index = two_zero(&mut arena);
            apply(&mut arena, children, index)
        };
        let codomain = {
            let next = constant(&mut arena, D_NEXT_NEG);
            let label = two_zero(&mut arena);
            let at = apply(&mut arena, next, label);
            let bound = variable(&mut arena, 0);
            let required = apply(&mut arena, at, bound);
            embedded.indexed_w(&mut arena, required)
        };
        pi(&mut arena, domain, codomain)
    };
    let context = context.extend(g_binding);
    let derivation = {
        let label = two_zero(&mut arena);
        let children_fn = variable(&mut arena, 0);
        embedded.sup(&mut arena, label, children_fn)
    };
    let index = two_zero(&mut arena);
    let expected = embedded.indexed_w(&mut arena, index);
    check_type(&mut arena, &context, derivation, expected, &mut budget()).expect(
        "isup over the negatively-embedding description checks — only self-naming is refused",
    );
}

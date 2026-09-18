//! The quotient receiver-policy leg. An author may explicitly select
//! the squashed projection `Box (Squash (R a b))` of a proof-relevant
//! relation, and the interface then yields only squashed evidence:
//! `effective` returns the boxed proposition and `unbox` reaches
//! `Squash (R a b)`, but no elimination reopens the relevant `R a b`
//! witness. The exact assumption closure is the receiver's policy
//! input, so the second half pairs a quotient operation with a
//! representative-level theorem under a quotient-refusing policy: the
//! refusing receiver accepts the quotient-free closure and rejects the
//! assumption-bearing quotient proof, while a receiver admitting the
//! interface accepts both — the same verified certificate decided
//! differently by policy, never by the kernel.

use std::collections::BTreeSet;

use crate::mathematical_core::tests::{
    box_elim, box_intro, boxed, default_budget, id, lambda, quotient_signature, sigma, squash,
    squash_elim, squash_intro, two, two_quotient, variable,
};
use crate::mathematical_core::{
    Context, CoreError, Level, MathematicalCertificate, QUOTIENT, QUOTIENT_BETA,
    QUOTIENT_EFFECTIVE, QUOTIENT_ELIM, QUOTIENT_LIFT_PRECONDITION, QUOTIENT_PROJECT, QUOTIENT_SET,
    QUOTIENT_SOUND, QuotientFamily, Signature, TermArena, assumption_closure,
    certificate_assumption_closure, check_type, infer_type, quotient_scheme,
    verify_mathematical_certificate,
};

/// A closed description for tests: carrier `Two`, relation `λa. λb.
/// Box (Squash (Id Two a b))` — the explicit squashed projection of
/// the equality relation — at levels `[0, 0]`.
fn squashed_two_quotient(arena: &mut TermArena) -> QuotientFamily {
    let carrier = two(arena);
    let relation = {
        let domain = two(arena);
        let inner_domain = two(arena);
        let ty = two(arena);
        let left = variable(arena, 1);
        let right = variable(arena, 0);
        let body = id(arena, ty, left, right);
        let body = squash(arena, body);
        let body = boxed(arena, body);
        let inner = lambda(arena, inner_domain, body);
        lambda(arena, domain, inner)
    };
    QuotientFamily {
        levels: [Level::Constant(0), Level::Constant(0)],
        carrier,
        relation,
    }
}

#[test]
fn a_squashed_relation_yields_only_squashed_evidence() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = squashed_two_quotient(&mut arena);
    let signature = quotient_signature(&mut arena);

    // Context `a : Two`, `b : Two`, `q : Id Two a b`, `r : Box (Squash
    // (Id Two a b))`, `p : Id Q (project a) (project b)` — ambient
    // indices p = 0, r = 1, q = 2, b = 3, a = 4. Each entry sees its
    // own prefix: `q` sees `[a, b]` (a = 1, b = 0), `r` sees
    // `[a, b, q]` (a = 2, b = 1) and `p` sees `[a, b, q, r]` (a = 3,
    // b = 2).
    let identity_ab = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        id(&mut arena, ty, a, b)
    };
    let squashed_evidence = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 2);
        let b = variable(&mut arena, 1);
        let body = id(&mut arena, ty, a, b);
        let body = squash(&mut arena, body);
        boxed(&mut arena, body)
    };
    let projection_identity = {
        let a = variable(&mut arena, 3);
        let left = family.project(&mut arena, a);
        let b = variable(&mut arena, 2);
        let right = family.project(&mut arena, b);
        let ty = family.quotient(&mut arena);
        id(&mut arena, ty, left, right)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(two(&mut arena))
        .extend(two(&mut arena))
        .extend(identity_ab)
        .extend(squashed_evidence)
        .extend(projection_identity);

    // `sound a b r : Id Q (project a) (project b)` — the boxed-squash
    // evidence feeds the congruence law at exactly the selected
    // relation.
    let congruence = {
        let a = variable(&mut arena, 4);
        let b = variable(&mut arena, 3);
        let r = variable(&mut arena, 1);
        family.sound(&mut arena, a, b, r)
    };
    let ambient_projection_identity = {
        let a = variable(&mut arena, 4);
        let left = family.project(&mut arena, a);
        let b = variable(&mut arena, 3);
        let right = family.project(&mut arena, b);
        let ty = family.quotient(&mut arena);
        id(&mut arena, ty, left, right)
    };
    check_type(
        &mut arena,
        &context,
        congruence,
        ambient_projection_identity,
        &mut budget,
    )
    .unwrap();

    // `effective a b p : R a b ≡ Box (Squash (Id Two a b))` —
    // effectivity returns evidence of exactly the selected relation:
    // the squashed projection, never the identity itself.
    let effectivity = {
        let a = variable(&mut arena, 4);
        let b = variable(&mut arena, 3);
        let p = variable(&mut arena, 0);
        family.effective(&mut arena, a, b, p)
    };
    let ambient_squashed_identity = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 4);
        let b = variable(&mut arena, 3);
        let body = id(&mut arena, ty, a, b);
        let body = squash(&mut arena, body);
        boxed(&mut arena, body)
    };
    check_type(
        &mut arena,
        &context,
        effectivity,
        ambient_squashed_identity,
        &mut budget,
    )
    .unwrap();

    // Unboxing reaches the strict proposition:
    // `unbox (λ(_ : Box (Squash (Id Two a b))). Squash (Id Two a b))
    // (λx. x) (effective a b p) : Squash (Id Two a b)`. Under the
    // motive's binder the ambient variables shift to a = 5, b = 4.
    let unboxed = {
        let motive = {
            let body = {
                let ty = two(&mut arena);
                let a = variable(&mut arena, 5);
                let b = variable(&mut arena, 4);
                let body = id(&mut arena, ty, a, b);
                squash(&mut arena, body)
            };
            lambda(&mut arena, ambient_squashed_identity, body)
        };
        let body = {
            let domain = {
                let ty = two(&mut arena);
                let a = variable(&mut arena, 4);
                let b = variable(&mut arena, 3);
                let body = id(&mut arena, ty, a, b);
                squash(&mut arena, body)
            };
            let bound = variable(&mut arena, 0);
            lambda(&mut arena, domain, bound)
        };
        let scrutinee = {
            let a = variable(&mut arena, 4);
            let b = variable(&mut arena, 3);
            let p = variable(&mut arena, 0);
            family.effective(&mut arena, a, b, p)
        };
        box_elim(&mut arena, motive, body, scrutinee)
    };
    let strict_evidence = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 4);
        let b = variable(&mut arena, 3);
        let body = id(&mut arena, ty, a, b);
        squash(&mut arena, body)
    };
    check_type(&mut arena, &context, unboxed, strict_evidence, &mut budget).unwrap();

    // Squashed relations yield only squashed evidence: eliminating the
    // squash into the relevant `Id Two a b` is extraction, and the
    // eliminator's target must be a strict proposition.
    let extraction = {
        let proposition = {
            let ty = two(&mut arena);
            let a = variable(&mut arena, 4);
            let b = variable(&mut arena, 3);
            id(&mut arena, ty, a, b)
        };
        let function = {
            let domain = {
                let ty = two(&mut arena);
                let a = variable(&mut arena, 4);
                let b = variable(&mut arena, 3);
                id(&mut arena, ty, a, b)
            };
            let bound = variable(&mut arena, 0);
            lambda(&mut arena, domain, bound)
        };
        squash_elim(&mut arena, proposition, function, unboxed)
    };
    let error = infer_type(&mut arena, &context, extraction, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::SquashTargetNotStrict { .. }),
        "squashed relation evidence never opens to the relevant witness: {error:?}"
    );

    // The raw relevant witness `q : Id Two a b` is not `R a b`: the
    // squashed projection must be selected explicitly as
    // `box (sq q)`, and only that selected form feeds the congruence
    // law.
    let raw_congruence = {
        let a = variable(&mut arena, 4);
        let b = variable(&mut arena, 3);
        let q = variable(&mut arena, 2);
        family.sound(&mut arena, a, b, q)
    };
    let error = infer_type(&mut arena, &context, raw_congruence, &mut budget).unwrap_err();
    assert!(
        matches!(error, CoreError::ArgumentTypeMismatch { .. }),
        "a relevant identity witness is not evidence of the squashed relation: {error:?}"
    );
    let selected = {
        let identity = {
            let ty = two(&mut arena);
            let a = variable(&mut arena, 4);
            let b = variable(&mut arena, 3);
            id(&mut arena, ty, a, b)
        };
        let q = variable(&mut arena, 2);
        let squashed = squash_intro(&mut arena, identity, q);
        let strict = {
            let ty = two(&mut arena);
            let a = variable(&mut arena, 4);
            let b = variable(&mut arena, 3);
            let body = id(&mut arena, ty, a, b);
            squash(&mut arena, body)
        };
        box_intro(&mut arena, strict, squashed)
    };
    let selected_congruence = {
        let a = variable(&mut arena, 4);
        let b = variable(&mut arena, 3);
        family.sound(&mut arena, a, b, selected)
    };
    check_type(
        &mut arena,
        &context,
        selected_congruence,
        ambient_projection_identity,
        &mut budget,
    )
    .unwrap();

    // As a certificate the effectivity judgment re-decides
    // independently, and its closure names exactly the admitted
    // fragment it commits to: `Q`, `project` and `effective`.
    let certificate = {
        // Certificate context `a : Two`, `b : Two`, `p : Id Q
        // (project a) (project b)` — ambient p = 0, b = 1, a = 2;
        // `p`'s entry sees `[a, b]` (a = 1, b = 0).
        let projection_identity = {
            let a = variable(&mut arena, 1);
            let left = family.project(&mut arena, a);
            let b = variable(&mut arena, 0);
            let right = family.project(&mut arena, b);
            let ty = family.quotient(&mut arena);
            id(&mut arena, ty, left, right)
        };
        let term = {
            let a = variable(&mut arena, 2);
            let b = variable(&mut arena, 1);
            let p = variable(&mut arena, 0);
            family.effective(&mut arena, a, b, p)
        };
        let expected = {
            let ty = two(&mut arena);
            let a = variable(&mut arena, 2);
            let b = variable(&mut arena, 1);
            let body = id(&mut arena, ty, a, b);
            let body = squash(&mut arena, body);
            boxed(&mut arena, body)
        };
        MathematicalCertificate {
            signature: quotient_scheme(&mut arena),
            level_arity: 0,
            context: vec![two(&mut arena), two(&mut arena), projection_identity],
            term,
            expected,
        }
    };
    verify_mathematical_certificate(&mut arena, &certificate, &mut budget).unwrap();
    let closure = certificate_assumption_closure(&arena, &certificate);
    let expected_closure: BTreeSet<u32> = [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE]
        .into_iter()
        .collect();
    assert_eq!(
        closure, expected_closure,
        "the squashed effectivity judgment commits to exactly the interface it cites"
    );
}

#[test]
fn a_quotient_refusing_policy_distinguishes_the_same_certificate() {
    let mut arena = TermArena::new();
    let mut budget = default_budget();
    let family = two_quotient(&mut arena);
    let declarations = quotient_scheme(&mut arena);
    let signature = Signature::from_declarations(declarations.clone());

    // The assumption-bearing quotient judgment — `coverage z :
    // Squash (Σ(a : Two). Id Q (project a) z)` under `z : Q`.
    let element = family.quotient(&mut arena);
    let covered = {
        let z = variable(&mut arena, 0);
        family.coverage(&mut arena, z)
    };
    let covered_type = {
        let a_domain = two(&mut arena);
        let codomain = {
            let a = variable(&mut arena, 0);
            let projected = family.project(&mut arena, a);
            let z = variable(&mut arena, 1);
            let ty = family.quotient(&mut arena);
            id(&mut arena, ty, projected, z)
        };
        let packed = sigma(&mut arena, a_domain, codomain);
        squash(&mut arena, packed)
    };
    let quotient_certificate = MathematicalCertificate {
        signature: declarations.clone(),
        level_arity: 0,
        context: vec![element],
        term: covered,
        expected: covered_type,
    };
    verify_mathematical_certificate(&mut arena, &quotient_certificate, &mut budget).unwrap();

    // The representative-level judgment — `idSym Two a b p : Id Two b
    // a` under `a : Two`, `b : Two`, `p : Id Two a b` — cites a checked
    // definition in the same signature, so it carries no quotient
    // assumption at all. Ambient indices: p = 0, b = 1, a = 2; `p`'s
    // entry sees `[a, b]` (a = 1, b = 0).
    let identity_ab = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 1);
        let b = variable(&mut arena, 0);
        id(&mut arena, ty, a, b)
    };
    let symmetrized = {
        let ty = two(&mut arena);
        let a = variable(&mut arena, 2);
        let b = variable(&mut arena, 1);
        let p = variable(&mut arena, 0);
        family.id_sym(&mut arena, Level::Constant(0), ty, a, b, p)
    };
    let symmetrized_type = {
        let ty = two(&mut arena);
        let b = variable(&mut arena, 1);
        let a = variable(&mut arena, 2);
        id(&mut arena, ty, b, a)
    };
    let representative_certificate = MathematicalCertificate {
        signature: declarations.clone(),
        level_arity: 0,
        context: vec![two(&mut arena), two(&mut arena), identity_ab],
        term: symmetrized,
        expected: symmetrized_type,
    };
    verify_mathematical_certificate(&mut arena, &representative_certificate, &mut budget).unwrap();

    // A forged certificate — the same evidence claimed at a different
    // squash — is refused by verification itself: no policy ever sees a
    // judgment the kernel did not re-decide.
    let forged = MathematicalCertificate {
        signature: declarations,
        level_arity: 0,
        context: vec![element],
        term: covered,
        expected: {
            let carrier = two(&mut arena);
            squash(&mut arena, carrier)
        },
    };
    let error =
        verify_mathematical_certificate(&mut arena, &forged, &mut default_budget()).unwrap_err();
    assert!(
        matches!(error, CoreError::TypeMismatch { .. }),
        "a forged certificate is refused before any policy runs: {error:?}"
    );

    // The exact closures: the quotient judgment commits to `Q`,
    // `project`, `sound` and `elim` — `coverage` derives through those
    // four — while the representative-level judgment's `idSym` citation
    // is a checked definition whose closure is empty.
    let quotient_closure = certificate_assumption_closure(&arena, &quotient_certificate);
    let expected_quotient_closure: BTreeSet<u32> =
        [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_SOUND, QUOTIENT_ELIM]
            .into_iter()
            .collect();
    assert_eq!(quotient_closure, expected_quotient_closure);
    let representative_closure =
        certificate_assumption_closure(&arena, &representative_certificate);
    assert!(
        representative_closure.is_empty(),
        "a checked definition drags no admitted assumption into the closure"
    );

    // Two receiving policies distinguish the same published
    // certificates: a quotient-refusing receiver admits no interface
    // assumption, so the representative theorem is accepted and the
    // quotient proof is refused; a quotient-admitting receiver accepts
    // both. Verification's verdict is unchanged — the policies run on
    // the closure of the same verified judgments.
    let quotient_refusing: BTreeSet<u32> = BTreeSet::new();
    let quotient_admitting: BTreeSet<u32> = [
        QUOTIENT,
        QUOTIENT_PROJECT,
        QUOTIENT_SET,
        QUOTIENT_SOUND,
        QUOTIENT_EFFECTIVE,
        QUOTIENT_ELIM,
        QUOTIENT_BETA,
        QUOTIENT_LIFT_PRECONDITION,
    ]
    .into_iter()
    .collect();
    assert!(representative_closure.is_subset(&quotient_refusing));
    assert!(!quotient_closure.is_subset(&quotient_refusing));
    assert!(representative_closure.is_subset(&quotient_admitting));
    assert!(quotient_closure.is_subset(&quotient_admitting));

    // The distinction is per assumption: a policy admitting `Q`,
    // `project` and `effective` but refusing `elim` accepts an
    // effectivity judgment and still refuses the elimination-derived
    // coverage proof.
    let effectivity_closure = assumption_closure(&arena, &signature, &[QUOTIENT_EFFECTIVE]);
    let expected_effectivity_closure: BTreeSet<u32> =
        [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE]
            .into_iter()
            .collect();
    assert_eq!(effectivity_closure, expected_effectivity_closure);
    let elim_refusing: BTreeSet<u32> = [QUOTIENT, QUOTIENT_PROJECT, QUOTIENT_EFFECTIVE]
        .into_iter()
        .collect();
    assert!(effectivity_closure.is_subset(&elim_refusing));
    assert!(!quotient_closure.is_subset(&elim_refusing));
}

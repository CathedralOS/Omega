//! Native inductive families at the strict layer: a squashed inductive family
//! is the profile's *logical inductive relation* — existence of a derivation
//! without witness access — and this file pins the refusal boundary:
//!
//! * `Squash (Leaf i)` is `Strict`-sorted existence over the family.
//!   `unsq` eliminates it only into a strict proposition; asking it to land in
//!   relevant data (`Two`), feeding it the un-squashed derivation, treating
//!   the squashed witness as a derivation, or squashing an already-strict
//!   carrier are each refused by name.
//! * `Leaf.rec`'s motive must land in `Type w`. A `Strict`-codomain motive is
//!   refused; the reference core's route is `Box`: eliminate into
//!   `Box (Squash P)` and `unbox` to the strict target.
//! * Negative recursion inside an inductive constructor is refused by the
//!   kernel's strict-positivity check (`InductiveNotStrictlyPositive`), while
//!   embedding an already-completed inductive negatively in a later inductive
//!   is accepted.

use crate::common::*;
use proof_admission::{
    Budget, Context, CoreError, Declaration, Level, MathematicalCertificate, Signature, Sort,
    TermArena, TermHandle, certificate_assumption_closure, check_signature, check_type,
    declare_inductive_family, infer_sort, infer_type, verify_mathematical_certificate,
    weak_head_normalize,
};

const LEAF: u32 = 0;
const LEAF_MK: u32 = 1;
const LEAF_REC: u32 = 2;

/// `Leaf : Π(i : Two). Type 0`, `mk : Π(i : Two). Leaf i`, `Leaf.rec`.
fn leaf_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let two_ty = two(arena);
    let type0 = type_sort(arena, 0);
    let leaf_ty = pi(arena, two_ty, type0);
    let mk_ty = {
        let head = constant(arena, LEAF, Vec::new());
        let i = variable(arena, 0);
        let ret = apply(arena, head, i);
        pi(arena, two_ty, ret)
    };
    declare_inductive_family(arena, LEAF, 0, 0, leaf_ty, &[mk_ty])
}

fn leaf_signature(arena: &mut TermArena) -> Signature {
    let declarations = leaf_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

fn leaf_at(arena: &mut TermArena, index: TermHandle) -> TermHandle {
    let head = constant(arena, LEAF, Vec::new());
    apply(arena, head, index)
}

fn leaf_mk(arena: &mut TermArena, index: TermHandle) -> TermHandle {
    let head = constant(arena, LEAF_MK, Vec::new());
    apply(arena, head, index)
}

fn zero_position(arena: &mut TermArena) -> TermHandle {
    let ty = two(arena);
    let z = two_zero(arena);
    id(arena, ty, z, z)
}

#[test]
fn a_squashed_family_is_existence_without_witness_access() {
    let mut arena = TermArena::new();
    let signature = leaf_signature(&mut arena);
    let context = Context::empty().with_signature(signature);

    let z = two_zero(&mut arena);
    let leaf_zero = leaf_at(&mut arena, z);
    assert!(matches!(
        infer_sort(&mut arena, &context, leaf_zero, &mut budget()).unwrap(),
        Sort::Type(_)
    ));
    let squashed = squash(&mut arena, leaf_zero);
    assert!(
        matches!(
            infer_sort(&mut arena, &context, squashed, &mut budget()).unwrap(),
            Sort::Strict(_)
        ),
        "Squash moves the relevant family into the strict layer"
    );

    let derivation = leaf_mk(&mut arena, z);
    check_type(&mut arena, &context, derivation, leaf_zero, &mut budget()).unwrap();

    let witness = squash_intro(&mut arena, leaf_zero, derivation);
    let inferred = infer_type(&mut arena, &context, witness, &mut budget()).unwrap();
    assert!(arena.structurally_equal(inferred, squashed));

    // `unsq` reaches a strict proposition: `P := Squash (Id Two zero zero)`,
    // `f := λ(_ : Leaf zero). sq (refl Two zero)`, and computes on `sq d`.
    let proposition = {
        let target = zero_position(&mut arena);
        squash(&mut arena, target)
    };
    let expected_normal = {
        let ty = two(&mut arena);
        let proof = refl(&mut arena, ty, z);
        let target = zero_position(&mut arena);
        squash_intro(&mut arena, target, proof)
    };
    let function = lambda(&mut arena, leaf_zero, expected_normal);
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
    assert!(arena.structurally_equal(normal, expected_normal));
    assert!(before - spent.remaining() > 0);

    // Zero budget refuses the same computation.
    assert!(matches!(
        weak_head_normalize(
            &mut arena,
            context.signature(),
            elimination,
            &mut Budget::new(0)
        ),
        Err(CoreError::StepCeiling)
    ));

    // Refusal cells:
    // 1. `unsq` into relevant target `Two` -> `SquashTargetNotStrict`.
    let relevant_prop = two(&mut arena);
    let refused = squash_elim(&mut arena, relevant_prop, function, witness);
    assert!(matches!(
        infer_type(&mut arena, &context, refused, &mut budget()),
        Err(CoreError::SquashTargetNotStrict { .. })
    ));

    // 2. `unsq` on un-squashed derivation -> `NotASquash`.
    let refused = squash_elim(&mut arena, proposition, function, derivation);
    assert!(matches!(
        infer_type(&mut arena, &context, refused, &mut budget()),
        Err(CoreError::NotASquash { .. })
    ));

    // 3. Squashed witness is not a derivation -> `TypeMismatch`.
    assert!(matches!(
        check_type(&mut arena, &context, witness, leaf_zero, &mut budget()),
        Err(CoreError::TypeMismatch { .. })
    ));

    // 4. Double `Squash` and `Box` of relevant carrier are refused.
    let double_squash = squash(&mut arena, squashed);
    assert!(matches!(
        infer_sort(&mut arena, &context, double_squash, &mut budget()),
        Err(CoreError::StrictSquashDomain { .. })
    ));
    let bad_box = boxed(&mut arena, leaf_zero);
    assert!(matches!(
        infer_sort(&mut arena, &context, bad_box, &mut budget()),
        Err(CoreError::NonStrictBoxDomain { .. })
    ));

    // Certificate re-decides with an empty assumption closure (native inductive
    // declarations are not assumptions).
    let certificate = MathematicalCertificate::with_signature(
        leaf_declarations(&mut arena),
        Vec::new(),
        witness,
        squashed,
    );
    let mut spent = measure_budget();
    verify_mathematical_certificate(&mut arena, &certificate, &mut spent).unwrap();
    let closure = certificate_assumption_closure(&arena, &certificate);
    assert!(closure.is_empty());
}

#[test]
fn indexed_elimination_reaches_a_strict_target_through_boxing() {
    let mut arena = TermArena::new();
    let signature = leaf_signature(&mut arena);

    // Γ = a : Two (a = 0).
    let two_ty = two(&mut arena);
    let context = Context::empty().with_signature(signature).extend(two_ty);
    let a = variable(&mut arena, 0);
    let derivation = leaf_mk(&mut arena, a);
    let at_a = leaf_at(&mut arena, a);
    check_type(&mut arena, &context, derivation, at_a, &mut budget()).unwrap();

    // `Q := λ(i : Two). λ(_ : Leaf i). Box (Squash (Id Two i i))`
    let motive = {
        let i = variable(&mut arena, 0);
        let t_dom = leaf_at(&mut arena, i);
        let i1 = variable(&mut arena, 1);
        let eq = id(&mut arena, two_ty, i1, i1);
        let sq = squash(&mut arena, eq);
        let bx = boxed(&mut arena, sq);
        let inner = lambda(&mut arena, t_dom, bx);
        lambda(&mut arena, two_ty, inner)
    };

    // `m_mk := λ(i : Two). box (sq (refl Two i))`
    let m_mk = {
        let i = variable(&mut arena, 0);
        let eq = id(&mut arena, two_ty, i, i);
        let proof = refl(&mut arena, two_ty, i);
        let sq_proof = squash_intro(&mut arena, eq, proof);
        let payload = squash(&mut arena, eq);
        let body = box_intro(&mut arena, payload, sq_proof);
        lambda(&mut arena, two_ty, body)
    };

    let rec = constant(&mut arena, LEAF_REC, vec![Level::Constant(0)]);
    let elimination = apps(&mut arena, rec, &[motive, m_mk, a, derivation]);
    let expected_type = apps(&mut arena, motive, &[a, derivation]);
    check_type(
        &mut arena,
        &context,
        elimination,
        expected_type,
        &mut budget(),
    )
    .unwrap();

    let mut spent = measure_budget();
    let before = spent.remaining();
    let normal =
        weak_head_normalize(&mut arena, context.signature(), elimination, &mut spent).unwrap();
    let eq_a = id(&mut arena, two_ty, a, a);
    let sq_refl_a = {
        let proof = refl(&mut arena, two_ty, a);
        squash_intro(&mut arena, eq_a, proof)
    };
    let expected_normal = {
        let payload = squash(&mut arena, eq_a);
        box_intro(&mut arena, payload, sq_refl_a)
    };
    assert!(arena.structurally_equal(normal, expected_normal));
    assert!(before - spent.remaining() > 0);

    // `unbox` lands at the strict target `Squash (Id Two a a)`.
    let landing = squash(&mut arena, eq_a);
    let unbox_motive = {
        let domain = boxed(&mut arena, landing);
        let a1 = variable(&mut arena, 1);
        let eq1 = id(&mut arena, two_ty, a1, a1);
        let body = squash(&mut arena, eq1);
        lambda(&mut arena, domain, body)
    };
    let v0 = variable(&mut arena, 0);
    let unbox_body = lambda(&mut arena, landing, v0);
    let unboxed = box_elim(&mut arena, unbox_motive, unbox_body, elimination);
    check_type(&mut arena, &context, unboxed, landing, &mut budget()).unwrap();
    let normal_unboxed =
        weak_head_normalize(&mut arena, context.signature(), unboxed, &mut budget()).unwrap();
    assert!(arena.structurally_equal(normal_unboxed, sq_refl_a));

    // A `Strict`-codomain motive is refused by `Leaf.rec`.
    let strict_motive_ty = {
        let i = variable(&mut arena, 0);
        let t_dom = leaf_at(&mut arena, i);
        let strict0 = strict_sort(&mut arena, 0);
        let inner = pi(&mut arena, t_dom, strict0);
        pi(&mut arena, two_ty, inner)
    };
    let strict_ctx = context.extend(strict_motive_ty);
    let q_s = variable(&mut arena, 0);
    let refused = apply(&mut arena, rec, q_s);
    assert_type_mismatch(&mut arena, &strict_ctx, refused, &mut budget());
}

#[test]
fn a_family_description_cannot_be_non_positive() {
    let mut arena = TermArena::new();
    let type0 = type_sort(&mut arena, 0);
    let two_ty = two(&mut arena);

    // Negative self-reference: `Bad : Type 0`, `mk : Π(_ : Π(_ : Bad). Two). Bad`
    // is rejected by the kernel's strict-positivity check.
    let bad_head = constant(&mut arena, 0, Vec::new());
    let neg_dom = pi(&mut arena, bad_head, two_ty);
    let bad_ctor = pi(&mut arena, neg_dom, bad_head);
    let bad_decls = declare_inductive_family(&mut arena, 0, 0, 0, type0, &[bad_ctor]);
    let err = check_signature(&mut arena, &bad_decls, &mut budget()).unwrap_err();
    assert!(matches!(err, CoreError::NonStrictlyPositiveOccurrence));

    // Control: a *completed* inductive `Done : Type 0` embedded negatively
    // inside a later inductive `Wrap : Type 0` (`mk : Π(_ : Π(_ : Done). Two). Wrap`)
    // is accepted.
    let mut control_decls = leaf_declarations(&mut arena); // 0..=2
    let wrap_base = control_decls.len() as u32;
    let z = two_zero(&mut arena);
    let done_ty = leaf_at(&mut arena, z);
    let fn_dom = pi(&mut arena, done_ty, two_ty);
    let wrap_head = constant(&mut arena, wrap_base, Vec::new());
    let wrap_ctor = pi(&mut arena, fn_dom, wrap_head);
    control_decls.extend(declare_inductive_family(
        &mut arena,
        wrap_base,
        0,
        0,
        type0,
        &[wrap_ctor],
    ));
    check_signature(&mut arena, &control_decls, &mut budget()).unwrap();
}

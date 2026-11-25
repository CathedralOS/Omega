//! The profile's "derivation context/conclusion indices" demonstration:
//! a native two-index inductive family of natural-deduction derivations
//! checked end-to-end by the kernel.
//!
//! ```text
//! Deriv : Π(Γ : Ctx). Π(φ : Frm). Type 0
//! assm  : Π(φ : Frm). Π(Γ : Ctx). Deriv (consF φ Γ) φ
//! impI  : Π(φ : Frm). Π(ψ : Frm). Π(Γ : Ctx).
//!           Π(d : Deriv (consF φ Γ) ψ). Deriv Γ (impF φ ψ)
//! impE  : Π(φ : Frm). Π(ψ : Frm). Π(Γ : Ctx).
//!           Π(d₁ : Deriv Γ (impF φ ψ)). Π(d₂ : Deriv Γ φ). Deriv Γ ψ
//! ```
//!
//! `Ctx`, `Frm`, `consF` and `impF` are signature *assumptions* at positions
//! 0–3; `Deriv`, `assm`, `impI`, `impE`, and `Deriv.rec` sit at positions 4–8.

use crate::common::*;
use proof_admission::{
    Context, CoreError, Declaration, Level, Signature, TermArena, TermHandle, check_signature,
    check_type, convertible, declare_inductive_family,
};

const CTX: u32 = 0;
const FRM: u32 = 1;
const CONS: u32 = 2;
const IMP: u32 = 3;
const DERIV: u32 = 4;
const ASSM: u32 = 5;
const IMP_I: u32 = 6;
const IMP_E: u32 = 7;
const DERIV_REC: u32 = 8;

fn ctx(arena: &mut TermArena) -> TermHandle {
    constant(arena, CTX, Vec::new())
}

fn frm(arena: &mut TermArena) -> TermHandle {
    constant(arena, FRM, Vec::new())
}

fn cons_f(arena: &mut TermArena, formula: TermHandle, context: TermHandle) -> TermHandle {
    let head = constant(arena, CONS, Vec::new());
    apps(arena, head, &[formula, context])
}

fn imp_f(arena: &mut TermArena, antecedent: TermHandle, consequent: TermHandle) -> TermHandle {
    let head = constant(arena, IMP, Vec::new());
    apps(arena, head, &[antecedent, consequent])
}

fn deriv_at(arena: &mut TermArena, context: TermHandle, formula: TermHandle) -> TermHandle {
    let head = constant(arena, DERIV, Vec::new());
    apps(arena, head, &[context, formula])
}

fn grammar_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // 0: Ctx
    declarations.push(Declaration::assumption(0, type_sort(arena, 0))); // 1: Frm
    let cons_ty = {
        let f = frm(arena);
        let c = ctx(arena);
        let inner = pi(arena, c, c);
        pi(arena, f, inner)
    };
    declarations.push(Declaration::assumption(0, cons_ty)); // 2: consF
    let imp_ty = {
        let f = frm(arena);
        let inner = pi(arena, f, f);
        pi(arena, f, inner)
    };
    declarations.push(Declaration::assumption(0, imp_ty)); // 3: impF
    declarations
}

fn derivation_declarations(arena: &mut TermArena) -> Vec<Declaration> {
    let mut declarations = grammar_declarations(arena);

    // `Deriv : Π(Γ : Ctx). Π(φ : Frm). Type 0`
    let deriv_ty = {
        let c = ctx(arena);
        let f = frm(arena);
        let t0 = type_sort(arena, 0);
        let inner = pi(arena, f, t0);
        pi(arena, c, inner)
    };

    // `assm : Π(φ : Frm). Π(Γ : Ctx). Deriv (consF φ Γ) φ`
    let assm_ty = {
        let f = frm(arena);
        let c = ctx(arena);
        let phi = variable(arena, 1);
        let gamma = variable(arena, 0);
        let ext = cons_f(arena, phi, gamma);
        let ret = deriv_at(arena, ext, phi);
        let inner = pi(arena, c, ret);
        pi(arena, f, inner)
    };

    // `impI : Π(φ : Frm). Π(ψ : Frm). Π(Γ : Ctx). Π(d : Deriv (consF φ Γ) ψ). Deriv Γ (impF φ ψ)`
    let impi_ty = {
        let f = frm(arena);
        let c = ctx(arena);
        let d_ty = {
            let phi = variable(arena, 2);
            let psi = variable(arena, 1);
            let gamma = variable(arena, 0);
            let ext = cons_f(arena, phi, gamma);
            deriv_at(arena, ext, psi)
        };
        let ret = {
            let phi = variable(arena, 3);
            let psi = variable(arena, 2);
            let gamma = variable(arena, 1);
            let imp = imp_f(arena, phi, psi);
            deriv_at(arena, gamma, imp)
        };
        let over_d = pi(arena, d_ty, ret);
        let over_g = pi(arena, c, over_d);
        let over_psi = pi(arena, f, over_g);
        pi(arena, f, over_psi)
    };

    // `impE : Π(φ : Frm). Π(ψ : Frm). Π(Γ : Ctx).
    //         Π(d1 : Deriv Γ (impF φ ψ)). Π(d2 : Deriv Γ φ). Deriv Γ ψ`
    let impe_ty = {
        let f = frm(arena);
        let c = ctx(arena);
        let d1_ty = {
            let phi = variable(arena, 2);
            let psi = variable(arena, 1);
            let gamma = variable(arena, 0);
            let imp = imp_f(arena, phi, psi);
            deriv_at(arena, gamma, imp)
        };
        let d2_ty = {
            let phi = variable(arena, 3);
            let gamma = variable(arena, 1);
            deriv_at(arena, gamma, phi)
        };
        let ret = {
            let psi = variable(arena, 3);
            let gamma = variable(arena, 2);
            deriv_at(arena, gamma, psi)
        };
        let over_d2 = pi(arena, d2_ty, ret);
        let over_d1 = pi(arena, d1_ty, over_d2);
        let over_g = pi(arena, c, over_d1);
        let over_psi = pi(arena, f, over_g);
        pi(arena, f, over_psi)
    };

    declarations.extend(declare_inductive_family(
        arena,
        DERIV,
        0,
        0,
        deriv_ty,
        &[assm_ty, impi_ty, impe_ty],
    ));
    declarations
}

fn derivation_signature(arena: &mut TermArena) -> Signature {
    let declarations = derivation_declarations(arena);
    check_signature(arena, &declarations, &mut budget()).unwrap()
}

#[test]
fn induction_with_a_dependent_motive_proves_the_conclusion_identity() {
    let mut arena = TermArena::new();
    let mut budget = measure_budget();
    let total = budget.remaining();
    let signature = derivation_signature(&mut arena);

    // Dependent motive: `Q = λ(Γ : Ctx). λ(φ : Frm). λ(_ : Deriv Γ φ). Id Frm φ φ`.
    let motive = {
        let c = ctx(&mut arena);
        let f = frm(&mut arena);
        let d_dom = {
            let g = variable(&mut arena, 1);
            let phi = variable(&mut arena, 0);
            deriv_at(&mut arena, g, phi)
        };
        let body = {
            let phi = variable(&mut arena, 1);
            id(&mut arena, f, phi, phi)
        };
        let l3 = lambda(&mut arena, d_dom, body);
        let l2 = lambda(&mut arena, f, l3);
        lambda(&mut arena, c, l2)
    };

    // Minor premise for `assm`: `λ(φ : Frm). λ(Γ : Ctx). refl Frm φ`.
    let m_assm = {
        let f = frm(&mut arena);
        let c = ctx(&mut arena);
        let phi = variable(&mut arena, 1);
        let body = refl(&mut arena, f, phi);
        let l2 = lambda(&mut arena, c, body);
        lambda(&mut arena, f, l2)
    };

    // Minor premise for `impI`:
    // `λ(φ:Frm). λ(ψ:Frm). λ(Γ:Ctx). λ(d:Deriv (consF φ Γ) ψ). λ(ih:Q (consF φ Γ) ψ d).
    //    refl Frm (impF φ ψ)`
    let m_impi = {
        let f = frm(&mut arena);
        let c = ctx(&mut arena);
        let d_ty = {
            let phi = variable(&mut arena, 2);
            let psi = variable(&mut arena, 1);
            let gamma = variable(&mut arena, 0);
            let ext = cons_f(&mut arena, phi, gamma);
            deriv_at(&mut arena, ext, psi)
        };
        let ih_ty = {
            let phi = variable(&mut arena, 3);
            let psi = variable(&mut arena, 2);
            let gamma = variable(&mut arena, 1);
            let d = variable(&mut arena, 0);
            let ext = cons_f(&mut arena, phi, gamma);
            apps(&mut arena, motive, &[ext, psi, d])
        };
        let body = {
            let phi = variable(&mut arena, 4);
            let psi = variable(&mut arena, 3);
            let imp = imp_f(&mut arena, phi, psi);
            refl(&mut arena, f, imp)
        };
        let l5 = lambda(&mut arena, ih_ty, body);
        let l4 = lambda(&mut arena, d_ty, l5);
        let l3 = lambda(&mut arena, c, l4);
        let l2 = lambda(&mut arena, f, l3);
        lambda(&mut arena, f, l2)
    };

    // Minor premise for `impE`:
    // `λφ. λψ. λΓ. λd1. λd2. λih1. λih2. refl Frm ψ`
    let m_impe = {
        let f = frm(&mut arena);
        let c = ctx(&mut arena);
        let d1_ty = {
            let phi = variable(&mut arena, 2);
            let psi = variable(&mut arena, 1);
            let gamma = variable(&mut arena, 0);
            let imp = imp_f(&mut arena, phi, psi);
            deriv_at(&mut arena, gamma, imp)
        };
        let d2_ty = {
            let phi = variable(&mut arena, 3);
            let gamma = variable(&mut arena, 1);
            deriv_at(&mut arena, gamma, phi)
        };
        let ih1_ty = {
            let phi = variable(&mut arena, 4);
            let psi = variable(&mut arena, 3);
            let gamma = variable(&mut arena, 2);
            let d1 = variable(&mut arena, 1);
            let imp = imp_f(&mut arena, phi, psi);
            apps(&mut arena, motive, &[gamma, imp, d1])
        };
        let ih2_ty = {
            let phi = variable(&mut arena, 5);
            let gamma = variable(&mut arena, 3);
            let d2 = variable(&mut arena, 1);
            apps(&mut arena, motive, &[gamma, phi, d2])
        };
        let body = {
            let psi = variable(&mut arena, 5);
            refl(&mut arena, f, psi)
        };
        let l7 = lambda(&mut arena, ih2_ty, body);
        let l6 = lambda(&mut arena, ih1_ty, l7);
        let l5 = lambda(&mut arena, d2_ty, l6);
        let l4 = lambda(&mut arena, d1_ty, l5);
        let l3 = lambda(&mut arena, c, l4);
        let l2 = lambda(&mut arena, f, l3);
        lambda(&mut arena, f, l2)
    };

    // Context: `φ : Frm, ψ : Frm, Γ : Ctx, d1 : Deriv Γ (impF φ ψ), d2 : Deriv Γ φ`.
    // Indices: d2 = 0, d1 = 1, Γ = 2, ψ = 3, φ = 4.
    let f = frm(&mut arena);
    let c = ctx(&mut arena);
    let d1_ty = {
        let phi = variable(&mut arena, 2);
        let psi = variable(&mut arena, 1);
        let gamma = variable(&mut arena, 0);
        let imp = imp_f(&mut arena, phi, psi);
        deriv_at(&mut arena, gamma, imp)
    };
    let d2_ty = {
        let phi = variable(&mut arena, 3);
        let gamma = variable(&mut arena, 1);
        deriv_at(&mut arena, gamma, phi)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(f)
        .extend(f)
        .extend(c)
        .extend(d1_ty)
        .extend(d2_ty);

    let phi = variable(&mut arena, 4);
    let psi = variable(&mut arena, 3);
    let gamma = variable(&mut arena, 2);
    let d1 = variable(&mut arena, 1);
    let d2 = variable(&mut arena, 0);
    let impe = constant(&mut arena, IMP_E, Vec::new());
    let mp = apps(&mut arena, impe, &[phi, psi, gamma, d1, d2]);

    let rec = constant(&mut arena, DERIV_REC, vec![Level::Constant(0)]);
    let elimination = apps(
        &mut arena,
        rec,
        &[motive, m_assm, m_impi, m_impe, gamma, psi, mp],
    );

    let shared = id(&mut arena, f, psi, psi);
    check_type(&mut arena, &context, elimination, shared, &mut budget).unwrap();
    let reflected = refl(&mut arena, f, psi);
    assert!(
        convertible(
            &mut arena,
            &context,
            elimination,
            reflected,
            shared,
            &mut budget
        )
        .unwrap()
    );

    // Native 1-step iota-reduction replaces the ~2,958-step W-encoding unfold.
    let spent = total - budget.remaining();
    assert!(spent > 0 && spent < 200, "unexpected step spend: {spent}");
}

#[test]
fn derivations_reject_wrong_judgments_and_malformed_declarations() {
    let mut arena = TermArena::new();
    let mut budget = budget();
    let signature = derivation_signature(&mut arena);

    // Context: `φ : Frm, ψ : Frm, Γ : Ctx, d1 : Deriv Γ (impF φ ψ), d2 : Deriv Γ φ`.
    // Indices: d2 = 0, d1 = 1, Γ = 2, ψ = 3, φ = 4.
    let f = frm(&mut arena);
    let c = ctx(&mut arena);
    let d1_ty = {
        let phi = variable(&mut arena, 2);
        let psi = variable(&mut arena, 1);
        let gamma = variable(&mut arena, 0);
        let imp = imp_f(&mut arena, phi, psi);
        deriv_at(&mut arena, gamma, imp)
    };
    let d2_ty = {
        let phi = variable(&mut arena, 3);
        let gamma = variable(&mut arena, 1);
        deriv_at(&mut arena, gamma, phi)
    };
    let context = Context::empty()
        .with_signature(signature)
        .extend(f)
        .extend(f)
        .extend(c)
        .extend(d1_ty)
        .extend(d2_ty);

    let phi = variable(&mut arena, 4);
    let psi = variable(&mut arena, 3);
    let gamma = variable(&mut arena, 2);
    let d1 = variable(&mut arena, 1);
    let d2 = variable(&mut arena, 0);
    let impe = constant(&mut arena, IMP_E, Vec::new());
    let mp = apps(&mut arena, impe, &[phi, psi, gamma, d1, d2]);

    // `impE` lands at `Deriv Γ ψ`, never at `Deriv Γ φ`.
    let wrong_conclusion = deriv_at(&mut arena, gamma, phi);
    let err = check_type(&mut arena, &context, mp, wrong_conclusion, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::TypeMismatch { .. }));

    // …nor at the extended context `Deriv (consF φ Γ) ψ`.
    let ext = cons_f(&mut arena, phi, gamma);
    let wrong_context = deriv_at(&mut arena, ext, psi);
    let err = check_type(&mut arena, &context, mp, wrong_context, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::TypeMismatch { .. }));

    // Swapping major and minor premises is refused.
    let swapped = apps(&mut arena, impe, &[phi, psi, gamma, d2, d1]);
    let expected = deriv_at(&mut arena, gamma, psi);
    assert_check_type_mismatch(&mut arena, &context, swapped, expected, &mut budget);

    // `assm φ Γ` lands at `Deriv (consF φ Γ) φ`, never at `Deriv Γ φ`.
    let assm = constant(&mut arena, ASSM, Vec::new());
    let leaf = apps(&mut arena, assm, &[phi, gamma]);
    let err = check_type(&mut arena, &context, leaf, wrong_conclusion, &mut budget).unwrap_err();
    assert!(matches!(err, CoreError::TypeMismatch { .. }));

    // `impI φ φ Γ (assm φ Γ)` lands at `Deriv Γ (impF φ φ)`.
    let impi = constant(&mut arena, IMP_I, Vec::new());
    let id_deriv = apps(&mut arena, impi, &[phi, phi, gamma, leaf]);
    let imp_phi_phi = imp_f(&mut arena, phi, phi);
    let id_ty = deriv_at(&mut arena, gamma, imp_phi_phi);
    check_type(&mut arena, &context, id_deriv, id_ty, &mut budget).unwrap();

    // A malformed inductive declaration whose constructor supplies a `Frm`
    // where a `Ctx` index is required is rejected by `check_signature`.
    let mut bad_decls = grammar_declarations(&mut arena);
    let deriv_ty = {
        let t0 = type_sort(&mut arena, 0);
        let inner = pi(&mut arena, f, t0);
        pi(&mut arena, c, inner)
    };
    let bad_ctor = {
        let phi = variable(&mut arena, 0);
        let ret = deriv_at(&mut arena, phi, phi);
        pi(&mut arena, f, ret)
    };
    bad_decls.extend(declare_inductive_family(
        &mut arena,
        DERIV,
        0,
        0,
        deriv_ty,
        &[bad_ctor],
    ));
    let err = check_signature(&mut arena, &bad_decls, &mut budget).unwrap_err();
    assert!(matches!(
        err,
        CoreError::ArgumentTypeMismatch { .. } | CoreError::TypeMismatch { .. }
    ));
}

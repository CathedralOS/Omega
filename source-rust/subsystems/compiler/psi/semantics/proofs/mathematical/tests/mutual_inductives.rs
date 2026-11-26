//! Option C Stress Probes:
//!   0. Mutual Soundness Negative Tests
//!   1. Stress 1 — Mutual Inductives (`Expr` + `Stmt`)
//!   2. Stress 2 — Nested Inductive (`Tree` + `Forest`)
//!   3. Stress 3 — 50-Case Payloaded AST (`Ast50`)
//!   4. Stress 4 — Deep Closed `Nat` Arithmetic (`plus`, `mul`)

use std::collections::BTreeSet;
use std::time::Instant;

use mathematical_kernel::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, Level, Term, TermArena,
    TermHandle, check_signature, check_type, convertible, declare_inductive_family,
    declare_mutual_inductive_family, judgment_assumption_closure, shift,
};

const STRESS_BUDGET: u32 = 20_000_000;

mod support;
use support::*;

// ============================================================================
// Generic Mutual Payloaded AST Builder (handles M >= 1, arbitrary field order)
// ============================================================================

#[derive(Clone, Copy, Debug)]
enum FieldKind {
    Scalar,
    Rec(usize),
}

struct MutualCtorSpec {
    target_m: usize,
    fields: Vec<FieldKind>,
}

fn field_type(arena: &mut TermArena, base: u32, scalar: TermHandle, kind: FieldKind) -> TermHandle {
    match kind {
        FieldKind::Scalar => scalar,
        FieldKind::Rec(m) => c0(arena, base + m as u32),
    }
}

fn rec_positions(fields: &[FieldKind]) -> Vec<(usize, usize)> {
    fields
        .iter()
        .enumerate()
        .filter_map(|(i, &k)| match k {
            FieldKind::Rec(m) => Some((i, m)),
            FieldKind::Scalar => None,
        })
        .collect()
}

fn mutual_ast_declarations(
    arena: &mut TermArena,
    base: u32,
    num_mutual: usize,
    scalar: TermHandle,
    ctors: &[MutualCtorSpec],
) -> Vec<Declaration> {
    let t0 = type_n(arena, 0);
    let ind_tys = vec![t0; num_mutual];
    let mut ctor_pairs = Vec::with_capacity(ctors.len());
    for spec in ctors {
        let mut ty = c0(arena, base + spec.target_m as u32);
        for &fk in spec.fields.iter().rev() {
            let dom = field_type(arena, base, scalar, fk);
            ty = pi(arena, dom, ty);
        }
        ctor_pairs.push((spec.target_m, ty));
    }
    declare_mutual_inductive_family(arena, base, 0, 0, &ind_tys, &ctor_pairs)
}

fn mutual_copy_defs(
    arena: &mut TermArena,
    base: u32,
    num_mutual: usize,
    scalar: TermHandle,
    ctors: &[MutualCtorSpec],
) -> Vec<Declaration> {
    let nm = num_mutual;
    let nc = ctors.len();

    let motives: Vec<TermHandle> = (0..nm)
        .map(|m| {
            let im = c0(arena, base + m as u32);
            lam(arena, im, im)
        })
        .collect();

    let mut minors = Vec::with_capacity(nc);
    for (c, spec) in ctors.iter().enumerate() {
        let nf = spec.fields.len();
        let recs = rec_positions(&spec.fields);
        let nr = recs.len();

        let mut body = c0(arena, base + (nm + c) as u32);
        let mut q = 0usize;
        for (i, &fk) in spec.fields.iter().enumerate() {
            let arg = match fk {
                FieldKind::Scalar => var(arena, (nr + (nf - 1 - i)) as u32),
                FieldKind::Rec(_) => {
                    let v = var(arena, (nr - 1 - q) as u32);
                    q += 1;
                    v
                }
            };
            body = apps(arena, body, &[arg]);
        }
        for &(_, rec_m) in recs.iter().rev() {
            let im = c0(arena, base + rec_m as u32);
            body = lam(arena, im, body);
        }
        for &fk in spec.fields.iter().rev() {
            let dom = field_type(arena, base, scalar, fk);
            body = lam(arena, dom, body);
        }
        minors.push(body);
    }

    (0..nm)
        .map(|m| {
            let im = c0(arena, base + m as u32);
            let rec = c_levels(arena, base + (nm + nc + m) as u32, vec![Level::Constant(0)]);
            let mut rec_args = Vec::with_capacity(nm + nc + 1);
            rec_args.extend_from_slice(&motives);
            rec_args.extend_from_slice(&minors);
            rec_args.push(var(arena, 0));
            let rec_app = apps(arena, rec, &rec_args);
            let body = lam(arena, im, rec_app);
            let ty = pi(arena, im, im);
            Declaration::definition(0, ty, body)
        })
        .collect()
}

fn mutual_copy_id_proofs(
    arena: &mut TermArena,
    base: u32,
    num_mutual: usize,
    scalar: TermHandle,
    ctors: &[MutualCtorSpec],
    copy_base: u32,
) -> Vec<(TermHandle, TermHandle)> {
    let nm = num_mutual;
    let nc = ctors.len();

    let mut motives = Vec::with_capacity(nm);
    let mut statements = Vec::with_capacity(nm);
    for m in 0..nm {
        let im = c0(arena, base + m as u32);
        let copy_m = c0(arena, copy_base + m as u32);
        let x0 = var(arena, 0);
        let copy_x = apps(arena, copy_m, &[x0]);
        let eq_x = id_ty(arena, im, copy_x, x0);
        motives.push(lam(arena, im, eq_x));
        statements.push(pi(arena, im, eq_x));
    }

    let mut minors = Vec::with_capacity(nc);
    for (c, spec) in ctors.iter().enumerate() {
        let nf = spec.fields.len();
        let recs = rec_positions(&spec.fields);
        let nr = recs.len();
        let target_im = c0(arena, base + spec.target_m as u32);
        let ctor = c0(arena, base + (nm + c) as u32);

        let build_ctor =
            |arena: &mut TermArena, extra_shift: u32, replace_up_to: usize, y_at: Option<usize>| {
                let mut args = Vec::with_capacity(nf);
                let mut q = 0usize;
                for (i, &fk) in spec.fields.iter().enumerate() {
                    let fi = var(arena, (nr + (nf - 1 - i)) as u32 + extra_shift);
                    match fk {
                        FieldKind::Scalar => args.push(fi),
                        FieldKind::Rec(rec_m) => {
                            if y_at == Some(q) {
                                args.push(var(arena, 1));
                            } else if q < replace_up_to {
                                args.push(fi);
                            } else {
                                let copy_r = c0(arena, copy_base + rec_m as u32);
                                args.push(apps(arena, copy_r, &[fi]));
                            }
                            q += 1;
                        }
                    }
                }
                apps(arena, ctor, &args)
            };

        let start_val = build_ctor(arena, 0, 0, None);
        let mut proof = refl(arena, target_im, start_val);
        for (q, &(j, rec_m)) in recs.iter().enumerate() {
            let rec_im = c0(arena, base + rec_m as u32);
            let copy_r = c0(arena, copy_base + rec_m as u32);
            let fj = var(arena, (nr + (nf - 1 - j)) as u32);
            let ih_q = var(arena, (nr - 1 - q) as u32);
            let copy_fj = apps(arena, copy_r, &[fj]);
            let copy_fj_1 = shift(arena, copy_fj, 0, 1);
            let v0 = var(arena, 0);
            let y_eq_ty = id_ty(arena, rec_im, copy_fj_1, v0);
            let start_s = build_ctor(arena, 2, 0, None);
            let rhs = build_ctor(arena, 2, q, Some(q));
            let goal_id = id_ty(arena, target_im, start_s, rhs);
            let elim_motive = lam_many(arena, &[rec_im, y_eq_ty], goal_id);
            proof = arena.insert(Term::IdElim {
                motive: elim_motive,
                base: proof,
                endpoint: fj,
                proof: ih_q,
            });
        }

        for (q, &(j, rec_m)) in recs.iter().enumerate().rev() {
            let rec_im = c0(arena, base + rec_m as u32);
            let copy_r = c0(arena, copy_base + rec_m as u32);
            let fj = var(arena, (q + (nf - 1 - j)) as u32);
            let copy_fj = apps(arena, copy_r, &[fj]);
            let ih_ty = id_ty(arena, rec_im, copy_fj, fj);
            proof = lam(arena, ih_ty, proof);
        }
        for &fk in spec.fields.iter().rev() {
            let dom = field_type(arena, base, scalar, fk);
            proof = lam(arena, dom, proof);
        }
        minors.push(proof);
    }

    (0..nm)
        .map(|m| {
            let im = c0(arena, base + m as u32);
            let rec = c_levels(arena, base + (nm + nc + m) as u32, vec![Level::Constant(0)]);
            let mut rec_args = Vec::with_capacity(nm + nc + 1);
            rec_args.extend_from_slice(&motives);
            rec_args.extend_from_slice(&minors);
            rec_args.push(var(arena, 0));
            let rec_app = apps(arena, rec, &rec_args);
            let proof = lam(arena, im, rec_app);
            (statements[m], proof)
        })
        .collect()
}

fn mutual_in_bounds_defs(
    arena: &mut TermArena,
    base: u32,
    num_mutual: usize,
    scalar: TermHandle,
    le: TermHandle,
    ctors: &[MutualCtorSpec],
) -> Vec<Declaration> {
    let nm = num_mutual;
    let nc = ctors.len();
    let t0 = type_n(arena, 0);
    let pred_ty = pi(arena, scalar, t0);

    let motives: Vec<TermHandle> = (0..nm)
        .map(|m| {
            let im = c0(arena, base + m as u32);
            lam(arena, im, pred_ty)
        })
        .collect();

    let mut minors = Vec::with_capacity(nc);
    for spec in ctors {
        let nf = spec.fields.len();
        let recs = rec_positions(&spec.fields);
        let nr = recs.len();

        let b = var(arena, 0);
        let mut conjuncts = Vec::with_capacity(nf);
        let mut q = 0usize;
        for (i, &fk) in spec.fields.iter().enumerate() {
            match fk {
                FieldKind::Scalar => {
                    let xi = var(arena, (1 + nr + (nf - 1 - i)) as u32);
                    conjuncts.push(apps(arena, le, &[xi, b]));
                }
                FieldKind::Rec(_) => {
                    let ih_q = var(arena, (1 + (nr - 1 - q)) as u32);
                    conjuncts.push(apps(arena, ih_q, &[b]));
                    q += 1;
                }
            }
        }
        let mut body = if conjuncts.is_empty() {
            arena.insert(Term::Two)
        } else {
            let mut acc = *conjuncts.last().unwrap();
            for &c_ty in conjuncts[..conjuncts.len() - 1].iter().rev() {
                acc = prod(arena, c_ty, acc);
            }
            acc
        };
        body = lam(arena, scalar, body);
        for _ in 0..nr {
            body = lam(arena, pred_ty, body);
        }
        for &fk in spec.fields.iter().rev() {
            let dom = field_type(arena, base, scalar, fk);
            body = lam(arena, dom, body);
        }
        minors.push(body);
    }

    (0..nm)
        .map(|m| {
            let im = c0(arena, base + m as u32);
            let rec_t1 = c_levels(arena, base + (nm + nc + m) as u32, vec![Level::Constant(1)]);
            let mut rec_args = Vec::with_capacity(nm + nc + 1);
            rec_args.extend_from_slice(&motives);
            rec_args.extend_from_slice(&minors);
            rec_args.push(var(arena, 0));
            let rec_app = apps(arena, rec_t1, &rec_args);
            let body = lam(arena, im, rec_app);
            let ty = pi(arena, im, pred_ty);
            Declaration::definition(0, ty, body)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn mutual_bounds_mono_proofs(
    arena: &mut TermArena,
    base: u32,
    num_mutual: usize,
    scalar: TermHandle,
    le: TermHandle,
    le_trans: TermHandle,
    ib_base: u32,
    ctors: &[MutualCtorSpec],
) -> Vec<(TermHandle, TermHandle)> {
    let nm = num_mutual;
    let nc = ctors.len();

    let mono_for = |arena: &mut TermArena, m: usize, e_var: u32| {
        let in_bounds = c0(arena, ib_base + m as u32);
        let v0 = var(arena, 0);
        let v1 = var(arena, 1);
        let v2 = var(arena, 2);
        let ve3 = var(arena, e_var + 3);
        let ve4 = var(arena, e_var + 4);
        let le_12 = apps(arena, le, &[v1, v0]);
        let ib_1 = apps(arena, in_bounds, &[ve3, v2]);
        let ib_2 = apps(arena, in_bounds, &[ve4, v2]);
        pi_many(arena, &[scalar, scalar, le_12, ib_1], ib_2)
    };

    let mut motives = Vec::with_capacity(nm);
    let mut statements = Vec::with_capacity(nm);
    for m in 0..nm {
        let im = c0(arena, base + m as u32);
        let body = mono_for(arena, m, 0);
        motives.push(lam(arena, im, body));
        statements.push(pi(arena, im, body));
    }

    let mut minors = Vec::with_capacity(nc);
    for (c_idx, spec) in ctors.iter().enumerate() {
        let nf = spec.fields.len();
        let recs = rec_positions(&spec.fields);
        let nr = recs.len();

        let b1 = var(arena, 3);
        let b2 = var(arena, 2);
        let h_le = var(arena, 1);
        let h_ib = var(arena, 0);

        let mut extracted = Vec::with_capacity(nf);
        if nf > 0 {
            let mut rem = h_ib;
            for _ in 0..nf - 1 {
                extracted.push(fst(arena, rem));
                rem = snd(arena, rem);
            }
            extracted.push(rem);
        }

        let mut mapped = Vec::with_capacity(nf);
        let mut q = 0usize;
        for (i, &fk) in spec.fields.iter().enumerate() {
            match fk {
                FieldKind::Scalar => {
                    let xi = var(arena, (4 + nr + (nf - 1 - i)) as u32);
                    mapped.push(apps(arena, le_trans, &[xi, b1, b2, extracted[i], h_le]));
                }
                FieldKind::Rec(_) => {
                    let ih_q = var(arena, (4 + (nr - 1 - q)) as u32);
                    mapped.push(apps(arena, ih_q, &[b1, b2, h_le, extracted[i]]));
                    q += 1;
                }
            }
        }

        let mut body = if mapped.is_empty() {
            h_ib
        } else {
            let mut acc = *mapped.last().unwrap();
            for &val in mapped[..mapped.len() - 1].iter().rev() {
                acc = pair(arena, val, acc);
            }
            acc
        };

        let ctor = c0(arena, base + (nm + c_idx) as u32);
        let ctor_args: Vec<TermHandle> = (0..nf)
            .map(|i| var(arena, (3 + nr + (nf - 1 - i)) as u32))
            .collect();
        let ctor_val = apps(arena, ctor, &ctor_args);
        let in_bounds_target = c0(arena, ib_base + spec.target_m as u32);
        let v2 = var(arena, 2);
        let v1 = var(arena, 1);
        let v0 = var(arena, 0);
        let ib_ty = apps(arena, in_bounds_target, &[ctor_val, v2]);
        let le_ty = apps(arena, le, &[v1, v0]);
        body = lam_many(arena, &[scalar, scalar, le_ty, ib_ty], body);

        for (q, &(j, rec_m)) in recs.iter().enumerate().rev() {
            let ih_ty = mono_for(arena, rec_m, (q + (nf - 1 - j)) as u32);
            body = lam(arena, ih_ty, body);
        }
        for &fk in spec.fields.iter().rev() {
            let dom = field_type(arena, base, scalar, fk);
            body = lam(arena, dom, body);
        }
        minors.push(body);
    }

    (0..nm)
        .map(|m| {
            let im = c0(arena, base + m as u32);
            let rec_t0 = c_levels(arena, base + (nm + nc + m) as u32, vec![Level::Constant(0)]);
            let mut rec_args = Vec::with_capacity(nm + nc + 1);
            rec_args.extend_from_slice(&motives);
            rec_args.extend_from_slice(&minors);
            rec_args.push(var(arena, 0));
            let rec_app = apps(arena, rec_t0, &rec_args);
            let proof = lam(arena, im, rec_app);
            (statements[m], proof)
        })
        .collect()
}

// ============================================================================
// 0. Mutual Soundness Negative Tests
// ============================================================================

#[test]
fn probe_0_mutual_soundness_negative_tests() {
    eprintln!("\n=== Probe 0: Mutual Soundness Negative Tests ===");

    // (a) Mutual negative cycle:
    //     data A : Type 0 { MkA : (B -> Empty) -> A }
    //     data B : Type 0 { MkB : A -> B }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let a = c0(&mut arena, 0);
        let b = c0(&mut arena, 1);
        let empty = arena.insert(Term::Empty);
        let b_to_empty = pi(&mut arena, b, empty);
        let mka = pi(&mut arena, b_to_empty, a);
        let mkb = pi(&mut arena, a, b);
        let decls =
            declare_mutual_inductive_family(&mut arena, 0, 0, 0, &[t0, t0], &[(0, mka), (1, mkb)]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(STRESS_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::NonStrictlyPositiveOccurrence);
        eprintln!("  [PASS] Mutual negative cycle refused: {err:?}");
    }

    // (b) Mutual non-strictly-positive cycle:
    //     data A : Type 0 { MkA : ((B -> Two) -> B) -> A }
    //     data B : Type 0 { MkB : A -> B }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let a = c0(&mut arena, 0);
        let b = c0(&mut arena, 1);
        let two = arena.insert(Term::Two);
        let b_to_two = pi(&mut arena, b, two);
        let nsp = pi(&mut arena, b_to_two, b);
        let mka = pi(&mut arena, nsp, a);
        let mkb = pi(&mut arena, a, b);
        let decls =
            declare_mutual_inductive_family(&mut arena, 0, 0, 0, &[t0, t0], &[(0, mka), (1, mkb)]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(STRESS_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::NonStrictlyPositiveOccurrence);
        eprintln!("  [PASS] Mutual non-strictly-positive cycle refused: {err:?}");
    }

    // (c) Mutual universe cheat:
    //     data A : Type 0 { MkA : B -> A }
    //     data B : Type 1 { MkB : A -> B }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let t1 = type_n(&mut arena, 1);
        let a = c0(&mut arena, 0);
        let b = c0(&mut arena, 1);
        let mka = pi(&mut arena, b, a);
        let mkb = pi(&mut arena, a, b);
        let decls =
            declare_mutual_inductive_family(&mut arena, 0, 0, 0, &[t0, t1], &[(0, mka), (1, mkb)]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(STRESS_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InductiveUniverseViolation);
        eprintln!("  [PASS] Mutual universe cheat (A:Type0 taking B:Type1) refused: {err:?}");
    }

    // (d) Forged mutual recursor type refused
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let a = c0(&mut arena, 0);
        let b = c0(&mut arena, 1);
        let mka = pi(&mut arena, b, a);
        let mkb = pi(&mut arena, a, b);
        let mut decls =
            declare_mutual_inductive_family(&mut arena, 0, 0, 0, &[t0, t0], &[(0, mka), (1, mkb)]);
        // Tamper with B.rec (index 5)
        decls[5] = Declaration::recursor(1, 1, t0);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(STRESS_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InvalidRecursorType);
        eprintln!("  [PASS] Forged mutual recursor refused: {err:?}");
    }
}

// ============================================================================
// 1. Stress 1 — Mutual Inductives (`Expr` + `Stmt`)
// ============================================================================

#[test]
fn probe_1_mutual_expr_stmt() {
    eprintln!("\n=== Probe 1: Mutual Inductives (Expr + Stmt) ===");
    // Expr = 0, Stmt = 1
    // Expr: Lit(Scalar), Var(Scalar), Add(Expr, Expr), Block(Stmt, Expr)
    // Stmt: Assign(Scalar, Expr), Seq(Stmt, Stmt), If(Expr, Stmt, Stmt)
    let ctors = vec![
        MutualCtorSpec {
            target_m: 0,
            fields: vec![FieldKind::Scalar],
        },
        MutualCtorSpec {
            target_m: 0,
            fields: vec![FieldKind::Scalar],
        },
        MutualCtorSpec {
            target_m: 0,
            fields: vec![FieldKind::Rec(0), FieldKind::Rec(0)],
        },
        MutualCtorSpec {
            target_m: 0,
            fields: vec![FieldKind::Rec(1), FieldKind::Rec(0)],
        },
        MutualCtorSpec {
            target_m: 1,
            fields: vec![FieldKind::Scalar, FieldKind::Rec(0)],
        },
        MutualCtorSpec {
            target_m: 1,
            fields: vec![FieldKind::Rec(1), FieldKind::Rec(1)],
        },
        MutualCtorSpec {
            target_m: 1,
            fields: vec![FieldKind::Rec(0), FieldKind::Rec(1), FieldKind::Rec(1)],
        },
    ];

    let mut arena = TermArena::new();
    let mut decls = scalar_theory(&mut arena);
    let scalar = c0(&mut arena, 0);
    let le = c0(&mut arena, 1);
    let le_trans = c0(&mut arena, 2);
    let base = decls.len() as u32;

    let t0_decl = Instant::now();
    decls.extend(mutual_ast_declarations(&mut arena, base, 2, scalar, &ctors));
    let mut b_decl = Budget::new(STRESS_BUDGET);
    let sig_decl = check_signature(&mut arena, &decls, &mut b_decl).unwrap();
    let decl_ms = t0_decl.elapsed().as_secs_f64() * 1000.0;
    let decl_steps = STRESS_BUDGET - b_decl.remaining();
    let expr_c = c0(&mut arena, base);
    let stmt_c = c0(&mut arena, base + 1);
    let expr_rec = c_levels(&mut arena, base + 2 + 7, vec![Level::Constant(0)]);
    assert!(judgment_assumption_closure(&arena, &sig_decl, &[expr_c, stmt_c]).is_empty());
    assert_eq!(
        judgment_assumption_closure(&arena, &sig_decl, &[expr_rec]),
        BTreeSet::from([0u32])
    );
    eprintln!(
        "  Expr+Stmt decl          : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
        decl_steps,
        arena.len(),
        decl_ms,
        decl_steps <= DEFAULT_CONVERSION_STEPS
    );

    let copy_base = decls.len() as u32;
    decls.extend(mutual_copy_defs(&mut arena, base, 2, scalar, &ctors));
    let ib_base = decls.len() as u32;
    decls.extend(mutual_in_bounds_defs(
        &mut arena, base, 2, scalar, le, &ctors,
    ));
    let t0_defs = Instant::now();
    let mut b_defs = Budget::new(STRESS_BUDGET);
    let sig = check_signature(&mut arena, &decls, &mut b_defs).unwrap();
    let defs_ms = t0_defs.elapsed().as_secs_f64() * 1000.0;
    let defs_steps = STRESS_BUDGET - b_defs.remaining();
    eprintln!(
        "  copy+in_bounds defs     : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
        defs_steps,
        arena.len(),
        defs_ms,
        defs_steps <= DEFAULT_CONVERSION_STEPS
    );

    let ctx = Context::empty().with_signature(sig);

    let copy_ids = mutual_copy_id_proofs(&mut arena, base, 2, scalar, &ctors, copy_base);
    for (label, &(stmt, proof)) in ["copy_expr(e)=e", "copy_stmt(s)=s"]
        .iter()
        .zip(copy_ids.iter())
    {
        let mut q_arena = arena.clone();
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(&mut q_arena, &ctx, proof, stmt, &mut b).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            label,
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    let monos =
        mutual_bounds_mono_proofs(&mut arena, base, 2, scalar, le, le_trans, ib_base, &ctors);
    for (label, &(stmt, proof)) in ["bounds_mono_expr", "bounds_mono_stmt"]
        .iter()
        .zip(monos.iter())
    {
        let mut q_arena = arena.clone();
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(&mut q_arena, &ctx, proof, stmt, &mut b).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            label,
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }
}

// ============================================================================
// 2. Stress 2 — Nested Inductive (`Tree` + `Forest`)
// ============================================================================

#[test]
fn probe_2_nested_tree_forest() {
    eprintln!("\n=== Probe 2: Nested Inductive (Tree + Forest) ===");
    // Tree = 0: Node(val: Scalar, children: Forest)
    // Forest = 1: Nil, Cons(head: Tree, tail: Forest)
    let ctors = vec![
        MutualCtorSpec {
            target_m: 0,
            fields: vec![FieldKind::Scalar, FieldKind::Rec(1)],
        },
        MutualCtorSpec {
            target_m: 1,
            fields: vec![],
        },
        MutualCtorSpec {
            target_m: 1,
            fields: vec![FieldKind::Rec(0), FieldKind::Rec(1)],
        },
    ];

    let mut arena = TermArena::new();
    let mut decls = scalar_theory(&mut arena);
    let scalar = c0(&mut arena, 0);
    let le = c0(&mut arena, 1);
    let le_trans = c0(&mut arena, 2);
    let base = decls.len() as u32;

    let t0_decl = Instant::now();
    decls.extend(mutual_ast_declarations(&mut arena, base, 2, scalar, &ctors));
    let mut b_decl = Budget::new(STRESS_BUDGET);
    let _sig_decl = check_signature(&mut arena, &decls, &mut b_decl).unwrap();
    let decl_ms = t0_decl.elapsed().as_secs_f64() * 1000.0;
    let decl_steps = STRESS_BUDGET - b_decl.remaining();
    eprintln!(
        "  Tree+Forest decl        : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
        decl_steps,
        arena.len(),
        decl_ms,
        decl_steps <= DEFAULT_CONVERSION_STEPS
    );

    let copy_base = decls.len() as u32;
    decls.extend(mutual_copy_defs(&mut arena, base, 2, scalar, &ctors));
    let ib_base = decls.len() as u32;
    decls.extend(mutual_in_bounds_defs(
        &mut arena, base, 2, scalar, le, &ctors,
    ));
    let t0_defs = Instant::now();
    let mut b_defs = Budget::new(STRESS_BUDGET);
    let sig = check_signature(&mut arena, &decls, &mut b_defs).unwrap();
    let defs_ms = t0_defs.elapsed().as_secs_f64() * 1000.0;
    let defs_steps = STRESS_BUDGET - b_defs.remaining();
    eprintln!(
        "  copy+in_bounds defs     : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
        defs_steps,
        arena.len(),
        defs_ms,
        defs_steps <= DEFAULT_CONVERSION_STEPS
    );

    let ctx = Context::empty().with_signature(sig);

    let copy_ids = mutual_copy_id_proofs(&mut arena, base, 2, scalar, &ctors, copy_base);
    for (label, &(stmt, proof)) in ["copy_tree(t)=t", "copy_forest(f)=f"]
        .iter()
        .zip(copy_ids.iter())
    {
        let mut q_arena = arena.clone();
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(&mut q_arena, &ctx, proof, stmt, &mut b).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            label,
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    let monos =
        mutual_bounds_mono_proofs(&mut arena, base, 2, scalar, le, le_trans, ib_base, &ctors);
    for (label, &(stmt, proof)) in ["bounds_mono_tree", "bounds_mono_forest"]
        .iter()
        .zip(monos.iter())
    {
        let mut q_arena = arena.clone();
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(&mut q_arena, &ctx, proof, stmt, &mut b).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            label,
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }
}

// ============================================================================
// 3. Stress 3 — 50-Case Payloaded AST (`Ast50`)
// ============================================================================

#[test]
fn probe_3_ast50_payloaded() {
    eprintln!("\n=== Probe 3: 50-Case Payloaded AST (Ast50) ===");
    // Realistic mix of 0-3 Scalar payloads and 0-3 recursive children across 50 cases.
    // Case 49 (the last constructor) has 2 Scalar payloads and 3 recursive children.
    let mut ctors = Vec::with_capacity(50);
    for i in 0..50usize {
        let p = match i {
            49 => 2,
            _ => (i * 3 + 1) % 4, // 0..=3
        };
        let r = match i {
            49 => 3,
            _ => (i * 5 + 2) % 4, // 0..=3
        };
        let mut fields = Vec::with_capacity(p + r);
        for _ in 0..p {
            fields.push(FieldKind::Scalar);
        }
        for _ in 0..r {
            fields.push(FieldKind::Rec(0));
        }
        ctors.push(MutualCtorSpec {
            target_m: 0,
            fields,
        });
    }

    let mut arena = TermArena::new();
    let mut decls = scalar_theory(&mut arena);
    let scalar = c0(&mut arena, 0);
    let le = c0(&mut arena, 1);
    let le_trans = c0(&mut arena, 2);
    let base = decls.len() as u32;

    let t0_decl = Instant::now();
    decls.extend(mutual_ast_declarations(&mut arena, base, 1, scalar, &ctors));
    let mut b_decl = Budget::new(STRESS_BUDGET);
    let _sig_decl = check_signature(&mut arena, &decls, &mut b_decl).unwrap();
    let decl_ms = t0_decl.elapsed().as_secs_f64() * 1000.0;
    let decl_steps = STRESS_BUDGET - b_decl.remaining();
    eprintln!(
        "  Ast50 decl              : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
        decl_steps,
        arena.len(),
        decl_ms,
        decl_steps <= DEFAULT_CONVERSION_STEPS
    );

    let copy_base = decls.len() as u32;
    decls.extend(mutual_copy_defs(&mut arena, base, 1, scalar, &ctors));
    let ib_base = decls.len() as u32;
    decls.extend(mutual_in_bounds_defs(
        &mut arena, base, 1, scalar, le, &ctors,
    ));
    let t0_defs = Instant::now();
    let mut b_defs = Budget::new(STRESS_BUDGET);
    let sig = check_signature(&mut arena, &decls, &mut b_defs).unwrap();
    let defs_ms = t0_defs.elapsed().as_secs_f64() * 1000.0;
    let defs_steps = STRESS_BUDGET - b_defs.remaining();
    eprintln!(
        "  Ast50 copy+ib defs      : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
        defs_steps,
        arena.len(),
        defs_ms,
        defs_steps <= DEFAULT_CONVERSION_STEPS
    );

    let ctx = Context::empty().with_signature(sig);
    let ast = c0(&mut arena, base);
    let copy = c0(&mut arena, copy_base);

    // Single reduction on case 49 (2 Scalars, 3 recursive children)
    {
        let mut q_arena = arena.clone();
        let ctx_red = ctx
            .extend(scalar)
            .extend(scalar)
            .extend(ast)
            .extend(ast)
            .extend(ast);
        let p0 = var(&mut q_arena, 4);
        let p1 = var(&mut q_arena, 3);
        let r0 = var(&mut q_arena, 2);
        let r1 = var(&mut q_arena, 1);
        let r2 = var(&mut q_arena, 0);
        let c49 = c0(&mut q_arena, base + 1 + 49);
        let c49_app = apps(&mut q_arena, c49, &[p0, p1, r0, r1, r2]);
        let lhs = apps(&mut q_arena, copy, &[c49_app]);
        let cr0 = apps(&mut q_arena, copy, &[r0]);
        let cr1 = apps(&mut q_arena, copy, &[r1]);
        let cr2 = apps(&mut q_arena, copy, &[r2]);
        let rhs = apps(&mut q_arena, c49, &[p0, p1, cr0, cr1, cr2]);
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        assert!(convertible(&mut q_arena, &ctx_red, lhs, rhs, ast, &mut b).unwrap());
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  Ast50 single red C49    : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // copy(e) = e
    {
        let mut q_arena = arena.clone();
        let (stmt, proof) =
            mutual_copy_id_proofs(&mut q_arena, base, 1, scalar, &ctors, copy_base)[0];
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(&mut q_arena, &ctx, proof, stmt, &mut b).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  Ast50 copy(e)=e         : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // bounds_mono
    {
        let mut q_arena = arena.clone();
        let (stmt, proof) =
            mutual_bounds_mono_proofs(&mut q_arena, base, 1, scalar, le, le_trans, ib_base, &ctors)
                [0];
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(&mut q_arena, &ctx, proof, stmt, &mut b).unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  Ast50 bounds_mono       : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }
}

// ============================================================================
// 4. Stress 4 — Deep Closed `Nat` Arithmetic (finding the unary-`Nat` wall)
// ============================================================================

#[test]
fn probe_4_deep_closed_nat_arithmetic() {
    eprintln!("\n=== Probe 4: Deep Closed Nat Arithmetic ===");
    let mut arena = TermArena::new();
    let t0 = type_n(&mut arena, 0);
    let nat = c0(&mut arena, 0);
    let zero = c0(&mut arena, 1);
    let succ = c0(&mut arena, 2);
    let rec = c_levels(&mut arena, 3, vec![Level::Constant(0)]);
    let succ_ty = pi(&mut arena, nat, nat);

    let mut decls = declare_inductive_family(&mut arena, 0, 0, 0, t0, &[nat, succ_ty]);

    // plus : Nat -> Nat -> Nat = λa b. Nat.rec (λ_. Nat) b (λ_ ih. Succ ih) a
    let plus_ty = pi_many(&mut arena, &[nat, nat], nat);
    let motive = lam(&mut arena, nat, nat);
    let v0 = var(&mut arena, 0);
    let v1 = var(&mut arena, 1);
    let succ_v0 = apps(&mut arena, succ, &[v0]);
    let plus_step = lam_many(&mut arena, &[nat, nat], succ_v0);
    let plus_rec = apps(&mut arena, rec, &[motive, v0, plus_step, v1]);
    let plus_body = lam_many(&mut arena, &[nat, nat], plus_rec);
    decls.push(Declaration::definition(0, plus_ty, plus_body));
    let plus = c0(&mut arena, 4);

    // mul : Nat -> Nat -> Nat = λa b. Nat.rec (λ_. Nat) Zero (λ_ ih. plus b ih) a
    // Inside mul_step: λ(_:Nat). λ(ih:Nat). plus b ih  (b is var(2), ih is var(0))
    let v2 = var(&mut arena, 2);
    let plus_b_ih = apps(&mut arena, plus, &[v2, v0]);
    let mul_step = lam_many(&mut arena, &[nat, nat], plus_b_ih);
    let mul_rec = apps(&mut arena, rec, &[motive, zero, mul_step, v1]);
    let mul_body = lam_many(&mut arena, &[nat, nat], mul_rec);
    decls.push(Declaration::definition(0, plus_ty, mul_body));
    let mul = c0(&mut arena, 5);

    let sig = check_signature(&mut arena, &decls, &mut Budget::new(STRESS_BUDGET)).unwrap();
    let ctx = Context::empty().with_signature(sig);

    let nat_lit = |arena: &mut TermArena, n: usize| -> TermHandle {
        let mut cur = zero;
        for _ in 0..n {
            cur = apps(arena, succ, &[cur]);
        }
        cur
    };

    let queries: &[(&str, TermHandle, usize, usize, usize)] = &[
        ("plus(100, 100) == 200", plus, 100, 100, 200),
        ("plus(500, 500) == 1000", plus, 500, 500, 1000),
        ("mul(20, 20) == 400", mul, 20, 20, 400),
        ("mul(50, 50) == 2500", mul, 50, 50, 2500),
        ("mul(150, 150) == 22500", mul, 150, 150, 22500),
    ];

    for &(label, op, a, b_val, expected) in queries {
        let mut q_arena = arena.clone();
        let lhs_a = nat_lit(&mut q_arena, a);
        let lhs_b = nat_lit(&mut q_arena, b_val);
        let lhs = apps(&mut q_arena, op, &[lhs_a, lhs_b]);
        let rhs = nat_lit(&mut q_arena, expected);
        let t_start = Instant::now();
        let mut budget = Budget::new(STRESS_BUDGET);
        let ok = convertible(&mut q_arena, &ctx, lhs, rhs, nat, &mut budget).unwrap();
        let ms = t_start.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - budget.remaining();
        assert!(ok, "{label} should be convertible");
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            label,
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }
}

// ============================================================================
// 5. Stress 5 — Binary Natural Numbers (`Pos` + `BinNat`, 0 kernel changes)
// ============================================================================

#[test]
fn probe_5_binary_nat_arithmetic() {
    eprintln!("\n=== Probe 5: Binary Natural Numbers (Pos + BinNat) ===");
    let mut arena = TermArena::new();
    let t0 = type_n(&mut arena, 0);

    // 0: Pos : Type 0
    // 1: One : Pos
    // 2: Bit0 : Pos -> Pos
    // 3: Bit1 : Pos -> Pos
    // 4: Pos.rec
    let pos = c0(&mut arena, 0);
    let one = c0(&mut arena, 1);
    let bit0 = c0(&mut arena, 2);
    let bit1 = c0(&mut arena, 3);
    let pos_rec = c_levels(&mut arena, 4, vec![Level::Constant(0)]);
    let pos_to_pos = pi(&mut arena, pos, pos);

    let mut decls =
        declare_inductive_family(&mut arena, 0, 0, 0, t0, &[pos, pos_to_pos, pos_to_pos]);

    // 5: pos_succ : Pos -> Pos
    //    One => Bit0 One
    //    Bit0 p _ => Bit1 p
    //    Bit1 p ih => Bit0 ih
    let pos_motive = lam(&mut arena, pos, pos);
    let two_pos = apps(&mut arena, bit0, &[one]);
    let v0 = var(&mut arena, 0);
    let v1 = var(&mut arena, 1);
    let bit1_v1 = apps(&mut arena, bit1, &[v1]);
    let succ_bit0 = lam_many(&mut arena, &[pos, pos], bit1_v1);
    let bit0_v0 = apps(&mut arena, bit0, &[v0]);
    let succ_bit1 = lam_many(&mut arena, &[pos, pos], bit0_v0);
    let pos_succ_rec = apps(
        &mut arena,
        pos_rec,
        &[pos_motive, two_pos, succ_bit0, succ_bit1, v0],
    );
    let pos_succ_body = lam(&mut arena, pos, pos_succ_rec);
    decls.push(Declaration::definition(0, pos_to_pos, pos_succ_body));
    let pos_succ = c0(&mut arena, 5);

    // 6: pos_add : Pos -> Pos -> Pos
    //    One => pos_succ
    //    Bit0 p ih => λb. Pos.rec (λ_. Pos) (Bit1 p) (λq _. Bit0 (ih q)) (λq _. Bit1 (ih q)) b
    //    Bit1 p ih => λb. Pos.rec (λ_. Pos) (Bit0 (pos_succ p)) (λq _. Bit1 (ih q)) (λq _. Bit0 (pos_succ (ih q))) b
    let pos_add_ty = pi_many(&mut arena, &[pos, pos], pos);
    let pos_add_motive = lam(&mut arena, pos, pos_to_pos);
    let v2 = var(&mut arena, 2);
    let v3 = var(&mut arena, 3);
    let v4 = var(&mut arena, 4);

    // Inside Bit0 minor: [p: v4, ih: v3, b: v2, q: v1, _ih_q: v0] (inner Pos.rec branches)
    // At b level: [p: v2, ih: v1, b: v0]
    let bit0_b_one = apps(&mut arena, bit1, &[v2]);
    let ih_q = apps(&mut arena, v3, &[v1]);
    let bit0_ih_q = apps(&mut arena, bit0, &[ih_q]);
    let bit1_ih_q = apps(&mut arena, bit1, &[ih_q]);
    let bit0_b_bit0 = lam_many(&mut arena, &[pos, pos], bit0_ih_q);
    let bit0_b_bit1 = lam_many(&mut arena, &[pos, pos], bit1_ih_q);
    let add_bit0_rec = apps(
        &mut arena,
        pos_rec,
        &[pos_motive, bit0_b_one, bit0_b_bit0, bit0_b_bit1, v0],
    );
    let add_bit0 = lam_many(&mut arena, &[pos, pos_to_pos, pos], add_bit0_rec);

    let succ_p = apps(&mut arena, pos_succ, &[v2]);
    let bit1_b_one = apps(&mut arena, bit0, &[succ_p]);
    let bit1_b_bit0 = lam_many(&mut arena, &[pos, pos], bit1_ih_q);
    let succ_ih_q = apps(&mut arena, pos_succ, &[ih_q]);
    let bit0_succ_ih_q = apps(&mut arena, bit0, &[succ_ih_q]);
    let bit1_b_bit1 = lam_many(&mut arena, &[pos, pos], bit0_succ_ih_q);
    let add_bit1_rec = apps(
        &mut arena,
        pos_rec,
        &[pos_motive, bit1_b_one, bit1_b_bit0, bit1_b_bit1, v0],
    );
    let add_bit1 = lam_many(&mut arena, &[pos, pos_to_pos, pos], add_bit1_rec);

    let pos_add_rec = apps(
        &mut arena,
        pos_rec,
        &[pos_add_motive, pos_succ, add_bit0, add_bit1, v0],
    );
    let pos_add_body = lam(&mut arena, pos, pos_add_rec);
    decls.push(Declaration::definition(0, pos_add_ty, pos_add_body));
    let pos_add = c0(&mut arena, 6);

    // 7: pos_mul : Pos -> Pos -> Pos
    //    One => λb. b
    //    Bit0 p ih => λb. Bit0 (ih b)
    //    Bit1 p ih => λb. pos_add b (Bit0 (ih b))
    let mul_one = lam(&mut arena, pos, v0);
    let ih_b = apps(&mut arena, v1, &[v0]);
    let bit0_ih_b = apps(&mut arena, bit0, &[ih_b]);
    let mul_bit0 = lam_many(&mut arena, &[pos, pos_to_pos, pos], bit0_ih_b);
    let add_b_bit0_ih_b = apps(&mut arena, pos_add, &[v0, bit0_ih_b]);
    let mul_bit1 = lam_many(&mut arena, &[pos, pos_to_pos, pos], add_b_bit0_ih_b);
    let pos_mul_rec = apps(
        &mut arena,
        pos_rec,
        &[pos_add_motive, mul_one, mul_bit0, mul_bit1, v0],
    );
    let pos_mul_body = lam(&mut arena, pos, pos_mul_rec);
    decls.push(Declaration::definition(0, pos_add_ty, pos_mul_body));
    let pos_mul = c0(&mut arena, 7);

    // 8: BinNat : Type 0
    // 9: Zero : BinNat
    // 10: OfPos : Pos -> BinNat
    // 11: BinNat.rec
    let bin_base = decls.len() as u32;
    assert_eq!(bin_base, 8);
    let bin_nat = c0(&mut arena, bin_base);
    let bin_zero = c0(&mut arena, bin_base + 1);
    let of_pos = c0(&mut arena, bin_base + 2);
    let bin_rec = c_levels(&mut arena, bin_base + 3, vec![Level::Constant(0)]);
    let of_pos_ty = pi(&mut arena, pos, bin_nat);
    decls.extend(declare_inductive_family(
        &mut arena,
        bin_base,
        0,
        0,
        t0,
        &[bin_nat, of_pos_ty],
    ));

    // 12: bin_add : BinNat -> BinNat -> BinNat
    let bin_to_bin = pi(&mut arena, bin_nat, bin_nat);
    let bin_op_ty = pi_many(&mut arena, &[bin_nat, bin_nat], bin_nat);
    let bin_motive_fn = lam(&mut arena, bin_nat, bin_to_bin);
    let bin_motive_val = lam(&mut arena, bin_nat, bin_nat);
    let bin_id = lam(&mut arena, bin_nat, v0);
    // Inside OfPos p => λb. BinNat.rec (λ_. BinNat) (OfPos p) (λq. OfPos (pos_add p q)) b
    // Context at b: [p: v1, b: v0]; inside λq: [p: v2, b: v1, q: v0]
    let of_pos_p = apps(&mut arena, of_pos, &[v1]);
    let add_p_q = apps(&mut arena, pos_add, &[v2, v0]);
    let of_pos_add_body = apps(&mut arena, of_pos, &[add_p_q]);
    let of_pos_add_p_q = lam(&mut arena, pos, of_pos_add_body);
    let bin_add_inner = apps(
        &mut arena,
        bin_rec,
        &[bin_motive_val, of_pos_p, of_pos_add_p_q, v0],
    );
    let bin_add_of_pos = lam_many(&mut arena, &[pos, bin_nat], bin_add_inner);
    let bin_add_rec = apps(
        &mut arena,
        bin_rec,
        &[bin_motive_fn, bin_id, bin_add_of_pos, v0],
    );
    let bin_add_body = lam(&mut arena, bin_nat, bin_add_rec);
    decls.push(Declaration::definition(0, bin_op_ty, bin_add_body));
    let bin_add = c0(&mut arena, 12);

    // 13: bin_mul : BinNat -> BinNat -> BinNat
    let bin_const_zero = lam(&mut arena, bin_nat, bin_zero);
    let mul_p_q = apps(&mut arena, pos_mul, &[v2, v0]);
    let of_pos_mul_body = apps(&mut arena, of_pos, &[mul_p_q]);
    let of_pos_mul_p_q = lam(&mut arena, pos, of_pos_mul_body);
    let bin_mul_inner = apps(
        &mut arena,
        bin_rec,
        &[bin_motive_val, bin_zero, of_pos_mul_p_q, v0],
    );
    let bin_mul_of_pos = lam_many(&mut arena, &[pos, bin_nat], bin_mul_inner);
    let bin_mul_rec = apps(
        &mut arena,
        bin_rec,
        &[bin_motive_fn, bin_const_zero, bin_mul_of_pos, v0],
    );
    let bin_mul_body = lam(&mut arena, bin_nat, bin_mul_rec);
    decls.push(Declaration::definition(0, bin_op_ty, bin_mul_body));
    let bin_mul = c0(&mut arena, 13);

    let t0_sig = Instant::now();
    let mut b_sig = Budget::new(STRESS_BUDGET);
    let sig = check_signature(&mut arena, &decls, &mut b_sig).unwrap();
    let sig_ms = t0_sig.elapsed().as_secs_f64() * 1000.0;
    let sig_steps = STRESS_BUDGET - b_sig.remaining();
    eprintln!(
        "  Pos+BinNat sig+defs     : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
        sig_steps,
        arena.len(),
        sig_ms,
        sig_steps <= DEFAULT_CONVERSION_STEPS
    );
    let ctx = Context::empty().with_signature(sig);

    let pos_lit = |arena: &mut TermArena, mut n: u64| -> TermHandle {
        assert!(n > 0);
        let mut bits = Vec::new();
        while n > 1 {
            bits.push(n & 1);
            n >>= 1;
        }
        let mut cur = one;
        for bit in bits.into_iter().rev() {
            cur = apps(arena, if bit == 0 { bit0 } else { bit1 }, &[cur]);
        }
        cur
    };
    let bin_lit = |arena: &mut TermArena, n: u64| -> TermHandle {
        if n == 0 {
            bin_zero
        } else {
            let p = pos_lit(arena, n);
            apps(arena, of_pos, &[p])
        }
    };

    let queries: &[(&str, TermHandle, u64, u64, u64)] = &[
        ("bin_add(500, 500)", bin_add, 500, 500, 1_000),
        ("bin_mul(150, 150)", bin_mul, 150, 150, 22_500),
        ("bin_mul(1000, 1000)", bin_mul, 1_000, 1_000, 1_000_000),
        (
            "bin_mul(100k, 100k)",
            bin_mul,
            100_000,
            100_000,
            10_000_000_000,
        ),
    ];

    for &(label, op, a, b_val, expected) in queries {
        let mut q_arena = arena.clone();
        let lhs_a = bin_lit(&mut q_arena, a);
        let lhs_b = bin_lit(&mut q_arena, b_val);
        let lhs = apps(&mut q_arena, op, &[lhs_a, lhs_b]);
        let rhs = bin_lit(&mut q_arena, expected);
        let t_start = Instant::now();
        let mut budget = Budget::new(STRESS_BUDGET);
        let ok = convertible(&mut q_arena, &ctx, lhs, rhs, bin_nat, &mut budget).unwrap();
        let ms = t_start.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - budget.remaining();
        assert!(ok, "{label} should be convertible");
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            label,
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // Proof 1: pos_add_one_r : Π(p : Pos). Id Pos (pos_add p One) (pos_succ p)
    let pos_add_one_r_ty = {
        let lhs = apps(&mut arena, pos_add, &[v0, one]);
        let rhs = apps(&mut arena, pos_succ, &[v0]);
        let eq = id_ty(&mut arena, pos, lhs, rhs);
        pi(&mut arena, pos, eq)
    };
    let pos_add_one_r_proof = {
        let lhs = apps(&mut arena, pos_add, &[v0, one]);
        let rhs = apps(&mut arena, pos_succ, &[v0]);
        let eq = id_ty(&mut arena, pos, lhs, rhs);
        let motive = lam(&mut arena, pos, eq);
        let m_one = refl(&mut arena, pos, two_pos);
        let ih_ty_p = eq;
        let r_bit0 = refl(&mut arena, pos, bit1_v1);
        let m_bit0 = lam_many(&mut arena, &[pos, ih_ty_p], r_bit0);
        let succ_v1 = apps(&mut arena, pos_succ, &[v1]);
        let bit0_succ_v1 = apps(&mut arena, bit0, &[succ_v1]);
        let r_bit1 = refl(&mut arena, pos, bit0_succ_v1);
        let m_bit1 = lam_many(&mut arena, &[pos, ih_ty_p], r_bit1);
        let rec_app = apps(&mut arena, pos_rec, &[motive, m_one, m_bit0, m_bit1, v0]);
        lam(&mut arena, pos, rec_app)
    };
    {
        let mut q_arena = arena.clone();
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(
            &mut q_arena,
            &ctx,
            pos_add_one_r_proof,
            pos_add_one_r_ty,
            &mut b,
        )
        .unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            "proof pos_add_one_r",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // Proof 2 (genuine IH + IdElim):
    // pos_add_succ_r : Π(a : Pos). Π(b : Pos). Id Pos (pos_add a (pos_succ b)) (pos_succ (pos_add a b))
    decls.push(Declaration::definition(
        0,
        pos_add_one_r_ty,
        pos_add_one_r_proof,
    ));
    let pos_add_one_r = c0(&mut arena, 14);
    let sig2 = check_signature(&mut arena, &decls, &mut Budget::new(STRESS_BUDGET)).unwrap();
    let ctx2 = Context::empty().with_signature(sig2);

    // Helper: congr_pos(f, x, y, eq_xy) : Id Pos (f x) (f y) via IdElim
    let congr_pos = |arena: &mut TermArena,
                     f: TermHandle,
                     x: TermHandle,
                     y: TermHandle,
                     eq_xy: TermHandle|
     -> TermHandle {
        let x1 = shift(arena, x, 0, 1);
        let x2 = shift(arena, x, 0, 2);
        let f2 = shift(arena, f, 0, 2);
        let eq_ty = id_ty(arena, pos, x1, v0);
        let fx2 = apps(arena, f2, &[x2]);
        let fy2 = apps(arena, f2, &[v1]);
        let eq_f = id_ty(arena, pos, fx2, fy2);
        let motive = lam_many(arena, &[pos, eq_ty], eq_f);
        let fx = apps(arena, f, &[x]);
        let base = refl(arena, pos, fx);
        arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: y,
            proof: eq_xy,
        })
    };

    let goal_for_a = |arena: &mut TermArena, a_var: TermHandle| -> TermHandle {
        let a_s = shift(arena, a_var, 0, 1);
        let succ_v0 = apps(arena, pos_succ, &[v0]);
        let lhs = apps(arena, pos_add, &[a_s, succ_v0]);
        let add_a_v0 = apps(arena, pos_add, &[a_s, v0]);
        let rhs = apps(arena, pos_succ, &[add_a_v0]);
        let eq = id_ty(arena, pos, lhs, rhs);
        pi(arena, pos, eq)
    };
    let goal_v0 = goal_for_a(&mut arena, v0);
    let pos_add_succ_r_ty = pi(&mut arena, pos, goal_v0);
    let pos_add_succ_r_motive = lam(&mut arena, pos, goal_v0);

    // a = One: λb. Refl Pos (pos_succ (pos_succ b))
    let succ_v0 = apps(&mut arena, pos_succ, &[v0]);
    let succ_succ_v0 = apps(&mut arena, pos_succ, &[succ_v0]);
    let r_one = refl(&mut arena, pos, succ_succ_v0);
    let m_one = lam(&mut arena, pos, r_one);

    // Build inner case split on b for a = Bit0 p or a = Bit1 p:
    // Context at inner Pos.rec: [p: v2, ih: v1, b: v0]
    let build_step =
        |arena: &mut TermArena, outer_bit: TermHandle, other_bit: TermHandle| -> TermHandle {
            let a_term = apps(arena, outer_bit, &[v2]);
            let b_motive = {
                let a_s = shift(arena, a_term, 0, 1);
                let s_v0 = apps(arena, pos_succ, &[v0]);
                let lhs = apps(arena, pos_add, &[a_s, s_v0]);
                let add_a_v0 = apps(arena, pos_add, &[a_s, v0]);
                let rhs = apps(arena, pos_succ, &[add_a_v0]);
                let eq = id_ty(arena, pos, lhs, rhs);
                lam(arena, pos, eq)
            };
            // b = One: congr outer_bit (pos_add p One) (pos_succ p) (pos_add_one_r p)
            let add_p_one = apps(arena, pos_add, &[v2, one]);
            let s_p = apps(arena, pos_succ, &[v2]);
            let eq_p_one = apps(arena, pos_add_one_r, &[v2]);
            let b_one = congr_pos(arena, outer_bit, add_p_one, s_p, eq_p_one);
            // Inner b-branch context: [p: v4, ih: v3, b: v2, q: v1, _ih_q: v0]
            let ih_b_ty = {
                // Type of _ih_q under [p: v3, ih: v2, b: v1, q: v0]
                let a_q = apps(arena, outer_bit, &[v3]);
                let s_v0 = apps(arena, pos_succ, &[v0]);
                let lhs = apps(arena, pos_add, &[a_q, s_v0]);
                let add_a_q = apps(arena, pos_add, &[a_q, v0]);
                let rhs = apps(arena, pos_succ, &[add_a_q]);
                id_ty(arena, pos, lhs, rhs)
            };
            // b = Bit0 q: Refl Pos (other_bit (pos_add p q)) or (other_bit (pos_succ (pos_add p q)))
            let add_p_q = apps(arena, pos_add, &[v4, v1]);
            let b_bit0_val = if outer_bit == bit0 {
                apps(arena, other_bit, &[add_p_q])
            } else {
                let s_add = apps(arena, pos_succ, &[add_p_q]);
                apps(arena, other_bit, &[s_add])
            };
            let r_bit0 = refl(arena, pos, b_bit0_val);
            let b_bit0 = lam_many(arena, &[pos, ih_b_ty], r_bit0);
            // b = Bit1 q: congr outer_bit (pos_add p (pos_succ q)) (pos_succ (pos_add p q)) (ih q)
            let s_q = apps(arena, pos_succ, &[v1]);
            let add_p_sq = apps(arena, pos_add, &[v4, s_q]);
            let s_add_pq = apps(arena, pos_succ, &[add_p_q]);
            let ih_of_q = apps(arena, v3, &[v1]);
            let c_bit1 = congr_pos(arena, outer_bit, add_p_sq, s_add_pq, ih_of_q);
            let b_bit1 = lam_many(arena, &[pos, ih_b_ty], c_bit1);
            let ih_a_ty = goal_for_a(arena, v0);
            let inner_rec = apps(arena, pos_rec, &[b_motive, b_one, b_bit0, b_bit1, v0]);
            lam_many(arena, &[pos, ih_a_ty, pos], inner_rec)
        };

    let m_bit0 = build_step(&mut arena, bit0, bit1);
    let m_bit1 = build_step(&mut arena, bit1, bit0);
    let full_rec = apps(
        &mut arena,
        pos_rec,
        &[pos_add_succ_r_motive, m_one, m_bit0, m_bit1, v0],
    );
    let pos_add_succ_r_proof = lam(&mut arena, pos, full_rec);
    {
        let mut q_arena = arena.clone();
        let t0 = Instant::now();
        let mut b = Budget::new(STRESS_BUDGET);
        check_type(
            &mut q_arena,
            &ctx2,
            pos_add_succ_r_proof,
            pos_add_succ_r_ty,
            &mut b,
        )
        .unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = STRESS_BUDGET - b.remaining();
        eprintln!(
            "  {:<23} : {:>7} steps | {:>6} terms | {:>7.2} ms | fits 65,536: {}",
            "proof pos_add_succ_r",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }
}

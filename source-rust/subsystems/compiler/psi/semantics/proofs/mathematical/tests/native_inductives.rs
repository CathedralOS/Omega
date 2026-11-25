//! Option C (Unified Native Inductive Types — Lean 4 / Rocq model)
//! soundness tests and benchmark suites.

use mathematical_kernel::{
    Budget, Context, CoreError, DEFAULT_CONVERSION_STEPS, Declaration, Level, Sort, Term,
    TermArena, TermHandle, check_signature, check_type, convertible, declare_inductive_family,
    judgment_assumption_closure, shift,
};

const BENCH_BUDGET: u32 = 20_000_000;

fn pi(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Pi { domain, codomain })
}

fn pi_many(arena: &mut TermArena, domains: &[TermHandle], mut codomain: TermHandle) -> TermHandle {
    for &d in domains.iter().rev() {
        codomain = pi(arena, d, codomain);
    }
    codomain
}

fn lam(arena: &mut TermArena, domain: TermHandle, body: TermHandle) -> TermHandle {
    arena.insert(Term::Lambda { domain, body })
}

fn lam_many(arena: &mut TermArena, domains: &[TermHandle], mut body: TermHandle) -> TermHandle {
    for &d in domains.iter().rev() {
        body = lam(arena, d, body);
    }
    body
}

fn sigma(arena: &mut TermArena, domain: TermHandle, codomain: TermHandle) -> TermHandle {
    arena.insert(Term::Sigma { domain, codomain })
}

fn prod(arena: &mut TermArena, left: TermHandle, right: TermHandle) -> TermHandle {
    let shifted_right = shift(arena, right, 0, 1);
    sigma(arena, left, shifted_right)
}

fn pair(arena: &mut TermArena, first: TermHandle, second: TermHandle) -> TermHandle {
    arena.insert(Term::Pair { first, second })
}

fn fst(arena: &mut TermArena, p: TermHandle) -> TermHandle {
    arena.insert(Term::Fst { pair: p })
}

fn snd(arena: &mut TermArena, p: TermHandle) -> TermHandle {
    arena.insert(Term::Snd { pair: p })
}

fn var(arena: &mut TermArena, index: u32) -> TermHandle {
    arena.insert(Term::Variable(index))
}

fn c0(arena: &mut TermArena, declaration: u32) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels: Vec::new(),
    })
}

fn c_levels(arena: &mut TermArena, declaration: u32, levels: Vec<Level>) -> TermHandle {
    arena.insert(Term::Constant {
        declaration,
        levels,
    })
}

fn type_n(arena: &mut TermArena, n: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Type(Level::Constant(n))))
}

fn strict_n(arena: &mut TermArena, n: u32) -> TermHandle {
    arena.insert(Term::Sort(Sort::Strict(Level::Constant(n))))
}

fn apps(arena: &mut TermArena, head: TermHandle, args: &[TermHandle]) -> TermHandle {
    arena.apply_arguments(head, args)
}

fn id_ty(arena: &mut TermArena, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
    arena.insert(Term::Id { ty, left, right })
}

fn refl(arena: &mut TermArena, ty: TermHandle, value: TermHandle) -> TermHandle {
    arena.insert(Term::Refl { ty, value })
}

fn cong_unary(
    arena: &mut TermArena,
    arg_ty: TermHandle,
    res_ty: TermHandle,
    f_fn: TermHandle,
    from: TermHandle,
    to: TermHandle,
    eq: TermHandle,
) -> TermHandle {
    let x0 = var(arena, 0);
    let x1 = var(arena, 1);
    let from_1 = shift(arena, from, 0, 1);
    let arg_ty_1 = shift(arena, arg_ty, 0, 1);
    let eq_ty = id_ty(arena, arg_ty_1, from_1, x0);
    let f_from = apps(arena, f_fn, &[from]);
    let f_from_2 = shift(arena, f_from, 0, 2);
    let f_fn_2 = shift(arena, f_fn, 0, 2);
    let res_ty_2 = shift(arena, res_ty, 0, 2);
    let f_x = apps(arena, f_fn_2, &[x1]);
    let body = id_ty(arena, res_ty_2, f_from_2, f_x);
    let inner = lam(arena, eq_ty, body);
    let motive = lam(arena, arg_ty, inner);
    let base = refl(arena, res_ty, f_from);
    arena.insert(Term::IdElim {
        motive,
        base,
        endpoint: to,
        proof: eq,
    })
}

// ============================================================================
// 1. Strict-positivity & soundness negative tests (+ positive generality)
// ============================================================================

#[test]
fn negative_strict_positivity_and_soundness_rejections() {
    // (a) Negative recursion: data Bad : Type 0 { Mk : (Bad -> Empty) -> Bad }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let bad = c0(&mut arena, 0);
        let empty = arena.insert(Term::Empty);
        let neg = pi(&mut arena, bad, empty);
        let mk_ty = pi(&mut arena, neg, bad);
        let decls = declare_inductive_family(&mut arena, 0, 0, 0, t0, &[mk_ty]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::NonStrictlyPositiveOccurrence);
    }

    // (b) Non-strictly-positive: data Bad : Type 0 { Mk : ((Bad -> Two) -> Bad) -> Bad }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let bad = c0(&mut arena, 0);
        let two = arena.insert(Term::Two);
        let bad_to_two = pi(&mut arena, bad, two);
        let nsp = pi(&mut arena, bad_to_two, bad);
        let mk_ty = pi(&mut arena, nsp, bad);
        let decls = declare_inductive_family(&mut arena, 0, 0, 0, t0, &[mk_ty]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::NonStrictlyPositiveOccurrence);
    }

    // (c) Universe cheat: data Bad : Type 0 { Mk : Type 0 -> Bad }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let bad = c0(&mut arena, 0);
        let mk_ty = pi(&mut arena, t0, bad);
        let decls = declare_inductive_family(&mut arena, 0, 0, 0, t0, &[mk_ty]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InductiveUniverseViolation);
    }

    // (d) Bad return head: data Bad : Type 0 { Mk : Two }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let two = arena.insert(Term::Two);
        let decls = declare_inductive_family(&mut arena, 0, 0, 0, t0, &[two]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InvalidConstructorReturnType);
    }

    // (e) Mismatched uniform parameter: data Bad (A : Type 0) : Type 0 { Mk : Bad Two }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let ind_ty = pi(&mut arena, t0, t0);
        let bad = c0(&mut arena, 0);
        let two = arena.insert(Term::Two);
        let bad_two = apps(&mut arena, bad, &[two]);
        let mk_ty = pi(&mut arena, t0, bad_two);
        let decls = declare_inductive_family(&mut arena, 0, 0, 1, ind_ty, &[mk_ty]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InductiveParameterMismatch);
    }

    // (f) Mismatched uniform parameter in recursive field:
    //     data Bad (A : Type 0) : Type 0 { Mk : Bad Two -> Bad A }
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let ind_ty = pi(&mut arena, t0, t0);
        let bad = c0(&mut arena, 0);
        let two = arena.insert(Term::Two);
        let bad_two = apps(&mut arena, bad, &[two]);
        let v1 = var(&mut arena, 1);
        let bad_a = apps(&mut arena, bad, &[v1]);
        let inner = pi(&mut arena, bad_two, bad_a);
        let mk_ty = pi(&mut arena, t0, inner);
        let decls = declare_inductive_family(&mut arena, 0, 0, 1, ind_ty, &[mk_ty]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InductiveParameterMismatch);
    }

    // (g) Inductive landing in Strict 0 is rejected.
    {
        let mut arena = TermArena::new();
        let s0 = strict_n(&mut arena, 0);
        let bad = c0(&mut arena, 0);
        let decls = declare_inductive_family(&mut arena, 0, 0, 0, s0, &[bad]);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InductiveSortNotRelevant);
    }

    // (h) Strict motive rejection:
    //     (h1) Passing a Strict-valued motive to Nat.rec is refused by typing.
    //     (h2) Forging a recursor declaration with Strict motive is refused by check_signature.
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let nat = c0(&mut arena, 0);
        let succ_ty = pi(&mut arena, nat, nat);
        let mut decls = declare_inductive_family(&mut arena, 0, 0, 0, t0, &[nat, succ_ty]);
        let sig = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap();
        let ctx = Context::empty().with_signature(sig);

        let rec = c_levels(&mut arena, 3, vec![Level::Constant(0)]);
        let empty = arena.insert(Term::Empty);
        let strict_motive = lam(&mut arena, nat, empty);
        let bad_app = apps(&mut arena, rec, &[strict_motive]);
        assert!(
            mathematical_kernel::infer_type(
                &mut arena,
                &ctx,
                bad_app,
                &mut Budget::new(BENCH_BUDGET),
            )
            .is_err()
        );

        let s0 = strict_n(&mut arena, 0);
        let nat_to_s0 = pi(&mut arena, nat, s0);
        let forged_ty = pi(&mut arena, nat_to_s0, s0);
        decls[3] = Declaration::recursor(1, 0, forged_ty);
        let err = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap_err();
        assert_eq!(err, CoreError::InvalidRecursorType);
    }

    // (i) Positive generality check: parameterized List<A> AND higher-order branching Tree
    //     (Node : (Two -> Tree) -> Tree) both check and compute!
    {
        let mut arena = TermArena::new();
        let t0 = type_n(&mut arena, 0);
        let list = c0(&mut arena, 0);
        let list_ty = pi(&mut arena, t0, t0);
        let v0 = var(&mut arena, 0);
        let list_v0 = apps(&mut arena, list, &[v0]);
        let nil_ty = pi(&mut arena, t0, list_v0);
        let v1 = var(&mut arena, 1);
        let v2 = var(&mut arena, 2);
        let list_v1 = apps(&mut arena, list, &[v1]);
        let list_v2 = apps(&mut arena, list, &[v2]);
        let cons_ty = pi_many(&mut arena, &[t0, v0, list_v1], list_v2);
        let mut decls = declare_inductive_family(&mut arena, 0, 0, 1, list_ty, &[nil_ty, cons_ty]);

        let tree_base = decls.len() as u32;
        let tree = c0(&mut arena, tree_base);
        let two = arena.insert(Term::Two);
        let two_to_tree = pi(&mut arena, two, tree);
        let node_ty = pi(&mut arena, two_to_tree, tree);
        decls.extend(declare_inductive_family(
            &mut arena,
            tree_base,
            0,
            0,
            t0,
            &[tree, node_ty],
        ));

        let sig = check_signature(&mut arena, &decls, &mut Budget::new(BENCH_BUDGET)).unwrap();
        assert!(judgment_assumption_closure(&arena, &sig, &[list, tree]).is_empty());

        let ctx = Context::empty().with_signature(sig);
        let leaf = c0(&mut arena, tree_base + 1);
        let node = c0(&mut arena, tree_base + 2);
        let tree_rec = c_levels(&mut arena, tree_base + 3, vec![Level::Constant(0)]);
        let motive = lam(&mut arena, tree, two);
        let zero = arena.insert(Term::TwoZero);
        let one = arena.insert(Term::TwoOne);
        let two_to_two = pi(&mut arena, two, two);
        let ih_var = var(&mut arena, 0);
        let ih_zero = apps(&mut arena, ih_var, &[zero]);
        let minor_node = lam_many(&mut arena, &[two_to_tree, two_to_two], ih_zero);
        let children_fn = lam(&mut arena, two, leaf);
        let sample_tree = apps(&mut arena, node, &[children_fn]);
        let reduced = apps(
            &mut arena,
            tree_rec,
            &[motive, one, minor_node, sample_tree],
        );
        let mut budget = Budget::new(BENCH_BUDGET);
        assert!(convertible(&mut arena, &ctx, reduced, one, two, &mut budget).unwrap());
    }
}

// ============================================================================
// 2. Benchmark Suite 1 — Plain recursive `data`
// ============================================================================

fn plain_data_family(arena: &mut TermArena, base: u32, arities: &[u32]) -> Vec<Declaration> {
    let t0 = type_n(arena, 0);
    let data = c0(arena, base);
    let ctor_tys: Vec<TermHandle> = arities
        .iter()
        .map(|&r| {
            let mut ty = data;
            for _ in 0..r {
                ty = pi(arena, data, ty);
            }
            ty
        })
        .collect();
    declare_inductive_family(arena, base, 0, 0, t0, &ctor_tys)
}

fn plain_copy_def(arena: &mut TermArena, base: u32, arities: &[u32]) -> Declaration {
    let data = c0(arena, base);
    let rec = c_levels(
        arena,
        base + 1 + arities.len() as u32,
        vec![Level::Constant(0)],
    );
    let motive = lam(arena, data, data);
    let mut minors = Vec::with_capacity(arities.len());
    for (c, &r) in arities.iter().enumerate() {
        let mut body = c0(arena, base + 1 + c as u32);
        for q in 0..r {
            let ih = var(arena, r - 1 - q);
            body = apps(arena, body, &[ih]);
        }
        for _ in 0..(2 * r) {
            body = lam(arena, data, body);
        }
        minors.push(body);
    }
    let mut rec_args = Vec::with_capacity(arities.len() + 2);
    rec_args.push(motive);
    rec_args.extend(minors);
    let x0 = var(arena, 0);
    rec_args.push(x0);
    let rec_app = apps(arena, rec, &rec_args);
    let body = lam(arena, data, rec_app);
    let ty = pi(arena, data, data);
    Declaration::definition(0, ty, body)
}

fn plain_copy_id_proof(
    arena: &mut TermArena,
    base: u32,
    arities: &[u32],
    copy_decl: u32,
) -> (TermHandle, TermHandle) {
    let data = c0(arena, base);
    let copy = c0(arena, copy_decl);
    let rec = c_levels(
        arena,
        base + 1 + arities.len() as u32,
        vec![Level::Constant(0)],
    );

    let x0 = var(arena, 0);
    let copy_x = apps(arena, copy, &[x0]);
    let eq_x = id_ty(arena, data, copy_x, x0);
    let motive = lam(arena, data, eq_x);
    let statement = pi(arena, data, eq_x);

    let mut minors = Vec::with_capacity(arities.len());
    for (c, &r) in arities.iter().enumerate() {
        let ctor = c0(arena, base + 1 + c as u32);
        let cur_args: Vec<TermHandle> = (0..r)
            .map(|j| {
                let fj = var(arena, 2 * r - 1 - j);
                apps(arena, copy, &[fj])
            })
            .collect();
        let start_val = apps(arena, ctor, &cur_args);
        let mut proof = refl(arena, data, start_val);

        for j in 0..r {
            let fj = var(arena, 2 * r - 1 - j);
            let ih_j = var(arena, r - 1 - j);
            let copy_fj = apps(arena, copy, &[fj]);
            let copy_fj_1 = shift(arena, copy_fj, 0, 1);
            let v0 = var(arena, 0);
            let y_eq_ty = id_ty(arena, data, copy_fj_1, v0);

            let mut rhs_args = Vec::with_capacity(r as usize);
            for k in 0..r {
                if k == j {
                    rhs_args.push(var(arena, 1));
                } else {
                    let fk = var(arena, 2 * r - 1 - k + 2);
                    if k < j {
                        rhs_args.push(fk);
                    } else {
                        rhs_args.push(apps(arena, copy, &[fk]));
                    }
                }
            }
            let start_shifted_args: Vec<TermHandle> = (0..r)
                .map(|k| {
                    let fk = var(arena, 2 * r - 1 - k + 2);
                    apps(arena, copy, &[fk])
                })
                .collect();
            let start_shifted = apps(arena, ctor, &start_shifted_args);
            let rhs = apps(arena, ctor, &rhs_args);
            let goal_id = id_ty(arena, data, start_shifted, rhs);
            let elim_motive = lam_many(arena, &[data, y_eq_ty], goal_id);
            proof = arena.insert(Term::IdElim {
                motive: elim_motive,
                base: proof,
                endpoint: fj,
                proof: ih_j,
            });
        }

        for j in (0..r).rev() {
            let fj = var(arena, r - 1 - j + j);
            let copy_fj = apps(arena, copy, &[fj]);
            let ih_ty = id_ty(arena, data, copy_fj, fj);
            proof = lam(arena, ih_ty, proof);
        }
        for _ in 0..r {
            proof = lam(arena, data, proof);
        }
        minors.push(proof);
    }

    let mut rec_args = Vec::with_capacity(arities.len() + 2);
    rec_args.push(motive);
    rec_args.extend(minors);
    rec_args.push(x0);
    let rec_app = apps(arena, rec, &rec_args);
    let proof = lam(arena, data, rec_app);
    (statement, proof)
}

#[test]
fn benchmark_suite_1_plain_recursive_data() {
    eprintln!("\n=== Benchmark Suite 1: Plain Recursive Data (Option C) ===");
    let shapes: &[(&str, &[u32])] = &[
        ("2-case [0,1]", &[0, 1]),
        ("4-case [0,0,2,2]", &[0, 0, 2, 2]),
        ("6-case [0,0,1,1,2,2]", &[0, 0, 1, 1, 2, 2]),
        ("8-case [0,0,1,1,2,2,2,3]", &[0, 0, 1, 1, 2, 2, 2, 3]),
        (
            "12-case [0,0,0,1,1,1,2,2,2,2,3,3]",
            &[0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 3, 3],
        ),
    ];
    for &(label, arities) in shapes {
        let mut arena = TermArena::new();
        let decls = plain_data_family(&mut arena, 0, arities);
        let mut budget = Budget::new(BENCH_BUDGET);
        let sig = check_signature(&mut arena, &decls, &mut budget).unwrap();
        let steps = BENCH_BUDGET - budget.remaining();
        let root = c0(&mut arena, 0);
        assert!(judgment_assumption_closure(&arena, &sig, &[root]).is_empty());
        eprintln!(
            "  decl {:<34}: {:>6} steps | {:>5} terms | fits 65,536: {}",
            label,
            steps,
            arena.len(),
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // Nat: plus single reduction, right identity, associativity
    {
        let mut arena = TermArena::new();
        let mut decls = plain_data_family(&mut arena, 0, &[0, 1]);
        let nat = c0(&mut arena, 0);
        let zero = c0(&mut arena, 1);
        let succ = c0(&mut arena, 2);
        let rec = c_levels(&mut arena, 3, vec![Level::Constant(0)]);

        let plus_ty = pi_many(&mut arena, &[nat, nat], nat);
        let motive = lam(&mut arena, nat, nat);
        let v0 = var(&mut arena, 0);
        let v1 = var(&mut arena, 1);
        let succ_v0 = apps(&mut arena, succ, &[v0]);
        let step = lam_many(&mut arena, &[nat, nat], succ_v0);
        let plus_rec = apps(&mut arena, rec, &[motive, v0, step, v1]);
        let plus_body = lam_many(&mut arena, &[nat, nat], plus_rec);
        decls.push(Declaration::definition(0, plus_ty, plus_body));
        let plus = c0(&mut arena, 4);

        let mut b_sig = Budget::new(BENCH_BUDGET);
        let sig = check_signature(&mut arena, &decls, &mut b_sig).unwrap();

        // Single reduction: convertible(plus(Succ(m), b), Succ(plus(m, b)))
        let ctx = Context::empty()
            .with_signature(sig.clone())
            .extend(nat)
            .extend(nat);
        let m = var(&mut arena, 1);
        let b = var(&mut arena, 0);
        let succ_m = apps(&mut arena, succ, &[m]);
        let lhs = apps(&mut arena, plus, &[succ_m, b]);
        let plus_mb = apps(&mut arena, plus, &[m, b]);
        let rhs = apps(&mut arena, succ, &[plus_mb]);
        let mut b_red = Budget::new(BENCH_BUDGET);
        assert!(convertible(&mut arena, &ctx, lhs, rhs, nat, &mut b_red).unwrap());
        let red_steps = BENCH_BUDGET - b_red.remaining();
        eprintln!(
            "  Nat single reduction              : {:>6} steps | fits 65,536: {}",
            red_steps,
            red_steps <= DEFAULT_CONVERSION_STEPS
        );

        // Nat right identity: Π(a : Nat). Id Nat (plus a Zero) a
        let ctx0 = Context::empty().with_signature(sig.clone());
        let a0 = var(&mut arena, 0);
        let plus_a0 = apps(&mut arena, plus, &[a0, zero]);
        let rid_body = id_ty(&mut arena, nat, plus_a0, a0);
        let rid_stmt = pi(&mut arena, nat, rid_body);
        let rid_motive = lam(&mut arena, nat, rid_body);
        let rid_base = refl(&mut arena, nat, zero);
        let m1 = var(&mut arena, 1);
        let ih0 = var(&mut arena, 0);
        let plus_m1_0 = apps(&mut arena, plus, &[m1, zero]);
        let rid_step_body = cong_unary(&mut arena, nat, nat, succ, plus_m1_0, m1, ih0);
        let rid_step = lam_many(&mut arena, &[nat, rid_body], rid_step_body);
        let rid_rec = apps(&mut arena, rec, &[rid_motive, rid_base, rid_step, a0]);
        let rid_proof = lam(&mut arena, nat, rid_rec);
        let mut b_rid = Budget::new(BENCH_BUDGET);
        check_type(&mut arena, &ctx0, rid_proof, rid_stmt, &mut b_rid).unwrap();
        let rid_steps = BENCH_BUDGET - b_rid.remaining();
        eprintln!(
            "  Nat right identity                : {:>6} steps | fits 65,536: {}",
            rid_steps,
            rid_steps <= DEFAULT_CONVERSION_STEPS
        );

        // Nat associativity
        let assoc_eq = |arena: &mut TermArena, a: TermHandle, b: TermHandle, c: TermHandle| {
            let ab = apps(arena, plus, &[a, b]);
            let lhs = apps(arena, plus, &[ab, c]);
            let bc = apps(arena, plus, &[b, c]);
            let rhs = apps(arena, plus, &[a, bc]);
            id_ty(arena, nat, lhs, rhs)
        };
        let assoc_for_a = {
            let va = var(&mut arena, 2);
            let vb = var(&mut arena, 1);
            let vc = var(&mut arena, 0);
            let body = assoc_eq(&mut arena, va, vb, vc);
            pi_many(&mut arena, &[nat, nat], body)
        };
        let assoc_stmt = pi(&mut arena, nat, assoc_for_a);
        let assoc_motive = lam(&mut arena, nat, assoc_for_a);
        let assoc_base = {
            let vb = var(&mut arena, 1);
            let vc = var(&mut arena, 0);
            let bc = apps(&mut arena, plus, &[vb, vc]);
            let r = refl(&mut arena, nat, bc);
            lam_many(&mut arena, &[nat, nat], r)
        };
        let assoc_step = {
            let m = var(&mut arena, 3);
            let ih = var(&mut arena, 2);
            let b = var(&mut arena, 1);
            let c = var(&mut arena, 0);
            let mb = apps(&mut arena, plus, &[m, b]);
            let from = apps(&mut arena, plus, &[mb, c]);
            let bc = apps(&mut arena, plus, &[b, c]);
            let to = apps(&mut arena, plus, &[m, bc]);
            let ih_bc = apps(&mut arena, ih, &[b, c]);
            let body = cong_unary(&mut arena, nat, nat, succ, from, to, ih_bc);
            let body = lam_many(&mut arena, &[nat, nat], body);
            lam_many(&mut arena, &[nat, assoc_for_a], body)
        };
        let v2 = var(&mut arena, 2);
        let rec_a = apps(&mut arena, rec, &[assoc_motive, assoc_base, assoc_step, v2]);
        let rec_abc = apps(&mut arena, rec_a, &[v1, v0]);
        let assoc_proof = lam_many(&mut arena, &[nat, nat, nat], rec_abc);
        let mut b_assoc = Budget::new(BENCH_BUDGET);
        check_type(&mut arena, &ctx0, assoc_proof, assoc_stmt, &mut b_assoc).unwrap();
        let assoc_steps = BENCH_BUDGET - b_assoc.remaining();
        eprintln!(
            "  Nat associativity                 : {:>6} steps | fits 65,536: {}",
            assoc_steps,
            assoc_steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // 4-case and 8-case copy(x) = x by induction
    for &(label, arities) in &[
        ("4-case [0,0,2,2] copy(x)=x", &[0u32, 0, 2, 2][..]),
        (
            "8-case [0,0,1,1,2,2,2,3] copy(x)=x",
            &[0u32, 0, 1, 1, 2, 2, 2, 3][..],
        ),
    ] {
        let mut arena = TermArena::new();
        let mut decls = plain_data_family(&mut arena, 0, arities);
        let copy_decl = decls.len() as u32;
        decls.push(plain_copy_def(&mut arena, 0, arities));
        let mut b_sig = Budget::new(BENCH_BUDGET);
        let sig = check_signature(&mut arena, &decls, &mut b_sig).unwrap();
        let ctx = Context::empty().with_signature(sig);
        let (stmt, proof) = plain_copy_id_proof(&mut arena, 0, arities, copy_decl);
        let mut b_thm = Budget::new(BENCH_BUDGET);
        check_type(&mut arena, &ctx, proof, stmt, &mut b_thm).unwrap();
        let steps = BENCH_BUDGET - b_thm.remaining();
        eprintln!(
            "  {:<34}: {:>6} steps | fits 65,536: {}",
            label,
            steps,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }
}

// ============================================================================
// 3. Benchmark Suite 2 — Payloaded ASTs + `bounds_mono`
// ============================================================================

fn scalar_theory(arena: &mut TermArena) -> Vec<Declaration> {
    let t0 = type_n(arena, 0);
    let scalar = c0(arena, 0);
    let le = c0(arena, 1);
    let le_ty = pi_many(arena, &[scalar, scalar], t0);
    let v1 = var(arena, 1);
    let v2 = var(arena, 2);
    let v4 = var(arena, 4);
    let le_ab = apps(arena, le, &[v2, v1]);
    let le_bc = apps(arena, le, &[v2, v1]);
    let le_ac = apps(arena, le, &[v4, v2]);
    let le_trans_ty = pi_many(arena, &[scalar, scalar, scalar, le_ab, le_bc], le_ac);
    vec![
        Declaration::assumption(0, t0),
        Declaration::assumption(0, le_ty),
        Declaration::assumption(0, le_trans_ty),
    ]
}

fn payloaded_ast_family(
    arena: &mut TermArena,
    base: u32,
    scalar: TermHandle,
    cases: &[(u32, u32)],
) -> Vec<Declaration> {
    let t0 = type_n(arena, 0);
    let ast = c0(arena, base);
    let ctor_tys: Vec<TermHandle> = cases
        .iter()
        .map(|&(p, r)| {
            let mut ty = ast;
            for _ in 0..r {
                ty = pi(arena, ast, ty);
            }
            for _ in 0..p {
                ty = pi(arena, scalar, ty);
            }
            ty
        })
        .collect();
    declare_inductive_family(arena, base, 0, 0, t0, &ctor_tys)
}

fn payloaded_copy_def(
    arena: &mut TermArena,
    base: u32,
    scalar: TermHandle,
    cases: &[(u32, u32)],
) -> Declaration {
    let ast = c0(arena, base);
    let rec = c_levels(
        arena,
        base + 1 + cases.len() as u32,
        vec![Level::Constant(0)],
    );
    let motive = lam(arena, ast, ast);
    let mut minors = Vec::with_capacity(cases.len());
    for (c, &(p, r)) in cases.iter().enumerate() {
        let mut body = c0(arena, base + 1 + c as u32);
        for i in 0..p {
            let pi_var = var(arena, 2 * r + (p - 1 - i));
            body = apps(arena, body, &[pi_var]);
        }
        for q in 0..r {
            let ih_var = var(arena, r - 1 - q);
            body = apps(arena, body, &[ih_var]);
        }
        for _ in 0..(2 * r) {
            body = lam(arena, ast, body);
        }
        for _ in 0..p {
            body = lam(arena, scalar, body);
        }
        minors.push(body);
    }
    let mut rec_args = Vec::with_capacity(cases.len() + 2);
    rec_args.push(motive);
    rec_args.extend(minors);
    let x0 = var(arena, 0);
    rec_args.push(x0);
    let rec_app = apps(arena, rec, &rec_args);
    let body = lam(arena, ast, rec_app);
    let ty = pi(arena, ast, ast);
    Declaration::definition(0, ty, body)
}

fn payloaded_copy_id_proof(
    arena: &mut TermArena,
    base: u32,
    scalar: TermHandle,
    cases: &[(u32, u32)],
    copy_decl: u32,
) -> (TermHandle, TermHandle) {
    let ast = c0(arena, base);
    let copy = c0(arena, copy_decl);
    let rec = c_levels(
        arena,
        base + 1 + cases.len() as u32,
        vec![Level::Constant(0)],
    );

    let x0 = var(arena, 0);
    let copy_x = apps(arena, copy, &[x0]);
    let eq_x = id_ty(arena, ast, copy_x, x0);
    let motive = lam(arena, ast, eq_x);
    let statement = pi(arena, ast, eq_x);

    let mut minors = Vec::with_capacity(cases.len());
    for (c, &(p, r)) in cases.iter().enumerate() {
        let ctor = c0(arena, base + 1 + c as u32);
        let build_ctor =
            |arena: &mut TermArena, extra_shift: u32, replace_up_to: u32, y_at: Option<u32>| {
                let mut args = Vec::with_capacity((p + r) as usize);
                for i in 0..p {
                    args.push(var(arena, 2 * r + (p - 1 - i) + extra_shift));
                }
                for k in 0..r {
                    if y_at == Some(k) {
                        args.push(var(arena, 1));
                    } else {
                        let rk = var(arena, 2 * r - 1 - k + extra_shift);
                        if k < replace_up_to {
                            args.push(rk);
                        } else {
                            args.push(apps(arena, copy, &[rk]));
                        }
                    }
                }
                apps(arena, ctor, &args)
            };
        let start_val = build_ctor(arena, 0, 0, None);
        let mut proof = refl(arena, ast, start_val);
        for j in 0..r {
            let rj = var(arena, 2 * r - 1 - j);
            let ih_j = var(arena, r - 1 - j);
            let copy_rj = apps(arena, copy, &[rj]);
            let copy_rj_1 = shift(arena, copy_rj, 0, 1);
            let v0 = var(arena, 0);
            let y_eq_ty = id_ty(arena, ast, copy_rj_1, v0);
            let start_s = build_ctor(arena, 2, 0, None);
            let rhs = build_ctor(arena, 2, j, Some(j));
            let goal_id = id_ty(arena, ast, start_s, rhs);
            let elim_motive = lam_many(arena, &[ast, y_eq_ty], goal_id);
            proof = arena.insert(Term::IdElim {
                motive: elim_motive,
                base: proof,
                endpoint: rj,
                proof: ih_j,
            });
        }
        for j in (0..r).rev() {
            let rj = var(arena, r - 1 - j + j);
            let copy_rj = apps(arena, copy, &[rj]);
            let ih_ty = id_ty(arena, ast, copy_rj, rj);
            proof = lam(arena, ih_ty, proof);
        }
        for _ in 0..r {
            proof = lam(arena, ast, proof);
        }
        for _ in 0..p {
            proof = lam(arena, scalar, proof);
        }
        minors.push(proof);
    }

    let mut rec_args = Vec::with_capacity(cases.len() + 2);
    rec_args.push(motive);
    rec_args.extend(minors);
    rec_args.push(x0);
    let rec_app = apps(arena, rec, &rec_args);
    let proof = lam(arena, ast, rec_app);
    (statement, proof)
}

fn payloaded_in_bounds_def(
    arena: &mut TermArena,
    base: u32,
    scalar: TermHandle,
    le: TermHandle,
    cases: &[(u32, u32)],
) -> Declaration {
    let t0 = type_n(arena, 0);
    let ast = c0(arena, base);
    let rec_t1 = c_levels(
        arena,
        base + 1 + cases.len() as u32,
        vec![Level::Constant(1)],
    );
    let pred_ty = pi(arena, scalar, t0);
    let motive = lam(arena, ast, pred_ty);

    let mut minors = Vec::with_capacity(cases.len());
    for &(p, r) in cases {
        let b = var(arena, 0);
        let mut conjuncts = Vec::with_capacity((p + r) as usize);
        for i in 0..p {
            let xi = var(arena, 1 + 2 * r + (p - 1 - i));
            conjuncts.push(apps(arena, le, &[xi, b]));
        }
        for q in 0..r {
            let ih_q = var(arena, 1 + (r - 1 - q));
            conjuncts.push(apps(arena, ih_q, &[b]));
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
        for _ in 0..r {
            body = lam(arena, pred_ty, body);
        }
        for _ in 0..r {
            body = lam(arena, ast, body);
        }
        for _ in 0..p {
            body = lam(arena, scalar, body);
        }
        minors.push(body);
    }

    let mut rec_args = Vec::with_capacity(cases.len() + 2);
    rec_args.push(motive);
    rec_args.extend(minors);
    let x0 = var(arena, 0);
    rec_args.push(x0);
    let rec_app = apps(arena, rec_t1, &rec_args);
    let body = lam(arena, ast, rec_app);
    let ty = pi(arena, ast, pred_ty);
    Declaration::definition(0, ty, body)
}

fn payloaded_bounds_mono_proof(
    arena: &mut TermArena,
    base: u32,
    scalar: TermHandle,
    le: TermHandle,
    le_trans: TermHandle,
    in_bounds_decl: u32,
    cases: &[(u32, u32)],
) -> (TermHandle, TermHandle) {
    let ast = c0(arena, base);
    let in_bounds = c0(arena, in_bounds_decl);
    let rec_t0 = c_levels(
        arena,
        base + 1 + cases.len() as u32,
        vec![Level::Constant(0)],
    );

    let mono_for = |arena: &mut TermArena, e_var: u32| {
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
    let motive_body = mono_for(arena, 0);
    let motive = lam(arena, ast, motive_body);
    let statement = pi(arena, ast, motive_body);

    let mut minors = Vec::with_capacity(cases.len());
    for (c_idx, &(p, r)) in cases.iter().enumerate() {
        let total = (p + r) as usize;
        let b1 = var(arena, 3);
        let b2 = var(arena, 2);
        let h_le = var(arena, 1);
        let h_ib = var(arena, 0);

        let mut extracted = Vec::with_capacity(total);
        if total > 0 {
            let mut rem = h_ib;
            for _ in 0..total - 1 {
                extracted.push(fst(arena, rem));
                rem = snd(arena, rem);
            }
            extracted.push(rem);
        }

        let mut mapped = Vec::with_capacity(total);
        for (i, &extracted_i) in extracted.iter().enumerate().take(p as usize) {
            let xi = var(arena, 4 + 2 * r + (p - 1 - i as u32));
            mapped.push(apps(arena, le_trans, &[xi, b1, b2, extracted_i, h_le]));
        }
        for q in 0..r as usize {
            let ih_q = var(arena, 4 + (r - 1 - q as u32));
            mapped.push(apps(
                arena,
                ih_q,
                &[b1, b2, h_le, extracted[p as usize + q]],
            ));
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

        let mut ctor_args = Vec::with_capacity(total);
        for i in 0..p {
            ctor_args.push(var(arena, 3 + 2 * r + (p - 1 - i)));
        }
        for q in 0..r {
            ctor_args.push(var(arena, 3 + r + (r - 1 - q)));
        }
        let ctor = c0(arena, base + 1 + c_idx as u32);
        let ctor_val = apps(arena, ctor, &ctor_args);
        let v2 = var(arena, 2);
        let v1 = var(arena, 1);
        let v0 = var(arena, 0);
        let ib_ty = apps(arena, in_bounds, &[ctor_val, v2]);
        let le_ty = apps(arena, le, &[v1, v0]);
        body = lam_many(arena, &[scalar, scalar, le_ty, ib_ty], body);

        for q in (0..r).rev() {
            let rq_var = (r - 1 - q) + q;
            let ih_ty = mono_for(arena, rq_var);
            body = lam(arena, ih_ty, body);
        }
        for _ in 0..r {
            body = lam(arena, ast, body);
        }
        for _ in 0..p {
            body = lam(arena, scalar, body);
        }
        minors.push(body);
    }

    let mut rec_args = Vec::with_capacity(cases.len() + 2);
    rec_args.push(motive);
    rec_args.extend(minors);
    let x0 = var(arena, 0);
    rec_args.push(x0);
    let rec_app = apps(arena, rec_t0, &rec_args);
    let proof = lam(arena, ast, rec_app);
    (statement, proof)
}

#[test]
fn benchmark_suite_2_payloaded_asts_and_bounds_mono() {
    eprintln!("\n=== Benchmark Suite 2: Payloaded ASTs + bounds_mono (Option C) ===");
    let expr4: &[(u32, u32)] = &[(1, 0), (1, 0), (0, 2), (0, 2)];
    let ast8: &[(u32, u32)] = &[
        (1, 0),
        (1, 0),
        (0, 1),
        (1, 1),
        (1, 2),
        (2, 1),
        (0, 3),
        (1, 2),
    ];
    let ast12: &[(u32, u32)] = &[
        (1, 0),
        (1, 0),
        (2, 0),
        (0, 1),
        (1, 1),
        (2, 1),
        (0, 2),
        (1, 2),
        (1, 2),
        (2, 2),
        (0, 3),
        (1, 3),
    ];

    for &(label, cases, run_theorems, run_single_red) in &[
        ("4-case Expr", expr4, true, false),
        ("8-case Ast8", ast8, false, false),
        ("12-case Ast12", ast12, true, true),
    ] {
        let mut arena = TermArena::new();
        let mut decls = scalar_theory(&mut arena);
        let base = decls.len() as u32;
        let scalar = c0(&mut arena, 0);
        let le = c0(&mut arena, 1);
        let le_trans = c0(&mut arena, 2);

        decls.extend(payloaded_ast_family(&mut arena, base, scalar, cases));
        let mut b_decl = Budget::new(BENCH_BUDGET);
        let _sig_decl = check_signature(&mut arena, &decls, &mut b_decl).unwrap();
        let decl_steps = BENCH_BUDGET - b_decl.remaining();
        eprintln!(
            "  {:<14} decl               : {:>6} steps | {:>5} terms | fits 65,536: {}",
            label,
            decl_steps,
            arena.len(),
            decl_steps <= DEFAULT_CONVERSION_STEPS
        );

        if run_single_red || run_theorems {
            let copy_decl = decls.len() as u32;
            let copy_def = payloaded_copy_def(&mut arena, base, scalar, cases);
            decls.push(copy_def);
            let ib_decl = decls.len() as u32;
            let ib_def = payloaded_in_bounds_def(&mut arena, base, scalar, le, cases);
            decls.push(ib_def);
            let mut b_sig = Budget::new(BENCH_BUDGET);
            let sig = check_signature(&mut arena, &decls, &mut b_sig).unwrap();
            let ctx = Context::empty().with_signature(sig);
            let ast = c0(&mut arena, base);
            let copy = c0(&mut arena, copy_decl);

            if run_single_red {
                let ctx_red = ctx.extend(scalar).extend(ast).extend(ast);
                let p_v = var(&mut arena, 2);
                let l_v = var(&mut arena, 1);
                let r_v = var(&mut arena, 0);
                let c7 = c0(&mut arena, base + 1 + 7);
                let c7_app = apps(&mut arena, c7, &[p_v, l_v, r_v]);
                let lhs = apps(&mut arena, copy, &[c7_app]);
                let copy_l = apps(&mut arena, copy, &[l_v]);
                let copy_r = apps(&mut arena, copy, &[r_v]);
                let rhs = apps(&mut arena, c7, &[p_v, copy_l, copy_r]);
                let mut b_red = Budget::new(BENCH_BUDGET);
                assert!(convertible(&mut arena, &ctx_red, lhs, rhs, ast, &mut b_red).unwrap());
                let red_steps = BENCH_BUDGET - b_red.remaining();
                eprintln!(
                    "  {:<14} single red C7      : {:>6} steps | fits 65,536: {}",
                    label,
                    red_steps,
                    red_steps <= DEFAULT_CONVERSION_STEPS
                );
            }

            if run_theorems {
                let (copy_stmt, copy_proof) =
                    payloaded_copy_id_proof(&mut arena, base, scalar, cases, copy_decl);
                let mut b_copy = Budget::new(BENCH_BUDGET);
                check_type(&mut arena, &ctx, copy_proof, copy_stmt, &mut b_copy).unwrap();
                let copy_steps = BENCH_BUDGET - b_copy.remaining();
                eprintln!(
                    "  {:<14} copy(e)=e          : {:>6} steps | fits 65,536: {}",
                    label,
                    copy_steps,
                    copy_steps <= DEFAULT_CONVERSION_STEPS
                );

                let (mono_stmt, mono_proof) = payloaded_bounds_mono_proof(
                    &mut arena, base, scalar, le, le_trans, ib_decl, cases,
                );
                let mut b_mono = Budget::new(BENCH_BUDGET);
                check_type(&mut arena, &ctx, mono_proof, mono_stmt, &mut b_mono).unwrap();
                let mono_steps = BENCH_BUDGET - b_mono.remaining();
                eprintln!(
                    "  {:<14} bounds_mono        : {:>6} steps | fits 65,536: {}",
                    label,
                    mono_steps,
                    mono_steps <= DEFAULT_CONVERSION_STEPS
                );
            }
        }
    }
}

// ============================================================================
// 4. Benchmark Suite 3 — Native Value-Indexed Family (`Vector<Length, Total>`)
// ============================================================================

#[test]
fn benchmark_suite_3_native_indexed_vector() {
    eprintln!("\n=== Benchmark Suite 3: Native Indexed Vector<Length, Total> (Option C) ===");
    let mut arena = TermArena::new();
    let t0 = type_n(&mut arena, 0);

    let scalar = c0(&mut arena, 0);
    let zero = c0(&mut arena, 1);
    let one = c0(&mut arena, 2);
    let ten = c0(&mut arena, 3);
    let add = c0(&mut arena, 4);
    let le = c0(&mut arena, 5);
    let bound = c0(&mut arena, 6);
    let bound_nil = c0(&mut arena, 7);
    let bound_cons = c0(&mut arena, 8);

    let add_ty = pi_many(&mut arena, &[scalar, scalar], scalar);
    let le_ty = pi_many(&mut arena, &[scalar, scalar], t0);
    let bound_ty = pi_many(&mut arena, &[scalar, scalar], t0);
    let bound_nil_ty = apps(&mut arena, bound, &[zero, zero]);
    let bound_cons_ty = {
        let v0 = var(&mut arena, 0);
        let v2 = var(&mut arena, 2);
        let v3 = var(&mut arena, 3);
        let v4 = var(&mut arena, 4);
        let h_le = apps(&mut arena, le, &[v0, ten]);
        let ih = apps(&mut arena, bound, &[v3, v2]);
        let next_len = apps(&mut arena, add, &[v4, one]);
        let next_tot = apps(&mut arena, add, &[v3, v2]);
        let res = apps(&mut arena, bound, &[next_len, next_tot]);
        pi_many(&mut arena, &[scalar, scalar, scalar, h_le, ih], res)
    };

    let mut decls = vec![
        Declaration::assumption(0, t0),
        Declaration::assumption(0, scalar),
        Declaration::assumption(0, scalar),
        Declaration::assumption(0, scalar),
        Declaration::assumption(0, add_ty),
        Declaration::assumption(0, le_ty),
        Declaration::assumption(0, bound_ty),
        Declaration::assumption(0, bound_nil_ty),
        Declaration::assumption(0, bound_cons_ty),
    ];

    let base = decls.len() as u32;
    let vector = c0(&mut arena, base);
    let nil = c0(&mut arena, base + 1);
    let cons = c0(&mut arena, base + 2);
    let vec_rec = c_levels(&mut arena, base + 3, vec![Level::Constant(0)]);

    let vec_ty = pi_many(&mut arena, &[scalar, scalar], t0);
    let nil_ty = apps(&mut arena, vector, &[zero, zero]);
    let cons_ty = {
        let v0 = var(&mut arena, 0);
        let v2 = var(&mut arena, 2);
        let v3 = var(&mut arena, 3);
        let v4 = var(&mut arena, 4);
        let h_le = apps(&mut arena, le, &[v0, ten]);
        let tail = apps(&mut arena, vector, &[v3, v2]);
        let next_len = apps(&mut arena, add, &[v4, one]);
        let next_tot = apps(&mut arena, add, &[v3, v2]);
        let res = apps(&mut arena, vector, &[next_len, next_tot]);
        pi_many(&mut arena, &[scalar, scalar, scalar, h_le, tail], res)
    };
    decls.extend(declare_inductive_family(
        &mut arena,
        base,
        0,
        0,
        vec_ty,
        &[nil_ty, cons_ty],
    ));

    let mut b_decl = Budget::new(BENCH_BUDGET);
    let sig = check_signature(&mut arena, &decls, &mut b_decl).unwrap();
    let decl_steps = BENCH_BUDGET - b_decl.remaining();
    eprintln!(
        "  Vector<Length,Total> decl         : {:>6} steps | {:>5} terms | fits 65,536: {}",
        decl_steps,
        arena.len(),
        decl_steps <= DEFAULT_CONVERSION_STEPS
    );

    let ctx = Context::empty().with_signature(sig);

    let v0 = var(&mut arena, 0);
    let le_head_ten = apps(&mut arena, le, &[v0, ten]);
    let ctx_cons = ctx.extend(scalar).extend(le_head_ten);
    let head = var(&mut arena, 1);
    let h_le = var(&mut arena, 0);
    let cons_term = apps(&mut arena, cons, &[zero, zero, head, h_le, nil]);
    let len1 = apps(&mut arena, add, &[zero, one]);
    let tot1 = apps(&mut arena, add, &[zero, head]);
    let cons_expected = apps(&mut arena, vector, &[len1, tot1]);
    let mut b_cons = Budget::new(BENCH_BUDGET);
    check_type(&mut arena, &ctx_cons, cons_term, cons_expected, &mut b_cons).unwrap();
    let cons_steps = BENCH_BUDGET - b_cons.remaining();
    eprintln!(
        "  Cons typing                       : {:>6} steps | fits 65,536: {}",
        cons_steps,
        cons_steps <= DEFAULT_CONVERSION_STEPS
    );

    // Negative twin: Cons with wrong tail index (`Cons one zero head h_le Nil`) MUST be refused!
    let wrong_cons = apps(&mut arena, cons, &[one, zero, head, h_le, nil]);
    let wrong_len = apps(&mut arena, add, &[one, one]);
    let wrong_expected = apps(&mut arena, vector, &[wrong_len, tot1]);
    assert!(
        check_type(
            &mut arena,
            &ctx_cons,
            wrong_cons,
            wrong_expected,
            &mut Budget::new(BENCH_BUDGET),
        )
        .is_err()
    );

    // Prove `vector_bound : Π(len tot : Scalar). Vector len tot -> bound len tot`
    let v1 = var(&mut arena, 1);
    let v2 = var(&mut arena, 2);
    let vec_lt = apps(&mut arena, vector, &[v1, v0]);
    let bound_lt = apps(&mut arena, bound, &[v2, v1]);
    let motive = lam_many(&mut arena, &[scalar, scalar, vec_lt], bound_lt);
    let thm_stmt = pi_many(&mut arena, &[scalar, scalar, vec_lt], bound_lt);

    let minor_cons = {
        let v0 = var(&mut arena, 0);
        let v2 = var(&mut arena, 2);
        let v3 = var(&mut arena, 3);
        let v4 = var(&mut arena, 4);
        let v5 = var(&mut arena, 5);
        let h_le_ty = apps(&mut arena, le, &[v0, ten]);
        let tail_ty = apps(&mut arena, vector, &[v3, v2]);
        let ih_ty = apps(&mut arena, bound, &[v4, v3]);
        let body = apps(&mut arena, bound_cons, &[v5, v4, v3, v2, v0]);
        lam_many(
            &mut arena,
            &[scalar, scalar, scalar, h_le_ty, tail_ty, ih_ty],
            body,
        )
    };

    let rec_app = apps(
        &mut arena,
        vec_rec,
        &[motive, bound_nil, minor_cons, v2, v1, v0],
    );
    let thm_proof = lam_many(&mut arena, &[scalar, scalar, vec_lt], rec_app);

    let mut b_thm = Budget::new(BENCH_BUDGET);
    check_type(&mut arena, &ctx, thm_proof, thm_stmt, &mut b_thm).unwrap();
    let thm_steps = BENCH_BUDGET - b_thm.remaining();
    eprintln!(
        "  vector_bound theorem              : {:>6} steps | fits 65,536: {}",
        thm_steps,
        thm_steps <= DEFAULT_CONVERSION_STEPS
    );

    // Single reduction on Cons
    let ctx_red = {
        let v0 = var(&mut arena, 0);
        let v2 = var(&mut arena, 2);
        let v3 = var(&mut arena, 3);
        let h_le_ty = apps(&mut arena, le, &[v0, ten]);
        let tail_ty = apps(&mut arena, vector, &[v3, v2]);
        ctx.extend(scalar)
            .extend(scalar)
            .extend(scalar)
            .extend(h_le_ty)
            .extend(tail_ty)
    };
    let r_len = var(&mut arena, 4);
    let r_tot = var(&mut arena, 3);
    let r_head = var(&mut arena, 2);
    let r_h = var(&mut arena, 1);
    let r_tail = var(&mut arena, 0);
    let next_len = apps(&mut arena, add, &[r_len, one]);
    let next_tot = apps(&mut arena, add, &[r_tot, r_head]);
    let c_val = apps(&mut arena, cons, &[r_len, r_tot, r_head, r_h, r_tail]);
    let lhs = apps(&mut arena, thm_proof, &[next_len, next_tot, c_val]);
    let rec_tail = apps(&mut arena, thm_proof, &[r_len, r_tot, r_tail]);
    let rhs = apps(
        &mut arena,
        bound_cons,
        &[r_len, r_tot, r_head, r_h, rec_tail],
    );
    let expected_ty = apps(&mut arena, bound, &[next_len, next_tot]);
    let mut b_red = Budget::new(BENCH_BUDGET);
    assert!(convertible(&mut arena, &ctx_red, lhs, rhs, expected_ty, &mut b_red).unwrap());
    let red_steps = BENCH_BUDGET - b_red.remaining();
    eprintln!(
        "  Vector single reduction on Cons   : {:>6} steps | fits 65,536: {}",
        red_steps,
        red_steps <= DEFAULT_CONVERSION_STEPS
    );
}

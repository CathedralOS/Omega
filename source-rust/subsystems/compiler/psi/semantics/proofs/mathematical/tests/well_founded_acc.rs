//! Option C Well-Founded Recursion via Accessibility (`Acc`) — Zero New Kernel Rules, Zero Assumptions.
//!
//! Directly tests whether Option C's unified native inductives support:
//! 1. `Acc<u, v> (A : Type u) (R : A -> A -> Type v) : A -> Type max(u, v)` as a higher-order,
//!    indexed, universe-polymorphic native inductive with 0 assumptions, plus 1-step higher-order
//!    iota-reduction of `Acc.rec` on `Acc.intro`.
//! 2. `nat_wf : Π(n : Nat). Acc<0,0> Nat lt n` proved constructively with 0 assumptions.
//! 3. Real `div` and `mod` via well-founded recursion on `Acc Nat lt` (no fuel hack!), with:
//!    - closed computation: `div(17, 5) == 3`, `mod(17, 5) == 2`
//!    - full theorem: `Π(a b : Nat) (hb : lt Zero b). Id Nat (plus (mul (div a b hb) b) (mod a b hb)) a`
//!    - remainder bound: `Π(a b : Nat) (hb : lt Zero b). lt (mod a b hb) b`
//!    - all with empty `judgment_assumption_closure`!
//! 4. Negative soundness tests:
//!    - `no_acc_refl : Π(A : Type 0) (R : A -> A -> Type 0) (x : A). R x x -> Acc A R x -> Empty`
//!      (so `Acc Nat (λ x y. Id Nat x y) Zero` is provably empty!)
//!    - bogus non-decreasing recursive calls rejected by `check_type`.

use std::time::Instant;

use mathematical_kernel::{
    Budget, Context, DEFAULT_CONVERSION_STEPS, Declaration, Level, Sort, Term, TermArena,
    TermHandle, check_signature, check_signature_with_attribution, check_type, convertible,
    declare_inductive_family, judgment_assumption_closure, shift, weak_head_normalize,
};

const WF_BUDGET: u32 = 20_000_000;

/// Minimal named-binder term builder over `TermArena` to keep dependent proofs readable
/// and prevent de Bruijn index arithmetic errors.
struct B<'a> {
    arena: &'a mut TermArena,
    scope: Vec<&'static str>,
}

impl<'a> B<'a> {
    fn new(arena: &'a mut TermArena) -> Self {
        Self {
            arena,
            scope: Vec::new(),
        }
    }

    fn var(&mut self, name: &str) -> TermHandle {
        let pos = self
            .scope
            .iter()
            .rposition(|&s| s == name)
            .unwrap_or_else(|| panic!("unbound variable `{name}` in scope {:?}", self.scope));
        let idx = (self.scope.len() - 1 - pos) as u32;
        self.arena.insert(Term::Variable(idx))
    }

    fn c0(&mut self, declaration: u32) -> TermHandle {
        self.arena.insert(Term::Constant {
            declaration,
            levels: Vec::new(),
        })
    }

    fn c_levels(&mut self, declaration: u32, levels: Vec<Level>) -> TermHandle {
        self.arena.insert(Term::Constant {
            declaration,
            levels,
        })
    }

    fn type_n(&mut self, n: u32) -> TermHandle {
        self.arena
            .insert(Term::Sort(Sort::Type(Level::Constant(n))))
    }

    fn type_level(&mut self, level: Level) -> TermHandle {
        self.arena.insert(Term::Sort(Sort::Type(level)))
    }

    fn pi(
        &mut self,
        name: &'static str,
        domain: TermHandle,
        f: impl FnOnce(&mut Self) -> TermHandle,
    ) -> TermHandle {
        self.scope.push(name);
        let codomain = f(self);
        self.scope.pop();
        self.arena.insert(Term::Pi { domain, codomain })
    }

    fn arrow(&mut self, domain: TermHandle, codomain: TermHandle) -> TermHandle {
        let shifted = shift(self.arena, codomain, 0, 1);
        self.arena.insert(Term::Pi {
            domain,
            codomain: shifted,
        })
    }

    fn lam(
        &mut self,
        name: &'static str,
        domain: TermHandle,
        f: impl FnOnce(&mut Self) -> TermHandle,
    ) -> TermHandle {
        self.scope.push(name);
        let body = f(self);
        self.scope.pop();
        self.arena.insert(Term::Lambda { domain, body })
    }

    fn apps(&mut self, head: TermHandle, args: &[TermHandle]) -> TermHandle {
        self.arena.apply_arguments(head, args)
    }

    fn id(&mut self, ty: TermHandle, left: TermHandle, right: TermHandle) -> TermHandle {
        self.arena.insert(Term::Id { ty, left, right })
    }

    fn refl(&mut self, ty: TermHandle, value: TermHandle) -> TermHandle {
        self.arena.insert(Term::Refl { ty, value })
    }

    fn void_elim(&mut self, c: CoreDecls, ty: TermHandle, scrutinee: TermHandle) -> TermHandle {
        let void = self.c0(c.void);
        let void_rec = self.c_levels(c.void_rec, vec![Level::Constant(0)]);
        let ty_1 = shift(self.arena, ty, 0, 1);
        let motive = self.arena.insert(Term::Lambda {
            domain: void,
            body: ty_1,
        });
        self.apps(void_rec, &[motive, scrutinee])
    }

    fn two(&mut self) -> TermHandle {
        self.arena.insert(Term::Two)
    }

    fn two_zero(&mut self) -> TermHandle {
        self.arena.insert(Term::TwoZero)
    }

    /// `cong_unary`: from `eq : Id arg_ty from to`, build `Id res_ty (f_fn from) (f_fn to)`.
    fn cong(
        &mut self,
        arg_ty: TermHandle,
        res_ty: TermHandle,
        f_fn: TermHandle,
        from: TermHandle,
        to: TermHandle,
        eq: TermHandle,
    ) -> TermHandle {
        let from_1 = shift(self.arena, from, 0, 1);
        let arg_ty_1 = shift(self.arena, arg_ty, 0, 1);
        let f_from = self.apps(f_fn, &[from]);
        let f_from_2 = shift(self.arena, f_from, 0, 2);
        let f_fn_2 = shift(self.arena, f_fn, 0, 2);
        let res_ty_2 = shift(self.arena, res_ty, 0, 2);
        let v0 = self.arena.insert(Term::Variable(0));
        let v1 = self.arena.insert(Term::Variable(1));
        let eq_ty = self.id(arg_ty_1, from_1, v0);
        let f_x = self.apps(f_fn_2, &[v1]);
        let body = self.id(res_ty_2, f_from_2, f_x);
        let inner = self.arena.insert(Term::Lambda {
            domain: eq_ty,
            body,
        });
        let motive = self.arena.insert(Term::Lambda {
            domain: arg_ty,
            body: inner,
        });
        let base = self.refl(res_ty, f_from);
        self.arena.insert(Term::IdElim {
            motive,
            base,
            endpoint: to,
            proof: eq,
        })
    }

    /// `id_trans`: from `eq_ab : Id ty a b` and `eq_bc : Id ty b c`, build `Id ty a c`.
    fn id_trans(
        &mut self,
        ty: TermHandle,
        a: TermHandle,
        b: TermHandle,
        c: TermHandle,
        eq_ab: TermHandle,
        eq_bc: TermHandle,
    ) -> TermHandle {
        let ty_1 = shift(self.arena, ty, 0, 1);
        let b_1 = shift(self.arena, b, 0, 1);
        let ty_2 = shift(self.arena, ty, 0, 2);
        let a_2 = shift(self.arena, a, 0, 2);
        let v0 = self.arena.insert(Term::Variable(0));
        let v1 = self.arena.insert(Term::Variable(1));
        let eq_bx = self.id(ty_1, b_1, v0);
        let body = self.id(ty_2, a_2, v1);
        let inner = self.arena.insert(Term::Lambda {
            domain: eq_bx,
            body,
        });
        let motive = self.arena.insert(Term::Lambda {
            domain: ty,
            body: inner,
        });
        self.arena.insert(Term::IdElim {
            motive,
            base: eq_ab,
            endpoint: c,
            proof: eq_bc,
        })
    }
}

/// Indices of the core native inductives (`Void`, `Acc`, `Nat`, `Le`, `Sum`).
#[derive(Clone, Copy)]
struct CoreDecls {
    void: u32,
    void_rec: u32,
    acc: u32,
    acc_intro: u32,
    acc_rec: u32,
    nat: u32,
    zero: u32,
    succ: u32,
    nat_rec: u32,
    le: u32,
    le_refl: u32,
    le_step: u32,
    le_rec: u32,
    sum: u32,
    inl: u32,
    inr: u32,
    sum_rec: u32,
}

fn declare_void(arena: &mut TermArena, base: u32) -> Vec<Declaration> {
    let t0 = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    declare_inductive_family(arena, base, 0, 0, t0, &[])
}

fn declare_acc(arena: &mut TermArena, base: u32) -> Vec<Declaration> {
    let mut b = B::new(arena);
    let u = Level::Parameter(0);
    let v = Level::Parameter(1);
    let max_uv = u.clone().maximum(v.clone());
    let tu = b.type_level(u.clone());
    let tv = b.type_level(v.clone());
    let tmax = b.type_level(max_uv);
    let acc_const = b.c_levels(base, vec![u.clone(), v.clone()]);

    // Acc<u,v> (A : Type u) (R : A -> A -> Type v) : A -> Type max(u, v)
    let acc_ty = b.pi("A", tu, |b| {
        let a = b.var("A");
        let r_ty = b.pi("_", a, |b| {
            let a = b.var("A");
            b.arrow(a, tv)
        });
        b.pi("R", r_ty, |b| {
            let a = b.var("A");
            b.arrow(a, tmax)
        })
    });

    // Acc.intro : Π(A : Type u) (R : A -> A -> Type v) (x : A).
    //             (Π(y : A). R y x -> Acc<u,v> A R y) -> Acc<u,v> A R x
    let intro_ty = b.pi("A", tu, |b| {
        let a = b.var("A");
        let r_ty = b.pi("_", a, |b| {
            let a = b.var("A");
            b.arrow(a, tv)
        });
        b.pi("R", r_ty, |b| {
            let a = b.var("A");
            b.pi("x", a, |b| {
                let a = b.var("A");
                let f_ty = b.pi("y", a, |b| {
                    let r = b.var("R");
                    let y = b.var("y");
                    let x = b.var("x");
                    let r_yx = b.apps(r, &[y, x]);
                    b.pi("_h", r_yx, |b| {
                        let a = b.var("A");
                        let r = b.var("R");
                        let y = b.var("y");
                        b.apps(acc_const, &[a, r, y])
                    })
                });
                b.pi("_f", f_ty, |b| {
                    let a = b.var("A");
                    let r = b.var("R");
                    let x = b.var("x");
                    b.apps(acc_const, &[a, r, x])
                })
            })
        })
    });

    declare_inductive_family(arena, base, 2, 2, acc_ty, &[intro_ty])
}

fn declare_nat(arena: &mut TermArena, base: u32) -> Vec<Declaration> {
    let mut b = B::new(arena);
    let t0 = b.type_n(0);
    let nat = b.c0(base);
    let succ_ty = b.arrow(nat, nat);
    declare_inductive_family(arena, base, 0, 0, t0, &[nat, succ_ty])
}

fn declare_le(arena: &mut TermArena, base: u32, nat_decl: u32, succ_decl: u32) -> Vec<Declaration> {
    let mut b = B::new(arena);
    let t0 = b.type_n(0);
    let nat = b.c0(nat_decl);
    let succ = b.c0(succ_decl);
    let le = b.c0(base);

    // Le (n : Nat) : Nat -> Type 0   (1 uniform param `n`, 1 index `m`)
    let le_ty = b.pi("n", nat, |b| b.arrow(nat, t0));

    // le_refl : Π(n : Nat). Le n n
    let refl_ty = b.pi("n", nat, |b| {
        let n = b.var("n");
        b.apps(le, &[n, n])
    });

    // le_step : Π(n : Nat) (m : Nat). Le n m -> Le n (Succ m)
    let step_ty = b.pi("n", nat, |b| {
        b.pi("m", nat, |b| {
            let n = b.var("n");
            let m = b.var("m");
            let le_nm = b.apps(le, &[n, m]);
            b.pi("_h", le_nm, |b| {
                let n = b.var("n");
                let m = b.var("m");
                let sm = b.apps(succ, &[m]);
                b.apps(le, &[n, sm])
            })
        })
    });

    declare_inductive_family(arena, base, 0, 1, le_ty, &[refl_ty, step_ty])
}

fn declare_sum(arena: &mut TermArena, base: u32) -> Vec<Declaration> {
    let mut b = B::new(arena);
    let u = Level::Parameter(0);
    let v = Level::Parameter(1);
    let max_uv = u.clone().maximum(v.clone());
    let tu = b.type_level(u.clone());
    let tv = b.type_level(v.clone());
    let tmax = b.type_level(max_uv);
    let sum_const = b.c_levels(base, vec![u, v]);

    let sum_ty = b.pi("A", tu, |b| b.pi("B", tv, |_| tmax));
    let inl_ty = b.pi("A", tu, |b| {
        b.pi("B", tv, |b| {
            let a = b.var("A");
            b.pi("_a", a, |b| {
                let a = b.var("A");
                let b_ty = b.var("B");
                b.apps(sum_const, &[a, b_ty])
            })
        })
    });
    let inr_ty = b.pi("A", tu, |b| {
        b.pi("B", tv, |b| {
            let b_ty = b.var("B");
            b.pi("_b", b_ty, |b| {
                let a = b.var("A");
                let b_ty = b.var("B");
                b.apps(sum_const, &[a, b_ty])
            })
        })
    });

    declare_inductive_family(arena, base, 2, 2, sum_ty, &[inl_ty, inr_ty])
}

fn declare_core_inductives(arena: &mut TermArena) -> (Vec<Declaration>, CoreDecls) {
    let mut decls = Vec::new();

    let void = decls.len() as u32;
    decls.extend(declare_void(arena, void));
    let void_rec = void + 1;

    let acc = decls.len() as u32;
    decls.extend(declare_acc(arena, acc));
    let acc_intro = acc + 1;
    let acc_rec = acc + 2;

    let nat = decls.len() as u32;
    decls.extend(declare_nat(arena, nat));
    let zero = nat + 1;
    let succ = nat + 2;
    let nat_rec = nat + 3;

    let le = decls.len() as u32;
    decls.extend(declare_le(arena, le, nat, succ));
    let le_refl = le + 1;
    let le_step = le + 2;
    let le_rec = le + 3;

    let sum = decls.len() as u32;
    decls.extend(declare_sum(arena, sum));
    let inl = sum + 1;
    let inr = sum + 2;
    let sum_rec = sum + 3;

    (
        decls,
        CoreDecls {
            void,
            void_rec,
            acc,
            acc_intro,
            acc_rec,
            nat,
            zero,
            succ,
            nat_rec,
            le,
            le_refl,
            le_step,
            le_rec,
            sum,
            inl,
            inr,
            sum_rec,
        },
    )
}

fn nat_lit(arena: &mut TermArena, c: CoreDecls, n: u32) -> TermHandle {
    let mut t = arena.insert(Term::Constant {
        declaration: c.zero,
        levels: Vec::new(),
    });
    let succ = arena.insert(Term::Constant {
        declaration: c.succ,
        levels: Vec::new(),
    });
    for _ in 0..n {
        t = arena.apply_arguments(succ, &[t]);
    }
    t
}

/// Build a closed proof of `Le (Succ Zero) (nat_lit n)` for `n >= 1`.
fn proof_lt_zero(arena: &mut TermArena, c: CoreDecls, n: u32) -> TermHandle {
    assert!(n >= 1, "n must be >= 1");
    let one = nat_lit(arena, c, 1);
    let le_refl = arena.insert(Term::Constant {
        declaration: c.le_refl,
        levels: Vec::new(),
    });
    let le_step = arena.insert(Term::Constant {
        declaration: c.le_step,
        levels: Vec::new(),
    });
    let mut p = arena.apply_arguments(le_refl, &[one]);
    for m in 1..n {
        let m_term = nat_lit(arena, c, m);
        p = arena.apply_arguments(le_step, &[one, m_term, p]);
    }
    p
}

// ============================================================================
// Test 1: `Acc` declaration + 1-step higher-order iota-reduction
// ============================================================================

#[test]
fn test_1_acc_declaration_and_higher_order_iota() {
    eprintln!("\n=== Test 1: Acc Declaration & Higher-Order Iota-Reduction ===");
    let mut arena = TermArena::new();
    let t0 = Instant::now();
    let decls = declare_acc(&mut arena, 0);
    let mut budget = Budget::new(WF_BUDGET);
    let sig = check_signature(&mut arena, &decls, &mut budget).unwrap();
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    let steps = WF_BUDGET - budget.remaining();

    let acc_c = arena.insert(Term::Constant {
        declaration: 0,
        levels: vec![Level::Constant(0), Level::Constant(0)],
    });
    let intro_c = arena.insert(Term::Constant {
        declaration: 1,
        levels: vec![Level::Constant(0), Level::Constant(0)],
    });
    let rec_c = arena.insert(Term::Constant {
        declaration: 2,
        levels: vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
    });
    assert!(judgment_assumption_closure(&arena, &sig, &[acc_c, intro_c, rec_c]).is_empty());
    eprintln!(
        "  Acc<u,v> decl            : {:>7} steps | {:>5} terms | {:>7.2} ms | 0 assumptions",
        steps,
        arena.len(),
        ms
    );

    // Check 1-step higher-order iota-reduction:
    // Acc.rec A R P step x (Acc.intro A R x f)
    //   ==>  step x f (λ (y : A) (h : R y x). Acc.rec A R P step y (f y h))
    let (ctx, lhs, rhs, expected_ty) = {
        let mut b = B::new(&mut arena);
        let t0 = b.type_n(0);
        let acc00 = b.c_levels(0, vec![Level::Constant(0), Level::Constant(0)]);
        let intro00 = b.c_levels(1, vec![Level::Constant(0), Level::Constant(0)]);
        let rec000 = b.c_levels(
            2,
            vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        );

        let mut ctx = Context::empty().with_signature(sig);
        // Bind A : Type 0
        ctx = ctx.extend(t0);
        b.scope.push("A");

        // Bind R : A -> A -> Type 0
        let r_ty = {
            let a = b.var("A");
            b.pi("_", a, |b| {
                let a = b.var("A");
                b.arrow(a, t0)
            })
        };
        ctx = ctx.extend(r_ty);
        b.scope.push("R");

        // Bind P : Π(x : A). Acc A R x -> Type 0
        let p_ty = {
            let a = b.var("A");
            b.pi("x", a, |b| {
                let a = b.var("A");
                let r = b.var("R");
                let x = b.var("x");
                let acc_x = b.apps(acc00, &[a, r, x]);
                b.arrow(acc_x, t0)
            })
        };
        ctx = ctx.extend(p_ty);
        b.scope.push("P");

        // Bind step : Π(x : A) (f : Π(y : A). R y x -> Acc A R y).
        //             (Π(y : A) (h : R y x). P y (f y h)) -> P x (Acc.intro A R x f)
        let step_ty = {
            let a = b.var("A");
            b.pi("x", a, |b| {
                let a = b.var("A");
                let f_ty = b.pi("y", a, |b| {
                    let r = b.var("R");
                    let y = b.var("y");
                    let x = b.var("x");
                    let r_yx = b.apps(r, &[y, x]);
                    b.pi("_h", r_yx, |b| {
                        let a = b.var("A");
                        let r = b.var("R");
                        let y = b.var("y");
                        b.apps(acc00, &[a, r, y])
                    })
                });
                b.pi("f", f_ty, |b| {
                    let a = b.var("A");
                    let ih_ty = b.pi("y", a, |b| {
                        let r = b.var("R");
                        let y = b.var("y");
                        let x = b.var("x");
                        let r_yx = b.apps(r, &[y, x]);
                        b.pi("h", r_yx, |b| {
                            let p = b.var("P");
                            let y = b.var("y");
                            let f = b.var("f");
                            let h = b.var("h");
                            let fyh = b.apps(f, &[y, h]);
                            b.apps(p, &[y, fyh])
                        })
                    });
                    b.pi("_ih", ih_ty, |b| {
                        let p = b.var("P");
                        let a = b.var("A");
                        let r = b.var("R");
                        let x = b.var("x");
                        let f = b.var("f");
                        let intro = b.apps(intro00, &[a, r, x, f]);
                        b.apps(p, &[x, intro])
                    })
                })
            })
        };
        ctx = ctx.extend(step_ty);
        b.scope.push("step");

        // Bind x : A
        let a_ty = b.var("A");
        ctx = ctx.extend(a_ty);
        b.scope.push("x");

        // Bind f : Π(y : A). R y x -> Acc A R y
        let f_ty = {
            let a = b.var("A");
            b.pi("y", a, |b| {
                let r = b.var("R");
                let y = b.var("y");
                let x = b.var("x");
                let r_yx = b.apps(r, &[y, x]);
                b.pi("_h", r_yx, |b| {
                    let a = b.var("A");
                    let r = b.var("R");
                    let y = b.var("y");
                    b.apps(acc00, &[a, r, y])
                })
            })
        };
        ctx = ctx.extend(f_ty);
        b.scope.push("f");

        let a = b.var("A");
        let r = b.var("R");
        let p = b.var("P");
        let step = b.var("step");
        let x = b.var("x");
        let f = b.var("f");

        let intro_xf = b.apps(intro00, &[a, r, x, f]);
        let lhs = b.apps(rec000, &[a, r, p, step, x, intro_xf]);

        let rec_ih = b.lam("y", a, |b| {
            let r = b.var("R");
            let y = b.var("y");
            let x = b.var("x");
            let r_yx = b.apps(r, &[y, x]);
            b.lam("h", r_yx, |b| {
                let a = b.var("A");
                let r = b.var("R");
                let p = b.var("P");
                let step = b.var("step");
                let y = b.var("y");
                let f = b.var("f");
                let h = b.var("h");
                let fyh = b.apps(f, &[y, h]);
                b.apps(rec000, &[a, r, p, step, y, fyh])
            })
        });
        let rhs = b.apps(step, &[x, f, rec_ih]);
        let expected_ty = b.apps(p, &[x, intro_xf]);
        (ctx, lhs, rhs, expected_ty)
    };

    // 1-step WHNF check: exact budget of 1 step reduces `lhs` to `rhs`!
    let mut b_whnf = Budget::new(1);
    let whnf = weak_head_normalize(&mut arena, ctx.signature(), lhs, &mut b_whnf).unwrap();
    assert_eq!(
        1 - b_whnf.remaining(),
        1,
        "Acc.rec on Acc.intro must take exactly 1 iota step"
    );
    assert!(
        arena.structurally_equal(whnf, rhs),
        "1-step WHNF of Acc.rec on Acc.intro must match higher-order RHS structurally"
    );

    // Typed conversion check
    let mut b_conv = Budget::new(WF_BUDGET);
    assert!(convertible(&mut arena, &ctx, lhs, rhs, expected_ty, &mut b_conv).unwrap());
    let conv_steps = WF_BUDGET - b_conv.remaining();
    eprintln!("  Acc.rec higher-order iota: 1 WHNF step | {conv_steps} conv steps (PASS)");
}

// ============================================================================
// Tests 2, 3, 4: `nat_wf`, real `div`/`mod` without fuel, `div*b + mod = a`,
//                closed computation `div(17,5)=3`, `mod(17,5)=2`, & negative tests
// ============================================================================

#[test]
fn test_2_3_4_nat_wf_div_mod_identity_and_soundness() {
    eprintln!("\n=== Tests 2-4: nat_wf, Well-Founded div/mod (No Fuel!), & Soundness ===");
    let mut arena = TermArena::new();

    // --- Step A: Core inductives (Acc, Nat, Le, Sum) ---
    let t_core = Instant::now();
    let (mut decls, c) = declare_core_inductives(&mut arena);
    let mut b_core = Budget::new(WF_BUDGET);
    let sig_core = check_signature(&mut arena, &decls, &mut b_core).unwrap();
    let core_ms = t_core.elapsed().as_secs_f64() * 1000.0;
    let core_steps = WF_BUDGET - b_core.remaining();
    let core_roots = [
        arena.insert(Term::Constant {
            declaration: c.acc,
            levels: vec![Level::Constant(0), Level::Constant(0)],
        }),
        arena.insert(Term::Constant {
            declaration: c.nat,
            levels: Vec::new(),
        }),
        arena.insert(Term::Constant {
            declaration: c.le,
            levels: Vec::new(),
        }),
        arena.insert(Term::Constant {
            declaration: c.sum,
            levels: vec![Level::Constant(0), Level::Constant(0)],
        }),
    ];
    assert!(judgment_assumption_closure(&arena, &sig_core, &core_roots).is_empty());
    eprintln!(
        "  Void+Acc+Nat+Le+Sum decls: {:>7} steps | {:>5} terms | {:>7.2} ms | 0 assumptions",
        core_steps,
        arena.len(),
        core_ms
    );

    // Helper to push a definition and return its declaration index.
    let push_def = |decls: &mut Vec<Declaration>, ty: TermHandle, body: TermHandle| -> u32 {
        let idx = decls.len() as u32;
        decls.push(Declaration::definition(0, ty, body));
        idx
    };

    // --- Step B: `lt`, order lemmas, and `nat_wf` ---
    // 1. lt : Nat -> Nat -> Type 0 := λ a b. Le (Succ a) b
    let lt_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let t0 = b.type_n(0);
        let ty = b.pi("_", nat, |b| b.arrow(nat, t0));
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let a = b.var("a");
                let b_v = b.var("b");
                let sa = b.apps(succ, &[a]);
                b.apps(le, &[sa, b_v])
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 2. pred : Nat -> Nat
    let pred_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.arrow(nat, nat);
        let body = b.lam("n", nat, |b| {
            let motive = b.lam("_", nat, |_| nat);
            let step = b.lam("k", nat, |b| b.lam("_ih", nat, |b| b.var("k")));
            let n = b.var("n");
            b.apps(nat_rec, &[motive, zero, step, n])
        });
        push_def(&mut decls, ty, body)
    };

    // 3. le_trans : Π(a b c : Nat). Le a b -> Le b c -> Le a c
    let le_trans_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let le = b.c0(c.le);
        let le_step = b.c0(c.le_step);
        let le_rec = b.c_levels(c.le_rec, vec![Level::Constant(0)]);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                b.pi("c", nat, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let cv = b.var("c");
                    let le_ab = b.apps(le, &[a, bv]);
                    let le_bc = b.apps(le, &[bv, cv]);
                    b.pi("_hab", le_ab, |b| {
                        let le_bc = shift(b.arena, le_bc, 0, 1);
                        b.pi("_hbc", le_bc, |b| {
                            let a = b.var("a");
                            let cv = b.var("c");
                            b.apps(le, &[a, cv])
                        })
                    })
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                b.lam("c", nat, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let cv = b.var("c");
                    let le_ab = b.apps(le, &[a, bv]);
                    let le_bc = b.apps(le, &[bv, cv]);
                    b.lam("hab", le_ab, |b| {
                        let le_bc = shift(b.arena, le_bc, 0, 1);
                        b.lam("hbc", le_bc, |b| {
                            let bv = b.var("b");
                            let motive = b.lam("k", nat, |b| {
                                let bv = b.var("b");
                                let k = b.var("k");
                                let le_bk = b.apps(le, &[bv, k]);
                                b.lam("_", le_bk, |b| {
                                    let a = b.var("a");
                                    let k = b.var("k");
                                    b.apps(le, &[a, k])
                                })
                            });
                            let minor_refl = b.var("hab");
                            let minor_step = b.lam("m", nat, |b| {
                                let bv = b.var("b");
                                let m = b.var("m");
                                let le_bm = b.apps(le, &[bv, m]);
                                b.lam("_hbm", le_bm, |b| {
                                    let a = b.var("a");
                                    let m = b.var("m");
                                    let le_am = b.apps(le, &[a, m]);
                                    b.lam("ih", le_am, |b| {
                                        let a = b.var("a");
                                        let m = b.var("m");
                                        let ih = b.var("ih");
                                        b.apps(le_step, &[a, m, ih])
                                    })
                                })
                            });
                            let cv = b.var("c");
                            let hbc = b.var("hbc");
                            b.apps(le_rec, &[bv, motive, minor_refl, minor_step, cv, hbc])
                        })
                    })
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 4. le_pred_self : Π(m : Nat). Le (pred m) m
    let le_pred_self_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let le = b.c0(c.le);
        let le_refl = b.c0(c.le_refl);
        let le_step = b.c0(c.le_step);
        let pred = b.c0(pred_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.pi("m", nat, |b| {
            let m = b.var("m");
            let pm = b.apps(pred, &[m]);
            b.apps(le, &[pm, m])
        });
        let body = b.lam("m", nat, |b| {
            let motive = b.lam("k", nat, |b| {
                let k = b.var("k");
                let pk = b.apps(pred, &[k]);
                b.apps(le, &[pk, k])
            });
            let base = b.apps(le_refl, &[zero]);
            let step = b.lam("k", nat, |b| {
                let k = b.var("k");
                let pk = b.apps(pred, &[k]);
                let ih_ty = b.apps(le, &[pk, k]);
                b.lam("_ih", ih_ty, |b| {
                    let k = b.var("k");
                    let rk = b.apps(le_refl, &[k]);
                    b.apps(le_step, &[k, k, rk])
                })
            });
            let m = b.var("m");
            b.apps(nat_rec, &[motive, base, step, m])
        });
        push_def(&mut decls, ty, body)
    };

    // 5. pred_le : Π(a b : Nat). Le a b -> Le (pred a) (pred b)
    let pred_le_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let le = b.c0(c.le);
        let le_refl = b.c0(c.le_refl);
        let le_rec = b.c_levels(c.le_rec, vec![Level::Constant(0)]);
        let pred = b.c0(pred_decl);
        let le_trans = b.c0(le_trans_decl);
        let le_pred_self = b.c0(le_pred_self_decl);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let le_ab = b.apps(le, &[a, bv]);
                b.pi("_h", le_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let pa = b.apps(pred, &[a]);
                    let pb = b.apps(pred, &[bv]);
                    b.apps(le, &[pa, pb])
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let le_ab = b.apps(le, &[a, bv]);
                b.lam("h", le_ab, |b| {
                    let a = b.var("a");
                    let motive = b.lam("k", nat, |b| {
                        let a = b.var("a");
                        let k = b.var("k");
                        let le_ak = b.apps(le, &[a, k]);
                        b.lam("_", le_ak, |b| {
                            let a = b.var("a");
                            let k = b.var("k");
                            let pa = b.apps(pred, &[a]);
                            let pk = b.apps(pred, &[k]);
                            b.apps(le, &[pa, pk])
                        })
                    });
                    let pa = b.apps(pred, &[a]);
                    let minor_refl = b.apps(le_refl, &[pa]);
                    let minor_step = b.lam("m", nat, |b| {
                        let a = b.var("a");
                        let m = b.var("m");
                        let le_am = b.apps(le, &[a, m]);
                        b.lam("_ham", le_am, |b| {
                            let a = b.var("a");
                            let m = b.var("m");
                            let pa = b.apps(pred, &[a]);
                            let pm = b.apps(pred, &[m]);
                            let ih_ty = b.apps(le, &[pa, pm]);
                            b.lam("ih", ih_ty, |b| {
                                let a = b.var("a");
                                let m = b.var("m");
                                let pa = b.apps(pred, &[a]);
                                let pm = b.apps(pred, &[m]);
                                let ih = b.var("ih");
                                let lps = b.apps(le_pred_self, &[m]);
                                b.apps(le_trans, &[pa, pm, m, ih, lps])
                            })
                        })
                    });
                    let bv = b.var("b");
                    let h = b.var("h");
                    b.apps(le_rec, &[a, motive, minor_refl, minor_step, bv, h])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 6. is_zero_empty : Nat -> Type 0 (Zero => Void, Succ _ => Two)
    let is_zero_empty_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let t0 = b.type_n(0);
        let void = b.c0(c.void);
        let two = b.two();
        let nat_rec1 = b.c_levels(c.nat_rec, vec![Level::Constant(1)]);
        let ty = b.arrow(nat, t0);
        let body = b.lam("n", nat, |b| {
            let motive = b.lam("_", nat, |_| t0);
            let step = b.lam("_k", nat, |b| b.lam("_ih", t0, |_| two));
            let n = b.var("n");
            b.apps(nat_rec1, &[motive, void, step, n])
        });
        push_def(&mut decls, ty, body)
    };

    // 7. no_lt_zero : Π(y : Nat). Le (Succ y) Zero -> Void
    let no_lt_zero_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let le_rec = b.c_levels(c.le_rec, vec![Level::Constant(0)]);
        let ize = b.c0(is_zero_empty_decl);
        let void = b.c0(c.void);
        let two_zero = b.two_zero();
        let ty = b.pi("y", nat, |b| {
            let y = b.var("y");
            let sy = b.apps(succ, &[y]);
            let le_sy0 = b.apps(le, &[sy, zero]);
            b.arrow(le_sy0, void)
        });
        let body = b.lam("y", nat, |b| {
            let y = b.var("y");
            let sy = b.apps(succ, &[y]);
            let le_sy0 = b.apps(le, &[sy, zero]);
            b.lam("h", le_sy0, |b| {
                let y = b.var("y");
                let sy = b.apps(succ, &[y]);
                let motive = b.lam("k", nat, |b| {
                    let y = b.var("y");
                    let sy = b.apps(succ, &[y]);
                    let k = b.var("k");
                    let le_syk = b.apps(le, &[sy, k]);
                    b.lam("_", le_syk, |b| {
                        let k = b.var("k");
                        b.apps(ize, &[k])
                    })
                });
                let minor_refl = two_zero;
                let minor_step = b.lam("m", nat, |b| {
                    let y = b.var("y");
                    let sy = b.apps(succ, &[y]);
                    let m = b.var("m");
                    let le_sym = b.apps(le, &[sy, m]);
                    b.lam("_h", le_sym, |b| {
                        let m = b.var("m");
                        let ize_m = b.apps(ize, &[m]);
                        b.lam("_ih", ize_m, |_| two_zero)
                    })
                });
                let h = b.var("h");
                b.apps(le_rec, &[sy, motive, minor_refl, minor_step, zero, h])
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 8. wf_aux : Π(n : Nat) (y : Nat). lt y n -> Acc<0,0> Nat lt y
    let wf_aux_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let succ = b.c0(c.succ);
        let acc00 = b.c_levels(c.acc, vec![Level::Constant(0), Level::Constant(0)]);
        let intro00 = b.c_levels(c.acc_intro, vec![Level::Constant(0), Level::Constant(0)]);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let lt = b.c0(lt_decl);
        let le_trans = b.c0(le_trans_decl);
        let pred_le = b.c0(pred_le_decl);
        let no_lt_zero = b.c0(no_lt_zero_decl);

        let ty = b.pi("n", nat, |b| {
            b.pi("y", nat, |b| {
                let y = b.var("y");
                let n = b.var("n");
                let lt_yn = b.apps(lt, &[y, n]);
                b.pi("_h", lt_yn, |b| {
                    let y = b.var("y");
                    b.apps(acc00, &[nat, lt, y])
                })
            })
        });

        let body = b.lam("n", nat, |b| {
            let motive = b.lam("k", nat, |b| {
                b.pi("y", nat, |b| {
                    let y = b.var("y");
                    let k = b.var("k");
                    let lt_yk = b.apps(lt, &[y, k]);
                    b.pi("_h", lt_yk, |b| {
                        let y = b.var("y");
                        b.apps(acc00, &[nat, lt, y])
                    })
                })
            });
            let base = b.lam("y", nat, |b| {
                let y = b.var("y");
                let zero = b.c0(c.zero);
                let lt_y0 = b.apps(lt, &[y, zero]);
                b.lam("h", lt_y0, |b| {
                    let y = b.var("y");
                    let h = b.var("h");
                    let acc_y = b.apps(acc00, &[nat, lt, y]);
                    let absurd = b.apps(no_lt_zero, &[y, h]);
                    b.void_elim(c, acc_y, absurd)
                })
            });
            let step = b.lam("k", nat, |b| {
                let ih_ty = b.pi("z", nat, |b| {
                    let z = b.var("z");
                    let k = b.var("k");
                    let lt_zk = b.apps(lt, &[z, k]);
                    b.pi("_hz", lt_zk, |b| {
                        let z = b.var("z");
                        b.apps(acc00, &[nat, lt, z])
                    })
                });
                b.lam("ih", ih_ty, |b| {
                    b.lam("y", nat, |b| {
                        let y = b.var("y");
                        let k = b.var("k");
                        let sk = b.apps(succ, &[k]);
                        let lt_ysk = b.apps(lt, &[y, sk]);
                        b.lam("h", lt_ysk, |b| {
                            let y = b.var("y");
                            let f = b.lam("z", nat, |b| {
                                let z = b.var("z");
                                let y = b.var("y");
                                let lt_zy = b.apps(lt, &[z, y]);
                                b.lam("hz", lt_zy, |b| {
                                    let z = b.var("z");
                                    let sz = b.apps(succ, &[z]);
                                    let y = b.var("y");
                                    let sy = b.apps(succ, &[y]);
                                    let k = b.var("k");
                                    let sk = b.apps(succ, &[k]);
                                    let hz = b.var("hz");
                                    let h = b.var("h");
                                    let ih = b.var("ih");
                                    let le_yk = b.apps(pred_le, &[sy, sk, h]);
                                    let lt_zk = b.apps(le_trans, &[sz, y, k, hz, le_yk]);
                                    b.apps(ih, &[z, lt_zk])
                                })
                            });
                            b.apps(intro00, &[nat, lt, y, f])
                        })
                    })
                })
            });
            let n = b.var("n");
            b.apps(nat_rec, &[motive, base, step, n])
        });
        push_def(&mut decls, ty, body)
    };

    // 9. nat_wf : Π(n : Nat). Acc<0,0> Nat lt n
    let nat_wf_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let lt = b.c0(lt_decl);
        let acc00 = b.c_levels(c.acc, vec![Level::Constant(0), Level::Constant(0)]);
        let intro00 = b.c_levels(c.acc_intro, vec![Level::Constant(0), Level::Constant(0)]);
        let wf_aux = b.c0(wf_aux_decl);
        let ty = b.pi("n", nat, |b| {
            let n = b.var("n");
            b.apps(acc00, &[nat, lt, n])
        });
        let body = b.lam("n", nat, |b| {
            let n = b.var("n");
            let aux_n = b.apps(wf_aux, &[n]);
            b.apps(intro00, &[nat, lt, n, aux_n])
        });
        push_def(&mut decls, ty, body)
    };

    let t_wf = Instant::now();
    let mut b_wf = Budget::new(WF_BUDGET);
    let sig_wf = check_signature_with_attribution(&mut arena, &decls, &mut b_wf).unwrap();
    let wf_ms = t_wf.elapsed().as_secs_f64() * 1000.0;
    let wf_steps = WF_BUDGET - b_wf.remaining();
    let nat_wf_c = arena.insert(Term::Constant {
        declaration: nat_wf_decl,
        levels: Vec::new(),
    });
    assert!(judgment_assumption_closure(&arena, &sig_wf, &[nat_wf_c]).is_empty());
    eprintln!(
        "  nat_wf proof (0 assumpt) : {:>7} steps | {:>5} terms | {:>7.2} ms | fits 65,536: {}",
        wf_steps,
        arena.len(),
        wf_ms,
        wf_steps <= DEFAULT_CONVERSION_STEPS
    );

    // --- Step C: Arithmetic (`plus`, `mul`, `sub`), `lt_dec`, and lemmas ---
    // 10. plus : Nat -> Nat -> Nat
    let plus_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let succ = b.c0(c.succ);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.pi("_", nat, |b| b.arrow(nat, nat));
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let motive = b.lam("_", nat, |_| nat);
                let bv = b.var("b");
                let step = b.lam("_k", nat, |b| {
                    b.lam("ih", nat, |b| {
                        let ih = b.var("ih");
                        b.apps(succ, &[ih])
                    })
                });
                let a = b.var("a");
                b.apps(nat_rec, &[motive, bv, step, a])
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 11. mul : Nat -> Nat -> Nat (mul Zero b = Zero, mul (Succ q) b = plus b (mul q b))
    let mul_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let plus = b.c0(plus_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.pi("_", nat, |b| b.arrow(nat, nat));
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let motive = b.lam("_", nat, |_| nat);
                let step = b.lam("_k", nat, |b| {
                    b.lam("ih", nat, |b| {
                        let bv = b.var("b");
                        let ih = b.var("ih");
                        b.apps(plus, &[bv, ih])
                    })
                });
                let a = b.var("a");
                b.apps(nat_rec, &[motive, zero, step, a])
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 12. sub : Nat -> Nat -> Nat
    //     sub Zero _ = Zero
    //     sub (Succ a') Zero = Succ a'
    //     sub (Succ a') (Succ b') = sub a' b'
    let sub_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let nat_to_nat = b.arrow(nat, nat);
        let ty = b.pi("_", nat, |b| b.arrow(nat, nat));
        let body = b.lam("a", nat, |b| {
            let motive = b.lam("_", nat, |_| nat_to_nat);
            let base = b.lam("_b", nat, |_| zero);
            let step = b.lam("a'", nat, |b| {
                b.lam("ih", nat_to_nat, |b| {
                    b.lam("b", nat, |b| {
                        let m_b = b.lam("_", nat, |_| nat);
                        let a_p = b.var("a'");
                        let sa_p = b.apps(succ, &[a_p]);
                        let step_b = b.lam("b'", nat, |b| {
                            b.lam("_ih_b", nat, |b| {
                                let ih = b.var("ih");
                                let b_p = b.var("b'");
                                b.apps(ih, &[b_p])
                            })
                        });
                        let bv = b.var("b");
                        b.apps(nat_rec, &[m_b, sa_p, step_b, bv])
                    })
                })
            });
            let a = b.var("a");
            b.apps(nat_rec, &[motive, base, step, a])
        });
        push_def(&mut decls, ty, body)
    };

    // 13. le_zero : Π(n : Nat). Le Zero n
    let le_zero_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let le = b.c0(c.le);
        let le_refl = b.c0(c.le_refl);
        let le_step = b.c0(c.le_step);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.pi("n", nat, |b| {
            let n = b.var("n");
            b.apps(le, &[zero, n])
        });
        let body = b.lam("n", nat, |b| {
            let motive = b.lam("k", nat, |b| {
                let k = b.var("k");
                b.apps(le, &[zero, k])
            });
            let base = b.apps(le_refl, &[zero]);
            let step = b.lam("k", nat, |b| {
                let k = b.var("k");
                let ih_ty = b.apps(le, &[zero, k]);
                b.lam("ih", ih_ty, |b| {
                    let k = b.var("k");
                    let ih = b.var("ih");
                    b.apps(le_step, &[zero, k, ih])
                })
            });
            let n = b.var("n");
            b.apps(nat_rec, &[motive, base, step, n])
        });
        push_def(&mut decls, ty, body)
    };

    // 14. le_succ_mono : Π(a b : Nat). Le a b -> Le (Succ a) (Succ b)
    let le_succ_mono_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let le_refl = b.c0(c.le_refl);
        let le_step = b.c0(c.le_step);
        let le_rec = b.c_levels(c.le_rec, vec![Level::Constant(0)]);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let le_ab = b.apps(le, &[a, bv]);
                b.pi("_h", le_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let sa = b.apps(succ, &[a]);
                    let sb = b.apps(succ, &[bv]);
                    b.apps(le, &[sa, sb])
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let le_ab = b.apps(le, &[a, bv]);
                b.lam("h", le_ab, |b| {
                    let a = b.var("a");
                    let motive = b.lam("k", nat, |b| {
                        let a = b.var("a");
                        let k = b.var("k");
                        let le_ak = b.apps(le, &[a, k]);
                        b.lam("_", le_ak, |b| {
                            let a = b.var("a");
                            let k = b.var("k");
                            let sa = b.apps(succ, &[a]);
                            let sk = b.apps(succ, &[k]);
                            b.apps(le, &[sa, sk])
                        })
                    });
                    let sa = b.apps(succ, &[a]);
                    let minor_refl = b.apps(le_refl, &[sa]);
                    let minor_step = b.lam("m", nat, |b| {
                        let a = b.var("a");
                        let m = b.var("m");
                        let le_am = b.apps(le, &[a, m]);
                        b.lam("_h", le_am, |b| {
                            let a = b.var("a");
                            let m = b.var("m");
                            let sa = b.apps(succ, &[a]);
                            let sm = b.apps(succ, &[m]);
                            let ih_ty = b.apps(le, &[sa, sm]);
                            b.lam("ih", ih_ty, |b| {
                                let a = b.var("a");
                                let m = b.var("m");
                                let sa = b.apps(succ, &[a]);
                                let sm = b.apps(succ, &[m]);
                                let ih = b.var("ih");
                                b.apps(le_step, &[sa, sm, ih])
                            })
                        })
                    });
                    let bv = b.var("b");
                    let h = b.var("h");
                    b.apps(le_rec, &[a, motive, minor_refl, minor_step, bv, h])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 15. lt_dec : Π(a b : Nat). Sum<0,0> (lt a b) (Le b a)
    let lt_dec_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let le_refl = b.c0(c.le_refl);
        let lt = b.c0(lt_decl);
        let le_zero = b.c0(le_zero_decl);
        let le_succ_mono = b.c0(le_succ_mono_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let sum00 = b.c_levels(c.sum, vec![Level::Constant(0), Level::Constant(0)]);
        let inl00 = b.c_levels(c.inl, vec![Level::Constant(0), Level::Constant(0)]);
        let inr00 = b.c_levels(c.inr, vec![Level::Constant(0), Level::Constant(0)]);
        let sum_rec = b.c_levels(
            c.sum_rec,
            vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        );

        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let lt_ab = b.apps(lt, &[a, bv]);
                let le_ba = b.apps(le, &[bv, a]);
                b.apps(sum00, &[lt_ab, le_ba])
            })
        });

        let body = b.lam("a", nat, |b| {
            let motive_a = b.lam("x", nat, |b| {
                b.pi("b", nat, |b| {
                    let x = b.var("x");
                    let bv = b.var("b");
                    let lt_xb = b.apps(lt, &[x, bv]);
                    let le_bx = b.apps(le, &[bv, x]);
                    b.apps(sum00, &[lt_xb, le_bx])
                })
            });
            // Base a = Zero:
            let base_a = b.lam("b", nat, |b| {
                let motive_b = b.lam("y", nat, |b| {
                    let y = b.var("y");
                    let lt_0y = b.apps(lt, &[zero, y]);
                    let le_y0 = b.apps(le, &[y, zero]);
                    b.apps(sum00, &[lt_0y, le_y0])
                });
                let lt_00 = b.apps(lt, &[zero, zero]);
                let le_00 = b.apps(le, &[zero, zero]);
                let r0 = b.apps(le_refl, &[zero]);
                let base_b = b.apps(inr00, &[lt_00, le_00, r0]);
                let step_b = b.lam("b'", nat, |b| {
                    let b_p = b.var("b'");
                    let ih_ty = {
                        let lt_0bp = b.apps(lt, &[zero, b_p]);
                        let le_bp0 = b.apps(le, &[b_p, zero]);
                        b.apps(sum00, &[lt_0bp, le_bp0])
                    };
                    b.lam("_ih_b", ih_ty, |b| {
                        let b_p = b.var("b'");
                        let sb_p = b.apps(succ, &[b_p]);
                        let lt_0s = b.apps(lt, &[zero, sb_p]);
                        let le_s0 = b.apps(le, &[sb_p, zero]);
                        let lz = b.apps(le_zero, &[b_p]);
                        let h = b.apps(le_succ_mono, &[zero, b_p, lz]);
                        b.apps(inl00, &[lt_0s, le_s0, h])
                    })
                });
                let bv = b.var("b");
                b.apps(nat_rec, &[motive_b, base_b, step_b, bv])
            });

            // Step a = Succ a':
            let step_a = b.lam("a'", nat, |b| {
                let ih_a_ty = b.pi("b", nat, |b| {
                    let a_p = b.var("a'");
                    let bv = b.var("b");
                    let lt_ab = b.apps(lt, &[a_p, bv]);
                    let le_ba = b.apps(le, &[bv, a_p]);
                    b.apps(sum00, &[lt_ab, le_ba])
                });
                b.lam("ih_a", ih_a_ty, |b| {
                    b.lam("b", nat, |b| {
                        let motive_b = b.lam("y", nat, |b| {
                            let a_p = b.var("a'");
                            let sa_p = b.apps(succ, &[a_p]);
                            let y = b.var("y");
                            let lt_sy = b.apps(lt, &[sa_p, y]);
                            let le_ys = b.apps(le, &[y, sa_p]);
                            b.apps(sum00, &[lt_sy, le_ys])
                        });
                        let a_p = b.var("a'");
                        let sa_p = b.apps(succ, &[a_p]);
                        let lt_s0 = b.apps(lt, &[sa_p, zero]);
                        let le_0s = b.apps(le, &[zero, sa_p]);
                        let lz = b.apps(le_zero, &[sa_p]);
                        let base_b = b.apps(inr00, &[lt_s0, le_0s, lz]);
                        let step_b = b.lam("b'", nat, |b| {
                            let a_p = b.var("a'");
                            let sa_p = b.apps(succ, &[a_p]);
                            let b_p = b.var("b'");
                            let ih_b_ty = {
                                let lt_sb = b.apps(lt, &[sa_p, b_p]);
                                let le_bs = b.apps(le, &[b_p, sa_p]);
                                b.apps(sum00, &[lt_sb, le_bs])
                            };
                            b.lam("_ih_b", ih_b_ty, |b| {
                                let a_p = b.var("a'");
                                let b_p = b.var("b'");
                                let sa_p = b.apps(succ, &[a_p]);
                                let sb_p = b.apps(succ, &[b_p]);
                                let dom_l = b.apps(lt, &[a_p, b_p]);
                                let dom_r = b.apps(le, &[b_p, a_p]);
                                let res_l = b.apps(lt, &[sa_p, sb_p]);
                                let res_r = b.apps(le, &[sb_p, sa_p]);
                                let res_sum = b.apps(sum00, &[res_l, res_r]);
                                let sum_dom = b.apps(sum00, &[dom_l, dom_r]);
                                let m_sum = b.lam("_", sum_dom, |b| shift(b.arena, res_sum, 0, 1));
                                let minor_l = b.lam("hl", dom_l, |b| {
                                    let a_p = b.var("a'");
                                    let b_p = b.var("b'");
                                    let sa_p = b.apps(succ, &[a_p]);
                                    let sb_p = b.apps(succ, &[b_p]);
                                    let res_l = b.apps(lt, &[sa_p, sb_p]);
                                    let res_r = b.apps(le, &[sb_p, sa_p]);
                                    let hl = b.var("hl");
                                    let lifted = b.apps(le_succ_mono, &[sa_p, b_p, hl]);
                                    b.apps(inl00, &[res_l, res_r, lifted])
                                });
                                let minor_r = b.lam("hr", dom_r, |b| {
                                    let a_p = b.var("a'");
                                    let b_p = b.var("b'");
                                    let sa_p = b.apps(succ, &[a_p]);
                                    let sb_p = b.apps(succ, &[b_p]);
                                    let res_l = b.apps(lt, &[sa_p, sb_p]);
                                    let res_r = b.apps(le, &[sb_p, sa_p]);
                                    let hr = b.var("hr");
                                    let lifted = b.apps(le_succ_mono, &[b_p, a_p, hr]);
                                    b.apps(inr00, &[res_l, res_r, lifted])
                                });
                                let ih_a = b.var("ih_a");
                                let scrut = b.apps(ih_a, &[b_p]);
                                b.apps(sum_rec, &[dom_l, dom_r, m_sum, minor_l, minor_r, scrut])
                            })
                        });
                        let bv = b.var("b");
                        b.apps(nat_rec, &[motive_b, base_b, step_b, bv])
                    })
                })
            });

            let a = b.var("a");
            b.apps(nat_rec, &[motive_a, base_a, step_a, a])
        });
        push_def(&mut decls, ty, body)
    };

    // 16. sub_zero : Π(a : Nat). Id Nat (sub a Zero) a
    let sub_zero_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let sub = b.c0(sub_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.pi("a", nat, |b| {
            let a = b.var("a");
            let sa0 = b.apps(sub, &[a, zero]);
            b.id(nat, sa0, a)
        });
        let body = b.lam("a", nat, |b| {
            let motive = b.lam("k", nat, |b| {
                let k = b.var("k");
                let sk0 = b.apps(sub, &[k, zero]);
                b.id(nat, sk0, k)
            });
            let base = b.refl(nat, zero);
            let step = b.lam("k", nat, |b| {
                let k = b.var("k");
                let sk0 = b.apps(sub, &[k, zero]);
                let ih_ty = b.id(nat, sk0, k);
                b.lam("_ih", ih_ty, |b| {
                    let k = b.var("k");
                    let sk = b.apps(succ, &[k]);
                    b.refl(nat, sk)
                })
            });
            let a = b.var("a");
            b.apps(nat_rec, &[motive, base, step, a])
        });
        push_def(&mut decls, ty, body)
    };

    // 17. sub_le : Π(a b : Nat). Le (sub a b) a
    let sub_le_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let le_refl = b.c0(c.le_refl);
        let le_step = b.c0(c.le_step);
        let sub = b.c0(sub_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let sab = b.apps(sub, &[a, bv]);
                b.apps(le, &[sab, a])
            })
        });
        let body = b.lam("a", nat, |b| {
            let motive_a = b.lam("x", nat, |b| {
                b.pi("b", nat, |b| {
                    let x = b.var("x");
                    let bv = b.var("b");
                    let sxb = b.apps(sub, &[x, bv]);
                    b.apps(le, &[sxb, x])
                })
            });
            let base_a = b.lam("_b", nat, |b| b.apps(le_refl, &[zero]));
            let step_a = b.lam("a'", nat, |b| {
                let ih_a_ty = b.pi("b", nat, |b| {
                    let a_p = b.var("a'");
                    let bv = b.var("b");
                    let sab = b.apps(sub, &[a_p, bv]);
                    b.apps(le, &[sab, a_p])
                });
                b.lam("ih_a", ih_a_ty, |b| {
                    b.lam("b", nat, |b| {
                        let motive_b = b.lam("y", nat, |b| {
                            let a_p = b.var("a'");
                            let sa_p = b.apps(succ, &[a_p]);
                            let y = b.var("y");
                            let s = b.apps(sub, &[sa_p, y]);
                            b.apps(le, &[s, sa_p])
                        });
                        let a_p = b.var("a'");
                        let sa_p = b.apps(succ, &[a_p]);
                        let base_b = b.apps(le_refl, &[sa_p]);
                        let step_b = b.lam("b'", nat, |b| {
                            let a_p = b.var("a'");
                            let sa_p = b.apps(succ, &[a_p]);
                            let b_p = b.var("b'");
                            let s = b.apps(sub, &[sa_p, b_p]);
                            let ih_b_ty = b.apps(le, &[s, sa_p]);
                            b.lam("_ih_b", ih_b_ty, |b| {
                                let a_p = b.var("a'");
                                let b_p = b.var("b'");
                                let sab = b.apps(sub, &[a_p, b_p]);
                                let ih_a = b.var("ih_a");
                                let h = b.apps(ih_a, &[b_p]);
                                b.apps(le_step, &[sab, a_p, h])
                            })
                        });
                        let bv = b.var("b");
                        b.apps(nat_rec, &[motive_b, base_b, step_b, bv])
                    })
                })
            });
            let a = b.var("a");
            b.apps(nat_rec, &[motive_a, base_a, step_a, a])
        });
        push_def(&mut decls, ty, body)
    };

    // 18. sub_lt : Π(a b : Nat). lt Zero b -> Le b a -> lt (sub a b) a
    let sub_lt_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let lt = b.c0(lt_decl);
        let sub = b.c0(sub_decl);
        let sub_le = b.c0(sub_le_decl);
        let le_succ_mono = b.c0(le_succ_mono_decl);
        let no_lt_zero = b.c0(no_lt_zero_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);

        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.pi("_hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let hle_ty = b.apps(le, &[bv, a]);
                    b.pi("_hle", hle_ty, |b| {
                        let a = b.var("a");
                        let bv = b.var("b");
                        let sab = b.apps(sub, &[a, bv]);
                        b.apps(lt, &[sab, a])
                    })
                })
            })
        });

        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                // Case split on `b`:
                let motive_b = b.lam("y", nat, |b| {
                    let y = b.var("y");
                    let hb_ty = b.apps(lt, &[zero, y]);
                    b.pi("_hb", hb_ty, |b| {
                        let a = b.var("a");
                        let y = b.var("y");
                        let hle_ty = b.apps(le, &[y, a]);
                        b.pi("_hle", hle_ty, |b| {
                            let a = b.var("a");
                            let y = b.var("y");
                            let say = b.apps(sub, &[a, y]);
                            b.apps(lt, &[say, a])
                        })
                    })
                });
                let lt_00 = b.apps(lt, &[zero, zero]);
                let base_b = b.lam("hb", lt_00, |b| {
                    let a = b.var("a");
                    let hle_ty = b.apps(le, &[zero, a]);
                    b.lam("_hle", hle_ty, |b| {
                        let a = b.var("a");
                        let sa0 = b.apps(sub, &[a, zero]);
                        let goal = b.apps(lt, &[sa0, a]);
                        let hb = b.var("hb");
                        let absurd = b.apps(no_lt_zero, &[zero, hb]);
                        b.void_elim(c, goal, absurd)
                    })
                });
                let step_b = b.lam("b'", nat, |b| {
                    let ih_b_ty = {
                        let b_p = b.var("b'");
                        let hb_ty = b.apps(lt, &[zero, b_p]);
                        b.pi("_hb", hb_ty, |b| {
                            let a = b.var("a");
                            let b_p = b.var("b'");
                            let hle_ty = b.apps(le, &[b_p, a]);
                            b.pi("_hle", hle_ty, |b| {
                                let a = b.var("a");
                                let b_p = b.var("b'");
                                let sab = b.apps(sub, &[a, b_p]);
                                b.apps(lt, &[sab, a])
                            })
                        })
                    };
                    b.lam("_ih_b", ih_b_ty, |b| {
                        let b_p = b.var("b'");
                        let sb_p = b.apps(succ, &[b_p]);
                        let hb_ty = b.apps(lt, &[zero, sb_p]);
                        b.lam("_hb", hb_ty, |b| {
                            // Case split on `a`:
                            let motive_a = b.lam("x", nat, |b| {
                                let b_p = b.var("b'");
                                let sb_p = b.apps(succ, &[b_p]);
                                let x = b.var("x");
                                let hle_ty = b.apps(le, &[sb_p, x]);
                                b.pi("_hle", hle_ty, |b| {
                                    let b_p = b.var("b'");
                                    let sb_p = b.apps(succ, &[b_p]);
                                    let x = b.var("x");
                                    let sxs = b.apps(sub, &[x, sb_p]);
                                    b.apps(lt, &[sxs, x])
                                })
                            });
                            let b_p = b.var("b'");
                            let sb_p = b.apps(succ, &[b_p]);
                            let le_sb0 = b.apps(le, &[sb_p, zero]);
                            let base_a = b.lam("hle", le_sb0, |b| {
                                let b_p = b.var("b'");
                                let sb_p = b.apps(succ, &[b_p]);
                                let s0s = b.apps(sub, &[zero, sb_p]);
                                let goal = b.apps(lt, &[s0s, zero]);
                                let hle = b.var("hle");
                                let absurd = b.apps(no_lt_zero, &[b_p, hle]);
                                b.void_elim(c, goal, absurd)
                            });
                            let step_a = b.lam("a'", nat, |b| {
                                let ih_a_ty = {
                                    let b_p = b.var("b'");
                                    let sb_p = b.apps(succ, &[b_p]);
                                    let a_p = b.var("a'");
                                    let hle_ty = b.apps(le, &[sb_p, a_p]);
                                    b.pi("_hle", hle_ty, |b| {
                                        let b_p = b.var("b'");
                                        let sb_p = b.apps(succ, &[b_p]);
                                        let a_p = b.var("a'");
                                        let sas = b.apps(sub, &[a_p, sb_p]);
                                        b.apps(lt, &[sas, a_p])
                                    })
                                };
                                b.lam("_ih_a", ih_a_ty, |b| {
                                    let b_p = b.var("b'");
                                    let sb_p = b.apps(succ, &[b_p]);
                                    let a_p = b.var("a'");
                                    let sa_p = b.apps(succ, &[a_p]);
                                    let hle_ty = b.apps(le, &[sb_p, sa_p]);
                                    b.lam("_hle", hle_ty, |b| {
                                        let a_p = b.var("a'");
                                        let b_p = b.var("b'");
                                        let sab = b.apps(sub, &[a_p, b_p]);
                                        let h = b.apps(sub_le, &[a_p, b_p]);
                                        b.apps(le_succ_mono, &[sab, a_p, h])
                                    })
                                })
                            });
                            let a = b.var("a");
                            b.apps(nat_rec, &[motive_a, base_a, step_a, a])
                        })
                    })
                });
                let bv = b.var("b");
                b.apps(nat_rec, &[motive_b, base_b, step_b, bv])
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 19. plus_assoc : Π(a b c : Nat). Id Nat (plus (plus a b) c) (plus a (plus b c))
    let plus_assoc_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let succ = b.c0(c.succ);
        let plus = b.c0(plus_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                b.pi("c", nat, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let cv = b.var("c");
                    let ab = b.apps(plus, &[a, bv]);
                    let lhs = b.apps(plus, &[ab, cv]);
                    let bc = b.apps(plus, &[bv, cv]);
                    let rhs = b.apps(plus, &[a, bc]);
                    b.id(nat, lhs, rhs)
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            let motive = b.lam("x", nat, |b| {
                b.pi("b", nat, |b| {
                    b.pi("c", nat, |b| {
                        let x = b.var("x");
                        let bv = b.var("b");
                        let cv = b.var("c");
                        let xb = b.apps(plus, &[x, bv]);
                        let lhs = b.apps(plus, &[xb, cv]);
                        let bc = b.apps(plus, &[bv, cv]);
                        let rhs = b.apps(plus, &[x, bc]);
                        b.id(nat, lhs, rhs)
                    })
                })
            });
            let base = b.lam("b", nat, |b| {
                b.lam("c", nat, |b| {
                    let bv = b.var("b");
                    let cv = b.var("c");
                    let bc = b.apps(plus, &[bv, cv]);
                    b.refl(nat, bc)
                })
            });
            let step = b.lam("k", nat, |b| {
                let ih_ty = b.pi("b", nat, |b| {
                    b.pi("c", nat, |b| {
                        let k = b.var("k");
                        let bv = b.var("b");
                        let cv = b.var("c");
                        let kb = b.apps(plus, &[k, bv]);
                        let lhs = b.apps(plus, &[kb, cv]);
                        let bc = b.apps(plus, &[bv, cv]);
                        let rhs = b.apps(plus, &[k, bc]);
                        b.id(nat, lhs, rhs)
                    })
                });
                b.lam("ih", ih_ty, |b| {
                    b.lam("b", nat, |b| {
                        b.lam("c", nat, |b| {
                            let k = b.var("k");
                            let bv = b.var("b");
                            let cv = b.var("c");
                            let ih = b.var("ih");
                            let kb = b.apps(plus, &[k, bv]);
                            let from = b.apps(plus, &[kb, cv]);
                            let bc = b.apps(plus, &[bv, cv]);
                            let to = b.apps(plus, &[k, bc]);
                            let ih_bc = b.apps(ih, &[bv, cv]);
                            b.cong(nat, nat, succ, from, to, ih_bc)
                        })
                    })
                })
            });
            let a = b.var("a");
            b.apps(nat_rec, &[motive, base, step, a])
        });
        push_def(&mut decls, ty, body)
    };

    // 20. add_sub_cancel : Π(b a : Nat). Le b a -> Id Nat (plus b (sub a b)) a
    let add_sub_cancel_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let plus = b.c0(plus_decl);
        let sub = b.c0(sub_decl);
        let sub_zero = b.c0(sub_zero_decl);
        let pred_le = b.c0(pred_le_decl);
        let no_lt_zero = b.c0(no_lt_zero_decl);
        let nat_rec = b.c_levels(c.nat_rec, vec![Level::Constant(0)]);

        let ty = b.pi("b", nat, |b| {
            b.pi("a", nat, |b| {
                let bv = b.var("b");
                let a = b.var("a");
                let le_ba = b.apps(le, &[bv, a]);
                b.pi("_h", le_ba, |b| {
                    let bv = b.var("b");
                    let a = b.var("a");
                    let sab = b.apps(sub, &[a, bv]);
                    let lhs = b.apps(plus, &[bv, sab]);
                    b.id(nat, lhs, a)
                })
            })
        });

        let body = b.lam("b", nat, |b| {
            let motive_b = b.lam("y", nat, |b| {
                b.pi("a", nat, |b| {
                    let y = b.var("y");
                    let a = b.var("a");
                    let le_ya = b.apps(le, &[y, a]);
                    b.pi("_h", le_ya, |b| {
                        let y = b.var("y");
                        let a = b.var("a");
                        let say = b.apps(sub, &[a, y]);
                        let lhs = b.apps(plus, &[y, say]);
                        b.id(nat, lhs, a)
                    })
                })
            });
            let base_b = b.lam("a", nat, |b| {
                let a = b.var("a");
                let le_0a = b.apps(le, &[zero, a]);
                b.lam("_h", le_0a, |b| {
                    let a = b.var("a");
                    b.apps(sub_zero, &[a])
                })
            });
            let step_b = b.lam("b'", nat, |b| {
                let ih_b_ty = b.pi("a", nat, |b| {
                    let b_p = b.var("b'");
                    let a = b.var("a");
                    let le_ba = b.apps(le, &[b_p, a]);
                    b.pi("_h", le_ba, |b| {
                        let b_p = b.var("b'");
                        let a = b.var("a");
                        let sab = b.apps(sub, &[a, b_p]);
                        let lhs = b.apps(plus, &[b_p, sab]);
                        b.id(nat, lhs, a)
                    })
                });
                b.lam("ih_b", ih_b_ty, |b| {
                    b.lam("a", nat, |b| {
                        let motive_a = b.lam("x", nat, |b| {
                            let b_p = b.var("b'");
                            let sb_p = b.apps(succ, &[b_p]);
                            let x = b.var("x");
                            let le_sx = b.apps(le, &[sb_p, x]);
                            b.pi("_h", le_sx, |b| {
                                let b_p = b.var("b'");
                                let sb_p = b.apps(succ, &[b_p]);
                                let x = b.var("x");
                                let sxs = b.apps(sub, &[x, sb_p]);
                                let lhs = b.apps(plus, &[sb_p, sxs]);
                                b.id(nat, lhs, x)
                            })
                        });
                        let b_p = b.var("b'");
                        let sb_p = b.apps(succ, &[b_p]);
                        let le_sb0 = b.apps(le, &[sb_p, zero]);
                        let base_a = b.lam("h", le_sb0, |b| {
                            let b_p = b.var("b'");
                            let sb_p = b.apps(succ, &[b_p]);
                            let s0s = b.apps(sub, &[zero, sb_p]);
                            let lhs = b.apps(plus, &[sb_p, s0s]);
                            let goal = b.id(nat, lhs, zero);
                            let h = b.var("h");
                            let absurd = b.apps(no_lt_zero, &[b_p, h]);
                            b.void_elim(c, goal, absurd)
                        });
                        let step_a = b.lam("a'", nat, |b| {
                            let ih_a_ty = {
                                let b_p = b.var("b'");
                                let sb_p = b.apps(succ, &[b_p]);
                                let a_p = b.var("a'");
                                let le_sa = b.apps(le, &[sb_p, a_p]);
                                b.pi("_h", le_sa, |b| {
                                    let b_p = b.var("b'");
                                    let sb_p = b.apps(succ, &[b_p]);
                                    let a_p = b.var("a'");
                                    let sas = b.apps(sub, &[a_p, sb_p]);
                                    let lhs = b.apps(plus, &[sb_p, sas]);
                                    b.id(nat, lhs, a_p)
                                })
                            };
                            b.lam("_ih_a", ih_a_ty, |b| {
                                let b_p = b.var("b'");
                                let sb_p = b.apps(succ, &[b_p]);
                                let a_p = b.var("a'");
                                let sa_p = b.apps(succ, &[a_p]);
                                let le_ss = b.apps(le, &[sb_p, sa_p]);
                                b.lam("h", le_ss, |b| {
                                    let b_p = b.var("b'");
                                    let sb_p = b.apps(succ, &[b_p]);
                                    let a_p = b.var("a'");
                                    let sa_p = b.apps(succ, &[a_p]);
                                    let h = b.var("h");
                                    let ih_b = b.var("ih_b");
                                    let h_pred = b.apps(pred_le, &[sb_p, sa_p, h]);
                                    let sab = b.apps(sub, &[a_p, b_p]);
                                    let from = b.apps(plus, &[b_p, sab]);
                                    let eq_inner = b.apps(ih_b, &[a_p, h_pred]);
                                    b.cong(nat, nat, succ, from, a_p, eq_inner)
                                })
                            })
                        });
                        let a = b.var("a");
                        b.apps(nat_rec, &[motive_a, base_a, step_a, a])
                    })
                })
            });
            let bv = b.var("b");
            b.apps(nat_rec, &[motive_b, base_b, step_b, bv])
        });
        push_def(&mut decls, ty, body)
    };

    // --- Step D: Well-founded `div_mod_spec`, `div`, `mod`, `div_mod_identity`, `mod_lt` ---
    // Declare `DivModResult (a b : Nat) : Type 0` as a 1-constructor dependent native inductive:
    //   mk_div_mod : Π(a b : Nat) (q r : Nat). Id Nat (plus (mul q b) r) a -> lt r b -> DivModResult a b
    let dm_base = decls.len() as u32;
    let dm_ctor = dm_base + 1;
    let dm_rec = dm_base + 2;
    {
        let (dm_ty, mk_ty) = {
            let mut b = B::new(&mut arena);
            let nat = b.c0(c.nat);
            let t0 = b.type_n(0);
            let plus = b.c0(plus_decl);
            let mul = b.c0(mul_decl);
            let lt = b.c0(lt_decl);
            let dm = b.c0(dm_base);

            let dm_ty = b.pi("a", nat, |b| b.pi("b", nat, |_| t0));
            let mk_ty = b.pi("a", nat, |b| {
                b.pi("b", nat, |b| {
                    b.pi("q", nat, |b| {
                        b.pi("r", nat, |b| {
                            let a = b.var("a");
                            let bv = b.var("b");
                            let q = b.var("q");
                            let r = b.var("r");
                            let qb = b.apps(mul, &[q, bv]);
                            let lhs = b.apps(plus, &[qb, r]);
                            let eq_ty = b.id(nat, lhs, a);
                            b.pi("_eq", eq_ty, |b| {
                                let r = b.var("r");
                                let bv = b.var("b");
                                let r_lt = b.apps(lt, &[r, bv]);
                                b.pi("_r_lt", r_lt, |b| {
                                    let a = b.var("a");
                                    let bv = b.var("b");
                                    b.apps(dm, &[a, bv])
                                })
                            })
                        })
                    })
                })
            });
            (dm_ty, mk_ty)
        };
        decls.extend(declare_inductive_family(
            &mut arena,
            dm_base,
            0,
            2,
            dm_ty,
            &[mk_ty],
        ));
    }

    // Projections `dm_q`, `dm_r`, `dm_eq`, `dm_lt`:
    let dm_q_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let plus = b.c0(plus_decl);
        let mul = b.c0(mul_decl);
        let lt = b.c0(lt_decl);
        let dm = b.c0(dm_base);
        let dm_rec0 = b.c_levels(dm_rec, vec![Level::Constant(0)]);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.arrow(dm_ab, nat)
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.lam("d", dm_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let d = b.var("d");
                    let dm_ab = b.apps(dm, &[a, bv]);
                    let motive = b.lam("_", dm_ab, |_| nat);
                    let minor = b.lam("q", nat, |b| {
                        b.lam("r", nat, |b| {
                            let a = b.var("a");
                            let bv = b.var("b");
                            let q = b.var("q");
                            let r = b.var("r");
                            let qb = b.apps(mul, &[q, bv]);
                            let lhs = b.apps(plus, &[qb, r]);
                            let eq_ty = b.id(nat, lhs, a);
                            b.lam("_eq", eq_ty, |b| {
                                let r = b.var("r");
                                let bv = b.var("b");
                                let r_lt = b.apps(lt, &[r, bv]);
                                b.lam("_r_lt", r_lt, |b| b.var("q"))
                            })
                        })
                    });
                    b.apps(dm_rec0, &[a, bv, motive, minor, d])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    let dm_r_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let plus = b.c0(plus_decl);
        let mul = b.c0(mul_decl);
        let lt = b.c0(lt_decl);
        let dm = b.c0(dm_base);
        let dm_rec0 = b.c_levels(dm_rec, vec![Level::Constant(0)]);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.arrow(dm_ab, nat)
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.lam("d", dm_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let d = b.var("d");
                    let dm_ab = b.apps(dm, &[a, bv]);
                    let motive = b.lam("_", dm_ab, |_| nat);
                    let minor = b.lam("q", nat, |b| {
                        b.lam("r", nat, |b| {
                            let a = b.var("a");
                            let bv = b.var("b");
                            let q = b.var("q");
                            let r = b.var("r");
                            let qb = b.apps(mul, &[q, bv]);
                            let lhs = b.apps(plus, &[qb, r]);
                            let eq_ty = b.id(nat, lhs, a);
                            b.lam("_eq", eq_ty, |b| {
                                let r = b.var("r");
                                let bv = b.var("b");
                                let r_lt = b.apps(lt, &[r, bv]);
                                b.lam("_r_lt", r_lt, |b| b.var("r"))
                            })
                        })
                    });
                    b.apps(dm_rec0, &[a, bv, motive, minor, d])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    let dm_eq_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let plus = b.c0(plus_decl);
        let mul = b.c0(mul_decl);
        let lt = b.c0(lt_decl);
        let dm = b.c0(dm_base);
        let dm_rec0 = b.c_levels(dm_rec, vec![Level::Constant(0)]);
        let dm_q = b.c0(dm_q_decl);
        let dm_r = b.c0(dm_r_decl);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.pi("d", dm_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let d = b.var("d");
                    let q = b.apps(dm_q, &[a, bv, d]);
                    let r = b.apps(dm_r, &[a, bv, d]);
                    let qb = b.apps(mul, &[q, bv]);
                    let lhs = b.apps(plus, &[qb, r]);
                    b.id(nat, lhs, a)
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.lam("d", dm_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let d = b.var("d");
                    let dm_ab = b.apps(dm, &[a, bv]);
                    let motive = b.lam("x", dm_ab, |b| {
                        let a = b.var("a");
                        let bv = b.var("b");
                        let x = b.var("x");
                        let q = b.apps(dm_q, &[a, bv, x]);
                        let r = b.apps(dm_r, &[a, bv, x]);
                        let qb = b.apps(mul, &[q, bv]);
                        let lhs = b.apps(plus, &[qb, r]);
                        b.id(nat, lhs, a)
                    });
                    let minor = b.lam("q", nat, |b| {
                        b.lam("r", nat, |b| {
                            let a = b.var("a");
                            let bv = b.var("b");
                            let q = b.var("q");
                            let r = b.var("r");
                            let qb = b.apps(mul, &[q, bv]);
                            let lhs = b.apps(plus, &[qb, r]);
                            let eq_ty = b.id(nat, lhs, a);
                            b.lam("eq", eq_ty, |b| {
                                let r = b.var("r");
                                let bv = b.var("b");
                                let r_lt = b.apps(lt, &[r, bv]);
                                b.lam("_r_lt", r_lt, |b| b.var("eq"))
                            })
                        })
                    });
                    b.apps(dm_rec0, &[a, bv, motive, minor, d])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    let dm_lt_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let plus = b.c0(plus_decl);
        let mul = b.c0(mul_decl);
        let lt = b.c0(lt_decl);
        let dm = b.c0(dm_base);
        let dm_rec0 = b.c_levels(dm_rec, vec![Level::Constant(0)]);
        let dm_r = b.c0(dm_r_decl);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.pi("d", dm_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let d = b.var("d");
                    let r = b.apps(dm_r, &[a, bv, d]);
                    b.apps(lt, &[r, bv])
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let a = b.var("a");
                let bv = b.var("b");
                let dm_ab = b.apps(dm, &[a, bv]);
                b.lam("d", dm_ab, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let d = b.var("d");
                    let dm_ab = b.apps(dm, &[a, bv]);
                    let motive = b.lam("x", dm_ab, |b| {
                        let a = b.var("a");
                        let bv = b.var("b");
                        let x = b.var("x");
                        let r = b.apps(dm_r, &[a, bv, x]);
                        b.apps(lt, &[r, bv])
                    });
                    let minor = b.lam("q", nat, |b| {
                        b.lam("r", nat, |b| {
                            let a = b.var("a");
                            let bv = b.var("b");
                            let q = b.var("q");
                            let r = b.var("r");
                            let qb = b.apps(mul, &[q, bv]);
                            let lhs = b.apps(plus, &[qb, r]);
                            let eq_ty = b.id(nat, lhs, a);
                            b.lam("_eq", eq_ty, |b| {
                                let r = b.var("r");
                                let bv = b.var("b");
                                let r_lt = b.apps(lt, &[r, bv]);
                                b.lam("r_lt", r_lt, |b| b.var("r_lt"))
                            })
                        })
                    });
                    b.apps(dm_rec0, &[a, bv, motive, minor, d])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 21. div_mod_spec : Π(a b : Nat). lt Zero b -> DivModResult a b
    let div_mod_spec_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let succ = b.c0(c.succ);
        let le = b.c0(c.le);
        let lt = b.c0(lt_decl);
        let plus = b.c0(plus_decl);
        let mul = b.c0(mul_decl);
        let sub = b.c0(sub_decl);
        let lt_dec = b.c0(lt_dec_decl);
        let sub_lt = b.c0(sub_lt_decl);
        let plus_assoc = b.c0(plus_assoc_decl);
        let add_sub_cancel = b.c0(add_sub_cancel_decl);
        let nat_wf = b.c0(nat_wf_decl);
        let dm = b.c0(dm_base);
        let mk_dm = b.c0(dm_ctor);
        let dm_rec0 = b.c_levels(dm_rec, vec![Level::Constant(0)]);
        let acc00 = b.c_levels(c.acc, vec![Level::Constant(0), Level::Constant(0)]);
        let acc_rec000 = b.c_levels(
            c.acc_rec,
            vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        );
        let sum00 = b.c_levels(c.sum, vec![Level::Constant(0), Level::Constant(0)]);
        let sum_rec000 = b.c_levels(
            c.sum_rec,
            vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
        );

        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.pi("_hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    b.apps(dm, &[a, bv])
                })
            })
        });

        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.lam("hb", hb_ty, |b| {
                    let motive = b.lam("x", nat, |b| {
                        let x = b.var("x");
                        let acc_x = b.apps(acc00, &[nat, lt, x]);
                        b.lam("_acc", acc_x, |b| {
                            let x = b.var("x");
                            let bv = b.var("b");
                            b.apps(dm, &[x, bv])
                        })
                    });

                    let step = b.lam("x", nat, |b| {
                        let f_ty = b.pi("y", nat, |b| {
                            let y = b.var("y");
                            let x = b.var("x");
                            let lt_yx = b.apps(lt, &[y, x]);
                            b.pi("_h", lt_yx, |b| {
                                let y = b.var("y");
                                b.apps(acc00, &[nat, lt, y])
                            })
                        });
                        b.lam("_f", f_ty, |b| {
                            let ih_ty = b.pi("y", nat, |b| {
                                let y = b.var("y");
                                let x = b.var("x");
                                let lt_yx = b.apps(lt, &[y, x]);
                                b.pi("_h", lt_yx, |b| {
                                    let y = b.var("y");
                                    let bv = b.var("b");
                                    b.apps(dm, &[y, bv])
                                })
                            });
                            b.lam("ih", ih_ty, |b| {
                                let x = b.var("x");
                                let bv = b.var("b");
                                let dom_l = b.apps(lt, &[x, bv]);
                                let dom_r = b.apps(le, &[bv, x]);
                                let sum_dom = b.apps(sum00, &[dom_l, dom_r]);
                                let res_ty = b.apps(dm, &[x, bv]);
                                let sum_motive =
                                    b.lam("_", sum_dom, |b| shift(b.arena, res_ty, 0, 1));

                                // Branch 1: x < b  =>  mk_div_mod x b Zero x (refl x) h_lt
                                let minor_l = b.lam("h_lt", dom_l, |b| {
                                    let x = b.var("x");
                                    let bv = b.var("b");
                                    let h_lt = b.var("h_lt");
                                    let eq = b.refl(nat, x);
                                    b.apps(mk_dm, &[x, bv, zero, x, eq, h_lt])
                                });

                                // Branch 2: b <= x => recurse on (sub x b) via `ih`!
                                let minor_r = b.lam("h_le", dom_r, |b| {
                                    let x = b.var("x");
                                    let bv = b.var("b");
                                    let hb = b.var("hb");
                                    let h_le = b.var("h_le");
                                    let ih = b.var("ih");

                                    let sxb = b.apps(sub, &[x, bv]);
                                    let h_dec = b.apps(sub_lt, &[x, bv, hb, h_le]);
                                    let rec_res = b.apps(ih, &[sxb, h_dec]);

                                    let dm_sxb = b.apps(dm, &[sxb, bv]);
                                    let dm_motive = b.lam("_", dm_sxb, |b| {
                                        let x = b.var("x");
                                        let bv = b.var("b");
                                        b.apps(dm, &[x, bv])
                                    });
                                    let dm_minor = b.lam("q'", nat, |b| {
                                        b.lam("r'", nat, |b| {
                                            let x = b.var("x");
                                            let bv = b.var("b");
                                            let sxb = b.apps(sub, &[x, bv]);
                                            let q_p = b.var("q'");
                                            let r_p = b.var("r'");
                                            let qb = b.apps(mul, &[q_p, bv]);
                                            let qbr = b.apps(plus, &[qb, r_p]);
                                            let eq_p_ty = b.id(nat, qbr, sxb);
                                            b.lam("eq'", eq_p_ty, |b| {
                                                let r_p = b.var("r'");
                                                let bv = b.var("b");
                                                let r_lt_ty = b.apps(lt, &[r_p, bv]);
                                                b.lam("r_lt", r_lt_ty, |b| {
                                                    let x = b.var("x");
                                                    let bv = b.var("b");
                                                    let sxb = b.apps(sub, &[x, bv]);
                                                    let q_p = b.var("q'");
                                                    let r_p = b.var("r'");
                                                    let eq_p = b.var("eq'");
                                                    let r_lt = b.var("r_lt");
                                                    let h_le = b.var("h_le");

                                                    // Prove: plus (plus b (mul q' b)) r' = x
                                                    let qb = b.apps(mul, &[q_p, bv]);
                                                    let b_plus_qb = b.apps(plus, &[bv, qb]);
                                                    let t0 = b.apps(plus, &[b_plus_qb, r_p]);
                                                    let qbr = b.apps(plus, &[qb, r_p]);
                                                    let t1 = b.apps(plus, &[bv, qbr]);
                                                    let eq1 = b.apps(plus_assoc, &[bv, qb, r_p]);

                                                    let add_b = b.lam("k", nat, |b| {
                                                        let bv = b.var("b");
                                                        let k = b.var("k");
                                                        b.apps(plus, &[bv, k])
                                                    });
                                                    let t2 = b.apps(plus, &[bv, sxb]);
                                                    let eq2 =
                                                        b.cong(nat, nat, add_b, qbr, sxb, eq_p);
                                                    let eq3 =
                                                        b.apps(add_sub_cancel, &[bv, x, h_le]);

                                                    let eq12 =
                                                        b.id_trans(nat, t0, t1, t2, eq1, eq2);
                                                    let eq_all =
                                                        b.id_trans(nat, t0, t2, x, eq12, eq3);

                                                    let sq_p = b.apps(succ, &[q_p]);
                                                    b.apps(mk_dm, &[x, bv, sq_p, r_p, eq_all, r_lt])
                                                })
                                            })
                                        })
                                    });
                                    b.apps(dm_rec0, &[sxb, bv, dm_motive, dm_minor, rec_res])
                                });

                                let scrut = b.apps(lt_dec, &[x, bv]);
                                b.apps(
                                    sum_rec000,
                                    &[dom_l, dom_r, sum_motive, minor_l, minor_r, scrut],
                                )
                            })
                        })
                    });

                    let a = b.var("a");
                    let wf_a = b.apps(nat_wf, &[a]);
                    b.apps(acc_rec000, &[nat, lt, motive, step, a, wf_a])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 22. div : Π(a b : Nat). lt Zero b -> Nat
    let div_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let lt = b.c0(lt_decl);
        let spec = b.c0(div_mod_spec_decl);
        let dm_q = b.c0(dm_q_decl);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.arrow(hb_ty, nat)
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.lam("hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let hb = b.var("hb");
                    let res = b.apps(spec, &[a, bv, hb]);
                    b.apps(dm_q, &[a, bv, res])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 23. mod : Π(a b : Nat). lt Zero b -> Nat
    let mod_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let lt = b.c0(lt_decl);
        let spec = b.c0(div_mod_spec_decl);
        let dm_r = b.c0(dm_r_decl);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.arrow(hb_ty, nat)
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.lam("hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let hb = b.var("hb");
                    let res = b.apps(spec, &[a, bv, hb]);
                    b.apps(dm_r, &[a, bv, res])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 24. div_mod_identity : Π(a b : Nat) (hb : lt Zero b).
    //       Id Nat (plus (mul (div a b hb) b) (mod a b hb)) a
    let div_mod_id_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let lt = b.c0(lt_decl);
        let plus = b.c0(plus_decl);
        let mul = b.c0(mul_decl);
        let spec = b.c0(div_mod_spec_decl);
        let dm_eq = b.c0(dm_eq_decl);
        let div = b.c0(div_decl);
        let rem = b.c0(mod_decl);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.pi("hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let hb = b.var("hb");
                    let q = b.apps(div, &[a, bv, hb]);
                    let r = b.apps(rem, &[a, bv, hb]);
                    let qb = b.apps(mul, &[q, bv]);
                    let lhs = b.apps(plus, &[qb, r]);
                    b.id(nat, lhs, a)
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.lam("hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let hb = b.var("hb");
                    let res = b.apps(spec, &[a, bv, hb]);
                    b.apps(dm_eq, &[a, bv, res])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // 25. mod_lt : Π(a b : Nat) (hb : lt Zero b). lt (mod a b hb) b
    let mod_lt_decl = {
        let mut b = B::new(&mut arena);
        let nat = b.c0(c.nat);
        let zero = b.c0(c.zero);
        let lt = b.c0(lt_decl);
        let spec = b.c0(div_mod_spec_decl);
        let dm_lt = b.c0(dm_lt_decl);
        let rem = b.c0(mod_decl);
        let ty = b.pi("a", nat, |b| {
            b.pi("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.pi("hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let hb = b.var("hb");
                    let r = b.apps(rem, &[a, bv, hb]);
                    b.apps(lt, &[r, bv])
                })
            })
        });
        let body = b.lam("a", nat, |b| {
            b.lam("b", nat, |b| {
                let bv = b.var("b");
                let hb_ty = b.apps(lt, &[zero, bv]);
                b.lam("hb", hb_ty, |b| {
                    let a = b.var("a");
                    let bv = b.var("b");
                    let hb = b.var("hb");
                    let res = b.apps(spec, &[a, bv, hb]);
                    b.apps(dm_lt, &[a, bv, res])
                })
            })
        });
        push_def(&mut decls, ty, body)
    };

    // Check full signature up through div/mod/div_mod_identity/mod_lt!
    let t_full = Instant::now();
    let mut b_full = Budget::new(WF_BUDGET);
    let sig =
        check_signature_with_attribution(&mut arena, &decls, &mut b_full).unwrap_or_else(|e| {
            panic!(
                "check_signature failed at decl {}: {:?}\n  arena dump: {:?}",
                e.declaration,
                e.error,
                match e.error {
                    mathematical_kernel::CoreError::ArgumentTypeMismatch { expected, actual } =>
                        (arena.get(expected), arena.get(actual)),
                    _ => (Term::Dummy, Term::Dummy),
                }
            );
        });
    let full_ms = t_full.elapsed().as_secs_f64() * 1000.0;
    let full_steps = WF_BUDGET - b_full.remaining();

    let div_c = arena.insert(Term::Constant {
        declaration: div_decl,
        levels: Vec::new(),
    });
    let mod_c = arena.insert(Term::Constant {
        declaration: mod_decl,
        levels: Vec::new(),
    });
    let id_c = arena.insert(Term::Constant {
        declaration: div_mod_id_decl,
        levels: Vec::new(),
    });
    let mod_lt_c = arena.insert(Term::Constant {
        declaration: mod_lt_decl,
        levels: Vec::new(),
    });
    assert!(
        judgment_assumption_closure(&arena, &sig, &[div_c, mod_c, id_c, mod_lt_c]).is_empty(),
        "div, mod, div_mod_identity, and mod_lt must have 0 assumptions!"
    );
    eprintln!(
        "  Full div/mod + identity  : {:>7} steps | {:>5} terms | {:>7.2} ms | 0 assumptions | fits 65,536: {}",
        full_steps,
        arena.len(),
        full_ms,
        full_steps <= DEFAULT_CONVERSION_STEPS
    );

    // Check `div_mod_identity` standalone via `check_type` as well for exact isolated measurement:
    let ctx = Context::empty().with_signature(sig);
    {
        let id_decl = &decls[div_mod_id_decl as usize];
        let mut q_arena = arena.clone();
        let t0 = Instant::now();
        let mut b = Budget::new(WF_BUDGET);
        check_type(
            &mut q_arena,
            &ctx,
            id_decl.body.unwrap(),
            id_decl.ty,
            &mut b,
        )
        .unwrap();
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = WF_BUDGET - b.remaining();
        eprintln!(
            "  div_mod_identity check   : {:>7} steps | {:>5} terms | {:>7.2} ms | fits 65,536: {}",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // --- Step E: Closed computation: div(17, 5) == 3, mod(17, 5) == 2 ---
    let nat = arena.insert(Term::Constant {
        declaration: c.nat,
        levels: Vec::new(),
    });
    let n17 = nat_lit(&mut arena, c, 17);
    let n5 = nat_lit(&mut arena, c, 5);
    let n3 = nat_lit(&mut arena, c, 3);
    let n2 = nat_lit(&mut arena, c, 2);
    let hb5 = proof_lt_zero(&mut arena, c, 5);

    {
        let mut q_arena = arena.clone();
        let div_17_5 = q_arena.apply_arguments(div_c, &[n17, n5, hb5]);
        let t0 = Instant::now();
        let mut b = Budget::new(WF_BUDGET);
        assert!(convertible(&mut q_arena, &ctx, div_17_5, n3, nat, &mut b).unwrap());
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = WF_BUDGET - b.remaining();
        eprintln!(
            "  div(17, 5) == 3          : {:>7} steps | {:>5} terms | {:>7.2} ms | fits 65,536: {}",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    {
        let mut q_arena = arena.clone();
        let mod_17_5 = q_arena.apply_arguments(mod_c, &[n17, n5, hb5]);
        let t0 = Instant::now();
        let mut b = Budget::new(WF_BUDGET);
        assert!(convertible(&mut q_arena, &ctx, mod_17_5, n2, nat, &mut b).unwrap());
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = WF_BUDGET - b.remaining();
        eprintln!(
            "  mod(17, 5) == 2          : {:>7} steps | {:>5} terms | {:>7.2} ms | fits 65,536: {}",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // Also test a slightly larger case: div(29, 7) == 4, mod(29, 7) == 1
    {
        let mut q_arena = arena.clone();
        let n29 = nat_lit(&mut q_arena, c, 29);
        let n7 = nat_lit(&mut q_arena, c, 7);
        let n4 = nat_lit(&mut q_arena, c, 4);
        let n1 = nat_lit(&mut q_arena, c, 1);
        let hb7 = proof_lt_zero(&mut q_arena, c, 7);
        let div_29_7 = q_arena.apply_arguments(div_c, &[n29, n7, hb7]);
        let mod_29_7 = q_arena.apply_arguments(mod_c, &[n29, n7, hb7]);
        let t0 = Instant::now();
        let mut b = Budget::new(WF_BUDGET);
        assert!(convertible(&mut q_arena, &ctx, div_29_7, n4, nat, &mut b).unwrap());
        assert!(convertible(&mut q_arena, &ctx, mod_29_7, n1, nat, &mut b).unwrap());
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        let steps = WF_BUDGET - b.remaining();
        eprintln!(
            "  div(29,7)==4 & mod==1    : {:>7} steps | {:>5} terms | {:>7.2} ms | fits 65,536: {}",
            steps,
            q_arena.len(),
            ms,
            steps <= DEFAULT_CONVERSION_STEPS
        );
    }

    // --- Step F: Negative soundness tests ---
    // (1) Prove `no_acc_refl : Π(A : Type 0) (R : A -> A -> Type 0) (x : A). R x x -> Acc A R x -> Empty`
    //     and instantiate it at `Nat`, `λ x y. Id Nat x y`, `Zero`, `refl Nat Zero` to prove
    //     `Acc Nat (λ x y. Id Nat x y) Zero -> Empty`!
    {
        let mut q_arena = arena.clone();
        let (no_acc_eq_zero_ty, no_acc_eq_zero_proof) = {
            let mut b = B::new(&mut q_arena);
            let t0 = b.type_n(0);
            let nat = b.c0(c.nat);
            let zero = b.c0(c.zero);
            let empty = b.c0(c.void);
            let acc00 = b.c_levels(c.acc, vec![Level::Constant(0), Level::Constant(0)]);
            let acc_rec000 = b.c_levels(
                c.acc_rec,
                vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
            );

            // General irreflexivity of Acc:
            let no_acc_refl = b.lam("A", t0, |b| {
                let a = b.var("A");
                let r_ty = b.pi("_", a, |b| {
                    let a = b.var("A");
                    b.arrow(a, t0)
                });
                b.lam("R", r_ty, |b| {
                    let a = b.var("A");
                    b.lam("x", a, |b| {
                        let r = b.var("R");
                        let x = b.var("x");
                        let rxx = b.apps(r, &[x, x]);
                        b.lam("hxx", rxx, |b| {
                            let a = b.var("A");
                            let r = b.var("R");
                            let x = b.var("x");
                            let acc_x = b.apps(acc00, &[a, r, x]);
                            b.lam("acc", acc_x, |b| {
                                let a = b.var("A");
                                let r = b.var("R");
                                let x = b.var("x");
                                let hxx = b.var("hxx");
                                let acc = b.var("acc");
                                let motive = b.lam("y", a, |b| {
                                    let a = b.var("A");
                                    let r = b.var("R");
                                    let y = b.var("y");
                                    let acc_y = b.apps(acc00, &[a, r, y]);
                                    b.lam("_ay", acc_y, |b| {
                                        let r = b.var("R");
                                        let y = b.var("y");
                                        let ryy = b.apps(r, &[y, y]);
                                        b.arrow(ryy, empty)
                                    })
                                });
                                let step = b.lam("y", a, |b| {
                                    let a = b.var("A");
                                    let f_ty = b.pi("z", a, |b| {
                                        let r = b.var("R");
                                        let z = b.var("z");
                                        let y = b.var("y");
                                        let rzy = b.apps(r, &[z, y]);
                                        b.pi("_h", rzy, |b| {
                                            let a = b.var("A");
                                            let r = b.var("R");
                                            let z = b.var("z");
                                            b.apps(acc00, &[a, r, z])
                                        })
                                    });
                                    b.lam("_f", f_ty, |b| {
                                        let a = b.var("A");
                                        let ih_ty = b.pi("z", a, |b| {
                                            let r = b.var("R");
                                            let z = b.var("z");
                                            let y = b.var("y");
                                            let rzy = b.apps(r, &[z, y]);
                                            b.pi("_h", rzy, |b| {
                                                let r = b.var("R");
                                                let z = b.var("z");
                                                let rzz = b.apps(r, &[z, z]);
                                                b.arrow(rzz, empty)
                                            })
                                        });
                                        b.lam("ih", ih_ty, |b| {
                                            let r = b.var("R");
                                            let y = b.var("y");
                                            let ryy = b.apps(r, &[y, y]);
                                            b.lam("hyy", ryy, |b| {
                                                let ih = b.var("ih");
                                                let y = b.var("y");
                                                let hyy = b.var("hyy");
                                                b.apps(ih, &[y, hyy, hyy])
                                            })
                                        })
                                    })
                                });
                                let fn_empty = b.apps(acc_rec000, &[a, r, motive, step, x, acc]);
                                b.apps(fn_empty, &[hxx])
                            })
                        })
                    })
                })
            });

            let eq_nat = b.lam("x", nat, |b| {
                b.lam("y", nat, |b| {
                    let x = b.var("x");
                    let y = b.var("y");
                    b.id(nat, x, y)
                })
            });
            let acc_eq_zero = b.apps(acc00, &[nat, eq_nat, zero]);
            let ty = b.arrow(acc_eq_zero, empty);
            let r0 = b.refl(nat, zero);
            let proof = b.apps(no_acc_refl, &[nat, eq_nat, zero, r0]);
            (ty, proof)
        };
        let mut b = Budget::new(WF_BUDGET);
        check_type(
            &mut q_arena,
            &ctx,
            no_acc_eq_zero_proof,
            no_acc_eq_zero_ty,
            &mut b,
        )
        .unwrap();
        eprintln!(
            "  [PASS] Proved Acc Nat (λ x y. Id Nat x y) Zero -> Empty ({} steps)",
            WF_BUDGET - b.remaining()
        );
    }

    // (2) Trying to build `Acc Nat (λ x y. Id Nat x y) Zero` directly without a decrease is refused.
    {
        let mut q_arena = arena.clone();
        let (bad_ty, bad_term) = {
            let mut b = B::new(&mut q_arena);
            let nat = b.c0(c.nat);
            let zero = b.c0(c.zero);
            let acc00 = b.c_levels(c.acc, vec![Level::Constant(0), Level::Constant(0)]);
            let intro00 = b.c_levels(c.acc_intro, vec![Level::Constant(0), Level::Constant(0)]);
            let eq_nat = b.lam("x", nat, |b| {
                b.lam("y", nat, |b| {
                    let x = b.var("x");
                    let y = b.var("y");
                    b.id(nat, x, y)
                })
            });
            let ty = b.apps(acc00, &[nat, eq_nat, zero]);
            // Bogus attempt: pass `λ y h. nat_wf y` (wrong relation: `lt` instead of `eq_nat`)
            let nat_wf = b.c0(nat_wf_decl);
            let bogus_f = b.lam("y", nat, |b| {
                let y = b.var("y");
                let eq_y0 = b.id(nat, y, zero);
                b.lam("_h", eq_y0, |b| {
                    let y = b.var("y");
                    b.apps(nat_wf, &[y])
                })
            });
            let term = b.apps(intro00, &[nat, eq_nat, zero, bogus_f]);
            (ty, term)
        };
        let err = check_type(
            &mut q_arena,
            &ctx,
            bad_term,
            bad_ty,
            &mut Budget::new(WF_BUDGET),
        )
        .unwrap_err();
        eprintln!("  [PASS] Bogus Acc Nat eq Zero refused: {err:?}");
    }

    // (3) Bogus non-decreasing recursive call in `Acc.rec` (calling `ih x (le_refl x)` where
    //     `lt x x` is required) is refused by typing!
    {
        let mut q_arena = arena.clone();
        let (fn_ty, bad_rec) = {
            let mut b = B::new(&mut q_arena);
            let nat = b.c0(c.nat);
            let le_refl = b.c0(c.le_refl);
            let lt = b.c0(lt_decl);
            let nat_wf = b.c0(nat_wf_decl);
            let acc00 = b.c_levels(c.acc, vec![Level::Constant(0), Level::Constant(0)]);
            let acc_rec000 = b.c_levels(
                c.acc_rec,
                vec![Level::Constant(0), Level::Constant(0), Level::Constant(0)],
            );
            let ty = b.arrow(nat, nat);
            let body = b.lam("a", nat, |b| {
                let motive = b.lam("x", nat, |b| {
                    let x = b.var("x");
                    let acc_x = b.apps(acc00, &[nat, lt, x]);
                    b.lam("_", acc_x, |_| nat)
                });
                let bad_step = b.lam("x", nat, |b| {
                    let f_ty = b.pi("y", nat, |b| {
                        let y = b.var("y");
                        let x = b.var("x");
                        let lt_yx = b.apps(lt, &[y, x]);
                        b.pi("_h", lt_yx, |b| {
                            let y = b.var("y");
                            b.apps(acc00, &[nat, lt, y])
                        })
                    });
                    b.lam("_f", f_ty, |b| {
                        let ih_ty = b.pi("y", nat, |b| {
                            let y = b.var("y");
                            let x = b.var("x");
                            let lt_yx = b.apps(lt, &[y, x]);
                            b.arrow(lt_yx, nat)
                        });
                        b.lam("ih", ih_ty, |b| {
                            let ih = b.var("ih");
                            let x = b.var("x");
                            // Non-decreasing call: `le_refl x : Le x x`, NOT `lt x x = Le (Succ x) x`!
                            let bad_h = b.apps(le_refl, &[x]);
                            b.apps(ih, &[x, bad_h])
                        })
                    })
                });
                let a = b.var("a");
                let wf_a = b.apps(nat_wf, &[a]);
                b.apps(acc_rec000, &[nat, lt, motive, bad_step, a, wf_a])
            });
            (ty, body)
        };
        let err = check_type(
            &mut q_arena,
            &ctx,
            bad_rec,
            fn_ty,
            &mut Budget::new(WF_BUDGET),
        )
        .unwrap_err();
        eprintln!("  [PASS] Bogus non-decreasing recursive call in Acc.rec refused: {err:?}");
    }
}

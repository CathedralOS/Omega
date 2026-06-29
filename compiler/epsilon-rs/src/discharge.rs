// Static contract discharge — the verification-compiler step in miniature.
//
// The runtime contract path (parse.rs) desugars `ensures` into asserts that TRAP if a
// postcondition is violated at run time, for the concrete inputs of that one call. Static
// discharge is the other half: for a contract the compiler can prove, it GENERATES a delta
// certificate that the obligation holds for ALL inputs -- a closed `∀params. obligation` --
// and the trust anchor (check.beta) validates it at BUILD time. A true contract is accepted;
// a false one is REJECTED before the program ever runs. The proof is carried, not trusted:
// the compiler is untrusted, the checker decides.
//
// Three obligation shapes are discharged, all against a single `return e`:
//   * EQUALITY  `ensures result == E`  ->  `∀a… (= e* E*)`, proved by `refl`. Definitional
//     equalities discharge (incl. `0 + a == a`, since the checker's p/m recurse on the FIRST
//     argument and reduce); a false claim like `result == a + a` vs `return a` is rejected.
//   * STRICT ORDER `ensures result < B` / `result > B`  ->  the standard `a < b == ∃w. a+(s w)=b`,
//     proved with a constant witness when the larger side is syntactically `smaller + k` (k≥1).
//   * NON-STRICT ORDER `ensures result <= B` / `result >= B`  ->  `a <= b == ∃w. a+w=b`, whose
//     witness is the additive gap. The gap must be a non-negative LITERAL: epsilon `i32` is
//     SIGNED but the proof is over naturals, so a parameter gap (`result <= a + b`, b possibly
//     negative) is refused -- it would "prove" a contract the runtime traps on.
//     So `ensures result > a` vs `return a + 1` discharges (witness 0). The order path is
//     CONSERVATIVE: when it can't find a constant gap (`result > a` vs `return a`) it emits no
//     certificate at all (the runtime contract still stands) -- it never emits a false one.
//     The build-time REJECTION of a lie is shown by the equality path, whose shape always
//     emits: `result == a + a` vs `return a` produces a certificate the checker rejects.
//
// LEMMA CITATION: when an obligation holds only up to a banked theorem, not by reduction, the
// proof cites it with `(use N)` against a fixed library (gen-contract-lib.py, concatenated by
// contracts.sh). Two lemmas are banked: add-zero-right (`∀x. x+0=x`, id 0) -- `return X + 0`
// vs `ensures result == X` discharges as `(inst (use 0) X)`, since the checker can't reduce
// `X+0` (p recurses on the left), so refl alone is rejected; and add-commutes (`∀x∀y. x+y=y+x`,
// id 5) -- `return L + R` vs `ensures result == R + L` discharges as `(inst (inst (use 5) R) L)`,
// a multi-argument lemma instantiated at both site terms.
// This is exactly how a real verified compiler discharges an obligation: cite a proven lemma
// at the site terms. (omega-rs's entailment engine does the same with its fact/lemma base.)
//
// de Bruijn: the cert binds each parameter with `All` (proof `gen`). Under the P binders a
// parameter is `(v i)` (shift 0). The existential adds one more binder, so inside it params
// are `(v i+1)` (shift 1) and the witness var is `(v 0)`; once `wit` consumes that binder the
// proof term drops back to shift 0. Lemma-dependent orders (`a + 0 == a`, commutativity,
// non-constant gaps) are soundly rejected here and await a lemma-citing slice.

use crate::ast::{BinaryOp, Expr, Machine, Program, Statement};

// Library def ids the compiler cites (must match gen-contract-lib.py, which asserts them).
const ADD_ZERO_RIGHT: usize = 0; // ∀x. x + 0 = x
const ADD_COMMUTES: usize = 5; // ∀x∀y. x + y = y + x  (def 5: pulls its dep lemmas into 1..4)
const LE_TRANS: usize = 9; // ∀x∀y∀z. x<=y -> y<=z -> x<=z  (def 9: pulls its dep lemmas into 6..8)
const MULT_COMMUTES: usize = 20; // ∀x∀y. x * y = y * x  (def 20: pulls its dep lemmas into 10..19)
const ADD_ASSOC: usize = 21; // ∀v0∀v1∀v2. (v0+v2)+v1 = v0+(v2+v1)  (def 21; inst order is v2,v1,v0)

// Translate an epsilon expression to a raw delta-checker term, each parameter rendered as the
// de Bruijn variable `(v i+shift)`. Returns None outside the checkable arithmetic fragment
// (parameters, non-negative literals, `+`, `*`) -- the machine is then not statically
// discharged (its runtime contract still stands).
fn term(expr_index: usize, program: &Program, param_count: usize, shift: usize) -> Option<String> {
    match program.expressions[expr_index] {
        Expr::Local(i) if i < param_count => Some(format!("(v {})", i + shift)),
        Expr::Int(k) if k >= 0 => {
            let mut s = String::from("z");
            for _ in 0..k {
                s = format!("(s {})", s);
            }
            Some(s)
        }
        Expr::Binary(BinaryOp::Add, l, r) => Some(format!(
            "(p {} {})",
            term(l, program, param_count, shift)?,
            term(r, program, param_count, shift)?
        )),
        Expr::Binary(BinaryOp::Mul, l, r) => Some(format!(
            "(m {} {})",
            term(l, program, param_count, shift)?,
            term(r, program, param_count, shift)?
        )),
        _ => None,
    }
}

// Structural equality of two expression nodes (over the checkable fragment).
fn expr_eq(a: usize, b: usize, program: &Program) -> bool {
    match (program.expressions[a], program.expressions[b]) {
        (Expr::Local(x), Expr::Local(y)) => x == y,
        (Expr::Int(x), Expr::Int(y)) => x == y,
        (Expr::Binary(oa, la, ra), Expr::Binary(ob, lb, rb)) => {
            binop_eq(oa, ob) && expr_eq(la, lb, program) && expr_eq(ra, rb, program)
        }
        _ => false,
    }
}

fn binop_eq(a: BinaryOp, b: BinaryOp) -> bool {
    use BinaryOp::*;
    matches!(
        (a, b),
        (Add, Add) | (Sub, Sub) | (Mul, Mul) | (Div, Div) | (Rem, Rem)
    )
}

// If `larger` is syntactically `smaller + k` with k >= 1, return k (the strict-order gap).
fn additive_gap(smaller: usize, larger: usize, program: &Program) -> Option<i32> {
    if let Expr::Binary(BinaryOp::Add, l, r) = program.expressions[larger] {
        if let Expr::Int(k) = program.expressions[r] {
            if k >= 1 && expr_eq(l, smaller, program) {
                return Some(k);
            }
        }
    }
    None
}

// If `larger` is syntactically `smaller + G` for ANY gap expression G (constant or a
// parameter), return G's node. Used for non-strict order: a <= smaller+G holds for any G>=0,
// so the witness can be a parameter, not just a literal. (Only `smaller + G` with smaller on
// the LEFT discharges -- `G + smaller` would need commutativity, which refl can't see.)
fn additive_gap_expr(smaller: usize, larger: usize, program: &Program) -> Option<usize> {
    if let Expr::Binary(BinaryOp::Add, l, r) = program.expressions[larger] {
        if expr_eq(l, smaller, program) {
            return Some(r);
        }
    }
    None
}

fn unary(n: i32) -> String {
    let mut s = String::from("z");
    for _ in 0..n {
        s = format!("(s {})", s);
    }
    s
}

// Wrap a goal proposition and its proof in `∀` over `param_count` parameters: P `All`s around
// the goal, P `gen`s around the proof. With zero params the binders vanish (a closed ground
// obligation), which is fine.
fn wrap_universal(param_count: usize, goal: &str, proof: &str) -> String {
    let mut g = goal.to_string();
    let mut p = proof.to_string();
    for _ in 0..param_count {
        g = format!("(All {})", g);
        p = format!("(gen {})", p);
    }
    format!("{} {}", g, p)
}

// For a machine whose contract is statically dischargeable -- exactly one postcondition and one
// `return e`, in a supported shape -- return the obligation certificate. Otherwise None.
// A machine may declare SEVERAL `ensures`; discharge each one independently and emit a separate
// certificate per dischargeable postcondition. (The runtime path already desugars each to its own
// assert; this gives the static half the same multiplicity.)
pub fn discharge_machine(machine: &Machine, program: &Program) -> Vec<String> {
    if machine.return_exprs.len() != 1 {
        return Vec::new();
    }
    (0..machine.postconditions.len())
        .filter_map(|pc| discharge_postcondition(machine, program, pc))
        .collect()
}

fn discharge_postcondition(machine: &Machine, program: &Program, pc: usize) -> Option<String> {
    let result_local = machine.result_local?;
    let returned = machine.return_exprs[0];
    let (op, lhs, rhs) = match program.expressions[machine.postconditions[pc]] {
        Expr::Binary(op, l, r) => (op, l, r),
        _ => return None,
    };
    // the postcondition's left side must be exactly `result`
    match program.expressions[lhs] {
        Expr::Local(i) if i == result_local => {}
        _ => return None,
    }
    let p = machine.param_count;

    match op {
        // result == E
        BinaryOp::EqEq => {
            // LEMMA CITATION — obligations the checker can't reduce, carried by a banked theorem:
            if let Expr::Binary(BinaryOp::Add, l, r) = program.expressions[returned] {
                // COMMUTATIVITY: `return L + R` vs `ensures result == R + L` -> cite add-commutes.
                // `(inst (inst (use 5) R) L)` proves `(p L R) = (p R L)`.
                if let Expr::Binary(BinaryOp::Add, el, er) = program.expressions[rhs] {
                    if expr_eq(el, r, program) && expr_eq(er, l, program) {
                        let lhs_t = term(returned, program, p, 0)?; // (p L R)
                        let rhs_t = term(rhs, program, p, 0)?; // (p R L)
                        let rt = term(r, program, p, 0)?;
                        let lt = term(l, program, p, 0)?;
                        let goal = format!("(= {} {})", lhs_t, rhs_t);
                        let proof = format!("(inst (inst (use {}) {}) {})", ADD_COMMUTES, rt, lt);
                        return Some(wrap_universal(p, &goal, &proof));
                    }
                }
                // ADD-ZERO: `return X + 0` vs `ensures result == X` is x+0=x, which the checker
                // does NOT reduce (p recurses on the left, so `(p X z)` is stuck). Cite add-zero-right.
                if matches!(program.expressions[r], Expr::Int(0)) && expr_eq(l, rhs, program) {
                    let lhs_t = term(returned, program, p, 0)?; // (p X z)
                    let x = term(rhs, program, p, 0)?; // X
                    let goal = format!("(= {} {})", lhs_t, x);
                    let proof = format!("(inst (use {}) {})", ADD_ZERO_RIGHT, x);
                    return Some(wrap_universal(p, &goal, &proof));
                }
            }
            // ADDITIVE ASSOCIATIVITY: `return (L + M) + R` vs `ensures result == L + (M + R)` -> cite
            // add-assoc. The checker can't reduce `(p (p L M) R)` (p recurses left, the inner sum is
            // stuck). The banked "+ associative" is `(v0+v2)+v1 = v0+(v2+v1)`, matched at v0=L, v2=M,
            // v1=R, so the instantiation order is M, R, L.
            if let Expr::Binary(BinaryOp::Add, lm, r) = program.expressions[returned] {
                if let Expr::Binary(BinaryOp::Add, l, m) = program.expressions[lm] {
                    if let Expr::Binary(BinaryOp::Add, l2, mr) = program.expressions[rhs] {
                        if let Expr::Binary(BinaryOp::Add, m2, r2) = program.expressions[mr] {
                            if expr_eq(l, l2, program)
                                && expr_eq(m, m2, program)
                                && expr_eq(r, r2, program)
                            {
                                let lhs_t = term(returned, program, p, 0)?; // (p (p L M) R)
                                let rhs_t = term(rhs, program, p, 0)?; // (p L (p M R))
                                let (lt, mt, rt) = (
                                    term(l, program, p, 0)?,
                                    term(m, program, p, 0)?,
                                    term(r, program, p, 0)?,
                                );
                                let goal = format!("(= {} {})", lhs_t, rhs_t);
                                let proof = format!(
                                    "(inst (inst (inst (use {}) {}) {}) {})",
                                    ADD_ASSOC, mt, rt, lt
                                );
                                return Some(wrap_universal(p, &goal, &proof));
                            }
                        }
                    }
                }
            }
            // MULTIPLICATIVE COMMUTATIVITY: `return L * R` vs `ensures result == R * L` -> cite
            // mult-commutes. `(inst (inst (use 20) R) L)` proves `(m L R) = (m R L)` (the * mirror of
            // the add-commutes branch; the checker can't reduce a*b=b*a since m recurses on the left).
            if let Expr::Binary(BinaryOp::Mul, l, r) = program.expressions[returned] {
                if let Expr::Binary(BinaryOp::Mul, el, er) = program.expressions[rhs] {
                    if expr_eq(el, r, program) && expr_eq(er, l, program) {
                        let lhs_t = term(returned, program, p, 0)?; // (m L R)
                        let rhs_t = term(rhs, program, p, 0)?; // (m R L)
                        let rt = term(r, program, p, 0)?;
                        let lt = term(l, program, p, 0)?;
                        let goal = format!("(= {} {})", lhs_t, rhs_t);
                        let proof = format!("(inst (inst (use {}) {}) {})", MULT_COMMUTES, rt, lt);
                        return Some(wrap_universal(p, &goal, &proof));
                    }
                }
            }
            // otherwise a definitional equality -> refl
            let e = term(returned, program, p, 0)?;
            let big = term(rhs, program, p, 0)?;
            let goal = format!("(= {} {})", e, big);
            let proof = format!("(refl {})", e);
            Some(wrap_universal(p, &goal, &proof))
        }
        // result < B (= e < B): discharge when B is `e + k`; smaller=e, larger=B
        // result > B (= B < e): discharge when e is `B + k`; smaller=B, larger=e
        BinaryOp::Lt | BinaryOp::Gt => {
            let (smaller_expr, larger_expr) = match op {
                BinaryOp::Lt => (returned, rhs),
                _ => (rhs, returned),
            };
            let k = additive_gap(smaller_expr, larger_expr, program)?;
            // a < b  ==  ∃w. a + (s w) = b ; witness k-1 makes a+(s(k-1)) = a+k = b
            let smaller1 = term(smaller_expr, program, p, 1)?;
            let larger1 = term(larger_expr, program, p, 1)?;
            let larger0 = term(larger_expr, program, p, 0)?;
            let body = format!("(= (p {} (s (v 0))) {})", smaller1, larger1);
            let goal = format!("(Exists {})", body);
            let proof = format!("(wit {} {} (refl {}))", body, unary(k - 1), larger0);
            Some(wrap_universal(p, &goal, &proof))
        }
        // result <= B (= e <= B): discharge when B is `e + G`; smaller=e, larger=B
        // result >= B (= B <= e): discharge when e is `B + G`; smaller=B, larger=e
        BinaryOp::Le | BinaryOp::Ge => {
            let (smaller_expr, larger_expr) = match op {
                BinaryOp::Le => (returned, rhs),
                _ => (rhs, returned),
            };
            // REFLEXIVE bound: `result <= X` (or `result >= X`) returning X. The gap is 0, but `(p X z)`
            // does NOT reduce (p recurses on the left, X is stuck), so refl can't see `(p X z) = X` --
            // cite add-zero-right, exactly like the `result == X` returned-as-`X + 0` equality case.
            if expr_eq(smaller_expr, larger_expr, program) {
                let s1 = term(smaller_expr, program, p, 1)?;
                let s0 = term(smaller_expr, program, p, 0)?;
                let body = format!("(= (p {} (v 0)) {})", s1, s1);
                let goal = format!("(Exists {})", body);
                let proof = format!("(wit {} z (inst (use {}) {}))", body, ADD_ZERO_RIGHT, s0);
                return Some(wrap_universal(p, &goal, &proof));
            }
            let gap = additive_gap_expr(smaller_expr, larger_expr, program)?;
            // a <= b  ==  ∃w. a + w = b, witness the gap. SOUNDNESS: epsilon `i32` is SIGNED, but
            // the proof is over delta NATURALS -- so the witness must be PROVABLY non-negative,
            // else a negative argument makes the runtime assert trap while this "proves" it.
            // A literal gap is >= 0 (unary minus desugars to Sub, so `Int` nodes are non-negative);
            // a parameter or expression gap could be negative, so refuse it (conservative).
            if !matches!(program.expressions[gap], Expr::Int(k) if k >= 0) {
                return None;
            }
            let smaller1 = term(smaller_expr, program, p, 1)?;
            let larger1 = term(larger_expr, program, p, 1)?;
            let larger0 = term(larger_expr, program, p, 0)?;
            let gap0 = term(gap, program, p, 0)?;
            let body = format!("(= (p {} (v 0)) {})", smaller1, larger1);
            let goal = format!("(Exists {})", body);
            let proof = format!("(wit {} {} (refl {}))", body, gap0, larger0);
            Some(wrap_universal(p, &goal, &proof))
        }
        _ => None,
    }
}

// ---- Call-site contract composition (omega's BoundedCallArgumentObligation) --------------
//
// When machine A calls B(args) and B has a `requires`, A owes a proof that B's precondition
// holds for the actual args. The modular case is PRECONDITION FORWARDING: a wrapper A that
// `requires P(a)` and calls B(a) where B also `requires P(a)`. The obligation is then
// `∀a. (A.requires -> B.requires[a])`, and since the two are identical it is discharged by
// `(lam (hyp 0))` -- assume A's precondition, hand it back as B's. The trust anchor accepts it
// only if the two propositions truly match, so a mismatched forward is (soundly) not emitted.
// This first slice scopes to a single-parameter wrapper forwarding its parameter to a callee
// with the same single order-precondition; richer entailment is the follow-on.

// Render `param OP const` (param at de Bruijn `param_db`) as a delta proposition. Order
// relations only (the existential `<=`/`<`/`>=`/`>` shapes); None otherwise.
fn order_prop(op: BinaryOp, c: i32, param_db: &str) -> Option<String> {
    let cc = unary(c);
    Some(match op {
        BinaryOp::Ge => format!("(Exists (= (p {} (v 0)) {}))", cc, param_db), // c <= param
        BinaryOp::Gt => format!("(Exists (= (p {} (s (v 0))) {}))", cc, param_db), // c < param
        BinaryOp::Le => format!("(Exists (= (p {} (v 0)) {}))", param_db, cc), // param <= c
        BinaryOp::Lt => format!("(Exists (= (p {} (s (v 0))) {}))", param_db, cc), // param < c
        _ => return None,
    })
}

// Collect (callee index, arg nodes) for every direct call statement (`B(args)` as an Eval or a
// Let initializer) in a machine, recursing into Blocks.
fn collect_calls(statements: &[Statement], program: &Program, out: &mut Vec<(usize, Vec<usize>)>) {
    for statement in statements {
        let call_node = match statement {
            Statement::Eval(e) | Statement::Let(_, e) | Statement::Return(e) => Some(*e),
            Statement::Block(inner) => {
                collect_calls(inner, program, out);
                None
            }
            _ => None,
        };
        if let Some(node) = call_node {
            if let Expr::Call(callee, start, count) = program.expressions[node] {
                out.push((callee, program.call_args[start..start + count].to_vec()));
            }
        }
    }
}

// As an order-precondition on a parameter, return `(param_index, op, const)` if the condition
// is `Local(i) OP Int(c)` with OP an order relation.
fn param_order(cond: usize, program: &Program) -> Option<(usize, BinaryOp, i32)> {
    if let Expr::Binary(op, l, r) = program.expressions[cond] {
        if let Expr::Local(i) = program.expressions[l] {
            if let Expr::Int(c) = program.expressions[r] {
                if matches!(op, BinaryOp::Ge | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Lt) {
                    return Some((i, op, c));
                }
            }
        }
    }
    None
}

// Forwarding certificates for one caller machine: for each order-precondition on a parameter
// the caller forwards verbatim to a callee demanding the SAME precondition, emit the proof that
// the callee's precondition follows from the caller's. Handles any arity -- the caller's
// parameters are universally bound, the forwarded one is named inside the existential body.
fn discharge_forwarding(caller_idx: usize, program: &Program) -> Vec<String> {
    let caller = &program.machines[caller_idx];
    let mut certs = Vec::new();
    let mut calls = Vec::new();
    collect_calls(&caller.entry, program, &mut calls);
    for block in &caller.states {
        collect_calls(block, program, &mut calls);
    }
    for &cpc in &caller.preconditions {
        let (ap, op, c) = match param_order(cpc, program) {
            Some(t) => t,
            None => continue,
        };
        for (callee_idx, args) in &calls {
            let callee = &program.machines[*callee_idx];
            // find an argument position pp at which the call forwards caller-parameter `ap`...
            for (pp, &arg) in args.iter().enumerate() {
                if !matches!(program.expressions[arg], Expr::Local(i) if i == ap) {
                    continue;
                }
                // ...and at which the callee demands an order-precondition of the same relation.
                let callee_pc = callee
                    .preconditions
                    .iter()
                    .filter_map(|&bpc| param_order(bpc, program))
                    .find(|(bpi, bop, _)| *bpi == pp && same_order(op, *bop));
                let bc = match callee_pc {
                    Some((_, _, bc)) => bc,
                    None => continue,
                };
                // Under the P caller binders the forwarded parameter `ap` is (v ap); inside an
                // existential body it is (v ap+1) (the witness var is (v 0)). `lam` does not
                // shift term vars, so in the proof body `ap` is still (v ap).
                let pidx = format!("(v {})", ap + 1);
                if c == bc {
                    // FORWARDING: caller and callee demand the same bound. ∀params. (P -> P),
                    // proved by `(lam (hyp 0))` -- assume the precondition, hand it back.
                    if let Some(p) = order_prop(op, c, &pidx) {
                        certs.push(wrap_universal(
                            caller.param_count,
                            &format!("(-> {0} {0})", p),
                            &format!("(lam {0} (hyp 0))", p),
                        ));
                    }
                } else if matches!(op, BinaryOp::Ge) && c > bc {
                    // WEAKENING (lower bound): caller demands `param >= c`, callee only `>= bc`
                    // with c > bc. From `c <= param` and the ground `bc <= c`, le-trans gives
                    // `bc <= param`. Proof: assume P_strong, apply le-trans(bc, c, param) to the
                    // ground premise `bc <= c` and the hypothesis.
                    if let (Some(ps), Some(pw)) =
                        (order_prop(op, c, &pidx), order_prop(op, bc, &pidx))
                    {
                        let (ca, cb, diff) = (unary(c), unary(bc), unary(c - bc));
                        let prem1 = format!(
                            "(wit (= (p {0} (v 0)) {1}) {2} (refl {1}))",
                            cb, ca, diff
                        );
                        let body = format!(
                            "(app (app (inst (inst (inst (use {}) {}) {}) (v {})) {}) (hyp 0))",
                            LE_TRANS, cb, ca, ap, prem1
                        );
                        certs.push(wrap_universal(
                            caller.param_count,
                            &format!("(-> {} {})", ps, pw),
                            &format!("(lam {} {})", ps, body),
                        ));
                    }
                } else if matches!(op, BinaryOp::Le) && c < bc {
                    // WEAKENING (upper bound): caller demands `param <= c`, callee only `<= bc`
                    // with c < bc. From the hypothesis `param <= c` and the ground `c <= bc`,
                    // le-trans(param, c, bc) gives `param <= bc`. (Mirror of the lower-bound case:
                    // the hypothesis is now the FIRST premise, the ground fact the second.)
                    if let (Some(ps), Some(pw)) =
                        (order_prop(op, c, &pidx), order_prop(op, bc, &pidx))
                    {
                        let (ca, cb, diff) = (unary(c), unary(bc), unary(bc - c));
                        let prem2 = format!(
                            "(wit (= (p {0} (v 0)) {1}) {2} (refl {1}))",
                            ca, cb, diff
                        );
                        let body = format!(
                            "(app (app (inst (inst (inst (use {}) (v {})) {}) {}) (hyp 0)) {})",
                            LE_TRANS, ap, ca, cb, prem2
                        );
                        certs.push(wrap_universal(
                            caller.param_count,
                            &format!("(-> {} {})", ps, pw),
                            &format!("(lam {} {})", ps, body),
                        ));
                    }
                }
            }
        }
    }
    certs
}

fn same_order(a: BinaryOp, b: BinaryOp) -> bool {
    use BinaryOp::*;
    matches!((a, b), (Ge, Ge) | (Gt, Gt) | (Le, Le) | (Lt, Lt))
}

// ---- Implicit ARRAY-BOUNDS obligations (compiler-emitted from context) --------------------
//
// Every `self.arr[i]` read or write owes a proof that `i < count`, where `count` is the array's
// declared length -- carried right in the access node, so the obligation needs no length lookup.
// Unlike `ensures`/`requires` this obligation is IMPLICIT (the programmer never wrote it); the
// compiler finds the accesses and discharges what it can: a literal index in range by a ground
// witness, a parameter index by forwarding the machine's matching `requires i < count`.

// Walk an expression subtree, pushing (index node, count) for each `self.arr[index]` READ.
fn collect_indices(node: usize, program: &Program, out: &mut Vec<(usize, i32)>) {
    match program.expressions[node] {
        Expr::SelfIndex(_, count, _, idx) => {
            out.push((idx, count));
            collect_indices(idx, program, out);
        }
        Expr::Binary(_, l, r) => {
            collect_indices(l, program, out);
            collect_indices(r, program, out);
        }
        Expr::Call(_, start, cnt) | Expr::SelfCall(_, start, cnt) => {
            for k in 0..cnt {
                collect_indices(program.call_args[start + k], program, out);
            }
        }
        _ => {}
    }
}

// Walk a machine's statements, collecting every array-access bounds obligation (index, count) --
// reads nested in any expression, writes (`StoreSelfIndex`), recursing into blocks/transitions.
fn collect_array_obligations(statements: &[Statement], program: &Program, out: &mut Vec<(usize, i32)>) {
    for s in statements {
        match s {
            Statement::Let(_, e)
            | Statement::Assign(_, e)
            | Statement::StoreSelfField(_, e)
            | Statement::Eval(e)
            | Statement::Return(e)
            | Statement::Exit(e)
            | Statement::WriteByte(e)
            | Statement::Assert(e) => collect_indices(*e, program, out),
            Statement::StoreSelfIndex(_, count, _, idx, val) => {
                out.push((*idx, *count));
                collect_indices(*idx, program, out);
                collect_indices(*val, program, out);
            }
            Statement::Transition(subj, arms) => {
                collect_indices(*subj, program, out);
                for arm in arms {
                    for &a in &arm.args {
                        collect_indices(a, program, out);
                    }
                }
            }
            Statement::Block(inner) => collect_array_obligations(inner, program, out),
            _ => {}
        }
    }
}

fn discharge_array_bounds(machine_idx: usize, program: &Program) -> Vec<String> {
    let machine = &program.machines[machine_idx];
    let mut obligations = Vec::new();
    collect_array_obligations(&machine.entry, program, &mut obligations);
    for block in &machine.states {
        collect_array_obligations(block, program, &mut obligations);
    }
    let p = machine.param_count;
    let mut certs = Vec::new();
    for (idx_node, count) in obligations {
        match program.expressions[idx_node] {
            // LITERAL index k in [0, count): a closed proof of `k < count` (witness count-k-1).
            // Conservative: an out-of-range literal (k >= count) emits NO cert -- never a false one.
            Expr::Int(k) if k >= 0 && k < count => {
                let body = format!("(= (p {} (s (v 0))) {})", unary(k), unary(count));
                certs.push(format!(
                    "(Exists {0}) (wit {0} {1} (refl {2}))",
                    body,
                    unary(count - k - 1),
                    unary(count)
                ));
            }
            // PARAMETER index i with a declared `requires i < count`: the bounds obligation IS the
            // precondition, so it is discharged by forwarding it (assume i < count, hand it back) --
            // the modular memory-safety statement: the access is in bounds under the contract.
            Expr::Local(i) => {
                let has_req = machine.preconditions.iter().any(|&pc| {
                    matches!(param_order(pc, program), Some((pi, BinaryOp::Lt, c)) if pi == i && c == count)
                });
                if has_req {
                    if let Some(prop) = order_prop(BinaryOp::Lt, count, &format!("(v {})", i + 1)) {
                        certs.push(wrap_universal(
                            p,
                            &format!("(-> {0} {0})", prop),
                            &format!("(lam {0} (hyp 0))", prop),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    certs
}

// One certificate per statically-dischargeable machine (postconditions), plus one per
// dischargeable call site (forwarded preconditions) and per dischargeable array access.
pub fn emit_contracts(program: &Program) -> Vec<String> {
    let mut certs: Vec<String> = program
        .machines
        .iter()
        .flat_map(|machine| discharge_machine(machine, program))
        .collect();
    for caller_idx in 0..program.machines.len() {
        certs.extend(discharge_forwarding(caller_idx, program));
        certs.extend(discharge_array_bounds(caller_idx, program));
    }
    certs
}

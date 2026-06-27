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
//     witness is the additive gap itself and so may be a PARAMETER, not just a literal -- e.g.
//     `result <= a + b` discharges with witness `b`.
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

use crate::ast::{BinaryOp, Expr, Machine, Program};

// Library def ids the compiler cites (must match gen-contract-lib.py, which asserts them).
const ADD_ZERO_RIGHT: usize = 0; // ∀x. x + 0 = x
const ADD_COMMUTES: usize = 5; // ∀x∀y. x + y = y + x  (def 5: pulls its dep lemmas into 1..4)

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
pub fn discharge_machine(machine: &Machine, program: &Program) -> Option<String> {
    let result_local = machine.result_local?;
    if machine.postconditions.len() != 1 || machine.return_exprs.len() != 1 {
        return None;
    }
    let returned = machine.return_exprs[0];
    let (op, lhs, rhs) = match program.expressions[machine.postconditions[0]] {
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
            let gap = additive_gap_expr(smaller_expr, larger_expr, program)?;
            // a <= b  ==  ∃w. a + w = b ; the witness is the gap itself (valid for any gap >= 0,
            // so it may be a parameter -- e.g. result <= a + b proved with witness b)
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

// One certificate per statically-dischargeable machine, in declaration order.
pub fn emit_contracts(program: &Program) -> Vec<String> {
    program
        .machines
        .iter()
        .filter_map(|machine| discharge_machine(machine, program))
        .collect()
}

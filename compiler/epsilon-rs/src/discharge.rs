// Static contract discharge — the verification-compiler step in miniature.
//
// The runtime contract path (parse.rs) desugars `ensures` into asserts that TRAP if a
// postcondition is violated at run time, for the concrete inputs of that one call. Static
// discharge is the other half: for a contract the compiler can prove, it GENERATES a delta
// certificate that the obligation holds SYMBOLICALLY -- for all inputs -- and the trust
// anchor (check.beta) validates it at BUILD time. A true contract is accepted; a false one
// is REJECTED before the program ever runs. The proof is carried, not trusted: the compiler
// is untrusted, the checker decides.
//
// This first slice discharges the equational shape `ensures result == E` against a single
// `return e`: the obligation is `e == E`, proved by `refl`. That is not as weak as it looks
// -- the checker's conversion reduces the Peano primitives (which recurse on their FIRST
// argument), so `return 0 + a` against `ensures result == a` discharges (because `(p z (v0))`
// reduces to `(v0)`), while a false claim like `ensures result == a + a` against `return a`
// is REJECTED at build time. Equalities that hold only up to a lemma -- `a + 0 == a`,
// commutativity -- are (soundly) rejected by refl alone and await a lemma-emitting slice. The
// mechanism that lands here is the point: contract -> symbolic obligation -> generated
// certificate -> checked by the trust anchor, with the compiler proving, not trusted.

use crate::ast::{BinaryOp, Expr, Machine, Program};

// Translate an epsilon expression to a raw delta-checker term, with each parameter rendered
// as the de Bruijn variable `(v i)`. Returns None for anything outside the checkable
// arithmetic fragment (parameters, non-negative literals, `+`, `*`) -- the machine is then
// simply not statically discharged (its runtime contract still stands).
fn term(expr_index: usize, program: &Program, param_count: usize) -> Option<String> {
    match program.expressions[expr_index] {
        Expr::Local(i) if i < param_count => Some(format!("(v {})", i)),
        Expr::Int(k) if k >= 0 => {
            let mut s = String::from("z");
            for _ in 0..k {
                s = format!("(s {})", s);
            }
            Some(s)
        }
        Expr::Binary(BinaryOp::Add, l, r) => Some(format!(
            "(p {} {})",
            term(l, program, param_count)?,
            term(r, program, param_count)?
        )),
        Expr::Binary(BinaryOp::Mul, l, r) => Some(format!(
            "(m {} {})",
            term(l, program, param_count)?,
            term(r, program, param_count)?
        )),
        _ => None,
    }
}

// For a machine whose contract is the dischargeable shape -- exactly one `ensures result == E`
// and exactly one `return e`, with E and e in the checkable fragment -- return the obligation
// certificate `(= e* E*) (refl e*)`. Otherwise None.
pub fn discharge_machine(machine: &Machine, program: &Program) -> Option<String> {
    let result_local = machine.result_local?;
    if machine.postconditions.len() != 1 || machine.return_exprs.len() != 1 {
        return None;
    }
    // the postcondition must be `result == E`
    let (lhs, rhs) = match program.expressions[machine.postconditions[0]] {
        Expr::Binary(BinaryOp::EqEq, l, r) => (l, r),
        _ => return None,
    };
    match program.expressions[lhs] {
        Expr::Local(i) if i == result_local => {}
        _ => return None,
    }
    let returned = term(machine.return_exprs[0], program, machine.param_count)?;
    let claimed = term(rhs, program, machine.param_count)?;
    Some(format!("(= {} {}) (refl {})", returned, claimed, returned))
}

// One certificate per statically-dischargeable machine, in declaration order.
pub fn emit_contracts(program: &Program) -> Vec<String> {
    program
        .machines
        .iter()
        .filter_map(|machine| discharge_machine(machine, program))
        .collect()
}

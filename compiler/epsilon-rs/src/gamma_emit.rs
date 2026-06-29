// EPSILON MEANING VIA GAMMA — the first thread of getting epsilon out of Rust.
//
// rungs/epsilon.md says epsilon's meaning is "Written in: Delta / Gamma" -- defined by the
// reference interpreter, not the native (Rust on-ramp) backend. This module is the seed of that:
// it translates the supported subset of an epsilon program into a GAMMA expression that the
// Rust-free reference interpreter (`compiler/gamma/interp.beta`, compiled by bc, run on the seed)
// evaluates. The epsilon-meaning DIAMOND then checks the two routes agree -- native execution
// (the on-ramp backend) vs gamma interpretation (the lattice's own semantics) -- so epsilon's
// meaning is pinned by a program the lower rungs already understand, the same move that put gamma
// (interp.beta) and the checker into the lineage.
//
// FIRST SLICE: a straight-line integer `main` -- a sequence of `let` bindings over +,-,* and a
// final `exit(e)`. The exit value becomes the gamma result (interp's exit code is its low byte,
// matching the native process exit code). Anything outside the subset returns None (the diamond
// skips it). States, mutation, calls, self-fields, and the other operators are follow-on slices.
use crate::ast::{BinaryOp, Expr, Program, Statement};

// Render an expression node as a gamma s-expression, or None if outside the integer subset.
// interp.beta's primitives are exactly `+ - * / %` and the comparisons `eq`/`lt` (it has no
// gt/le/ge/ne, nor bitwise/shift) -- so the other comparisons are encoded FAITHFULLY from lt/eq/+/-
// over integers (no overflow-prone `+1` tricks): a>b == b<a; a<=b == (a<b)|(a==b) == (a<b)+(a==b)
// (mutually exclusive, so the sum is 0/1); a>=b == (b<a)+(a==b); a!=b == 1-(a==b). The operands are
// pure here (locals/arithmetic, no ReadByte), so duplicating a/b in those encodings is sound.
fn gexpr(node: usize, program: &Program) -> Option<String> {
    match program.expressions[node] {
        Expr::Int(k) if k >= 0 => Some(k.to_string()),
        Expr::Local(i) => Some(format!("l{}", i)), // lowercase: gamma reserves uppercase for constructors
        Expr::Binary(o, l, r) => {
            let a = gexpr(l, program)?;
            let b = gexpr(r, program)?;
            Some(match o {
                BinaryOp::Add => format!("(+ {} {})", a, b),
                BinaryOp::Sub => format!("(- {} {})", a, b),
                BinaryOp::Mul => format!("(* {} {})", a, b),
                BinaryOp::Div => format!("(/ {} {})", a, b),
                BinaryOp::Rem => format!("(% {} {})", a, b),
                BinaryOp::Lt => format!("(lt {} {})", a, b),
                BinaryOp::EqEq => format!("(eq {} {})", a, b),
                BinaryOp::Gt => format!("(lt {} {})", b, a),
                BinaryOp::Le => format!("(+ (lt {a} {b}) (eq {a} {b}))", a = a, b = b),
                BinaryOp::Ge => format!("(+ (lt {b} {a}) (eq {a} {b}))", a = a, b = b),
                BinaryOp::Ne => format!("(- 1 (eq {} {}))", a, b),
                _ => return None, // bitwise/shift have no interp.beta primitive (a later slice)
            })
        }
        _ => None,
    }
}

// Translate a straight-line integer entry machine into a gamma expression, or None if it uses
// anything outside the first slice (states, non-`Let`/`Exit` statements, unsupported exprs).
pub fn emit_gamma(program: &Program) -> Option<String> {
    let entry = &program.machines[program.entry_machine];
    if !entry.states.is_empty() {
        return None; // states => not straight-line (a later slice models them as gamma functions)
    }
    let mut lets: Vec<(usize, String)> = Vec::new();
    let mut body: Option<String> = None;
    for statement in &entry.entry {
        match statement {
            Statement::Let(i, init, _domain) => {
                if body.is_some() {
                    return None; // a binding after the exit -> not the straight-line shape
                }
                lets.push((*i, gexpr(*init, program)?));
            }
            Statement::Exit(e) => {
                if body.is_some() {
                    return None; // two exits
                }
                body = Some(gexpr(*e, program)?);
            }
            _ => return None, // assign/store/call/transition/... are out of the first slice
        }
    }
    let mut out = body?; // require a terminal exit
    for (i, value) in lets.iter().rev() {
        out = format!("(let l{} {} {})", i, value, out);
    }
    Some(out)
}

// EPSILON MEANING VIA GAMMA — the thread of getting epsilon out of Rust.
//
// rungs/epsilon.md: epsilon's meaning is "Written in Delta/Gamma" -- defined by the reference
// interpreter, not the native (Rust on-ramp) backend. This module translates the supported subset
// of an epsilon program into a GAMMA expression the Rust-free reference interpreter
// (`compiler/gamma/interp.beta`, compiled by bc, run on the seed) evaluates. The epsilon-meaning
// DIAMOND (epsilon-meaning-diamond.sh) then checks the two routes agree -- native execution vs
// gamma interpretation -- so epsilon's meaning is pinned by a program the lower rungs understand.
//
// MODEL: a machine is a set of mutually-recursive gamma functions over its full LOCALS frame.
// `me` is the entry; each state `s_k` takes the same locals signature (l0..l_{n-1}); a transition is
// a guarded TAIL-CALL to another state; MUTATION (`let`/`assign`) is threaded SSA-style (each write
// rebinds via a fresh `let`, and a per-local name map tracks the current binding). The machine starts
// by calling `me` with all locals 0. The straight-line case (no states) falls out: `me` is just the
// entry's `let`s and final exit.
//
// SUPPORTED (returns None -> the diamond skips otherwise): the entry machine only; i32 LOCALS (no self
// data fields yet); `let`/`assign` over the integer subset (`+ - * / %` and all six comparisons,
// faithfully encoded from interp's eq/lt); PARAMETERLESS states (transition arms carry no args);
// integer-pattern / `_` transitions; `exit`/`return`. Self fields, state parameters, calls, bitwise/
// shift, and `read_byte` are follow-on slices.
use crate::ast::{BinaryOp, Expr, Pattern, Program, Statement};

// Render an expression node as a gamma s-expression under the current local-name map, or None if
// outside the integer subset. interp.beta's primitives are exactly `+ - * / %` and the comparisons
// `eq`/`lt`; the other comparisons are encoded FAITHFULLY over integers (no overflow-prone `+1`):
// a>b == b<a; a<=b == (a<b)|(a==b) == (a<b)+(a==b) (mutually exclusive, sum is 0/1); a>=b ==
// (b<a)+(a==b); a!=b == 1-(a==b). Operands here are pure (locals/arithmetic), so duplication is sound.
fn gexpr(node: usize, program: &Program, names: &[String]) -> Option<String> {
    match program.expressions[node] {
        Expr::Int(k) if k >= 0 => Some(k.to_string()),
        Expr::Local(i) => names.get(i).cloned(), // current SSA binding; out of range -> None
        Expr::Binary(o, l, r) => {
            let a = gexpr(l, program, names)?;
            let b = gexpr(r, program, names)?;
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
        _ => None, // SelfField/SelfIndex/ReadByte/Call -> later slices
    }
}

// A call into state `target`, forwarding the full current locals vector (parameterless states only).
fn call_state(target: usize, names: &[String]) -> String {
    if names.is_empty() {
        format!("(s{})", target)
    } else {
        format!("(s{} {})", target, names.join(" "))
    }
}

// A transition: guarded tail-calls. Arms 0..n-1 become `(if (eq subj pat) call rest)`; the LAST arm
// is the final else (a `_` default, or the exhaustive false-branch of a bool transition). Non-last
// arms must be integer patterns with no args (parameterless states); otherwise None.
fn translate_transition(
    subject: usize,
    arms: &[crate::ast::TransitionArm],
    program: &Program,
    names: &[String],
) -> Option<String> {
    let subj = gexpr(subject, program, names)?;
    let (last, rest) = arms.split_last()?;
    if !last.args.is_empty() {
        return None; // state parameters are a later slice
    }
    let mut acc = call_state(last.target, names);
    for arm in rest.iter().rev() {
        if !arm.args.is_empty() {
            return None;
        }
        let k = match arm.pattern {
            Pattern::Int(k) => k,
            Pattern::Wild => return None, // a non-terminal `_` would shadow the rest -> degenerate
        };
        acc = format!("(if (eq {} {}) {} {})", subj, k, call_state(arm.target, names), acc);
    }
    Some(acc)
}

// Translate a statement sequence (an entry or state body) from `idx`, threading the SSA name map.
// The sequence must terminate in a transition / exit / return.
fn translate_seq(
    stmts: &[Statement],
    idx: usize,
    program: &Program,
    names: &mut Vec<String>,
    fresh: &mut usize,
) -> Option<String> {
    match stmts.get(idx)? {
        Statement::Let(i, e, _) | Statement::Assign(i, e) => {
            let (i, e) = (*i, *e); // both variants bind a local index + an init/value node here
            if i >= names.len() {
                return None;
            }
            let value = gexpr(e, program, names)?; // evaluated under pre-write bindings
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            names[i] = nm.clone();
            let rest = translate_seq(stmts, idx + 1, program, names, fresh)?;
            Some(format!("(let {} {} {})", nm, value, rest))
        }
        Statement::Exit(e) | Statement::Return(e) => gexpr(*e, program, names),
        Statement::Transition(subject, arms) => translate_transition(*subject, arms, program, names),
        _ => None, // Store*/WriteByte/WriteLine/Eval/Assert/Block -> later slices
    }
}

// Translate the entry machine to a gamma program: one `(def …)` per state plus the entry `me`, then
// the initial call `(me 0 … 0)`. None if anything is outside the supported subset.
pub fn emit_gamma(program: &Program) -> Option<String> {
    let machine = &program.machines[program.entry_machine];
    let n = machine.local_count;
    let base: Vec<String> = (0..n).map(|i| format!("l{}", i)).collect();
    let sig = base.join(" ");
    let mut fresh = 0usize;
    let mut out = String::new();

    let mut names = base.clone();
    let entry = translate_seq(&machine.entry, 0, program, &mut names, &mut fresh)?;
    out.push_str(&format!("(def me ({}) {}) ", sig, entry));

    for (k, state) in machine.states.iter().enumerate() {
        let mut names = base.clone();
        let body = translate_seq(state, 0, program, &mut names, &mut fresh)?;
        out.push_str(&format!("(def s{} ({}) {}) ", k, sig, body));
    }

    if n == 0 {
        out.push_str("(me)");
    } else {
        out.push_str(&format!("(me {})", vec!["0"; n].join(" ")));
    }
    Some(out)
}

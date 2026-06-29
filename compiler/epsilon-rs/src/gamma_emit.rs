// EPSILON MEANING VIA GAMMA — the thread of getting epsilon out of Rust.
//
// rungs/epsilon.md: epsilon's meaning is "Written in Delta/Gamma" -- defined by the reference
// interpreter, not the native (Rust on-ramp) backend. This module translates the supported subset
// of an epsilon program into a GAMMA expression the Rust-free reference interpreter
// (`compiler/gamma/interp.beta`, compiled by bc, run on the seed) evaluates. The epsilon-meaning
// DIAMOND (epsilon-meaning-diamond.sh) then checks the two routes agree -- native execution vs
// gamma interpretation -- so epsilon's meaning is pinned by a program the lower rungs understand.
//
// MODEL: a machine is a set of mutually-recursive gamma functions over its mutable STATE -- the
// locals frame (l0..l_{n-1}) AND the entry data's scalar self FIELDS (g0..g_{m-1}, zero-initialised
// like epsilon's data instance). `me` is the entry; each state `s_k` takes the same state signature;
// a transition is a guarded TAIL-CALL; MUTATION (`let`/`assign`/`self.f = …`) is threaded SSA-style
// (each write rebinds via a fresh `let`, and an Env tracks the current binding of every slot). The
// machine starts by calling `me` with all slots 0. The straight-line case (no states, no fields)
// falls out: `me` is just the entry's `let`s and final exit.
//
// SUPPORTED (returns None -> the diamond skips otherwise): the entry machine only; i32 LOCALS and
// scalar self FIELDS; `let`/`assign`/`self.f = …` over the integer subset (`+ - * / %` and all six
// comparisons, faithfully encoded from interp's eq/lt); PARAMETERLESS states; integer-pattern / `_`
// transitions; `exit`/`return`. Self arrays, state parameters, cross-machine calls, bitwise/shift,
// `read_byte`, and overflow domains (gamma ints are unbounded) are follow-on slices.
use crate::ast::{BinaryOp, Expr, Machine, Pattern, Program, Statement};
use std::collections::BTreeSet;

// The current SSA binding of every mutable slot: locals (by index) and self fields (by byte offset).
#[derive(Clone)]
struct Env {
    locals: Vec<String>,    // current gamma name per local index
    field_off: Vec<i32>,    // distinct self-field byte offsets, ascending (canonical slot order)
    field_cur: Vec<String>, // current gamma name per field (parallel to field_off)
}

impl Env {
    fn field_name(&self, offset: i32) -> Option<&str> {
        self.field_off.iter().position(|&o| o == offset).map(|i| self.field_cur[i].as_str())
    }
    fn field_set(&mut self, offset: i32, name: String) -> Option<()> {
        let i = self.field_off.iter().position(|&o| o == offset)?;
        self.field_cur[i] = name;
        Some(())
    }
    // The full state vector (locals then fields), current bindings -- the args of a state tail-call.
    fn slots(&self) -> String {
        let mut v = self.locals.clone();
        v.extend(self.field_cur.iter().cloned());
        v.join(" ")
    }
}

// Render an expression node as a gamma s-expression under the current Env, or None if outside the
// integer subset. interp.beta's primitives are exactly `+ - * / %` and the comparisons `eq`/`lt`;
// the other comparisons are encoded FAITHFULLY over integers (no overflow-prone `+1`): a>b == b<a;
// a<=b == (a<b)|(a==b) == (a<b)+(a==b) (mutually exclusive, sum is 0/1); a>=b == (b<a)+(a==b);
// a!=b == 1-(a==b). Operands here are pure (locals/fields/arithmetic), so duplication is sound.
fn gexpr(node: usize, program: &Program, env: &Env) -> Option<String> {
    match program.expressions[node] {
        Expr::Int(k) if k >= 0 => Some(k.to_string()),
        Expr::Local(i) => env.locals.get(i).cloned(),
        Expr::SelfField(off) => env.field_name(off).map(|s| s.to_string()),
        Expr::Binary(o, l, r) => {
            let a = gexpr(l, program, env)?;
            let b = gexpr(r, program, env)?;
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
        // a free machine call B(args): call the callee's entry function, args filling its parameter
        // locals and 0 for the rest of its frame (zero-init). The callee returns a value (Return ->
        // gexpr) which is this expression's value. SELF/method callees are a later slice.
        Expr::Call(callee, start, count) => {
            let cm = &program.machines[callee];
            if !collect_field_offsets(cm, program).is_empty() {
                return None; // a callee touching self fields is a method-call slice
            }
            let mut parts = Vec::new();
            for j in 0..count {
                parts.push(gexpr(program.call_args[start + j], program, env)?);
            }
            for _ in cm.param_count..cm.local_count {
                parts.push("0".to_string()); // body locals beyond the params, zero-initialised
            }
            Some(if parts.is_empty() {
                format!("(m{}_me)", callee)
            } else {
                format!("(m{}_me {})", callee, parts.join(" "))
            })
        }
        _ => None, // SelfIndex/ReadByte/SelfCall -> later slices
    }
}

// A tail-call into state `target` of machine `mi`, forwarding the full current state vector
// (parameterless states only). State functions are named `m{mi}_s{target}`.
fn call_state(mi: usize, target: usize, env: &Env) -> String {
    let slots = env.slots();
    if slots.is_empty() {
        format!("(m{}_s{})", mi, target)
    } else {
        format!("(m{}_s{} {})", mi, target, slots)
    }
}

// A transition: guarded tail-calls. Arms 0..n-1 become `(if (eq subj pat) call rest)`; the LAST arm
// is the final else (a `_` default, or the exhaustive false-branch of a bool transition). Non-last
// arms must be integer patterns with no args (parameterless states); otherwise None.
fn translate_transition(
    mi: usize,
    subject: usize,
    arms: &[crate::ast::TransitionArm],
    program: &Program,
    env: &Env,
) -> Option<String> {
    let subj = gexpr(subject, program, env)?;
    let (last, rest) = arms.split_last()?;
    if !last.args.is_empty() {
        return None; // state parameters are a later slice
    }
    let mut acc = call_state(mi, last.target, env);
    for arm in rest.iter().rev() {
        if !arm.args.is_empty() {
            return None;
        }
        let k = match arm.pattern {
            Pattern::Int(k) => k,
            Pattern::Wild => return None, // a non-terminal `_` would shadow the rest -> degenerate
        };
        acc = format!("(if (eq {} {}) {} {})", subj, k, call_state(mi, arm.target, env), acc);
    }
    Some(acc)
}

// Translate a statement sequence (an entry or state body) from `idx`, threading the SSA Env. The
// sequence must terminate in a transition / exit / return.
fn translate_seq(
    mi: usize,
    stmts: &[Statement],
    idx: usize,
    program: &Program,
    env: &mut Env,
    fresh: &mut usize,
) -> Option<String> {
    match stmts.get(idx)? {
        // a local write: l_i = e  (Let init or Assign reassignment)
        Statement::Let(i, e, _) | Statement::Assign(i, e) => {
            let (i, e) = (*i, *e);
            if i >= env.locals.len() {
                return None;
            }
            let value = gexpr(e, program, env)?; // evaluated under pre-write bindings
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.locals[i] = nm.clone();
            let rest = translate_seq(mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, value, rest))
        }
        // a self-field write: self.<offset> = e
        Statement::StoreSelfField(off, val, _domain) => {
            let (off, val) = (*off, *val);
            let value = gexpr(val, program, env)?;
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.field_set(off, nm.clone())?;
            let rest = translate_seq(mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, value, rest))
        }
        Statement::Exit(e) | Statement::Return(e) => gexpr(*e, program, env),
        Statement::Transition(subject, arms) => translate_transition(mi, *subject, arms, program, env),
        _ => None, // StoreSelfIndex/WriteByte/WriteLine/Eval/Assert/Block -> later slices
    }
}

// Collect SelfField read offsets in an expression tree.
fn collect_expr_offsets(node: usize, program: &Program, out: &mut BTreeSet<i32>) {
    match program.expressions[node] {
        Expr::SelfField(off) => {
            out.insert(off);
        }
        Expr::Binary(_, l, r) => {
            collect_expr_offsets(l, program, out);
            collect_expr_offsets(r, program, out);
        }
        _ => {}
    }
}

// Collect every distinct scalar self-field byte offset the machine reads or writes (entry + states).
fn collect_field_offsets(machine: &Machine, program: &Program) -> Vec<i32> {
    let mut set = BTreeSet::new();
    let mut blocks: Vec<&[Statement]> = vec![&machine.entry];
    blocks.extend(machine.states.iter().map(|s| s.as_slice()));
    for block in blocks {
        for statement in block {
            match statement {
                Statement::Let(_, e, _) | Statement::Assign(_, e) => collect_expr_offsets(*e, program, &mut set),
                Statement::Exit(e) | Statement::Return(e) => collect_expr_offsets(*e, program, &mut set),
                Statement::StoreSelfField(off, val, _) => {
                    set.insert(*off);
                    collect_expr_offsets(*val, program, &mut set);
                }
                Statement::Transition(subj, _) => collect_expr_offsets(*subj, program, &mut set),
                _ => {}
            }
        }
    }
    set.into_iter().collect()
}

// Collect machine indices reached by Call exprs in an expression tree.
fn collect_callees_expr(node: usize, program: &Program, out: &mut BTreeSet<usize>) {
    match program.expressions[node] {
        Expr::Call(callee, start, count) => {
            out.insert(callee);
            for j in 0..count {
                collect_callees_expr(program.call_args[start + j], program, out);
            }
        }
        Expr::Binary(_, l, r) => {
            collect_callees_expr(l, program, out);
            collect_callees_expr(r, program, out);
        }
        _ => {}
    }
}

// Machine indices directly called from machine `mi` (entry + states).
fn machine_callees(mi: usize, program: &Program, out: &mut BTreeSet<usize>) {
    let machine = &program.machines[mi];
    let mut blocks: Vec<&[Statement]> = vec![&machine.entry];
    blocks.extend(machine.states.iter().map(|s| s.as_slice()));
    for block in blocks {
        for statement in block {
            match statement {
                Statement::Let(_, e, _) | Statement::Assign(_, e) | Statement::Exit(e)
                | Statement::Return(e) | Statement::StoreSelfField(_, e, _) => {
                    collect_callees_expr(*e, program, out)
                }
                Statement::Transition(subj, _) => collect_callees_expr(*subj, program, out),
                _ => {}
            }
        }
    }
}

// Emit machine `mi`'s defs (entry `m{mi}_me` plus each state `m{mi}_s{k}`) into `out`.
fn emit_machine_defs(mi: usize, program: &Program, fresh: &mut usize, out: &mut String) -> Option<()> {
    let machine = &program.machines[mi];
    let field_off = collect_field_offsets(machine, program);
    let base = Env {
        locals: (0..machine.local_count).map(|i| format!("l{}", i)).collect(),
        field_cur: (0..field_off.len()).map(|i| format!("g{}", i)).collect(),
        field_off,
    };
    let sig = base.slots();

    let mut env = base.clone();
    let entry = translate_seq(mi, &machine.entry, 0, program, &mut env, fresh)?;
    out.push_str(&format!("(def m{}_me ({}) {}) ", mi, sig, entry));
    for (k, state) in machine.states.iter().enumerate() {
        let mut env = base.clone();
        let body = translate_seq(mi, state, 0, program, &mut env, fresh)?;
        out.push_str(&format!("(def m{}_s{} ({}) {}) ", mi, k, sig, body));
    }
    Some(())
}

// Translate the program to a gamma expression: every machine reachable from the entry via Call
// becomes its own set of `m{idx}_*` defs; the program starts by calling the entry with all slots 0.
// None if anything is outside the supported subset.
pub fn emit_gamma(program: &Program) -> Option<String> {
    let entry = program.entry_machine;
    // reachable set: entry + transitive Call targets
    let mut seen: BTreeSet<usize> = [entry].into_iter().collect();
    let mut queue = vec![entry];
    let mut reachable = vec![entry];
    while let Some(mi) = queue.pop() {
        let mut callees = BTreeSet::new();
        machine_callees(mi, program, &mut callees);
        for c in callees {
            if seen.insert(c) {
                reachable.push(c);
                queue.push(c);
            }
        }
    }

    let mut fresh = 0usize;
    let mut out = String::new();
    for &mi in &reachable {
        emit_machine_defs(mi, program, &mut fresh, &mut out)?;
    }

    let slot_count = program.machines[entry].local_count
        + collect_field_offsets(&program.machines[entry], program).len();
    if slot_count == 0 {
        out.push_str(&format!("(m{}_me)", entry));
    } else {
        out.push_str(&format!("(m{}_me {})", entry, vec!["0"; slot_count].join(" ")));
    }
    Some(out)
}

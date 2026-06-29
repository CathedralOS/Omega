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

// The current SSA binding of every mutable slot: locals (by index), scalar self fields (by byte
// offset), and self arrays (by base byte offset; each holds a gamma LIST modeling the array).
#[derive(Clone)]
struct Env {
    locals: Vec<String>,    // current gamma name per local index
    field_off: Vec<i32>,    // distinct self-field byte offsets, ascending (canonical slot order)
    field_cur: Vec<String>, // current gamma name per field (parallel to field_off)
    array_off: Vec<i32>,    // distinct self-array base offsets, ascending
    array_cnt: Vec<i32>,    // element count per array (parallel to array_off) -- for zero-init
    array_cur: Vec<String>, // current gamma name per array (the list)
    input_cur: Option<String>, // the remaining stdin as a list, if this machine reads it (read_byte)
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
    fn array_name(&self, offset: i32) -> Option<&str> {
        self.array_off.iter().position(|&o| o == offset).map(|i| self.array_cur[i].as_str())
    }
    fn array_set(&mut self, offset: i32, name: String) -> Option<()> {
        let i = self.array_off.iter().position(|&o| o == offset)?;
        self.array_cur[i] = name;
        Some(())
    }
    // The full state vector (locals, fields, arrays, then the input stream), current bindings -- the
    // args a state tail-call forwards.
    fn slots(&self) -> String {
        let mut v = self.locals.clone();
        v.extend(self.field_cur.iter().cloned());
        v.extend(self.array_cur.iter().cloned());
        v.extend(self.input_cur.iter().cloned());
        v.join(" ")
    }
}

// Does this machine read stdin (a `read_byte()` anywhere in entry or states)?
fn uses_read_byte(machine: &Machine, program: &Program) -> bool {
    let mut blocks: Vec<&[Statement]> = vec![&machine.entry];
    blocks.extend(machine.states.iter().map(|s| s.as_slice()));
    blocks.iter().flat_map(|b| b.iter()).any(|s| stmt_value_is_read_byte(s, program))
}

// Is the statement a write whose value is exactly read_byte()? (the only supported read_byte shape)
fn stmt_value_is_read_byte(statement: &Statement, program: &Program) -> bool {
    let v = match statement {
        Statement::Let(_, e, _) | Statement::Assign(_, e) | Statement::StoreSelfField(_, e, _) => *e,
        _ => return false,
    };
    matches!(program.expressions[v], Expr::ReadByte)
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
        Expr::SelfIndex(off, _count, _eb, idx) => {
            let arr = env.array_name(off)?.to_string();
            Some(format!("(nth {} {})", arr, gexpr(idx, program, env)?))
        }
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
            if !collect_field_offsets(cm, program).is_empty() || uses_read_byte(cm, program) {
                return None; // a callee touching self fields or stdin is a later slice (no stream threaded in)
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

// The destination of a read_byte(): a local slot or a scalar self-field slot.
enum Target {
    Local(usize),
    Field(i32),
}

// Translate `<target> = read_byte()`: bind the target to the head of the input stream (or -1 at EOF)
// and advance the stream to its tail, both as fresh SSA bindings, then continue. Models epsilon's
// stateful read as two functional `match`es over the threaded input list. Requires an input slot.
fn read_byte_into(
    target: Target,
    mi: usize,
    stmts: &[Statement],
    idx: usize,
    program: &Program,
    env: &mut Env,
    fresh: &mut usize,
) -> Option<String> {
    let inp = env.input_cur.clone()?;
    let head = format!("(match {0} (Nil (- 0 1)) ((Cons h t) h))", inp); // next byte, or -1 at EOF
    let tail = format!("(match {0} (Nil Nil) ((Cons h t) t))", inp); // remaining stream
    let cnm = format!("t{}", *fresh);
    *fresh += 1;
    let inm = format!("t{}", *fresh);
    *fresh += 1;
    match target {
        Target::Local(i) => env.locals[i] = cnm.clone(),
        Target::Field(off) => env.field_set(off, cnm.clone())?,
    }
    env.input_cur = Some(inm.clone());
    let rest = translate_seq(mi, stmts, idx + 1, program, env, fresh)?;
    Some(format!("(let {} {} (let {} {} {}))", cnm, head, inm, tail, rest))
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
            if matches!(program.expressions[e], Expr::ReadByte) {
                return read_byte_into(Target::Local(i), mi, stmts, idx, program, env, fresh);
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
            if matches!(program.expressions[val], Expr::ReadByte) {
                return read_byte_into(Target::Field(off), mi, stmts, idx, program, env, fresh);
            }
            let value = gexpr(val, program, env)?;
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.field_set(off, nm.clone())?;
            let rest = translate_seq(mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, value, rest))
        }
        // a self-array write: self.<arr>[ix] = e  (functional update of the modeled list)
        Statement::StoreSelfIndex(off, _count, _eb, ix, val) => {
            let (off, ix, val) = (*off, *ix, *val);
            let arr = env.array_name(off)?.to_string();
            let update = format!("(setl {} {} {})", arr, gexpr(ix, program, env)?, gexpr(val, program, env)?);
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.array_set(off, nm.clone())?;
            let rest = translate_seq(mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, update, rest))
        }
        Statement::Exit(e) | Statement::Return(e) => gexpr(*e, program, env),
        Statement::Transition(subject, arms) => translate_transition(mi, *subject, arms, program, env),
        _ => None, // WriteByte/WriteLine/Eval/Assert/Block -> later slices
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
                Statement::StoreSelfIndex(_, _, _, ix, val) => {
                    collect_expr_offsets(*ix, program, &mut set);
                    collect_expr_offsets(*val, program, &mut set);
                }
                Statement::Transition(subj, _) => collect_expr_offsets(*subj, program, &mut set),
                _ => {}
            }
        }
    }
    set.into_iter().collect()
}

// Collect SelfIndex array (base offset -> element count) in an expression tree.
fn collect_arrays_expr(node: usize, program: &Program, out: &mut std::collections::BTreeMap<i32, i32>) {
    match program.expressions[node] {
        Expr::SelfIndex(off, count, _eb, idx) => {
            out.insert(off, count);
            collect_arrays_expr(idx, program, out);
        }
        Expr::Binary(_, l, r) => {
            collect_arrays_expr(l, program, out);
            collect_arrays_expr(r, program, out);
        }
        _ => {}
    }
}

// Collect every distinct self-array the machine reads or writes, as (base offset, element count),
// ascending by offset. Each becomes one threaded list slot, zero-initialised to `count` elements.
fn collect_arrays(machine: &Machine, program: &Program) -> Vec<(i32, i32)> {
    let mut map = std::collections::BTreeMap::new();
    let mut blocks: Vec<&[Statement]> = vec![&machine.entry];
    blocks.extend(machine.states.iter().map(|s| s.as_slice()));
    for block in blocks {
        for statement in block {
            match statement {
                Statement::Let(_, e, _) | Statement::Assign(_, e) | Statement::Exit(e)
                | Statement::Return(e) | Statement::StoreSelfField(_, e, _) => {
                    collect_arrays_expr(*e, program, &mut map)
                }
                Statement::StoreSelfIndex(off, count, _eb, ix, val) => {
                    map.insert(*off, *count);
                    collect_arrays_expr(*ix, program, &mut map);
                    collect_arrays_expr(*val, program, &mut map);
                }
                Statement::Transition(subj, _) => collect_arrays_expr(*subj, program, &mut map),
                _ => {}
            }
        }
    }
    map.into_iter().collect()
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
fn machine_env(mi: usize, program: &Program) -> Env {
    let machine = &program.machines[mi];
    let field_off = collect_field_offsets(machine, program);
    let arrays = collect_arrays(machine, program);
    Env {
        locals: (0..machine.local_count).map(|i| format!("l{}", i)).collect(),
        field_cur: (0..field_off.len()).map(|i| format!("g{}", i)).collect(),
        field_off,
        array_cur: (0..arrays.len()).map(|i| format!("a{}", i)).collect(),
        array_off: arrays.iter().map(|&(o, _)| o).collect(),
        array_cnt: arrays.iter().map(|&(_, c)| c).collect(),
        input_cur: if uses_read_byte(machine, program) { Some("inp".to_string()) } else { None },
    }
}

fn emit_machine_defs(mi: usize, program: &Program, fresh: &mut usize, out: &mut String) -> Option<()> {
    let machine = &program.machines[mi];
    let base = machine_env(mi, program);
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
pub fn emit_gamma(program: &Program, input: &[i32]) -> Option<String> {
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
    // only the entry machine reads stdin in this slice (its input stream isn't threaded into calls)
    if reachable.iter().skip(1).any(|&mi| uses_read_byte(&program.machines[mi], program)) {
        return None;
    }

    let entry_env = machine_env(entry, program);
    let mut fresh = 0usize;
    let mut out = String::new();

    // list helpers for self arrays: nth (read) and setl (functional update). Emitted only when the
    // entry uses arrays (free callees have no self, so only the entry can). interp's Cons/Nil + match.
    if !entry_env.array_off.is_empty() {
        out.push_str("(def nth (xs k) (match xs (Nil 0) ((Cons h t) (if (eq k 0) h (nth t (- k 1)))))) ");
        out.push_str("(def setl (xs k v) (match xs (Nil Nil) ((Cons h t) (if (eq k 0) (Cons v t) (Cons h (setl t (- k 1) v)))))) ");
    }

    for &mi in &reachable {
        emit_machine_defs(mi, program, &mut fresh, &mut out)?;
    }

    // initial call: 0 for every local and scalar field, a zero-list per array, and (if the entry reads
    // stdin) the baked input stream as a `(Cons b0 … Nil)` list -- the same bytes the diamond feeds native.
    let mut init: Vec<String> = Vec::new();
    init.extend(std::iter::repeat("0".to_string()).take(entry_env.locals.len()));
    init.extend(std::iter::repeat("0".to_string()).take(entry_env.field_off.len()));
    for &count in &entry_env.array_cnt {
        let mut list = String::from("Nil");
        for _ in 0..count {
            list = format!("(Cons 0 {})", list);
        }
        init.push(list);
    }
    if entry_env.input_cur.is_some() {
        let mut list = String::from("Nil");
        for &b in input.iter().rev() {
            list = format!("(Cons {} {})", b, list);
        }
        init.push(list);
    }
    if init.is_empty() {
        out.push_str(&format!("(m{}_me)", entry));
    } else {
        out.push_str(&format!("(m{}_me {})", entry, init.join(" ")));
    }
    Some(out)
}

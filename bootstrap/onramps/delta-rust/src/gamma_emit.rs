// DELTA MEANING VIA GAMMA — the thread of getting delta out of Rust.
//
// rungs/delta.md: delta's meaning is "Written in Delta/Gamma" -- defined by the reference
// interpreter, not the native (Rust on-ramp) backend. This module translates the supported subset
// of a Delta program into a GAMMA expression the Rust-free reference interpreter
// (`bootstrap/rungs/gamma/interp.beta`, compiled by bc, run on the seed) evaluates. The delta-meaning
// DIAMOND (delta-meaning-diamond.sh) then checks the two routes agree -- native execution vs
// gamma interpretation -- so delta's meaning is pinned by a program the lower rungs understand.
//
// MODEL: a machine is a set of mutually-recursive gamma functions over its mutable STATE -- the
// locals frame (l0..l_{n-1}) AND the entry data's scalar self FIELDS (g0..g_{m-1}, zero-initialised
// like delta's data instance). `me` is the entry; each state `s_k` takes the same state signature;
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
    output_cur: Option<String>, // the stdout written so far (reversed list), if this machine writes
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
    // The full state vector (locals, fields, arrays, input, output), current bindings -- the args a
    // state tail-call forwards.
    fn slots(&self) -> String {
        let mut v = self.locals.clone();
        v.extend(self.self_slots());
        v.join(" ")
    }
    // Just the SELF-STATE slots (fields, arrays, input, output) -- the part shared across a method
    // call, threaded in and bundled back out. Canonical order matches the signature.
    fn self_slots(&self) -> Vec<String> {
        let mut v: Vec<String> = self.field_cur.clone();
        v.extend(self.array_cur.iter().cloned());
        v.extend(self.input_cur.iter().cloned());
        v.extend(self.output_cur.iter().cloned());
        v
    }
    // Rebind every self-state slot from `names` (same order/length as self_slots()).
    fn set_self_slots(&mut self, names: &[String]) {
        let nf = self.field_cur.len();
        let na = self.array_cur.len();
        self.field_cur = names[0..nf].to_vec();
        self.array_cur = names[nf..nf + na].to_vec();
        let mut k = nf + na;
        if self.input_cur.is_some() {
            self.input_cur = Some(names[k].clone());
            k += 1;
        }
        if self.output_cur.is_some() {
            self.output_cur = Some(names[k].clone());
        }
    }
}

// Bundle self-state values into a right-nested Pair tuple (the single value if there's one slot).
// A method returns this so the caller can thread the mutated self back out.
fn bundle(slots: &[String]) -> String {
    let n = slots.len();
    let mut acc = slots[n - 1].clone();
    for s in slots[..n - 1].iter().rev() {
        acc = format!("(Pair {} {})", s, acc);
    }
    acc
}

// Wrap `rest` so it runs with `nss` bound to the components of the method-call result tuple.
fn unbundle(call: &str, nss: &[String], rest: &str, fresh: &mut usize) -> String {
    if nss.len() == 1 {
        return format!("(let {} {} {})", nss[0], call, rest);
    }
    let t = format!("t{}", *fresh);
    *fresh += 1;
    format!("(let {} {} {})", t, call, unbundle_matches(&t, nss, 0, rest, fresh))
}

fn unbundle_matches(src: &str, nss: &[String], from: usize, rest: &str, fresh: &mut usize) -> String {
    if from == nss.len() - 2 {
        format!("(match {} ((Pair {} {}) {}))", src, nss[from], nss[from + 1], rest)
    } else {
        let r = format!("t{}", *fresh);
        *fresh += 1;
        let inner = unbundle_matches(&r, nss, from + 1, rest, fresh);
        format!("(match {} ((Pair {} {}) {}))", src, nss[from], r, inner)
    }
}

// Does this machine write stdout (write_byte / write_line anywhere)?
fn uses_output(machine: &Machine) -> bool {
    let mut blocks: Vec<&[Statement]> = vec![&machine.entry];
    blocks.extend(machine.states.iter().map(|s| s.as_slice()));
    blocks
        .iter()
        .flat_map(|b| b.iter())
        .any(|s| matches!(s, Statement::WriteByte(_) | Statement::WriteLine(_)))
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
        // min(a,b)/max(a,b): the `(if (lt a b) ..)` select over interp's existing lt primitive.
        Expr::Min(l, r) => {
            let a = gexpr(l, program, env)?;
            let b = gexpr(r, program, env)?;
            Some(format!("(if (lt {a} {b}) {a} {b})", a = a, b = b))
        }
        Expr::Max(l, r) => {
            let a = gexpr(l, program, env)?;
            let b = gexpr(r, program, env)?;
            Some(format!("(if (lt {a} {b}) {b} {a})", a = a, b = b))
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
            if !collect_field_offsets(cm, program).is_empty() || uses_read_byte(cm, program) || uses_output(cm) {
                return None; // a callee touching self fields, stdin, or stdout is a later slice
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
// and advance the stream to its tail, both as fresh SSA bindings, then continue. Models delta's
// stateful read as two functional `match`es over the threaded input list. Requires an input slot.
fn read_byte_into(
    target: Target,
    as_method: bool,
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
    let rest = translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh)?;
    Some(format!("(let {} {} (let {} {} {}))", cnm, head, inm, tail, rest))
}

// Translate a statement sequence (an entry or state body) from `idx`, threading the SSA Env. The
// entry/states of the root must terminate in a transition / exit / return; a METHOD (as_method) may
// also fall off the end, which returns its mutated self-state bundle to the caller.
fn translate_seq(
    as_method: bool,
    mi: usize,
    stmts: &[Statement],
    idx: usize,
    program: &Program,
    env: &mut Env,
    fresh: &mut usize,
) -> Option<String> {
    let statement = match stmts.get(idx) {
        Some(s) => s,
        None => {
            // fell off the end: a void method yields its self-state bundle; the root needs explicit exit
            return if as_method { Some(method_return(env)) } else { None };
        }
    };
    match statement {
        // a local write: l_i = e  (Let init or Assign reassignment)
        Statement::Let(i, e, _) | Statement::Assign(i, e) => {
            let (i, e) = (*i, *e);
            if i >= env.locals.len() {
                return None;
            }
            if matches!(program.expressions[e], Expr::ReadByte) {
                return read_byte_into(Target::Local(i), as_method, mi, stmts, idx, program, env, fresh);
            }
            let value = gexpr(e, program, env)?; // evaluated under pre-write bindings
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.locals[i] = nm.clone();
            let rest = translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, value, rest))
        }
        // a self-field write: self.<offset> = e
        Statement::StoreSelfField(off, val, _domain) => {
            let (off, val) = (*off, *val);
            if matches!(program.expressions[val], Expr::ReadByte) {
                return read_byte_into(Target::Field(off), as_method, mi, stmts, idx, program, env, fresh);
            }
            let value = gexpr(val, program, env)?;
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.field_set(off, nm.clone())?;
            let rest = translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh)?;
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
            let rest = translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, update, rest))
        }
        // stdout writes: cons the byte(s) onto the reversed output accumulator (a later `rev` un-reverses)
        Statement::WriteByte(e) => {
            let out = env.output_cur.clone()?;
            let cons = format!("(Cons {} {})", gexpr(*e, program, env)?, out);
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.output_cur = Some(nm.clone());
            let rest = translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, cons, rest))
        }
        Statement::WriteLine(sidx) => {
            let out = env.output_cur.clone()?;
            let mut cons = out;
            for &b in program.strings.get(*sidx)? {
                cons = format!("(Cons {} {})", b, cons); // bytes in order -> innermost is the first byte
            }
            let nm = format!("t{}", *fresh);
            *fresh += 1;
            env.output_cur = Some(nm.clone());
            let rest = translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh)?;
            Some(format!("(let {} {} {})", nm, cons, rest))
        }
        // a terminating exit/return: a METHOD yields its self-state bundle; otherwise (the root) the
        // produced stdout in OUTPUT mode (un-reversed via rev), else the exit-code expression.
        Statement::Exit(e) | Statement::Return(e) => {
            if as_method {
                Some(method_return(env))
            } else {
                match &env.output_cur {
                    Some(out) => Some(format!("(rev {} Nil)", out)),
                    None => gexpr(*e, program, env),
                }
            }
        }
        Statement::Transition(subject, arms) => translate_transition(mi, *subject, arms, program, env),
        // a method call `self.m(args)` for effect: thread the whole self-state through the callee
        Statement::Eval(node) => {
            let (callee, start, count) = match program.expressions[*node] {
                Expr::SelfCall(c, s, n) => (c, s, n),
                _ => return None, // free-call for effect (discarded result) -> not modeled
            };
            let ss = env.self_slots();
            if ss.is_empty() {
                // nothing to thread -> the method is observably a no-op; skip it
                return translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh);
            }
            let cm = &program.machines[callee];
            let mut parts = Vec::new();
            for j in 0..count {
                parts.push(gexpr(program.call_args[start + j], program, env)?);
            }
            for _ in cm.param_count..cm.local_count {
                parts.push("0".to_string()); // method body locals beyond the params, zero-initialised
            }
            parts.extend(ss.iter().cloned()); // pass the caller's current self-state
            let call = format!("(m{}_me {})", callee, parts.join(" "));
            // fresh names for the updated self-state the method returns, then rebind and continue
            let nss: Vec<String> = (0..ss.len())
                .map(|_| {
                    let n = format!("t{}", *fresh);
                    *fresh += 1;
                    n
                })
                .collect();
            env.set_self_slots(&nss);
            let rest = translate_seq(as_method, mi, stmts, idx + 1, program, env, fresh)?;
            Some(unbundle(&call, &nss, &rest, fresh))
        }
        _ => None, // Assert/Block -> later slices
    }
}

// A method's terminal value: its self-state bundled for the caller to thread back (or 0 if it has none).
fn method_return(env: &Env) -> String {
    let ss = env.self_slots();
    if ss.is_empty() {
        "0".to_string()
    } else {
        bundle(&ss)
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
                Statement::Let(_, e, _) | Statement::Assign(_, e) | Statement::WriteByte(e) => {
                    collect_expr_offsets(*e, program, &mut set)
                }
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
                | Statement::Return(e) | Statement::StoreSelfField(_, e, _) | Statement::WriteByte(e) => {
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
                | Statement::Return(e) | Statement::StoreSelfField(_, e, _)
                | Statement::WriteByte(e) => collect_callees_expr(*e, program, out),
                Statement::StoreSelfIndex(_, _, _, ix, val) => {
                    collect_callees_expr(*ix, program, out);
                    collect_callees_expr(*val, program, out);
                }
                Statement::Transition(subj, _) => collect_callees_expr(*subj, program, out),
                // a method call `self.m(args)` for effect -- m is reachable, and its args may call too
                Statement::Eval(node) => {
                    if let Expr::SelfCall(c, s, n) = program.expressions[*node] {
                        out.insert(c);
                        for j in 0..n {
                            collect_callees_expr(program.call_args[s + j], program, out);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

// The program-wide UNIFIED self-state shared by every `&mut self` machine (they alias the same data
// instance): the union of all scalar fields, arrays, and the stdin/stdout streams any of them touches.
// Methods thread this whole bundle through a call, so the layout must be the SAME for caller and callee.
struct SelfState {
    field_off: Vec<i32>,
    array_off: Vec<i32>,
    array_cnt: Vec<i32>,
    has_input: bool,
    has_output: bool,
}

fn compute_self_state(reachable: &[usize], program: &Program) -> SelfState {
    let mut fields = BTreeSet::new();
    let mut arrays: std::collections::BTreeMap<i32, i32> = std::collections::BTreeMap::new();
    let (mut has_input, mut has_output) = (false, false);
    for &mi in reachable {
        let machine = &program.machines[mi];
        if !machine.has_self {
            continue; // free callees alias no self
        }
        fields.extend(collect_field_offsets(machine, program));
        for (off, count) in collect_arrays(machine, program) {
            arrays.insert(off, count);
        }
        has_input |= uses_read_byte(machine, program);
        has_output |= uses_output(machine);
    }
    SelfState {
        field_off: fields.into_iter().collect(),
        array_off: arrays.keys().copied().collect(),
        array_cnt: arrays.values().copied().collect(),
        has_input,
        has_output,
    }
}

// Build a machine's Env: its own locals, plus -- for a `&mut self` machine -- the UNIFIED self-state
// (so caller and callee agree on the bundle); a free callee gets locals only.
fn machine_env(mi: usize, program: &Program, unified: &SelfState) -> Env {
    let machine = &program.machines[mi];
    let locals: Vec<String> = (0..machine.local_count).map(|i| format!("l{}", i)).collect();
    if !machine.has_self {
        return Env {
            locals,
            field_off: Vec::new(),
            field_cur: Vec::new(),
            array_off: Vec::new(),
            array_cnt: Vec::new(),
            array_cur: Vec::new(),
            input_cur: None,
            output_cur: None,
        };
    }
    Env {
        locals,
        field_cur: (0..unified.field_off.len()).map(|i| format!("g{}", i)).collect(),
        field_off: unified.field_off.clone(),
        array_cur: (0..unified.array_off.len()).map(|i| format!("a{}", i)).collect(),
        array_off: unified.array_off.clone(),
        array_cnt: unified.array_cnt.clone(),
        input_cur: if unified.has_input { Some("inp".to_string()) } else { None },
        output_cur: if unified.has_output { Some("out".to_string()) } else { None },
    }
}

fn emit_machine_defs(
    mi: usize,
    entry: usize,
    program: &Program,
    unified: &SelfState,
    fresh: &mut usize,
    out: &mut String,
) -> Option<()> {
    let machine = &program.machines[mi];
    let base = machine_env(mi, program, unified);
    let sig = base.slots();
    // a non-entry `&mut self` machine is a METHOD: it terminates by returning its self-state bundle.
    let as_method = mi != entry && machine.has_self;

    let mut env = base.clone();
    let body = translate_seq(as_method, mi, &machine.entry, 0, program, &mut env, fresh)?;
    out.push_str(&format!("(def m{}_me ({}) {}) ", mi, sig, body));
    for (k, state) in machine.states.iter().enumerate() {
        let mut env = base.clone();
        let body = translate_seq(as_method, mi, state, 0, program, &mut env, fresh)?;
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
    // the program-wide self-state shared by every `&mut self` machine (methods thread it through calls)
    let unified = compute_self_state(&reachable, program);
    let entry_env = machine_env(entry, program, &unified);
    let mut fresh = 0usize;
    let mut out = String::new();

    // list helpers for self arrays: nth (read) and setl (functional update). Emitted only when the
    // entry uses arrays (free callees have no self, so only the entry can). interp's Cons/Nil + match.
    if !entry_env.array_off.is_empty() {
        out.push_str("(def nth (xs k) (match xs (Nil 0) ((Cons h t) (if (eq k 0) h (nth t (- k 1)))))) ");
        out.push_str("(def setl (xs k v) (match xs (Nil Nil) ((Cons h t) (if (eq k 0) (Cons v t) (Cons h (setl t (- k 1) v)))))) ");
    }
    // reverse helper for output: the accumulator conses bytes front-first, so the final value is rev'd
    if entry_env.output_cur.is_some() {
        out.push_str("(def rev (xs acc) (match xs (Nil acc) ((Cons h t) (rev t (Cons h acc))))) ");
    }

    for &mi in &reachable {
        emit_machine_defs(mi, entry, program, &unified, &mut fresh, &mut out)?;
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
    if entry_env.output_cur.is_some() {
        init.push("Nil".to_string()); // the output accumulator starts empty
    }
    if init.is_empty() {
        out.push_str(&format!("(m{}_me)", entry));
    } else {
        out.push_str(&format!("(m{}_me {})", entry, init.join(" ")));
    }
    Some(out)
}

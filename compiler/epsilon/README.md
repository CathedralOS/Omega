# `epsilon/` — the kept, Rust-free epsilon rung

`epsilon-rs/` is the **disposable Rust on-ramp** for the epsilon systems language. This directory is
where epsilon's **kept, Rust-free** artifacts live — the parts of epsilon whose *definition* has no
Rust on it, in the alpha→beta→bc trust lineage.

## Why this exists

`rungs/epsilon.md`: epsilon's meaning is *"Written in Delta/Gamma"* — defined by the reference
interpreter, not the native backend. The existing epsilon-meaning diamond
(`epsilon-rs/epsilon-meaning-diamond.sh`) pins that meaning against native execution, but its
translator — `epsilon-rs/src/gamma_emit.rs` — is **Rust**, so Rust still sat on the meaning route.

## What's here

- **`eps2gamma.beta`** — a Rust-free epsilon→gamma **meaning translator**, written in Beta (built by
  alpha→beta→bc, exactly like `gamma/interp.beta`). It reads epsilon source on stdin and prints a
  gamma s-expression; `gamma/interp.beta` (also Rust-free) then runs it. Both halves of the meaning
  route are now in the Rust-free lineage. It is the Rust-free counterpart to `gamma_emit.rs`.

- **`eps2gamma-diamond.sh`** — the gate. Each program is run two ways and the exit codes must agree:
  1. **native** — the epsilon-rs aarch64 backend (the reference being *checked*, not trusted);
  2. **eps2gamma** — `eps2gamma.beta` (Rust-free) → `interp.beta` (Rust-free).

  It also cross-checks that the Rust-free route agrees with the existing Rust `gamma_emit.rs` route
  (`EPS_EMIT=gamma`) — the two translators converge. Wired into `verify-lattice.sh`.

## Supported subset (grows slice by slice)

Straight-line integer `main` — a run of `let x: i32 = <expr>;` bindings and a final
`self.console.exit_process(<expr>)`. Locals render as gamma `l{index}` (gamma reserves
Uppercase-leading identifiers for constructors). This mirrors how the `gamma_emit.rs` diamond started;
the subset widens with each slice (the same features `gamma_emit.rs` already covers, ported to the
Rust-free translator one at a time).

- **Slice 0** — `<expr>` = integer arithmetic over `+ - * / %`, parens, integer literals, locals.
- **Slice 1** — `<expr>` also = comparisons `< > <= >= == !=` (lowest precedence, below `+ -`),
  rendered faithfully from interp's only two comparison primitives `eq`/`lt`
  (`a<=b` ⇒ `(+ (lt a b) (eq a b))`, `a!=b` ⇒ `(- 1 (eq a b))`, etc).
- **Slice 2** — **state machines**: `state name() { … }` + `transition <subj> { <pat> -> <state>() … }`
  and local assignment `x = <expr>;`. The machine becomes mutually-recursive gamma defs sharing the
  full-locals signature — `(def m0_me (l0 …) …) (def m0_s{k} (l0 …) …) (m0_me 0 …)`. Mutation is
  SSA-threaded (each write ⇒ a fresh `(let t{n} …)`, tracked per-block); a transition ⇒ a nested
  `(if (eq subj pat) (target <current names>) else)`, last arm the default. Patterns: int / `true` /
  `false` / `_`.
- **Slice 3** — **self data fields**: `self.f` reads and `self.f = <expr>;` writes. Each scalar i32
  field becomes a threaded slot `g{i}`, appended to every def signature after the locals and
  zero-initialised in the entry call — the same SSA machinery as locals. `self.console.exit_process`
  stays the boundary terminal (distinguished from a field store by the trailing `.method`).
- **Next** — cross-machine calls, arrays, read_byte.

## The long game

The full removal of Rust from epsilon needs both (a) the **meaning** route Rust-free (this directory)
and (b) the **native** route Rust-free (the `epsilon-rs/samples/lowermachine.alp` self-hoster, whose
own bootstrap still seeds through Rust). This directory tackles (a): the meaning of epsilon — what it
computes — is now a lattice-defined fact for the supported subset, with no Rust in its definition.

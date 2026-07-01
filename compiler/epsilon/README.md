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
- **Slice 4** — **cross-machine calls**: value-returning free machines `machine name(p: i32, …) -> i32
  { … }` alongside the entry. Every machine becomes its own `m{idx}_*` defs; a call `f(a, b)` ⇒
  `(m{f}_me <a> <b> <zeros for f's non-param locals>)`. `return <expr>;` is the free-machine terminal.
  Nesting, recursion, and calls in loops all fall out (interp registers all defs before eval).
- **Slice 5** — **self arrays**: `self.arr[i]` reads and `self.arr[i] = <expr>;` writes. Each array
  becomes a threaded gamma **list** slot `a{i}` (after locals + fields), zero-initialised to
  `(Cons 0 … Nil)` of its declared length (read from `data Main { arr: [i32; N]; }`). Reads lower to
  `(nth a{i} <ix>)`, writes to `(setl a{i} <ix> <val>)`; the two list helpers `nth`/`setl` are
  prepended once when the entry uses arrays.
- **Slice 6** — **read_byte** (stdin): `x = read_byte()` lowers to two `(let …)`s — bind `x` to the head
  `(match inp (Nil (- 0 1)) ((Cons h t) h))` (the byte, or −1 at EOF) and rebind the threaded input slot
  `inp` to the tail. The input is the last slot (present iff the entry reads). Since `eps2gamma` is a pure
  stdin→stdout filter, it emits a `STDIN` placeholder for the stream's initial value (the genuine external
  input, exactly like the Rust route's `EPS_GAMMA_INPUT`); the diamond substitutes the `(Cons … Nil)` byte
  list, feeding the *same* bytes to native stdin.
- **Next** — stdout (`write_byte`/`write_line`) + self-method calls (the certifier capstone).

## The long game

The full removal of Rust from epsilon needs both (a) the **meaning** route Rust-free (this directory)
and (b) the **native** route Rust-free (the `epsilon-rs/samples/lowermachine.alp` self-hoster, whose
own bootstrap still seeds through Rust). This directory tackles (a): the meaning of epsilon — what it
computes — is now a lattice-defined fact for the supported subset, with no Rust in its definition.

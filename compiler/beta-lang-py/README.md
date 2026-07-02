# `compiler/beta-lang-py/` — the SECOND, independent Beta compiler (closing the Thompson gap)

This directory holds **`bc2.py`**, a from-scratch Beta → Alpha-assembly compiler written in Python, and
**`diverse-double-compilation.sh`**, the gate that uses it. Its whole reason to exist is **decision D5 —
the one real Thompson-resistance gap in the lattice.**

## The gap it closes

`bc.beta` (the Beta compiler in Beta) is only ever turned from *source* into *assembly* by one thing: the
Rust on-ramp, `../beta-lang-rs`. The self-host gate (`../beta-lang/selfhost.sh`) proves `bc` *reproduces*
its own compilation byte-for-byte — but reproduction is not **diversity**. If the Rust on-ramp injected a
self-perpetuating Trojan (Thompson's attack), it would ride straight through the self-host fixed point
and every gate would still pass.

The classical defence (Wheeler's *diverse double compilation*) is a **second, independent compiler** for
the same language. Compile the compiler's source two independent ways; if the two self-compilations agree
byte-for-byte, neither path injected a surviving Trojan (barring both injecting the identical one). This
directory is that second path — deliberately **not** derived from `beta-lang-rs`, written fresh against
`../alpha/SEMANTICS.md` (the 21-opcode ISA) and the Beta grammar.

## Trust status: UNTRUSTED, like the other front-line tools

`bc2.py` is **not** part of the runtime trust base — no more than `elab.py`, `prover.py`, or
`tv-encode.py` are. Its output is **checked, never trusted**: the gate assembles and runs what it emits
and compares against the independent path. A bug or a Trojan in `bc2.py` produces a **disagreement — a
loud failure — never a silent pass.** That is exactly why Python is acceptable here: it is a verification
instrument, not a link in the trust chain. (The runtime lineage stays α → β → bc, with no Python in it.)

## Status — growing toward `bc.beta`

`bc2.py` is built slice by slice, mirroring how `bc.beta` itself was grown, until it can compile `bc.beta`.
At that point the gate becomes **true DDC**: `bc.beta` compiled through the independent path must reproduce
the official self-host fixed point.

| Slice | Covers | Done |
| --- | --- | --- |
| 1 | single `proc main()`, `let` locals, assignment, `+ - * / %` with parens/precedence, `return` | ✅ |
| 2 | the six comparisons (materialised 0/1) + `state`/`to..when` CFG control flow (loops) | ✅ |
| 3 | procedures, parameters, calls, recursion (args r0..r3, callee spills to frame) | ✅ |
| 4 | `byte[]`/`word[]` memory — `load`/`loadb`/`store`/`storeb` | ✅ |
| 5 | char literals `'x'` (+ escapes), `read_byte`/`write_byte`, call statements | ✅ |
| 6 | string literals via `emit("...")` (`db` data + a `__write_str` loop) | ✅ |
| ⇒ | **compile `bc.beta` → true diverse double compilation of `bc`** | ✅ **DONE** |

**The gap is closed — for the whole trust surface.** `bc2.py` compiles all of the substantial Beta
programs, and the two independent compilers (Rust on-ramp `bc0`, Python-lineage `bcA`) compile each one
to **byte-for-byte identical** assembly:

| program | role | asm lines |
| --- | --- | --- |
| `bc.beta` | the Beta compiler itself | 8716 |
| `check.beta` | the δ proof checker (the trust anchor) | 27208 |
| `eq.beta` | the δ equality checker | 6757 |
| `interp.beta` | the γ reference interpreter (the meaning substrate) | 7662 |
| `typeck.beta` | the γ type checker | 8484 |
| `omega2gamma.beta` | the Omega→Gamma elaborator | 24919 |

So the compilation of every trust-critical program is independent of which bootstrap compiler produced it —
a Trojan in either path would have to be present, identically, in *both* independent implementations.
Agreement on `bc.beta` alone would not prove agreement on the checker, so the gate checks the actual
programs whose compilation determines trust. (Along the way `bc2.py` also compiles the `calc.beta`
recursive-descent calculator and matches the on-ramp byte-for-byte on real input.)

Run the gate:

```sh
sh diverse-double-compilation.sh   # compiles a corpus with BOTH front-ends, runs both, asserts they agree
```

## `beta_interp.py` — a reference interpreter (compiler *correctness*, not just reproducibility)

DDC proves `bc` compiles **reproducibly** (Thompson); it says nothing about whether `bc` compiles
**correctly**. Beta has no formal spec — `bc.beta` is its de-facto definition — so `beta_interp.py` is a
**second, independent definition of Beta's meaning**: it tree-walks `bc2.py`'s AST (reusing the lexer +
parser, separate back end) and runs the program directly, mirroring Alpha's exact 64-bit signed semantics
(comparisons and div/mod signed, truncating toward zero; div-by-zero and INT_MIN/−1 trap; exit = low byte
of `main`'s result). `beta-correctness-fuzz.sh` runs random programs (`beta-fuzz-gen.py`, a DAG of
value-returning procs so they always terminate) two ways — **interpret** vs **compile-with-`bc`-and-run** —
and asserts they agree on exit and stdout. A `bc` miscompile shows up as a disagreement (a negative control
that breaks the interpreter's `+` is duly caught). 200 random programs agree.

```sh
sh beta-correctness-fuzz.sh        # interpret == compile+run over random programs
```

Both gates skip cleanly without `python3` or `cargo` (the on-ramp). Part of `../verify-lattice.sh`.

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
| — | char literals, `read_byte`/`write_byte`, call statements | next |
| — | string literals via `emit("...")` | |
| ⇒ | **compile `bc.beta` → true diverse double compilation of `bc`** | goal |

Run the gate:

```sh
sh diverse-double-compilation.sh   # compiles a corpus with BOTH front-ends, runs both, asserts they agree
```

Skips cleanly without `python3` or `cargo` (the on-ramp). Part of `../verify-lattice.sh`.

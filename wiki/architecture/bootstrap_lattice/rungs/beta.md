# Rung: Beta — structured compiler construction

[Lattice overview](../bootstrap_lattice.md) | Prev: [Alpha](alpha.md) | Next: [Gamma](gamma.md)

Beta turns raw Alpha construction into a small language suitable for writing
compilers. It deliberately resembles Omega control flow without importing the
types, ownership, effects, or proof machinery of higher rungs.

## Adds

- named procedures, parameters, locals, calls, and recursion;
- explicit stack frames and a fixed calling convention;
- CFG/Omega-shaped `state` blocks and guarded `to` transitions;
- one `i64` scalar plus raw byte/word memory;
- byte I/O, character literals, and fixed-text emission.

Beta has no `if` or `while`; loops and branches are explicit state-graph edges.
That gives later compilers an Omega-like control representation while keeping
the implementation small.

## Implementation and meaning

Beta compiles structurally to Alpha assembly, which the Alpha assembler lowers
to a tape governed by Alpha's written semantics. The steady-state compiler is
`compiler/beta-lang/bc.beta`, written in Beta and self-hosted to a byte-identical
fixed point. The first compiler was cold-started by `beta-lang-rs`; the current
artifact still needs complete lower-rooted validation against `bc.beta`. A fixed
point proves deterministic dependency closure, not compiler correctness or
source correspondence. DDC is not an architectural closure mechanism.

The Alpha assembler formerly lived in `compiler/beta/`, but it is an Alpha tool:
it is written in Alpha assembly and translates Alpha assembly to Alpha tapes.
Its canonical owner is now `bootstrap/rungs/alpha/assembler/`; `compiler/beta`
is only a compatibility symlink. “Beta” without qualification means the
structured language compiled by `bc`.

## Must not contain

No algebraic data types, pattern matching, safe type hierarchy, ownership,
regions, effects, generics, or proofs. Those capabilities belong to Gamma,
Delta, or Omega. Beta remains a small compiler-construction substrate with raw
memory.

## Current repository reality

- `bootstrap/rungs/alpha/assembler/assembler.alpha` — self-hosting Alpha assembler;
- `compiler/beta-lang/bc.beta` — self-hosting Beta compiler;
- `compiler/beta-lang-rs/` — retained Rust cold-start/reference producer;
- `compiler/beta-lang-py/` — historical mixed Python reference/refinement tools;
- `compiler/beta-lang/CALLING_CONVENTION.md` — Beta's frame and register
  discipline over Alpha;
- `compiler/beta-lang/LANGUAGE.md` — current Beta surface.

`compiler/beta-lang/selfhost.sh` and `test.sh` gate the fixed point and language
behavior. The legacy Python compiler-comparison script is optional diagnostic
scaffolding, not a principal lattice gate.

## Implementation frontiers

- Guard the explicit data stack against overflow.
- Close the complete `bc.beta` source-to-artifact refinement edge with authority
  rooted below the compiler being checked.
- Extend resource budgets only when a higher-rung implementation demonstrates a
  concrete need; do not import higher-rung language machinery speculatively.

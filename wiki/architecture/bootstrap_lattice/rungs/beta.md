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

Beta's runtime meaning is fixed by its written small-step
[`SEMANTICS.md`](../../../../bootstrap/rungs/beta/SEMANTICS.md). Beta compiles
structurally to Alpha assembly, which the Alpha assembler lowers to a tape
governed by Alpha's written semantics. The steady-state compiler is
`bootstrap/rungs/beta/bc.beta`, written in Beta and self-hosted to a byte-identical
fixed point. The first compiler was cold-started by the disposable
`bootstrap/onramps/beta-rust/` producer. The current persisted artifact is
instead reconstructed by the Alpha-written cold-start compiler and contains no
Rust producer in its lineage. Its independent ROOT checker now establishes
complete lower-rooted maximal-observation correspondence against `bc.beta` for
the supported `B_bc1` profile. The fixed point still proves only deterministic
dependency closure; authority comes from that separate refinement check.

The Alpha assembler formerly lived in `compiler/beta/`, but it is an Alpha tool:
it is written in Alpha assembly and translates Alpha assembly to Alpha tapes.
Its canonical owner is now `bootstrap/rungs/alpha/assembler/`; the old entry is
retired. “Beta” without qualification means the
structured language compiled by `bc`.

## Must not contain

No algebraic data types, pattern matching, safe type hierarchy, ownership,
regions, effects, generics, or proofs. Gamma and Omega own the facilities they
specify; provisional Delta retains only the independent facilities its bridge
source justifies.
Beta remains a small compiler-construction substrate with raw memory.

## Current repository reality

- `bootstrap/rungs/alpha/assembler/assembler.alpha` — self-hosting Alpha assembler;
- `bootstrap/rungs/beta/bc.beta` — self-hosting Beta compiler;
- `bootstrap/onramps/beta-rust/` — retained Rust diagnostic/reference producer;
- `bootstrap/rungs/beta/reference/` — executable Python reference meaning,
  parser, and semantic fuzzing;
- `bootstrap/assurance/refinement/beta/` — symbolic/refinement reconstruction;
- `bootstrap/rungs/beta/CALLING_CONVENTION.md` — Beta's frame and register
  discipline over Alpha;
- `bootstrap/rungs/beta/LANGUAGE.md` — current Beta surface.
- `bootstrap/rungs/beta/SEMANTICS.md` — canonical small-step runtime meaning and
  maximal observations.

`bootstrap/rungs/beta/cold-start/full-source.sh`, `selfhost.sh`, and `test.sh`
gate reconstruction, the fixed point, and language behavior. The obsolete
Python backend and gate were removed because they added no unique semantic or
lower-rooted refinement coverage.

## Implementation frontiers

- Guard the explicit data stack against overflow.
- Keep the closed `bc.beta`/persisted-artifact ROOT refinement and its mutation
  teeth green when the compiler or `B_bc1` profile changes.
- Extend resource budgets only when a higher-rung implementation demonstrates a
  concrete need; do not import higher-rung language machinery speculatively.

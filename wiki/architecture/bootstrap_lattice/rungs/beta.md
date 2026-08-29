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
[`SEMANTICS.md`](../../../../source/beta/SEMANTICS.md). Beta compiles
structurally to Alpha assembly, which the Alpha assembler lowers to a tape
governed by Alpha's written semantics. The steady-state compiler is
`source/beta/compiler/bc.beta`, written in Beta and self-hosted to a byte-identical
fixed point. The persisted artifact is reconstructed by the Alpha-written
cold-start compiler and contains no external producer in its lineage. Its
adjacent validation tree reconstructs extensive maximal-observation obligations
against `bc.beta` for the supported `B_bc1` profile. The authoritative Beta
checker tape is constructed by the Alpha-written cold compiler below `bc`;
the full source/artifact refinement claim is not yet encoded in that checker's
calculus. The settled route uses Beta's existing small-step semantics as a
constructive total trace and proves symbolic synchronization, observation
agreement, and well-founded silent stuttering with the checker's existing
first-order induction rules. It adds no coinductive kernel judgment. The fixed
point proves only deterministic dependency closure.

The Alpha assembler formerly lived in `compiler/beta/`, but it is an Alpha tool:
it is written in Alpha assembly and translates Alpha assembly to Alpha tapes.
Its canonical owner is now `source/alpha/assembler/`; the old entry is
retired. “Beta” without qualification means the
structured language compiled by `bc`.

## Must not contain

No algebraic data types, pattern matching, safe type hierarchy, ownership,
regions, effects, generics, or proofs. Gamma and Omega own the facilities they
specify; provisional Delta retains only the independent facilities its compiler
source and direct product edge justify.
Beta remains a small compiler-construction substrate with raw memory.

## Current repository reality

- `source/alpha/assembler/assembler.alpha` — self-hosting Alpha assembler;
- `source/beta/compiler/bc.beta` — self-hosting Beta compiler;
- `source/beta/reference/` — untrusted executable Python semantic
  reference, parser, and fuzzing;
- `source/beta/compiler/validation/` — symbolic/refinement reconstruction;
- `source/beta/CALLING_CONVENTION.md` — Beta's frame and register
  discipline over Alpha;
- `source/beta/LANGUAGE.md` — current Beta surface.
- `source/beta/SEMANTICS.md` — canonical small-step runtime meaning and
  maximal observations.

`source/beta/compiler/cold-start/full-source.sh`,
`source/beta/compiler/validation/selfhost.sh`, and
`source/beta/test.sh`
gate reconstruction, the fixed point, and language behavior. The obsolete
Python backend and gate were removed because they added no unique semantic or
lower-rooted refinement coverage.

## Implementation frontiers

- Guard the explicit data stack against overflow.
- Keep the canonical `bc.beta`/persisted-artifact ROOT reconstruction green when
  the compiler or `B_bc1` profile changes. Encode its full constructive-trace
  and synchronization theorem in the rooted checker before declaring
  source/artifact admission closed; ROOT success remains differential evidence.
- Extend resource budgets only when a higher-rung implementation demonstrates a
  concrete need; do not import higher-rung language machinery speculatively.

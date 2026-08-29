# Bounded symbolic differential

This optional diagnostic targets exactly:

```text
beta_compiler.alpha -> beta_compiler_bytecode.tape
```

It compiles each Beta case with the canonical Alpha-written compiler, derives
one closed-form scalar term from the emitted Alpha tape and another from the
Beta source, and asks the rooted checker to validate equality of those two
terms. Eight deterministic small-input trials independently compare each term
with the Alpha and Beta executable references. A perturbed term must be
rejected for every fixed case.

This is not an admission certificate. The symbolic evaluators, references,
parser, and proof producer are untrusted, and only finite differential trials
connect their generated terms to the written semantics. The observation is the
first output byte, or the low process-status byte when no output exists; it does
not represent full output, typed failure, traps, resource outcomes, or
divergence. No result from this directory is a premise of the compiler lattice.

## Bounded coverage

The thirteen fixed cases retain shapes not covered cheaply by the focused
compiler suite:

| Cases | Distinct shape |
| --- | --- |
| `weighted`, `tri_down`, `neq`, `fromto` | symbolic trip counts, linear series, down-counting, exact-hit and monus guards |
| `tri_nested`, `callloop`, `temploop` | nested summaries, a call in a loop, and a rewritten temporary |
| `bytemem`, `weightedread` | byte-memory truncation and a coefficiented input-stream sum |
| `absdiff`, `boolval`, `condloop`, `divguard` | symbolic branches, stored comparisons, conditional deltas, and guarded division |

The default also runs one deterministic generated case from each of five shape
families: straight-line arithmetic, a counter loop, loop composition, nesting,
and branching. `BETA_DIFF_FUZZ`, `BETA_DIFF_LOOP`, `BETA_DIFF_COMPOSE`,
`BETA_DIFF_NESTED`, and `BETA_DIFF_BRANCH` may raise those counts for a manual
stress run; the repository default remains intentionally small.

Run from any working directory:

```sh
sh source/beta/compiler/validation/differential/test.sh
```

Delete this entire directory when the exact checked Alpha-source/tape theorem
covers these code-generation shapes, or earlier if compiler changes make its
two parallel symbolic recognizers uneconomical to maintain. Delete an
individual case as soon as a cheaper canonical compiler test or the exact proof
subsumes its listed failure-detection role.

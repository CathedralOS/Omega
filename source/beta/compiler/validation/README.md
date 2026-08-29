# Beta compiler validation

This owner now retains only machinery with a direct adaptation to the
Alpha-written canonical Beta compiler or a bounded, named diagnostic role.

## Canonical-edge candidates

- `admission/bc-artifact-structure.alpha` is a general Alpha-tape structural
  checker. Its wrapper accepts an explicit candidate tape; its no-argument
  default remains the migration artifact until `beta_compiler.alpha` is
  promoted.
- `admission/fol/` proves that the selected first-order checker can express
  non-lockstep traces, silent stuttering with a decreasing rank, cyclic
  execution, and the required negative controls. It contains no exact
  `bc.beta` proposition.

The former 193-module exact-`bc.beta` ROOT reconstruction, source/PC witnesses,
`B_bc1` profile, and resource-cutpoint proofs were deleted. They described the
wrong canonical source and could not be adapted without replacing their exact
subjects, tables, counts, procedure identities, and most propositions. Generic
decoding and proof patterns are small enough to reimplement against the actual
Alpha-written subject.

## Diagnostics retained conditionally

- `cold-start/rebuild-artifact.sh --check` is the sole temporary owner of the
  independent `bc.beta` fixed-point comparison while the migration artifact is
  still consumed. A duplicate validation wrapper was deleted.
- `stress/refinement.sh` constructs the Alpha-written compiler candidate and
  proves source/tape agreement for 100 curated and generated programs inside
  the explicitly modeled arithmetic, state-machine, memory, I/O, and bounded
  control fragment. The measured full gate is about two minutes on the
  development host. This is a bounded compiler-edge diagnostic, not proof of
  the unmodeled language or a bootstrap premise.

The historical-compiler surface wrapper, a repository-shape ownership test,
and a duplicate three-checker certificate diamond were deleted. Their distinct
cases moved to the direct compiler suite; checker implementation agreement is
already Alpha-checker-owned.

No command here is a default lattice edge until it names the exact
`beta_compiler.alpha` source and canonical tape. Artifact identity,
source-language correctness, and tape structure remain separate obligations.

Run the retained focused seams directly:

```sh
sh source/beta/compiler/validation/admission/fol/trace-refinement-seam.sh
sh source/beta/compiler/validation/stress/refinement.sh
```

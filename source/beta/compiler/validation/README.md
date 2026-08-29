# Beta compiler validation

This owner now retains only machinery with a direct adaptation to the
Alpha-written canonical Beta compiler or a bounded, named diagnostic role.

## Canonical-edge checks

- `admission/bc-artifact-structure.alpha` is a general Alpha-tape structural
  checker. Its wrapper accepts an explicit tape; its no-argument default is the
  canonical compiler artifact.
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

- `stress/refinement.sh` constructs the canonical Alpha-written compiler and
  proves source/tape agreement for 71 curated and deterministic generated programs inside
  the explicitly modeled arithmetic, state-machine, memory, I/O, and bounded
  control fragment. Deeper fuzz counts are explicit environment overrides;
  the measured default is 114 seconds on the development host and is not part
  of the fast suite. This is a compiler-edge diagnostic, not proof of
  the unmodeled language or a bootstrap premise.

The historical-compiler surface wrapper, a repository-shape ownership test,
and a duplicate three-checker certificate diamond were deleted. Their distinct
cases moved to the direct compiler suite; checker implementation agreement is
already Alpha-checker-owned.

Artifact identity, source-language correctness, and tape structure remain
separate obligations.

Run the retained focused seams directly:

```sh
sh source/beta/compiler/validation/admission/fol/trace-refinement-seam.sh
sh source/beta/compiler/validation/stress/refinement.sh
```

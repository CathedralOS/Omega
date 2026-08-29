# Ordinary-FOL trace refinement

This directory owns a generic checked proof-development seam for direct
source-to-Alpha-tape refinement. It uses the existing intuitionistic
first-order checker and natural-number induction; it adds no trusted transition
system or coinduction rule.

`trace-refinement.proof` establishes:

- a finite source step erased to zero Alpha steps with unchanged observation
  and a strictly decreasing rank;
- one source step synchronized with two Alpha steps and an explicit
  nondecreasing synchronization witness;
- a primitive-recursive two-state cycle whose indexed states remain running
  and observationally silent; and
- a finite-prefix invariant carrying an owner-selected payload through an exact
  trace from separately reconstructed base and successor premises.

The negative controls reject constant-rank one-sided stuttering, a non-silent
unmatched step, and a claimed successor relation with no successor premise.
The gate requires the rooted checker, independent reference checker, and Gamma
checker to agree and reports certificate size, checking time, and child memory.

Run from any working directory:

```sh
sh source/beta/compiler/validation/admission/fol/trace-refinement-seam.sh
```

This proves that the proof architecture is expressible. It does not admit a
compiler artifact; the exact source/tape owner must instantiate the generic
schemas and reconstruct all machine, profile, observation, and progress facts.

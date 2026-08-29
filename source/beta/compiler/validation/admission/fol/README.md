# Ordinary-FOL trace refinement

This directory owns the checked proof-development seam for
`BETA-COMPILER-FOL-REFINEMENT`.  It uses the existing intuitionistic
first-order checker and natural-number induction; it does not add a transition-
system or coinduction rule.

`trace-refinement.elab` establishes the first closed architecture cases:

- a finite source transition erased to zero Alpha steps, with unchanged
  observation and a strictly decreasing rank;
- one source transition synchronized with two Alpha transitions and an
  explicit nondecreasing synchronization witness; and
- a primitive-recursive two-state cycle whose every indexed state remains
  running and observationally silent.

`bc-main-resource-refinement.elab` is the 4,254-byte proof-authoring form for
the first owner-bound subject seam. It models four symbolic Alpha cleanup
stages against the single Beta resource return, carries an arbitrary sticky
resource identity to typed `Exhaust`, proves every stage silent, decreases
stage debt exactly, and makes terminal and Invalid states self-loop. Its final
proposition instantiates the five canonical structural resource origins.
The default `bc-block-control.sh` gate now runs an Alpha-owned ledger over the
canonical `bc.beta`, tape, and `B_bc1`, emits the declaration/goal prefix,
byte-compares that prefix with the elaborated candidate, appends only the proof
term, and requires the rooted checker to accept. Two controls demonstrate that
otherwise-valid certificates with swapped subject or profile identities fail
at the owner boundary. The remaining exact tranche must expand the four stages
into the raw instruction-by-instruction `next_alpha` relation; this seam does
not pretend that the stages are Alpha instructions.

It also checks a reusable induction lemma over opaque source/target trace
functions, an opaque synchronization function, and a binary symbolic relation
schema.  This is the generic proof-DAG boundary that exact `bc.beta` relation
schemas can cite later.

The negative directory pins three load-bearing failures: constant-rank
one-sided stuttering, a non-silent unmatched step, and attempting to derive a
successor relation from the base case alone.  The gate requires the rooted
checker, independent reference checker, and Gamma checker to agree on every
verdict and prints certificate bytes, checker time, and peak child storage.

Run from any working directory:

```sh
sh source/beta/compiler/validation/admission/fol/trace-refinement-seam.sh
```

This seam proves that the selected proof architecture is expressible and has
operational teeth.  It does **not** admit `bc.beta`: raw Beta/Alpha state
reconstruction, instruction-level cleanup, all `B_bc1` observations and
terminal classes, and the complete divergence theorem remain open.

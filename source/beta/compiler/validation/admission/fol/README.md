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
the first exact-subject tranche. It models the exact four-instruction Alpha
cleanup against the single Beta resource return, carries an arbitrary sticky
resource identity to typed `Exhaust`, proves every internal step silent,
decreases instruction debt exactly, and makes terminal and Invalid states
self-loop. Its final proposition instantiates the five canonical structural
resource origins. It remains a candidate until an Alpha-owned ledger emits the
same declarations and goal from the canonical `bc.beta`, tape, and `B_bc1`;
the proof source cannot bind those subjects by naming them itself.

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
operational teeth.  It does **not** admit `bc.beta`: exact Beta/Alpha state
reconstruction, all `B_bc1` observations and terminal classes, exact-subject
binding, and the complete divergence theorem remain open.

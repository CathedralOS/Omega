# Ordinary-FOL trace refinement

This directory owns the checked proof-development seam for
`BETA-COMPILER-FOL-REFINEMENT`.  It uses the existing intuitionistic
first-order checker and natural-number induction; it does not add a transition-
system or coinduction rule.

`trace-refinement.proof` establishes the first closed architecture cases:

- a finite source transition erased to zero Alpha steps, with unchanged
  observation and a strictly decreasing rank;
- one source transition synchronized with two Alpha transitions and an
  explicit nondecreasing synchronization witness; and
- a primitive-recursive two-state cycle whose every indexed state remains
  running and observationally silent; and
- a generic finite-prefix state-invariant theorem that carries an arbitrary
  owner-selected payload through an exact trace from separately supplied base
  and successor-preservation premises. For the `bc` reachability proof, that
  payload is the root return slot; this lemma does not supply the exact-machine
  premises itself.

`bc-main-resource-refinement.proof` is the 7,539-byte proof-authoring form for
the first owner-bound instruction cutpoint. Conditional on the cutpoint
relation carrying root return slot 39, it maps one Beta resource return through
seven Alpha controls at PCs 40251, 40261, 40264, 40267, 40270, 40273, and 39.
It carries an arbitrary sticky resource identity to typed `Exhaust`, proves
every running control silent, decreases debt from seven to zero one instruction
at a time, and makes terminal and Invalid states self-loop. Cross-machine or
malformed control tags route to Invalid. The synthetic epilogue at
40274..40283 is not the declared successor of the return cutpoint.

Subject, profile, and observation identities are arguments to the reducing
functions rather than disconnected propositions. The final goal instantiates
all five canonical structural resource origins through those indexed
functions. The default `bc-block-control.sh` gate runs a dedicated Alpha ledger
over canonical `bc.beta`, its tape, and `B_bc1`; the ledger directly builds the
resource join after exact main-shape reconstruction and does not execute the
ROOT GFP or maximal-observation path. It emits the declaration/goal prefix,
byte-compares that prefix with the elaborated candidate, appends only the proof
term, and requires the rooted checker to accept. Subject, profile, observation,
and dynamic return-successor mutations are rejected by the checker as well as
differing from the owner prefix; changed source and tape bundles are rejected
by the Alpha ledger itself.

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
operational teeth. It does **not** admit `bc.beta`: reachability of the resource
cutpoint—including preservation of the root return slot through earlier
calls—exact register/frame reconstruction, the other `B_bc1` cases, and the
complete divergence theorem remain open.

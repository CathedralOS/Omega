# Disjoint register-pressure regions

Both branches of `region_sum` retain 32 separately produced runtime sums before
consuming them. The regions share no virtual values and occupy different blocks
of one selected function, so recovery can collect both placement failures from
one attempt. Main executes both branches and checks 1120 and 3232 before exiting
70; a wrong result exits 71. The gallery remains the frozen performance witness.
This fixture pins behavior, not a compile-time threshold or parent capability.

Execution receipts are retained with the compile-time experiment. Golden rows
await publication of the repaired compiler, as required by the current
[stamp provenance rule](../../../../../tools/testing.md#corpus-gate). This
compiler-change observation must not be relabeled as a fixture-only base run.

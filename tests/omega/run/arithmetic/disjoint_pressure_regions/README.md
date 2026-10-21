# Disjoint register-pressure regions

Both branches of `region_sum` retain 32 separately produced runtime sums before
consuming them. The regions share no virtual values and occupy different blocks
of one selected function, so recovery can collect both placement failures from
one attempt. Main executes both branches and checks 1120 and 3232 before exiting
70; a wrong result exits 71. The gallery remains the frozen performance witness.
This fixture pins behavior, not a compile-time threshold or parent capability.

Recorded on Windows against published compiler `c1aa4ca6360b9b2ab18a238dfeccd53320fb3305`: source
checking succeeds, native exit is 70, and Terminal Psi exits 70 in agreement.
The checked, native and both-route golden rows preserve every unrelated row and
diagnostic byte. Other hosted targets are declared here but were not executed
in this qualification. The gallery performance receipts remain separate.

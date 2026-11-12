# Projected loans retain the constructed owner's claim

`omega run --both tests/omega/pass/borrows/projected_claim_owner_shared_observation_exit/main.omg`

A constructed linear owner lends its whole storage to a mutating helper, then
lends shared whole storage and an unrestricted member to observing helpers.
The observations remain 27 and 43. The later explicit drop discharges the
original owner's claim exactly once; none of the loans transfers it.

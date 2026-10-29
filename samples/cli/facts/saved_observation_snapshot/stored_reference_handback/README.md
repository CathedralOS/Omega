# Predicate handback through a stored reference

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, current record-field predicate
handback through stored references. This is another production of the existing
saved-observation fact. `Holder.items` stores an actual mutable reference to
`[7,1]`; `preserve` copies the referent into an independent owned array, writes
`copied[0]=4`, and writes that copied value to original element one. The earlier
saved observation is consumed by `agree` with its unchanged requirement before
runtime checks observe both original cells and the carrier neighbors.

The twin changes only the final store destination from index one to zero.
It must reject preserve's own Ensures. Formation or access refusal does not
establish that obligation. The first frozen source checks and its twin rejects preserve’s own Ensures on
optimized B0. The strengthened runtime oracle also reads the original backing
array after the stored reference’s last use. Its source and runtime outcomes
remain unverified; no compiler defect is claimed. Fixed base:
52929b02d410af19e925c30218b1b00626734041.

Initial witness: [local](../local/stored_reference_handback/main.omg),
[twin](../local/stored_reference_handback/invalid/main.omg). Remaining places,
source/native checks and pins stay open until baseline and implementation.

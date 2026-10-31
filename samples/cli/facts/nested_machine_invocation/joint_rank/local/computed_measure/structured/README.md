# Structured declared rank bodies

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.
This production keeps the call-measure row's inputs4→7/7→7, both selectors,
range0..=18446744073709551615, counter3→6 and caller-visible canaries. The
measure selects `rank(value)` below6 and `rank(value)-1` otherwise; the crossing
from5 to6 strictly decreases by2. Every twin changes only strict progress+1 to+0.

The [frozen smaller probe](../../../parameter/computed_measure/structured/probe/main.omg)
is byte-identical to the original baseline probe. All nine places retain their
existing transport shapes in `computed_measure/structured/`, with matching
`invalid/` twins.

Unfinished. Fixed base05812db44c356c4bb96427b4e8dd286a895f8128's optimized725bcd4f
producer refuses the frozen program at rank-range/source-complete witness
checking. The current core2 producer60401e60 passes formation and joint-cycle
checking, then refuses `Unit graph entry rank has pre-observation control`.
The entry-rank dominance guard remains: native lowering must receive a completed
rank at the cyclic block, rather than refer backward to a later SSA result.
No native/Terminal acceptance or corpus closure is claimed. Raw producers,
frozen source hashes and diagnostics are retained in `build/structured-rank/`.

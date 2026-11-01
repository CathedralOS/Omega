# C scalar-record layout

Fact: library C layout preserves scalar values with C ABI field offsets, trailing padding and transparent alias identity.

Parent: `LIBRARY-REPRESENTATION-FAMILIES` in [TASKS.md](../../../../TASKS.md).
The ordinary [core policy](../../../../source/library/core/c_layout.omg)
computes scalar placement from the target-resolved schema. It selects no compiler
format registration and grants no foreign or storage authority.

The root record has `u8`, `u64` and `u16` fields. Its C member offsets are 0, 8
and 16, with extent 24 and alignment 8. Values 17, 42 and 9 survive interchange
between `CLayout<Record>` and its complete `LayoutStorage` expansion. A shared
view of fully initialized, aligned `[u64; 3]` backing observes the same fields
at the independently authored offsets. We do not recast uninitialized record
padding to bytes. Size and alignment queries through the alias check the complete
extent and alignment. Every observed discrepancy traps.

`invalid/wrong_policy` substitutes a distinct named policy with identical geometry;
transparent aliases do not erase policy identity. The other controls reject a
nested native record, numbered fields and a sum. The current policy supports
complete nonempty, unnumbered scalar records only; it refuses repeated or nested
shapes, empty records and sum cases rather than claiming their native Omega
geometry is C ABI evidence. Ordinary scalar domains and access obligations remain
in force; policy geometry grants no atomic or concurrent access permission.

## Current evidence

Full root execution is pending the measured general alias-static-operand and
query scalar-retention repairs. The frozen source retains both size queries.
A separate query-free diagnostic executed on optimized Mac ARM64 producer
`363cb9e6a2200663ea536e700352df48167908a9`, image
`1c69c8ae906f093b759f4d0094a613f5105a12fd7a4f7a1c21895edc2a84a1b9`:
one positive plus three schema controls passed in 36 seconds. That diagnostic
is not qualification of the complete root program. Wrong-policy checking
separately rejected with the exact incompatible policy types.

The nine transport places, complete nested C records/platform migration,
other-host execution and the parent capability remain open. The installed
SDK15.5 arm64 oracle confirms Darwin stat 144/8 and timespec 16/8, including
nanoseconds and reserved fields; no platform source migration is claimed here.

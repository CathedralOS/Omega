# Linear opaque transport across an independent component

Parent: **NORMALIZED-ABI-LOWERING**.

A linear closed Inert opaque moves exactly once through independent consuming calls, preserving libc host numbers 66051 and 513 and replacement 1027 while retaining each disjoint sibling until its own consumption.

The producer explicitly declares the opaque `[linear]` and selects its complete
closed Inert representation. The caller consumes every created address through
the actual C-backed independent provider. The invalid twin omits only the
sibling consumption; it must reject the remaining linear obligation.

Published main `3353916f9430` checked the frozen root and ran native exit 0,
but Terminal rejected its linear constructor. Actual optimized producer
`99513d52bde0` now runs that unchanged root at native 0 / Terminal 0. The
unconsumed-sibling twin rejects at the same exact scope-exit diagnostic.

Nine transport places and their self-contained corpus pins are now authored.
Their execution and other-host qualification remain pending; this is not yet
a closed linear row.

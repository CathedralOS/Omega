# Fixed representation storage

Owning task: `LIBRARY-REPRESENTATION-FAMILIES`, named `FixedLayout<T>` and general `LayoutStorage<T, Policy>` applications.

Fact: An ordinary FixedLayout<Pair> application preserves the established Delivered fact on Pair.lo, so reading lo and subtracting 42u64 remains justified through each storage transport and execution uses the authored lo-at-8 representation.

Every place transports the actual `LayoutStorage<Pair, PairLayout>` value. The same `Pair`, policy, field operation and bound are used throughout. `Pair.lo` carries the declared `Delivered` domain. `make` establishes it by writing 42; each reject twin changes only that source write to 41. The consuming subtraction reads the carried fact directly. A shared byte view additionally observes lo at offset 8 and hi at offset 0, distinguishing the authored geometry from ordinary record packing.

All nine positive programs bind five hosted entry targets and promise exit 0. Runtime acceptance and fixture pins are pending. The const-name place retains the aggregate representation value itself.

Remaining acceptance: baseline receipt, all nine native/Terminal observations, intended rejection diagnostics, pinned fixture pairs, generic/lifetime/ownership controls, source migration and exact-candidate review. No execution result is claimed yet.

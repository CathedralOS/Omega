# Core Mutable Slice Traversal

`omega::language::core::iteration` supplies the ordinary checked `for_each`
machine and the `ElementStep<Element>::apply` callback requirement. The callback
must terminate and remain service-free, nonblocking, nonsuspending and noncrashing.

The positive witness toggles and updates three records, visits an empty prefix
without changing those observations, then forwards a named mutable view of two
non-copy records through a different static callback. Native and Terminal
execution must both exit 0. All four hosted entries are declared; runtime
evidence belongs only to the executing host.

The callback mutability, reach and termination controls under
`tests/omega/fail/generics/core_slice_for_each_callback_*` must reject for their
own obligations. The compiler repair preserves collection-content writes apart
from descriptor writes in the shared complete frame algebra; opaque writes and
actual descriptor replacement still invalidate ranking premises.
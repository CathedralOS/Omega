# Default-domain field write

A field write that preserves `Interval`'s default domain keeps `self.hi <= 4`,
so `4 - hi` is exact.

Parent: `DOMAIN-DEFAULT-PREDICATE-UNIFICATION` in
[TASKS.md](../../../../../TASKS.md). This place is the write of that
predicate: the same disjunction that construction proves must hold again
after a field store, or the later exact use must reject.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |

`local` binds the four hosted targets and exits 0 on `omega run --both`.
The twin changes only `span.hi = 3` to `span.hi = 9`.

The other eight transport places are still open.

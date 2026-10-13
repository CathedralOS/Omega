# domain_axis_cast

Parameterized domain on a scalar carrier: `domain<const Axis: u64> i64::Coord<Axis>` — each instantiation is a distinct nominal domain. `value as i64 in Coord<7>` asserts the domain explicitly; `as i64` strips it back to the carrier at the boundary.

## Learnings

- Domain parameters are const-generic — `Coord<7>`/`Coord<9>` are different domains over `i64`.
- `match` arms can hold call results: `match flag { true -> mark(a), false -> mark(b) }` yields the call's value, still domain-typed until stripped.

Compile-checked only: run a check target, not native.

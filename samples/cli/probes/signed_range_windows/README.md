# Signed Range Windows

Compile-scope probe: signed `i64` index and range bounds discharged
from `requires` rosters on free machines, exclusive and inclusive range
subslices, a subslice view of a view, and the named numeric-conversion
machines. One core import (`numeric_conversion`); no standard-library
dependency.

- `head`/`window_len` carry `requires` rosters with `&&` chains and
  `.len` bounds — `0 <= index && index < items.len` proven from literal
  call sites.
- `items[start..end]` exclusive subslice and `view[0..=1]` inclusive
  subslice on a view of a view.
- `narrow_u64_to_i32_wrapping`/`widen_u8_to_i32` named conversion
  machines cross `u64`/`u8` -> `i32`; `as T in D` only re-qualifies
  within the same carrier width.

Probe learnings recorded here: transition-arm results are expressions
(assignments do not parse), and a user `domain [u8]::D requires m(...)`
cannot be granted from ordinary code — literal construction, type
annotation, subslice inheritance, and a `boundary operator` with
`ensures result in D` all fail to produce `x in D` for a caller
(`"cannot prove requires contract"`); routed slice-domain grants stay a
gap.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/signed_range_windows/main.omg
mbx run -p omega -- update --project samples/cli/probes/signed_range_windows
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

# Signed Range Windows

The program uses signed `i64` indices to read and slice a byte array. It checks that the selected byte and the measured lengths add up to 55. A different total traps.

- `head`/`window_len` carry `requires` rosters with `&&` chains and
  `.length` bounds — `0 <= index && index < items.length` proven from literal
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

Run the native and Psi checks with `mbx xtask sample-gate --filter samples/cli/probes/signed_range_windows`.

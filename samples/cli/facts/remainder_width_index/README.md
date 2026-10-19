# Remainder width index

The remainder `value % width` is below `width`, so when `width` is proved
equal to a view's length it indexes that view.

The operation is `buckets[value % width]` on a four-byte view whose length is
`width`. For `value == 10` the remainder is 2 and the byte is 9; for
`value == 15` it is 3 and the byte is 6. Each place copies that sentence and
performs that selection; only the transport of the width relation changes.

| Place | Positive | Invalid | Width transport |
| --- | --- | --- | --- |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) | a local copy of the view's length |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) | a record field whose contract equals the length |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) | an explicit parameter required to equal the length |
| call result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) | a call whose `ensures` equates its result with the length through the result carrier |
| match join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) | both arms denote the same length, and both arms run |
| const name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) | a named width equal to the length |
| another state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) | a successor state receives the width relation |
| another machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) | a forwarding machine supplies the width |
| another target body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) | each of four target bodies obtains the width from the declared measurement |

The root application and each place bind `ProgramEntry` for all four hosted
targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, `linux_arm64`) and
exit 0 under both Terminal Psi and native execution:

```sh
omega run --both samples/cli/facts/remainder_width_index/main.omg
omega run --both samples/cli/facts/remainder_width_index/local/main.omg
omega run --both samples/cli/facts/remainder_width_index/field/main.omg
omega run --both samples/cli/facts/remainder_width_index/parameter/main.omg
omega run --both samples/cli/facts/remainder_width_index/call_result/main.omg
omega run --both samples/cli/facts/remainder_width_index/match_join/main.omg
omega run --both samples/cli/facts/remainder_width_index/const_name/main.omg
omega run --both samples/cli/facts/remainder_width_index/other_state/main.omg
omega run --both samples/cli/facts/remainder_width_index/other_machine/main.omg
omega run --both samples/cli/facts/remainder_width_index/target_body/main.omg
```

Each place has an `invalid/` twin whose width is one greater than the view
length, so the remainder can equal the length and the selection must reject
with `cannot prove index ... is within unknown slice length`. Check a twin with
`omega --check <place>/invalid/main.omg`.

The call-result place drove a checking repair: a declared measurement written
as `ensures result == buckets.len as u64` states the same length through an
admitted Exact cast, and the measurement contract now recognizes the length
member under that cast. Without the repair the call-result place, the
measurement in each target body, and every width that flows from such a
measurement were refused. A non-Exact cast is not peeled:
`tests/omega/fail/collections/contract_length_wrapping_result_cast_rejected`
keeps rejecting the same selection. The row is pinned by
`tests/omega/{pass,fail}/facts/remainder_width_through_*` fixtures.

## Requirements and Evidence

Owning task: **CONSERVATION-CONTRACT** (`TASKS.md`) under
[numeric values](../../../../wiki/spec/language/numeric_values.md#division-and-remainder)
and [counts, indices, and addresses](../../../../wiki/spec/language/counts_and_addresses.md#bounds-and-lowering).
This row establishes that an unsigned remainder `value % width` with
`buckets.len > 0` and `width == buckets.len as u64` discharges the strict index
bound `0 <= value % width < buckets.len` across all nine places without post-hoc
caller guards.

- **Establishment:** `buckets.len > 0` guarantees a nonzero divisor `width > 0`
  and establishes `value % width < width`; combined with the exact equality
  `width == buckets.len as u64` from local assignment, field/parameter/state
  contracts, call `ensures`, match arms, or `const WIDTH`, the slice indexing
  obligation `value % width < buckets.len` is discharged directly at
  `buckets[value % width]`.
- **Identity and lifetime:** the fact tracks the exact slice descriptor length
  `buckets.len` and the `u64` width carrier (`width`, `window.width`, or
  `WIDTH`) across local bindings, record fields, call boundaries, and state
  transitions while `buckets` remains borrowed and unmodified.
- **Composition:** calls substitute exact actuals (`measure(buckets)` and
  `read_at(value, buckets, ...)`), `match_join` retains the length equality
  only when established on both `true` and `false` arms (both executed at
  runtime), and `other_state` proves the arrival contract on `land` before
  indexing. Each `<place>/invalid/main.omg` differs from `<place>/main.omg`
  only in widening the width relation to `buckets.len as u64 + 1` (and `5` at
  call sites where required by the widened contract).
- **Soundness boundaries:** every `invalid/` twin and corresponding
  `tests/omega/fail/facts/remainder_width_through_*_rejected` fixture rejects at
  `buckets[value % ...]` with `cannot prove index ... is within unknown slice length`,
  and non-Exact casts remain rejected by
  `tests/omega/fail/collections/contract_length_wrapping_result_cast_rejected`.
- **Terminal Psi and carrier lowering:** exact `u64` remainder and borrowed
  slice indexing lower into Terminal Psi preserving the borrowed `&[u8]` view
  descriptor and unsigned remainder evaluation across all nine places.
- **Native realization, frame/layout encoding, and image emission:** root
  `main.omg` and all nine positive places bind `ProgramEntry` for
  `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`, declare
  `// Expected exit: 0`, and pass `omega run --both` with native and Terminal
  Psi execution agreement (`exit 0`). What remains open: symbolic non-equality
  divisor upper bounds (`width <= buckets.len as u64` with `width > 0`) and
  signed remainder indexing bounds.

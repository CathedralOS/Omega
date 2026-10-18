# Remainder width index

The remainder `value % width` is below `width`, so when `width` is proved
equal to a view's length it indexes that view.

The operation is `buckets[value % width]` on a four-byte view whose length is
`width`. For `value == 10` the remainder is 2 and the byte is 9; for
`value == 15` it is 3 and the byte is 6. Each place copies that sentence and
performs that selection; only the transport of the width relation changes.

| Place | Project | Width transport |
| --- | --- | --- |
| local | `local/` | a local copy of the view's length |
| field | `field/` | a record field whose contract equals the length |
| parameter | `parameter/` | an explicit parameter required to equal the length |
| call result | `call_result/` | a call whose `ensures` equates its result with the length through the result carrier |
| match join | `match_join/` | both arms denote the same length, and both arms run |
| const name | `const_name/` | a named width equal to the length |
| another state | `other_state/` | a successor state receives the width relation |
| another machine | `other_machine/` | a forwarding machine supplies the width |
| another target body | `target_body/` | each of four target bodies obtains the width from the declared measurement |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/remainder_width_index/local/main.omg
omega run samples/cli/facts/remainder_width_index/field/main.omg
omega run samples/cli/facts/remainder_width_index/parameter/main.omg
omega run samples/cli/facts/remainder_width_index/call_result/main.omg
omega run samples/cli/facts/remainder_width_index/match_join/main.omg
omega run samples/cli/facts/remainder_width_index/const_name/main.omg
omega run samples/cli/facts/remainder_width_index/other_state/main.omg
omega run samples/cli/facts/remainder_width_index/other_machine/main.omg
omega run samples/cli/facts/remainder_width_index/target_body/main.omg
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

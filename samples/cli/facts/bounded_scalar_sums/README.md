# Bounded Scalar Sums

Compile-scope fact: a `u64 [a..=b]` bound on a parameter, result, or
record field proves an exact sum cannot overflow. The body stays a
plain `a + b`, with no Wrapping domain and no `as` widening. There is
no ProcessExit provider and no runtime exit oracle.

The original project at this folder's root mixes parameter bounds,
bounded record fields, and a bounded field receiving the call result.
The nine places below each carry that same sum fact alone:

| Place | Project |
| --- | --- |
| local | `local/` |
| field | `field/` |
| parameter | `parameter/` |
| call result | `call_result/` |
| match join | `match_join/` |
| const name | `const_name/` |
| another state | `other_state/` |
| another machine | `other_machine/` |
| another target body | `target_body/` |

Each `invalid/` child keeps the same place and widens the declared
bounds to every `u64` value, so the exact sum is refused.

```sh
omega --check samples/cli/facts/bounded_scalar_sums/local/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/field/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/parameter/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/call_result/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/match_join/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/const_name/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/other_state/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/other_machine/main.omg
omega --check samples/cli/facts/bounded_scalar_sums/target_body/main.omg
```

`omega --check` on each `invalid/main.omg` exits 1 and reports that the
exact sum may overflow `u64`.

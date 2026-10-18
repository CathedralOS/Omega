# Fixed array value carrier and in-bounds index preservation

A fixed-extent array `[u64; 2]` preserves its in-bounds index contract `index < 2`
and independent element values across all nine places.

Owning Task: `CONSERVATION-CONTRACT` / `TERMINAL-CONTENT-CLAIMS`
Reserved Claim Ticket: `89def26c09b74a34a50d8ab7487e3b39`

## Requirements and Evidence

- **Establishment**: Fixed-extent array `[u64; 2]` construction `[19, 42]` and entry precondition
  `requires index < 2` (or record constraint `where index < 2` / named constant `LIMIT = 2`) establish
  proven in-bounds access before indexing.
- **Identity and Lifetime**: The independent element values `values[0] == 19` and `values[1] == 42` are
  conserved across all value transfers and transitions.
- **Composition**: Machine calls, pattern match joins, and state transitions preserve exact preconditions
  and actual values without narrowing or corruption.
- **Soundness Boundaries**: Weakened preconditions (`index <= 2` or widened limits) are rejected at
  compile time with `cannot prove index ... is within length 2`, proving that out-of-bounds indexing is
  strictly prevented across all nine places.

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | The array and bounded index are copied into locals before indexing. |
| field | `field/` | A record field carries the bounded index alongside the fixed-extent array. |
| parameter | `parameter/` | The reading machine receives the bounded index and array directly as parameters. |
| call result | `call_result/` | The index arrives from a helper call with an ensures result < 2 contract. |
| match join | `match_join/` | Both branches of a match expression produce in-bounds indices. |
| const name | `const_name/` | A named constant bounds the index contract for the fixed-extent array. |
| other state | `other_state/` | A transition to a successor state forwards the bounded index and array. |
| other machine | `other_machine/` | A forwarding machine transports the bounded index and array to the reader. |
| target body | `target_body/` | Each of the four target bodies reads the array element using the bounded index. |

Each place is an application that exits 0:

```sh
omega run samples/cli/facts/fixed_array_value/local/main.omg
omega run samples/cli/facts/fixed_array_value/field/main.omg
omega run samples/cli/facts/fixed_array_value/parameter/main.omg
omega run samples/cli/facts/fixed_array_value/call_result/main.omg
omega run samples/cli/facts/fixed_array_value/match_join/main.omg
omega run samples/cli/facts/fixed_array_value/const_name/main.omg
omega run samples/cli/facts/fixed_array_value/other_state/main.omg
omega run samples/cli/facts/fixed_array_value/other_machine/main.omg
omega run samples/cli/facts/fixed_array_value/target_body/main.omg
```

Each place has an `invalid/` twin that weakens the index bound (e.g. to `index <= 2` or `LIMIT = 3`),
so the array index access is no longer proved in bounds. Every twin rejects at compile time with
`cannot prove index ... is within length 2`. Check a twin with `omega --check <place>/invalid/main.omg`.

The row is pinned by `tests/omega/{pass,fail}/facts/fixed_array_value_through_*` fixtures; every
positive records native exit 0.

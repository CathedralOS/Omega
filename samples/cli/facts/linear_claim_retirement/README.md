# Linear claim retirement

A linear Token [linear] carries its exact payload across the place and retires through its explicit consuming machine on every normal exit.

## Owning requirement and capability

- **Parent task**: `STATIC-MACHINE-MIGRATION` (claim ticket: `339ebefd000346a4915c9689f8350b11`)
- **Requirement**: Explicit linear claim transport and terminal retirement through static consuming machines across all nine language transport places, ensuring exact-once consumption without implicit discard or duplicate consumption.

## The nine transport places

| Place | Positive witness | Reject twin | Transport |
| --- | --- | --- | --- |
| `local` | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | A local binding carries the linear `Token` before retirement. |
| `field` | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | An enclosing record field carries the linear `Token` before retirement. |
| `parameter` | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | A parameter carries the linear `Token` into a consuming machine. |
| `call_result` | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | A call result yields the linear `Token` before retirement. |
| `match_join` | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | Both branches yield the linear `Token` into retirement. |
| `const_name` | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | A named constant `const EXPECTED: u64 = 42` supplies the payload carried by the linear `Token` before retirement. |
| `other_state` | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | A successor state receives the linear `Token` before retirement. |
| `other_machine` | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | A separate forwarding machine carries the linear `Token` before retirement. |
| `target_body` | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | Each target body retires the linear `Token`. |

## Execution and validation

Each place is an application that exits 0 with native and Terminal execution agreement:

```sh
omega run --both samples/cli/facts/linear_claim_retirement/local/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/field/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/parameter/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/call_result/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/match_join/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/const_name/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/other_state/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/other_machine/main.omg
omega run --both samples/cli/facts/linear_claim_retirement/target_body/main.omg
```

Each place has an `invalid/` twin that breaks only the linear obligation by consuming the transported `Token` a second time after retirement. The control must reject with `after ... was transferred or consumed`. Check a twin with:

```sh
omega --check samples/cli/facts/linear_claim_retirement/<place>/invalid/main.omg
```

## Requirements and evidence accounting

In accordance with `AGENTS.md#requirements-and-evidence`:
- **Establishment**: Explicit construction of `Token { value: 42 }` (or `EXPECTED` in `const_name`) establishes the linear obligation.
- **Identity and lifetime / invalidation**: Linear values track exact unique ownership; consuming the `Token` transfers it to `retire`, which consumes it via explicit `drop(token)` and invalidates the source place.
- **Composition**: Across calls, record fields, function parameters, match branches, successor states, forwarding machines, and target-scoped bodies, linear conservation holds. Reconverging branches agree on live ownership frontiers.
- **Soundness boundaries**: The reject twins demonstrate that attempting duplicate consumption after retirement is soundly refused with a precise diagnostic naming the consumed place.
- **Acceptance status**: This completed 9-place fact row proves transport and explicit retirement of linear claims across all carrier places under static machine semantics. Remaining open in `STATIC-MACHINE-MIGRATION` are the legacy `Binding<R>` receiver carrier removals, memory canaries, and static provider slot demands.

The row is pinned by `tests/omega/{pass,fail}/facts/linear_claim_retirement_through_*` fixtures; every positive records native exit 0.

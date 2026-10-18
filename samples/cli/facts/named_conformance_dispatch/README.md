# Named conformance dispatch

Parent task: `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`

## Governing requirement

Dispatching inline conformance members of a named implementation satisfying a generic trait application (`Selected satisfies Arithmetic<i32>`), preserving exact member contracts, bounds, and values across all nine places.

**A named implementation Selected satisfies Arithmetic<i32> dispatches its inline combine member on explicit i32 operands and preserves the bounded result across all nine places.**

Every place performs the same operation: dispatch the inline `combine` member of named implementation `Selected satisfies Arithmetic<i32>` through generic helper `apply<Selected>(20, 22)` under precondition bounds `0 <= left && left <= 50` and `0 <= right && right <= 50`, returning the exact sum under `ensures result == left + right`. The positive oracle asserts `result == 42` and exits 0 only when the bound and value hold. Only the transport of the operands changes.

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| local | `local/` | both bounded parameters are copied into locals before dispatching combine | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | bounded record fields carry both operands into the combine dispatch | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | the dispatching machine reads its two bounded parameters directly | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | both operands arrive from calls with declared result ranges | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both arms keep the operands inside their bounds for combine dispatch | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named ceiling bounds both operands into the combine dispatch | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | a successor state receives both bounded operands and dispatches combine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | a forwarding machine transports both bounded operands to the combine dispatch | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each of four target bodies dispatches combine on the bounded operands | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

### Verification commands

Each place is an application that exits 0 in both native and Terminal execution:

```sh
omega run --both samples/cli/facts/named_conformance_dispatch/local/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/field/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/parameter/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/call_result/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/match_join/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/const_name/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/other_state/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/other_machine/main.omg
omega run --both samples/cli/facts/named_conformance_dispatch/target_body/main.omg
```

Each place has an `invalid/` twin whose transported right operand lacks the upper bound `<= 50` required by `combine`. The control must reject with `cannot prove requires contract for call apply`. Check each twin with:

```sh
omega --check samples/cli/facts/named_conformance_dispatch/local/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/field/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/parameter/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/call_result/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/match_join/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/const_name/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/other_state/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/other_machine/invalid/main.omg
omega --check samples/cli/facts/named_conformance_dispatch/target_body/invalid/main.omg
```

### Requirements and evidence

- **Establishment:** The named conformance `Selected satisfies Arithmetic<i32>` defines inline member `combine` with contract `requires 0 <= left && left <= 50 && 0 <= right && right <= 50 ensures result == left + right`. Generic caller `apply<Operations satisfies Arithmetic<i32>>` establishes conformance satisfaction through its evidence parameter.
- **Identity and lifetime:** `Selected` has package-scoped conformance identity and carries no data subject or `Self` carrier. The explicit scalar arguments `20` and `22` are immutable values transported without aliasing or storage mutation.
- **Composition:** Conformance member call `Operations::combine(left, right)` strictly checks preconditions at call sites and rejoins its guaranteed equality `result == left + right` at every place.
- **Soundness boundaries:** Removing any operand precondition in the caller causes `omega --check` to reject the dispatch before realization, proving that unproved requirements cannot bypass conformance boundaries.

### Acceptance demonstrated and remaining scope

This row demonstrates exact conformance dispatch and contract preservation across all 9 standard places for static named implementations satisfying generic trait applications with inline members.
Remaining open under `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`: dynamic trait object dispatch (`dyn Trait`), cross-package visibility export of named conformances, and complex mixed lifetime telescopes.

The row is pinned by `tests/omega/{pass,fail}/facts/named_conformance_dispatch_through_*` fixtures; every positive records native exit 0.

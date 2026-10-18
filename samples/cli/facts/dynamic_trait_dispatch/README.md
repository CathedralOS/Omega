# Dynamic trait dispatch

Parent task: `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`
Ticket: `e5f8ec0912314fa78f865ec38c7c5b2d`

## Governing requirement

Dispatching inline conformance members through a runtime `dyn` trait selector over explicit data arguments without an implicit `Self` or receiver attachment, preserving exact contracts, bounds, and values across all nine places.

**Fact: a runtime dyn trait selector dispatches an inline conformance member over explicit data arguments to establish 100u64 when subtracting 100u64.**

Every place performs the same operation: dispatch the inline `combine` member of named implementation `UnitCombine satisfies WeightedCombine` via a runtime trait selector `selector: dyn WeightedCombine = UnitCombine as dyn WeightedCombine;` over explicit `u64` arguments `(60u64, 40u64)` under precondition bounds `left >= 60u64 && left <= 60u64` and `right >= 40u64 && right <= 40u64`, returning the sum under postcondition guarantee `ensures result >= 100u64 && result <= 100u64`. Exact subtraction `total - 100u64` evaluates to 0, establishing exit code 0 under native and Terminal Psi execution. Only the transport of the `dyn` selector or dispatched result changes across the nine places.

| Place | Project | Transport | Positive Witness | Reject Twin |
| --- | --- | --- | --- | --- |
| local | `local/` | the dyn trait selector is bound to a local before dispatching combine | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | `field/` | the dyn trait selector is stored in a record field before dispatching combine | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | `parameter/` | the dispatching probe reads its dyn trait selector as an explicit parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | `call_result/` | the dyn trait selector is obtained from a call result before dispatching combine | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | `match_join/` | both arms of a match join yield the dyn trait selector before dispatching combine | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | `const_name/` | a named constant defines the exact target value for subtraction | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | `other_state/` | a successor state receives the dyn trait selector and bounded operands to dispatch combine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | `other_machine/` | a forwarding machine transports the dyn trait selector and bounded operands to dispatch combine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | `target_body/` | each hosted target body dispatches combine through the dyn trait selector | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

### Verification commands

Each positive place is an application that exits 0 in both native and Terminal Psi execution:

```sh
omega run --both samples/cli/facts/dynamic_trait_dispatch/local/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/field/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/parameter/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/call_result/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/match_join/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/const_name/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/other_state/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/other_machine/main.omg
omega run --both samples/cli/facts/dynamic_trait_dispatch/target_body/main.omg
```

Each place has an `invalid/` twin whose transported right operand violates the combine precondition contract `right >= 40u64 && right <= 40u64`. The invalid control must reject with `cannot prove requires contract for call dyn WeightedCombine::combine`. Check each twin with:

```sh
omega --check samples/cli/facts/dynamic_trait_dispatch/local/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/field/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/parameter/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/call_result/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/match_join/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/const_name/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/other_state/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/other_machine/invalid/main.omg
omega --check samples/cli/facts/dynamic_trait_dispatch/target_body/invalid/main.omg
```

## Requirements and Evidence

1. **Establishment:** The trait `WeightedCombine` defines inline member `combine(left: u64, right: u64) -> u64` with precondition `requires left >= 60u64 && left <= 60u64 && right >= 40u64 && right <= 40u64` and postcondition `ensures result >= 100u64 && result <= 100u64`. Named implementation `UnitCombine satisfies WeightedCombine` establishes conformance satisfaction through its verified inline body. Coercion `UnitCombine as dyn WeightedCombine` establishes a runtime dynamic trait selector value citing `UnitCombine`'s closed requirement map.
2. **Identity and lifetime:** The dynamic selector `dyn WeightedCombine` carries package-scoped implementation selection identity without a data subject, `Self` carrier, receiver binding, or hidden allocation. The explicitly passed operands `60u64` and `40u64` are immutable scalar values transported independently with distinct lifetimes and disjoint value facts.
3. **Composition:** Dynamic calls `WeightedCombine::combine(selector, left, right)` substitute exact actuals, verify preconditions strictly at every call site, and rejoin the guaranteed postcondition `result >= 100u64 && result <= 100u64` on every incoming path across all nine places, proving that exact subtraction `total - 100u64` equals 0 without arithmetic underflow.
4. **Soundness boundaries:** Removing or weakening the operand bounds in each place's invalid twin causes `omega --check` to reject before native realization, confirming that unproved precondition obligations cannot bypass dynamic dispatch call boundaries.
5. **Terminal Psi and carrier lowering:** Coercion `UnitCombine as dyn WeightedCombine` lowers to `SelectImplementation`, producing an explicit implementation table reference. Dynamic invocation `WeightedCombine::combine(selector, a, b)` lowers directly to `CallDynamic`, keeping the selector operand distinct from explicit data arguments. There are no synthetic receiver parameters or existential type packings.
6. **Native realization, frame/layout encoding, and image emission:** Immutable dispatch tables are emitted with symbol relocations for `UnitCombine::combine`. Dynamic call lowering resolves table addresses, performs selected-slot indirect calls, and executes with exit 0 in both Terminal Psi and native code (`omega run --both`) across all four hosted targets (`windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`).

### Acceptance demonstrated and remaining scope

This row demonstrates dynamic trait object dispatch (`dyn Trait`) and contract preservation across all nine standard places for named implementations with inline members over explicit data arguments.
Parent outcome: `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION` (`TASKS.md`).
Ticket: `e5f8ec0912314fa78f865ec38c7c5b2d`.
Remaining open scope under `NAMED-IMPLEMENTATION-CONFORMANCE-MIGRATION`: multi-method dynamic vtable dispatch, finite generic method families with canonical value tuples, and cross-package dynamic conformance visibility export.

The row is pinned by `tests/omega/pass/facts/dynamic_trait_dispatch_through_*_exit` and `tests/omega/fail/facts/dynamic_trait_dispatch_unproved_*` fixtures; every positive records native exit 0.

# Literal calls to a declared generic family

`Family::row<16>` selects one declared generic row through a runtime
implementation selector. The method permits only 16 or 32. The compiler must
keep that complete list when it closes the call's argument to 16; 64 must still
reject. A runtime value cannot silently become a const argument.

Owning tasks: **BUILD-OWNED-TARGET-SELECTION**, replacing target prefixes with
ordinary code selected in Build; and **FINITE-GENERIC-DISPATCH**, qualifying
const-only forwarding, in [TASKS.md](../../../../TASKS.md).

Fact: A literal selection of a const-only finite-family method keeps its declaring roster and returns the selected row's value.

`Selected` returns the chosen Width. `Other` returns 17 for Width 16 and 33 for
Width 32. Both satisfy the same interface, which promises a u32 result and
publishes no result-equals-Width condition. Every positive carries both
implementations through its own place and checks 16 and 17 after the same
literal `<16>` call. The match runs both choices; selecting the wrong arm,
implementation or row makes the result check fail. The field place also checks
its surrounding 17/23 values.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Selected target code | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each paired invalid program differs only in the literal tuple: `<16>` becomes
`<64>`. The implementation bodies, transport, result checks and declared 16/32
list stay identical. The separate [runtime-const control](runtime_const/invalid/main.omg)
keeps its original source: proving a runtime input belongs to 16/32 does not
make it a compile-time argument.

The target place passes its selector as ordinary data to `FamilyForward`.
One checked `HostedFamily` implementation forwards the same call for every
platform; four explicit Build rows select it. The requirement declares empty
service reach, and the caller records its direct entry with `invokes`.
Every positive expects exit 0 and binds all four hosted targets.

## Requirements

- The original declaring method owns the complete 16/32 list. Closing one call
  must retain that method identity even after its parameter and equalities fold.
- Each selector keeps its exact named implementation and row. Distinct values
  from the two legal implementations make that identity observable.
- Locals, fields, parameters, calls, joins, constants, nested machines and Build
  selections carry the same literal call. A caller comparison observes its
  result; the declared list justifies forming the call before that comparison.
- Const arguments remain static. Membership proofs, ordinary u32 storage and
  implementation choice grant no extra access or type-packing mechanism.
- Portable lowering retains the selected implementation, complete family and
  tuple. The interpreter must agree with the native result.
- Provider selection, calling conventions, register/stack storage and final
  encoding preserve the code selector and u32 result. Actual execution is
  required; successful checking alone does not establish these obligations.

The governing rules are [finite specialization](../../../../wiki/spec/language/generics.md#finite-specialization-boundary),
[dynamic method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families)
and [Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

## Observations and remaining work

The unchanged executable sources at `e9cec2710f5f0f025947307c06ed9f0a13926c03`
pass all nine positive fixture checks and native/Terminal executions on Linux
x86-64. All ten invalid fixtures and projects reject as intended, including the
strict-const control. All nineteen public projects meet their expected results:
[baseline run 41559023368](https://github.com/CathedralOS/Omega/actions/runs/41559023368).
The checkpoint changes only this README's title/link from the fixed published
base. These results establish existing behavior, not a new compiler repair.
The strengthened examples and target migration pass
[run 41560780946](https://github.com/CathedralOS/Omega/actions/runs/41560780946),
source `74bbe3c7f0f173fe5063b15d085e810758a215be`. All nine positive fixtures
check and run with native/Terminal exit 0. All ten invalid fixtures reject for
their intended causes. All nineteen public projects meet their expected results
(nine executions and ten intended rejections). The optimized producer has
verified level 1; recorded inputs and outcomes retain that measured source.

Other-host execution and the broader parent tasks remain open. Remaining finite
family requirements include borrowed descriptor results, entry content claims
and other target bridges; this scalar row does not close those obligations.

The `target_body` place already selects `HostedFamily` through Build rows; it
now also binds `macos_x86_64` with its own row, so this host executes it. On
macOS x86-64 the positive exits 0 under `omega run --both` and the twin still
rejects with `requires exactly one closed roster tuple`. With a single
implementation there is no wrong row to detect, so no tag check is added.

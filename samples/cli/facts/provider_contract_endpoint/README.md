# Provider-dependent scalar contract endpoint

The selected provider makes `limit()` equal to 7. The input contract
`requires value <= limit()` preserves the bound needed to prove `7u64 - value`.

This row tests bounds computed by a selected provider and written in an ordinary
`requires` clause. It belongs to **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. No named
domain is needed solely to express that bound.

The [generic-function production](closed_owner/README.md) carries the same fact
when the selected provider call supplies constants inside a function that still
has its own generic parameter. It also checks both match outcomes and field
neighbors at runtime.

The [pending-call production](pending_call/README.md) keeps that same bound when
the helper first makes a call with its own requirement. Its twins require that
same pending helper result to admit the call. Its README retains that
production's separate qualification records.

The nine programs linked in the table below now make `limit()` admit input 1 against a calculated
prerequisite. The matching branch supplies anonymous `254 + 1 = 255`; that value
must fit `u8` before Wrapping addition produces 1. The fallback supplies 0,
so its result is 3. Each `main()` checks both results, the admitted call's result
257, and the selected provider limit 7. `limit()` itself checks the matching
calculation before choosing the provider endpoint; this is not a caller guard
used to justify an earlier partial operation.

Every place still checks input 7 against provider limit 7 and evaluates
`7u64 - value`; its twin changes only the provider limit to 6. The field program checks neighbors 31 and 47. The match
program retains its original two bound-7 cases and adds bound-7/bound-3 cases
whose differences are 0 and 4. The selected-target tags remain 17 and 29.
These extended sources still need fresh qualification; the earlier separate
`closed_argument` observations do not qualify the current programs.

Two focused controls retain the missing obligations without duplicating the
nine-place row:

- [Input conditions](../../../../tests/omega/fail/generics/declared_endpoint_contract_inputs/main.omg)
  require four distinct refusals: zero below the named floor, 2^63 - 1 below
  the unsigned floor, 256 at an exclusive upper bound, and input 2 above the
  calculated prerequisite 1.
- [Initial integer fit](../../../../tests/omega/fail/arithmetic/anonymous_matched_argument_initial_fit/main.omg)
  changes 254 to 255. Anonymous addition then produces 256, which must reject
  before entering the Wrapping `u8` helper.

The [guarded-crash production](guarded_crash/README.md) keeps the same bound
when its helper has a guarded crash contract. It requires the compiler to prove
the invocation safe before running the helper; changing only the divisor to zero
must reject during admission.

Run the [local program](local/main.omg) through the public CLI with:

```sh
omega run --both samples/cli/facts/provider_contract_endpoint/local/main.omg
```

The extended programs and consolidated controls are frozen but not yet freshly
qualified. Their current acceptance requires all nine positives to check and run
with native/Terminal agreement, all nine provider twins to reject for the bound,
and both focused controls to reject for every authored condition.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [provider returns 6](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [provider returns 6](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [provider returns 6](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [provider returns 6](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [provider returns 6](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [provider returns 6](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [provider returns 6](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [provider returns 6](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [provider returns 6](target_body/invalid/main.omg) |

Each reject changes only the selected provider's integer from 7 to 6. The
consuming value and subtraction stay identical. The missing obligation must
identify that the captured input 7 exceeds the selected closed endpoint 6.

## What the row must establish

- **Establishment:** the selected checked provider computes the limit. Each call must prove its input bound before using the call result. The bound cannot prove its own prerequisite.
- **Identity and lifetime:** the comparison concerns the input value captured for that call and the provider selected for its target. A later write or another provider cannot replace either one.
- **Composition:** helpers, fields and joins carry the same bound to `7u64 - value`. A caller must not add a guard after the call to recreate it.
- **Soundness:** anonymous 255 must fit `u8` before Wrapping addition begins. Calls used inside a contract must satisfy their own requirements and effects.
- **Lowering and execution:** every positive must run with native/Terminal agreement at exit 0. All four hosted entry bindings remain authored; current qualification targets Linux x86_64 and claims no execution on the other hosts. Proof-only bounds add no runtime arguments.

The nine places and their twins are pinned under `tests/omega/{pass,fail}/facts`.
[Existing soundness controls](controls/README.md) pin formation, target ownership,
write snapshots, result identity and authenticated Build request transport.
Current corpus and runtime qualification remain pending.

Historical evidence applies to earlier source only. Producer
`e89eee400dbaa88425faf8ae48554b7c020674d1` ran the earlier nine-place row on
macOS ARM64. Later [run 41896803781](https://github.com/CathedralOS/Omega/actions/runs/41896803781),
producer `e0eb38a3dfc80fe1bf9312f8b3406188d90b7097`, ran the separate
`closed_argument` programs on Linux x86-64 and observed exit 70 in both engines
for the unchanged declared-endpoint and Wrapping customers. Those records do
not establish acceptance for the extended programs linked here.

Parent acceptance still includes remaining field/case/recast
and collection compositions, filesystem parameter migration, product/bootstrap
source migration, and retiring the suffix representation after its customers
are migrated. This row alone does not close that parent outcome.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
Build selects an ordinary named implementation for each target. This follows
**BUILD-OWNED-TARGET-SELECTION**. The programs keep the same operation, inputs,
contracts and expected exit. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. The extended target-body program and twin need the same fresh qualification as
the other eight places. The other hosted targets keep their authored Build rows;
no new execution on those hosts is claimed here.

# Indexed predicate handback

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, indexed writes and current
record-field/owned-array predicate handback. This is another production of
`saved_observation_snapshot`, with the same fact, bound and consuming `agree`
operation as [predicate handback](../predicate_handback/README.md).

The public `preserve` machine receives two runtime indices, their bounds and
inequality, the current selected value 7, and an arbitrary replacement value.
Its postcondition owes both the unchanged selected 7 and the newly written
replacement. It cannot use a convenient caller's literal indices to establish
that public contract. Each twin changes only the assignment destination from
`changed` to `kept_index`; the unchanged postcondition must refuse at `preserve`.

Every positive first writes element zero while saving element one, then writes
element one while saving element zero. `agree` consumes the saved/current fact
before runtime canaries inspect both cells and the surrounding 11/13 fields.
The final values are [4,7] and [7,4], respectively. The match place exercises
both arms with distinct marker effects 11 and 13. Misplaced, omitted or
collateral stores, stale observations, wrong selections and copied-value aliasing
must fail the exit-zero oracle. Runtime canaries do not supply static premises.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [source](../local/indexed_handback/main.omg) | [source](../local/indexed_handback/invalid/main.omg) |
| Field | [source](../field/indexed_handback/main.omg) | [source](../field/indexed_handback/invalid/main.omg) |
| Parameter | [source](../parameter/indexed_handback/main.omg) | [source](../parameter/indexed_handback/invalid/main.omg) |
| Call result | [source](../call_result/indexed_handback/main.omg) | [source](../call_result/indexed_handback/invalid/main.omg) |
| Match join | [source](../match_join/indexed_handback/main.omg) | [source](../match_join/indexed_handback/invalid/main.omg) |
| Const name | [source](../const_name/indexed_handback/main.omg) | [source](../const_name/indexed_handback/invalid/main.omg) |
| Nested machine | [source](../other_state/indexed_handback/main.omg) | [source](../other_state/indexed_handback/invalid/main.omg) |
| Other machine | [source](../other_machine/indexed_handback/main.omg) | [source](../other_machine/indexed_handback/invalid/main.omg) |
| Selected implementation | [source](../target_body/indexed_handback/main.omg) | [source](../target_body/indexed_handback/invalid/main.omg) |

Exact source/Build copies and runtime/rejection oracles are pinned as
`tests/omega/{pass,fail}/facts/saved_observation_snapshot_indexed_handback_<place>_{exit,rejected}`.
Each positive authors all four hosted ProgramEntry bindings and requires
`omega run --both` with native/Terminal exits 0/0.

The local controls challenge the repair's premises:

- [RHS disjoint effect](../local/indexed_handback/rhs_disjoint/main.omg) changes
  a neighbor field while producing the replacement value. Its changed canary
  must be visible, and the saved cell must still be 7.
- [Current selector](../local/indexed_handback/selector_current/main.omg) changes
  a mutable selector after a store; the postcondition must read its new value.
- [No separation](../local/indexed_handback/controls/no_separation/main.omg),
  [RHS clobber](../local/indexed_handback/controls/rhs_clobber/main.omg) and
  [whole replacement](../local/indexed_handback/controls/whole_replacement/main.omg)
  must reject `preserve`'s own postcondition.
- [Selector overwrite](../local/indexed_handback/controls/selector_overwrite/main.omg)
  and [stale selector](../local/indexed_handback/controls/selector_stale/main.omg)
  must reject `select_current`'s own postcondition.
- [Predicate body](../local/indexed_handback/controls/predicate_body/main.omg)
  guarantees 6 but promises 7. Its result contract must reject: the predicate's
  name supplies no element facts.

Exact copies of these controls are pinned alongside the nine-place fixtures.

The checker retains an already observed cell only when the actual captured
store proves it disjoint, after RHS effects have retired their own facts.
Unknown call effects still use their conservative frames. Statement calls and
value calls share selected public-contract capture: eager inputs precede writes,
and changed referents get fresh normal-exit identities. Separate statement
handles distinguish captures in different arms. A specification read uses its
current scoped selector without inventing an executable read. Named collection
predicates contribute only necessary bounds proved by the existing checked-body
explorer on every true return; a descriptor alone contributes no element facts.

## Requirements and baseline

- Establishment: bounds, separation and the original selected 7 are independent
  entry obligations. The actual store establishes the replacement value.
- Identity and lifetime: saved scalar identities remain immutable. Current cells
  use their authenticated backing and completed selector identities; selector
  spellings do not identify values after mutation.
- Composition: the public postcondition carries the exact indexed observations
  across ordinary calls, returned scalars, stored fields, joins, nested arrivals
  and Build-selected implementation bodies. Each arrival owes its own contract.
- Soundness: overlapping and unknown effects retire observations in evaluation
  order. The predicate being proved supplies no premise. Access, ownership,
  selector formation, enclosing invariants and representation authority remain
  separate. See [mutation and subject identity](../../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity)
  and [fact validity](../../../../../wiki/spec/terminal-psi/fact_validity.md).
- Psi 06–08: exact reads, captured destinations, ordered effects and continuation
  must retain their behavior through canonical Terminal Psi and interpretation;
  static predicates erase.
- Omega 00–02: ordinary record/array geometry and calling plans retain both
  independently selected cells and their surrounding fields.
- Omega 03–06: register/frame homes must preserve saved scalar values across
  mutation and calls; all indexed writes are checked against runtime canaries.
- Omega 07–09 and emission: emitted native code must agree with Terminal Psi on
  both index orderings and both match arms. Authored target bindings are not
  observations of execution on those targets.

Fixed discovery base: `a91a8de86514f5b0ba71509b3af4d656916d6062`.
The exact optimized Linux x86-64 baseline rejects all nine positives and all nine
twins with `cannot prove ensures contract for exit from preserve`. Its ordinary
nonliteral indexed-store probe and the earlier fixed-index predicate-handback
local both check. Raw sources, hashes, diagnostics, producer profile and binary
identity are retained under `build/advance/` in the workbench and identified in
the publication record. Baseline producer SHA-256:
`e06aa2de036c83dbd5e87933c11dd007e4ba1f8ed5049f43684668a418139fa4`.

Before conflict integration, the optimized Linux x86-64 producer (opt-level 1), SHA-256
`d2e7d1cfd86c9f496f0bab20c15f8b5e2bb1e5fbe45eb69ba415a1449b44978a`,
runs all nine places and both additional positives with native/Terminal exits
0/0. All nine twins and all six additional invalid controls reject their own
authored contract obligation. The sample gate exits 0:

```sh
python3 tools/sample_native_gate.py --cli target/debug/omega --facts \
  --filter /indexed_handback --jobs 2 --timeout 90 \
  --write-results build/advance/final-sample-results.json \
  --history-dir build/advance/sample-history
```

`stable-build.json` records unchanged compiler input hashes across the build;
`baseline.json` preserves the frozen original refusals, and
`final-sample-results.json` records the current 11 passes and 15 intended
rejections. Earlier failed iterations remain historical observations.
The [published evidence manifest](https://github.com/CathedralOS/Omega/blob/6e5131c510b9e6227b4592652c9a5a39e350a970/wiki/drafts/measurements/indexed_handback_2026_10_31/manifest.json)
binds the compressed raw receipts, baseline replay inputs and retained source
bundle to their hashes. Both scoped golden-recording commands exit 0.

The focused corpus selection also includes the earlier predicate-handback row,
Unit reference completion and nested statement completion. Its native/Terminal
leg runs 33 positives: 31 exit 0/0. The two older
`nested_machine_statement_completion_{namespaced_theorem,target_body}_exit`
fixtures still refuse. The retained exact-base producer gives identical
diagnostics on unchanged source/library inputs (`corpus-base-attribution.json`):
the former fails a theorem premise, and the latter uses an outdated
`select_provider` call. These are baseline limitations, not accepted execution.
The separate checking leg rejects all 43 selected invalid fixtures; 41 match
their authored fragments. The two older private-guarantee/target-body controls
stop at that same obsolete Build call on both base and candidate. The private
guarantee's unchanged source is also replayed with current Build bindings in
scratch so its actual public-contract boundary can be checked. Eight older
predicate-handback twins now report only the failed writer postcondition;
their cascading caller errors disappear because normal calls transport the
declared postcondition. None of those invalid writers becomes accepted.

After resolving an actual shared-equation-intake conflict with main
`02d6d92b37d4cee20861321d9ad0dfc58a0bb3d8`, source
`da19ac492c58292e9ae1e20f1337544d63788974` repeats the full 11-positive /
15-invalid gate and the same scoped corpus outcomes. Its fresh opt-level 1
producer SHA-256 is
`f823cda3cf4478e5df5db3aeafe20a9e0f9be146bfd39ff64c8c3cb520c9e191`;
all 7,853 tracked compiler/library input hashes stay unchanged across its build.
The manifest's `integration` entry retains these current receipts and exact
source bundle separately from the earlier observations. Relative to integrated
main, both goldens retain every preceding byte and record only the 26 new
fixtures. The private-guarantee scratch control also rejects on this producer.

The broader parent retains other predicate productions, recasts, customer
migration, product-compiler parity and removal of range-suffix machinery. This
production does not establish those requirements or other-host execution.

# Selected native code layout

These programs ask where a compiled function starts and which instruction ranges
belong to it, then check that the answer survives ordinary records and calls.

The exact selected executable application supplies a nonzero native entry and finite attributed code regions at runtime.

Every standard place queries `code_layout<selected<u64, 7>>()` and checks the
same result: a nonzero entry, at least one region, the first region beginning
at that entry, and a positive first-region length. Only the route from the
query to the check changes. The root keeps its original nongeneric
`code_layout<selected>()` query. A query retains the selected function in the
image; it does not invoke that function.

This row advances **BUILD-OWNED-TARGET-SELECTION** and **ADDRESS-EXPOSURE**
(which also owns `NATIVE-CODE-LAYOUT-QUERIES`) in [TASKS.md](../../../../TASKS.md).
The target place uses one ordinary checked implementation of
`SelectedLayoutCheck`, selected by four explicit target-keyed Build rows.
The public requirement promises termination and an empty service reach; callers
receive no private metadata guarantee as a proof assumption.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | Original nongeneric query |
| local | [program](local/main.omg) | [twin](invalid_local/main.omg) | Local value |
| field | [program](field/main.omg) | [twin](invalid_field/main.omg) | Record field |
| parameter | [program](parameter/main.omg) | [twin](invalid_parameter/main.omg) | Explicit parameter |
| call result | [program](call_result/main.omg) | [twin](invalid_call_result/main.omg) | Returned record |
| match join | [program](match_join/main.omg) | [twin](invalid_match_join/main.omg) | Both selected arms |
| const name | [program](const_name/main.omg) | [twin](invalid_const_name/main.omg) | Named specialization argument |
| nested machine | [program](other_state/main.omg) | [twin](invalid_other_state/main.omg) | Nested machine parameter |
| another machine | [program](other_machine/main.omg) | [twin](invalid_other_machine/main.omg) | Helper forwarding |
| selected target | [program](target_body/main.omg) | [twin](invalid_target_body/main.omg) | Build-selected implementation |

The match runs both Boolean choices. One arm calls a helper and returns path
marker 1; the other queries inline and returns marker 0. The caller checks both
against independent expected literals while retaining every metadata check.
The marker travels beside the layout in an ordinary record. It is not a
borrowed input supplying the layout's returned lifetime.

All nine adjacent twins retain the query and original checks but consume the
query through a package-scoped semantic constant. They must reject with:

```text
late native code-layout metadata is unavailable to same-product semantic constant, type, layout, or optimization evaluation
```

An unrelated refusal does not establish this boundary. The target twin evaluates
the same ordinary query helper in that constant before its checked provider
returns the result; it does not try to interpret an opaque selected requirement.
All standard Main and Build files are mirrored under
`tests/omega/{pass,fail}/facts/selected_native_code_layout_through_*`.

Additional controls copy a complete `CodeRegion` beyond the helper that created
its layout ([snapshot](controls/region_snapshot/main.omg)), reject that same
snapshot during [constant evaluation](controls/region_snapshot/invalid/main.omg),
and reject writes through the shared region sequence even when the enclosing
record is mutable ([write control](controls/immutable_regions/main.omg)). The
snapshot checks its copied base and length after the creating helper returns;
no caller-owned `CodeLayout` keeps the old descriptor alive.

The [dormant-cycle control](controls/uncalled_cycle/main.omg) selects a function
whose helpers form a tail-call cycle. Its successful zero exit must come from
inspecting that function, without executing it. The matching
[constant twin](controls/uncalled_cycle/invalid/main.omg) retains the same query
and still refuses late metadata during semantic evaluation.

## Execution and metadata ownership

Every positive and the root bind `ProgramEntry` for Windows x86-64, Linux
x86-64, macOS ARM64 and Linux ARM64, and declare `Expected exit: 0`. For example:

```sh
omega run --both samples/cli/facts/selected_native_code_layout/main.omg
omega run --both samples/cli/facts/selected_native_code_layout/target_body/main.omg
```

Acceptance requires two actual executions with exit zero. A Terminal decline
is not agreement.

Native lowering derives instruction spans from the exact selected function
and its frame instructions after final layout and branch reflow. The image
producer resolves their relocations and validates their attribution; neighboring
symbols, padding, data pools and callees do not supply guessed regions.

For direct-image `run --both`, the runtime passes the explicitly retained,
completed native artifact to Terminal execution. It checks the artifact and
target, reads its validated layout, and supplies immutable typed region records.
These are the completed image's linked coordinates, not an observation of a
separately launched process's load address. Ordinary copied descriptors retain
the metadata's storage. The addresses grant neither memory access nor a storage
lifetime extension. Pure interpretation has no completed native image and still
refuses the query. Semantic constant, type, layout and optimization evaluation
cannot inspect the same product's late metadata.

See [native layout queries](../../../../wiki/spec/language/counts_and_addresses.md#native-code-layout-queries),
[returned views](../../../../wiki/spec/language/lifetimes.md#returned-views),
[Build selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
and [borrowed views](../../../../wiki/spec/terminal-psi/byte_views.md).

## Current evidence

The direct-image row now has actual native/Terminal agreement on all four hosted
targets. Evidence is retained in
`build/code-layout-selected-20261103` and [PR #1755](https://github.com/CathedralOS/Omega/pull/1755).

The fixed published base is `108f8a95dcd8`. Its verified retained producer
refused four original positive transfers. The other five ran natively at zero,
but Terminal declined every query. The nine semantic-constant twins rejected
for the intended rule. Older checked-in host records remain historical; they
do not establish current engine agreement.

The carrier repair made all nine original positives check. The first runtime
probe then exposed a region-field read failure. At exact source `39602178dbe4`,
the next focused replay produced 17 actual agreements, including eight original
places, but still failed source acceptance for the snapshot and uncalled
provider. A newly authored borrowed match-marker helper also violated the
existing lifetime rule; its ordinary record replacement is a source correction,
not a compiler repair. That failed replay did not reach ordinary project checks.

The later exact `42acf360b846` replay established actual zero agreements for
all nine original places, but still refused the complete-region snapshot.
The subsequent descriptor dependency repair at `b6026a4cba3` and `06899beec3f`
preserved those agreements on all four matching hosts. The snapshot then
reached native lowering and exposed a missing producer access contract.
Those full jobs remain failed; their successful rows retain their own sources.

A retained-producer trace identified that the layout's type was present, but a
second list of value producers omitted the native-layout query. The repair at
`f3399003ce5` replaces that duplicate list with one explicit classification of
produced values. Its focused replay passed the unchanged snapshot, both existing
atomic/activation positives, and every selected authority rejection.

Main then changed how borrowed records establish entry facts and how completed
records supply field information. Those changes affect this row, so they were
integrated as `b0fa0866008` and replayed on all four matching hosts.

All nine places, the copied-region snapshot and the dormant cycle now execute
at zero in both engines. All 24 ordinary projects
per host meet their authored oracles: 12 actual zero agreements and 12 intended
Source rejections. The 36 native-layout Source pins check or reject for their
intended obligation.

| Host | Query/control agreements | Corpus report | Ordinary projects |
| --- | --- | --- | --- |
| Linux x86-64 | 18 at exit zero | [run 41746709533](https://github.com/CathedralOS/Omega/actions/runs/41746709533) | [retained replay](https://github.com/CathedralOS/Omega/actions/runs/41750183833) |
| macOS ARM64 | 12 at exit zero | [run 41746709933](https://github.com/CathedralOS/Omega/actions/runs/41746709933) | [retained replay](https://github.com/CathedralOS/Omega/actions/runs/41750670926) |
| Windows x86-64 | 11 at exit zero | [run 41746709821](https://github.com/CathedralOS/Omega/actions/runs/41746709821) | Same successful run |
| Linux ARM64 | 11 at exit zero | [run 41746709614](https://github.com/CathedralOS/Omega/actions/runs/41746709614) | [retained replay](https://github.com/CathedralOS/Omega/actions/runs/41750681047) |

The counts include existing optional queries only where their Build files
already supply an entry. Windows's complete selected scope passes. The other
original full reports remain failed on supplemental controls; their valid
query observations are retained at their actual source, not relabeled as a
new successful full run.

The extra shared-parameter recast probe has an identical legalization failure
with the verified published-main-equivalent producer. Its source, oracle and
golden remain unchanged, and it supplies no passing execution coverage. The
separate scalar shared-view adjunct is slow: the Linux full Source leg timed
out, then checked in 102 seconds in an isolated same-binary replay with the same
120-second limit. Its current Mac execution leg still times out, and its earlier
Windows legs remain unverified. No compiler repair or scheduling cause is
inferred from that replay. Its relevant result roles are unchanged.

The additional atomic control has the same Linux ARM instruction-catalog refusal
at `06899beec3f`, before the result-role repair, and at `b0fa0866008`. The
[exact retained-producer comparison](https://github.com/CathedralOS/Omega/actions/runs/41751521903)
executes neither engine for that case. Its original zero-exit oracle remains
unmet; this row does not claim atomic support on that target. The atomic
positive does execute at zero on the other three hosts, and the forged-outcome
and activation conservation/lifetime controls retain their intended Source
refusals.

## Requirements and remaining acceptance

- **Establishment:** the query retains the exact selected realization. Runtime
  checks consume its actual entry and attributed region records; no caller
  guard introduces missing metadata.
- **Identity and lifetime:** the query's type and constant arguments remain
  exact. Stored shared descriptors have their own retirement obligations,
  separate from source-level copy permission. Copies retain their metadata
  storage; the snapshot control checks an ordinary copied region after return.
- **Composition:** calls, fields, joins and nested machines carry the same
  value. Both match arms carry distinct path markers. The target requirement
  uses explicit Build selection without publishing private facts to callers.
- **Soundness:** late metadata stays unavailable to same-product semantic
  evaluation. Selection, lifetime and shared-write controls retain their
  independent obligations; metadata does not grant code or memory authority.
- **Checked to Terminal Psi:** the retained query becomes the canonical query
  operation, with the checked descriptor multiplicity and exact selected
  machine identity. Actual Terminal execution must consume the supplied image;
  unsupported interpretation remains a refusal.
- **Provider, layout and calling plans:** four ProgramEntry rows and four
  provider selections name explicit targets. Ordinary record and view layouts
  and call plans carry the result; no source target family is needed here.
- **Instructions and frames:** the native layout producer includes attributed
  function and frame instructions. Register homes, spills and prologue changes
  feed final geometry rather than a second estimate of function size.
- **Encoding and images:** final instruction layout owns region lengths and
  relocated bases. Actual matching-host observations and exact-source records
  establish this direct-image row. Independent final review and publication
  verification remain required before landing.

This row does not close either parent. Real split/shared production with exact
fragment/frame custody, unsupported or ambiguous realization refusal,
bodyless-family migration, private-component and Dynamic-ELF inspection, and the
remaining address-exposure operations retain their own acceptance.

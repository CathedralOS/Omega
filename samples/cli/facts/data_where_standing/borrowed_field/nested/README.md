# Keep a saved reference when a pointer field changes

A helper saves a reference to a number, then replaces the pointer field it came
from. The saved reference must still name the original number and keep its bound,
so adding 70 remains safe. This row is unfinished: the latest completed program
run still rejected its valid examples during cleanup.

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, preserving field bounds
through reads, calls and returns involving references. This extends
[data_where_standing](../../README.md) and keeps the bound and operation from
the [borrowed-field production](../README.md).

## What remains to pass

Every positive below must exit 0 in both native and Terminal execution. Each
reject twin must still refuse construction of `Counter` when the input no
longer supplies its predicate. The [incoming-storage control](controls/incoming_snapshot/README.md)
and the identity/access controls below must also retain their authored outcomes.
All 58 frozen sample/fixture file pairs and their result and diagnostic oracles
remain unchanged; no place counts from checking alone.

`Holder.counter` points to a `Counter`, whose `value` points to an `i32`.
`capture` saves that scalar pointer and the neighboring fields from a
`SourceHolder`, replaces `holder.counter`, then constructs `Counter` from the
saved values. `SourceCounter` must supply the result's bound. Each twin removes
only that input predicate while retaining `Counter`'s construction requirement.
The returned record passes through the selected place before the addition.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [missing predicate](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [missing predicate](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [missing predicate](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [missing predicate](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [missing predicate](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [missing predicate](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [missing predicate](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [missing predicate](other_machine/invalid/main.omg) |
| Selected target implementation | [program](target_body/main.omg) | [missing predicate](target_body/invalid/main.omg) |

Entries test 60 to -17 and the reverse replacement: the saved pointers must
still produce 130 and 53. They check the source/output neighbors, backing
scalars, holder canaries and newly installed pointers. Both match selections run;
selected implementations have distinct tags. Const-name still produces its
inputs from `HIGH`/`LOW` through `Backing`. Every positive declares exit 0 and
binds all four hosted targets; bindings alone prove no foreign-host execution.

| Pointer control | Obligation |
| --- | --- |
| [Actual write destination](controls/incoming_compatibility/main.omg) | A saved reference permits a write to another scalar, while [its alias twin](controls/incoming_compatibility/invalid/main.omg) rejects a write to its own scalar. |
| [Saved pointer](controls/snapshot/main.omg) | A copied scalar pointer keeps its original referent after the local record pointer is replaced. |
| [Replacement identity](controls/snapshot/invalid/main.omg) | The old value cannot prove equality to a read through the replacement. |
| [Known local supplier](controls/known_source/main.omg) | A locally constructed descriptor keeps its established supplier. |
| [Write-only intermediate](controls/write_only/invalid/main.omg) | A write-only reference cannot authorize reading the nested scalar descriptor. |
| [Suspended descriptor home](controls/suspended_home/invalid/main.omg) | A live exclusive borrow prevents a conflicting descriptor read. |

## Requirements and evidence

- **Establishment:** `SourceCounter` supplies `bounded(value) == true`.
  `capture` must prove it when constructing `Counter` after replacement. The
  established predicate and `read`'s checked equation then justify the addition;
  no caller guard or receiving annotation supplies a missing premise.
- **Identity and lifetime:** new holder reads follow its current pointer. The
  saved pointer still names the original scalar and keeps its storage lifetime;
  the holder's address is not the scalar's.
- **Composition:** every place carries the same record, reference and predicate.
  Matches retain the justified sources from every arm. Replacement neither
  retargets the saved pointer nor discards its bound.
- **Soundness:** every possible source must permit the read. Access and lifetime
  checks apply at each crossing; names and types cannot prove disjoint storage.
- **Checked-to-Terminal Psi:** the bound erases; pointer loads, scalar reads,
  helper calls, selection and canary checks remain executable.
- **Layout and native execution:** record layout, calling plans and image
  emission must preserve both pointer crossings and surrounding fields, with
  native and Terminal agreement.

The [latest completed program run](https://github.com/CathedralOS/Omega/actions/runs/41884089427)
used an optimized compiler built from
`ef422b8dbabf39245587d4752a32033f640d29e9` on Linux x86-64.
All nine positives reached cleanup, then refused because no checked cleanup
covers a saved reference. The incoming-storage control still failed retirement
ordering, and the saved-pointer control reached a missing local-storage error
in Terminal lowering. None ran in either engine. The reject twins and the
identity/access controls retained their intended refusals. The actual-write,
known-supplier and recursive-reference neighbors ran with native and Terminal
agreement at exit 0; those successes do not qualify this row.

[The earlier program run](https://github.com/CathedralOS/Omega/actions/runs/41860338909)
at `98a751f` failed with “the body has no planned scalar completion”.
[The source `218d2fe` attempt](https://github.com/CathedralOS/Omega/actions/runs/41882364283)
failed during Rust compilation before any program ran. Those attempts remain
failed historical observations, with no results reassigned to the new source.

[Earlier investigation details](https://github.com/CathedralOS/Omega/blob/218d2feb15d115b3c8fa4d34c55f5346a3a61f7b/samples/cli/facts/data_where_standing/borrowed_field/nested/README.md#requirements-and-evidence)
retain the earlier producer identities and attempts. Input hashes and baseline
receipts remain in the PR evidence, starting with
`incoming-reference-row-initial-inputs.json`. Earlier program versions do not
qualify the frozen inputs; pins retain identical programs, Build files and oracles.

Use `omega update --project <place> --target <target> --offline`, then
`omega run --both <place>/main.omg`. Retain the actual source, target, verified
optimized binary digest and both engine observations.

The governing rules are
[entry and current scalar values](../../../../../../wiki/spec/terminal-psi/fact_validity.md#restored-scalar-values),
[replacement of carried borrows](../../../../../../wiki/spec/language/lifetimes.md),
[captured places](../../../../../../wiki/spec/terminal-psi/loans.md#captured-places-and-available-premises)
and [source-derived qualification](../../../../../../wiki/spec/language/domains.md#declaration-and-membership).

This production tests direct assignments. A saved scalar reference across a
replacement performed by a mutating helper still needs the same compatibility
proof. Unresolved dynamic paths and changing recursive call relations remain
implementation work. This row does not close the parent's remaining recursive,
case or collection inventories, returning proved propositions to callers,
filesystem qualification or source migration. GUI qualification remains pending
separately.

# Case-dependent default and named predicates

Case membership and the complete predicate recover the payload type and ordered bounds through the transported value.

Owner: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in [TASKS.md](../../../../TASKS.md).
The requirement is to retain an applied data value's complete predicate across
transport, saved scalar observations and restoration of current storage.

`pick<T>(Value<T>) -> T` extracts fixed `i32` and `bool` payloads using the
selected case's type equation. `NamedValue in Typed<T>` exercises the equivalent
named domain. `Interval<32>` and `NamedInterval in Ordered<32>` carry
`lo <= hi && hi <= 32` in their Range alternative. `width_range` consumes that
fact directly to establish `lo in AtMost<32>` and compute `hi - lo`.

The default interval now also crosses a restoration window. `width` copies the
transported interval, selects Range and lends it to `restored_width`. That
helper saves both endpoints, writes `hi = 0`, observes the intermediate zero,
and writes the saved upper endpoint back. Its current complete predicate must
be proved before the call to `width_range`. An entry predicate cannot stand
for current storage while the window is open. No caller guard supplies the
restored relation.

The bound, values, subtraction and exit oracle are unchanged. The two Bundles
exercise signed/Boolean payloads and Range/Empty results: 42/true/4 versus
-7/false/6, then -13/false/77 versus 23/true/77. Both match-join choices run.
Hexadecimal neighbors detect damaged record or ABI transfers. The intermediate
zero and unchanged lower endpoint distinguish omitted or collateral stores;
returning 78 instead of the expected width makes `main()` trap. The named
classifications also erase to their original Integer/Boolean tags.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| selected implementation | [source](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each twin removes only `hi <= Bound` from the default interval predicate.
The public consumer cannot promise the missing ceiling for every admitted
input. Exact pins live under
`tests/omega/{pass,fail}/facts/case_dependent_default_domain_through_<place>_{exit,rejected}`.
All four hosted entries are authored; the selected-implementation place binds
four ordinary named implementations through its entry roots.

## Requirements and evidence

- **Establishment:** constructors and qualifications establish the complete
  default or named predicate. The selected case establishes the extractor's
  type equation. The logical subject adds no runtime argument.
- **Identity and lifetime:** the exact T/Bound application survives transport.
  Completed endpoint reads retain their own identities. Restoration uses the
  current tag and every required current field, independently of the retired
  whole-value observation. See [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
- **Composition:** all nine places retain the same predicate, bound and
  subtraction. The closing store owes the complete predicate before consuming
  the restored value; both join choices execute.
- **Soundness:** ceiling twins, the [missing type equation](controls/missing_type_equation/main.omg),
  [other bound](controls/other_default_argument/main.omg), [stale write](controls/stale_write/main.omg)
  and [stale snapshot](controls/stale_snapshot_restore/main.omg) controls reject.
  The last changes the lower endpoint to 31: restoring the saved upper 21
  cannot borrow the old ordered relation. Unknown tags/fields supply no proof.
- **Psi 05–07:** source proof discharges restoration and scalar consumption
  before erasure. Ordinary tagged values, scalar copies, stores and calls lower
  to Terminal Psi and must execute under `--both`.
- **Omega 00–02:** Build entries, Bundle/interval layouts and ordinary calling
  plans transport the values; domains add no runtime data.
- **Omega 03–06:** signed/Boolean results, both tags, nonzero neighbor canaries
  and the intermediate/restored endpoint observations expose wrong homes,
  omitted stores and collateral writes.
- **Omega 07–09 and emission:** native exit 0 must agree with Terminal Psi.
  Actual execution establishes only its recorded host, not the other entries.

The earlier generic-carrier repair landed in [PR #1310](https://github.com/CathedralOS/Omega/pull/1310).
Its [immutable evidence](https://github.com/CathedralOS/Omega/tree/50d1746644217fd1583611ec5c4da9bd141ce26d/evidence/generic-case-predicates)
qualifies the previous read-only row and original exit-65 customer, not the
new restoration sources. At fixed base `1982527ed4be50c661ce88b7616d3b3befadf5e2`,
all nine new positives wrongly reject the open default-domain window at the
consuming call. The optimized producer is SHA-256
`495a491cb20de2e505355f2dabc3ff13a6714b33101121695c69eac2f3ec38d0`.
Fresh projects contain identical authored sources and no copied acceptance state;
raw baseline records are retained in the owning PR/claim evidence.

The restoration row passes all nine `omega run --both` executions on
`macos_arm64`: native and Terminal Psi both return 0. All nine ceiling twins
reach the intended `width_range` contract refusal. The stale-snapshot control
rejects its open default domain at borrowed return on both baseline and repaired
producers. Nongeneric, shared-entry and public generic compositions also check.
The repaired optimized CLI is SHA-256
`4b3fd797891dbfde52a8bf4a16a742f0ba5b12158bfd2da113d13d5a234a9425`,
built from the fixed base plus the owning PR's compiler patch; receipts retain
those original identities rather than relabeling them as a later checkpoint.
The focused corpus check observes 11 positives and 19 intended rejects; its
`--both` leg executes the 11 positives with native/Terminal exit 0. This includes
literal case restoration, ordinary record snapshots and write-only boundaries.

Existing guard-effects, literal restoration, named coverage, zero, invalid-case
and stale-type controls remain under [controls/](controls/). The parent retains
other predicate productions, source/product migration, other-host execution and
suffix retirement. GUI qualification stays pending.

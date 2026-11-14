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

A second production of the same restoration judgment follows it.
`both_widths` lends the Range interval to `restored_width` and then to
`upper_width`, and returns the first width only when both agree (79 otherwise).
`upper_width` saves only the upper endpoint, writes `hi = 0` and restores it,
then calls `width_range`. It never reads `lo`. The restored predicate must take
the lower endpoint from current storage: no recorded write touched it since the
borrow's entry, so its current version is the entry value. The Range tag reaches
`upper_width` through `restored_width`'s complete write frame, which writes only
payload fields. A collateral write to `lo` would change the agreed width and make
`main()` trap.

The bound, values, subtraction and exit oracle are unchanged. The two Bundles
exercise signed/Boolean payloads and Range/Empty results: 42/true/4 versus
-7/false/6, then -13/false/77 versus 23/true/77. Both match-join choices run.
Hexadecimal neighbors detect damaged record or ABI transfers. The intermediate
zero and unchanged lower endpoint distinguish omitted or collateral stores;
returning 78 instead of the expected width makes `main()` trap. The named
classifications also erase to their original Integer/Boolean tags.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [source](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| const name | [source](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| another machine | [source](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| selected implementation | [source](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

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
  A lower endpoint overwritten inside the window through an
  [alias](controls/alias_sibling_restore/main.omg) or a
  [recast view](controls/recast_sibling_restore/main.omg) formed before it
  also rejects at state exit: an untouched entry cell needs no overlapping
  write exposure.
  The last changes the lower endpoint to 31: restoring the saved upper 21
  cannot borrow the old ordered relation. Unknown tags/fields supply no proof.
- **Psi 06–08:** source proof discharges restoration and scalar consumption
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

The untouched-sibling production was red at fixed base
`4f87c9e08b1a7217960f7f69c440a9d2f09d278d` (shared optimized CLI SHA-256
`0a78edae3c9f54eaf0e365c3e9b42758ad641ba008619914ab64180f0235adae`): the
restoring store in `upper_width` left the window open, reported as
`data Interval<32>'s default domain is still FALSE at a call`. The stored
predicate judgment rebuilt the Range subject only from captured field
observations, and `lo` had none. The repair reads an entry formal's untouched
cell as its entry projection under the same write-exposure rule that already
admits declaration reads after disjoint stores; a written sibling still uses its
stored current value, so the stale-snapshot control keeps rejecting at state
exit. With the repaired optimized CLI (SHA-256
`f01f31fb00ade02a521fb1920129f2ac5b75b185e998726d3b9aaf6c0b005ded`, base plus
the owning PR's checking patch) all nine places return native and Terminal exit
0 on `macos_x86_64`, the host of this run; each place now also binds that host's
entry. The nine twins report the ceiling refusal at both `width_range` calls.
A focused 204-fixture corpus subset (restoration, window, case-payload and
recast rows) moves only by load-induced timeouts against the same subset run
with the base runner; the 9 positive pins execute with exit 0 in both engines.
After rebasing onto main `8d8506a6bca`, the repaired producer (CLI SHA-256
`6b3ec882092a7e6ec3d6dc11895e41e7a2aa393e836a7e04a5e07a8f7ec9c074`) still checks the
untouched-sibling witness, but every place, including main's unchanged sources
run by main's own producer, now fails Terminal lowering with
`InvalidUnitDomainPredicate` for `Ordered<32>` ("lowered domain predicate is not
closed over its subject"). That regression entered main after `4f87c9e08b1` and
is not yet bisected; the runtime evidence above is from the fixed base.
Callee-restored tag postconditions, caller field reads after such a call,
joined restoring stores and mutable payload recasts remain open in the parent.

Existing guard-effects, literal restoration, named coverage, zero, invalid-case
and stale-type controls remain under [controls/](controls/). The parent retains
other predicate productions, source/product migration, other-host execution and
suffix retirement. GUI qualification stays pending.

After resolving the stage-numbering conflict in integration checkpoint
`a7b775a623459ea459c1484310aee38803a06f2f`, the focused 30-fixture check and
11-fixture native/Terminal leg pass again with the freshly built optimized
corpus producer SHA-256
`80b323f981456412a7deb46e7c211fff73060ec3761beb8136826e080891b2e1`.
The earlier CLI observations retain their original producer identities.

# Restore a record before returning it

Each program saves `&write counter` in a local reference before passing
it to `fill`. This exercises the same loan-location record as the mutable scalar
reference production while retaining write-only access. These programs already
check on optimized baseline source `2713116252386e98ce5e87961aca764f8760b217`;
all nine now run with native and Terminal exits of zero on macOS ARM64 using
optimized producer `cd5340c6d0348e868e34d68ee85f250c431f79ba`. Their eighteen
controls reject for the intended reason. The
[saved-reference report](../../../../../wiki/drafts/measurements/saved_field_reference_calls.md)
records this execution separately from the earlier direct-call results.

A helper can temporarily put a record outside its declared bounds while updating
it. These programs set a field to 61 when the record requires at most 60, then
restore it before returning. The compiler must accept the completed update and
reject a helper that returns too soon or uses the record as valid too soon.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in `TASKS.md`.
Fact: **Counter.cell.value <= 60 makes exact i32 70 + value representable without a per-call bound.**
This exercises [the existing fact](../README.md) after writes through
`&write Counter`, which permits writes but forbids reads through that reference.

`fill` receives a valid `Counter` and a replacement bounded by its ordinary
call requirement. It temporarily stores 61 in `cell.value`, then restores the
bound with the replacement before returning. It never reads through the loan. `Cell` has no
predicate of its own: the bound belongs to the enclosing `Counter`.
The independent `peer <= 60` predicate must survive; the unchanged `before`,
`peer` and `after` fields retain 31, 13 and 47; `cell.left` and `cell.right`
retain 17 and 53. The produced `Counter` then
travels through each place before an independent consumer proves `70 + value`.
The two inputs 60 and -2147483648 must produce 130 and -2147483578. Both match
arms execute, and target selection has independent 17/29 implementation checks.

| Place | Restored update | Missing bound | Return before restoration |
| --- | --- | --- | --- |
| Local | [program](local/main.omg) | [reject](invalid_local/main.omg) | [reject](invalid_local_window/main.omg) |
| Field | [program](field/main.omg) | [reject](invalid_field/main.omg) | [reject](invalid_field_window/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](invalid_parameter/main.omg) | [reject](invalid_parameter_window/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](invalid_call_result/main.omg) | [reject](invalid_call_result_window/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](invalid_match_join/main.omg) | [reject](invalid_match_join_window/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](invalid_const_name/main.omg) | [reject](invalid_const_name_window/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](invalid_other_state/main.omg) | [reject](invalid_other_state_window/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](invalid_other_machine/main.omg) | [reject](invalid_other_machine_window/main.omg) |
| Selected implementation | [program](target_body/main.omg) | [reject](invalid_target_body/main.omg) | [reject](invalid_target_body_window/main.omg) |

Each existing twin removes only `Counter`'s value predicate, retains its peer
predicate, and must refuse the exact addition. Each place also has an
`invalid_window/main.omg` twin: it keeps both predicates but omits the restoring
store. That twin must reject when the helper returns. Since `70 + 61` fits i32, an
arithmetic overflow error would not prove that restoration is checked. Additional controls reject an
[insufficient replacement bound](controls/invalid_store/main.omg) and a
[read through write-only authority](controls/forbidden_read/main.omg).
The owner controls reject writes that violate an
[enclosing record](controls/enclosing_owner_violation/main.omg), an
[owner across an array element](controls/enclosing_array_violation/main.omg),
an [outer independent bound](controls/outer_owner_violation/main.omg),
or the [original owner behind a recast view](controls/recast_owner_violation/main.omg).
The [direct-field control](controls/direct_field/main.omg) checks restoration
when the bounded field is directly inside the record. The [relational window](controls/relational_window/main.omg)
restores equality before return; its [reject twin](controls/relational_debt/main.omg)
returns without restoring equality.
The [predicate helper](controls/predicate_helper/main.omg) expresses both
bounds through a checked Boolean call, preserving the same operation and oracle.
Samples and exact corpus pins bind all four hosted `ProgramEntry` targets.
Pins use `tests/omega/{pass,fail}/facts/data_where_standing_write_only_*`.

## Requirements and evidence

- **Establishment:** constructing the record proves its declared requirements.
  The replacement and each checked store then establish the new field value;
  the destination's annotation cannot prove its own input.
- **Identity and lifetime:** the write-only loan names existing valid storage;
  stores preserve sibling cells and their predicates without observing them.
  Writing the record grants no permission to read it or borrow one of its
  fields. Those checks still run separately. See [write-only authority](../../../../../wiki/spec/terminal-psi/structural_access.md#write-only-authority).
- **Composition:** the same established fact crosses all nine places. The
  consuming addition precedes the caller's result guard; the match and target
  checks distinguish alternative execution paths.
- **Soundness:** a missing standing fact cannot be recovered from later callers;
  an insufficient store bound and an unauthorized read still reject.
- **Psi lowering:** ordinary scalar stores, explicit calls, joins and nested
  transfers retain their effects in Terminal Psi, while proof clauses erase.
- **Layout, ABI and provider settlement:** the same nested record and
  explicit entry/provider bindings retain exact argument and field placement.
- **Instructions and storage:** native stores update the original caller home;
  the returned/copied record retains both written values and untouched canaries.
- **Encoding and execution:** qualification observes native and Terminal exits
  on macOS ARM64. The programs declare all four hosted entry bindings.

The earlier direct-call restoration checks used the fixed baseline
`3bcdc2d9e3b81ab4af705b8c14d04a6f4021a8dd` and the verified optimized compiler
from `c5cc3d8749224ed1a76fa7abad1707dfd9f5dd1d`; their compiler and library inputs
were identical. All nine earlier direct-call programs wrongly failed on return despite the restoring
store. The flat mutable and write-only controls failed immediately on storing
61. Frozen inputs and raw results are retained in the
[qualification evidence](https://github.com/CathedralOS/Omega/tree/b0a0e03ebb6f31884c1527a9d5e319283646c5f2). These baseline refusals demonstrate the defects.

The checker tracks each declared requirement separately. A write makes only the
requirements about changed storage need proof again; requirements about untouched
fields remain available. A later write can prove the affected requirements again.
The compiler does not infer validity merely from the destination's declaration.

The [mutable control](../controls/mutable_restoration/main.omg) and
[write-only control](../controls/write_only_restoration/main.omg) retain the
historical flat programs. The [snapshot control](../controls/mutable_restoration_snapshot/main.omg)
reads the temporary value and uses the untouched peer bound before restoration;
its runtime guard checks that those observations survived.
The [saved-copy reject](../controls/mutable_snapshot_bound/main.omg) and
[premature-use reject](../controls/mutable_premature_use/main.omg) prevent an
unrestored value from acquiring the bound from its declaration or a later store.

The child-record controls check that updating the scalar leaves an untouched,
already valid child usable. They include replacing the complete record before
the scalar update. The [atomic reject](../controls/atomic_resident_violation/main.omg)
checks a different rule: an atomic field must obey its bound on every write.
Storing 101 and then 99 cannot satisfy an atomic field's maximum of 100.

The [shared-source control](controls/shared_record_replacement/main.omg) restores
the field from a different valid record, using the source record's own bound.
The [array-source control](controls/shared_record_with_array/main.omg) also gives
that source an unrelated array and uses the bare `cell.value` predicate spelling.
Their [first](controls/invalid_shared_record_replacement/main.omg)
and [second](controls/invalid_shared_record_with_array/main.omg) twins weaken only
the source promise to 61; it cannot restore the destination's maximum of 60.

The [recast-owner restoration control](controls/recast_owner_restoration/main.omg)
performs the same temporary write through an array-backed view. Its
[twin](controls/invalid_recast_owner_restoration/main.omg) omits restoration and
must reject the original owner's requirement. Nonzero neighboring elements
check the actual write location.

The earlier direct-call qualification ran all nine restored programs with native
and Terminal exits of zero, and rejected all eighteen twins. Those executions
do not establish the changed saved-local programs. The
[record-restoration report](../../../../../wiki/drafts/measurements/record_update_restoration.md)
records the exact producer, wider controls and evidence.

Remaining parent-task work includes other record updates and loans, the
historical source audit, filesystem qualification and suffix retirement.

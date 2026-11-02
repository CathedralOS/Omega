# Write-only representation views

A program can write a record through another data type with the same representation
without gaining permission to read it. Each example writes 73 through a `Viewed`
view, then checks that the original `Original` record contains 73.

Fact: a write-only representation view writes 73 to the original record without
granting observation. `&write` is a borrow that permits writing but forbids reading
stored content. `Original` and `Viewed` each contain one unrestricted `u32`, so
their layouts and valid representations match in both directions. The recast
(`storage as &write Viewed`) changes the stated shape of the same storage; it
does not copy the record or grant readable access.

This row supports `DOMAIN-ACCESS-AUTHORITY` and the remaining source migration in
`BUILD-OWNED-TARGET-SELECTION` in `TASKS.md`. The target-body example uses one
`SelectedOverwrite` requirement and four ordinary checked implementations.
Explicit Build rows choose an implementation for each target. The selected body
performs the same recast and store as before; selection grants no extra access.
The public requirement promises no particular written value. The runtime check
observes the actual write after the call returns.

| Place | Positive | Reject twin | How the view travels |
| --- | --- | --- | --- |
| Root | [main.omg](main.omg) | — | Direct recast and write |
| Local | [local/main.omg](local/main.omg) | [invalid](local/invalid/main.omg) | Local binding |
| Field | [field/main.omg](field/main.omg) | [invalid](field/invalid/main.omg) | Record field |
| Parameter | [parameter/main.omg](parameter/main.omg) | [invalid](parameter/invalid/main.omg) | Helper argument |
| Call result | [call_result/main.omg](call_result/main.omg) | [invalid](call_result/invalid/main.omg) | Returned view |
| Match join | [match_join/main.omg](match_join/main.omg) | [invalid](match_join/invalid/main.omg) | Two branches, one with a marked helper call |
| Const name | [const_name/main.omg](const_name/main.omg) | [invalid](const_name/invalid/main.omg) | Store uses the same named value 73 |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [invalid](other_state/invalid/main.omg) | Transition argument |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) | Forwarding helper |
| Selected implementation | [target_body/main.omg](target_body/main.omg) | [invalid](target_body/invalid/main.omg) | Build-selected checked body |

Every positive, including the root, declares `Expected exit: 0` and binds
`ProgramEntry` for all four hosted targets. Every invalid program keeps the same
caller and store, but requests a readable `&mut` view from write-only storage.
That access increase must reject. Each invalid project's `sample.json` records
its intended diagnostic; the nine positive and negative programs are pinned
under `tests/omega/{pass,fail}/facts/write_only_recast_through_*`.

The match example executes both choices. Only its helper branch writes a separate
marker, and the helper's explicit result lifetime keeps the returned view tied to
`storage`. A reversed branch, omitted helper, or wrong backing fails the marker
or original-record check. The store and its bound remain the same in both branches.

## Requirements and evidence

The governing rules are [representation-compatible recasts](../../../../wiki/spec/layouts/recasts.md),
[write-only authority](../../../../wiki/spec/terminal-psi/structural_access.md#write-only-authority),
[checked providers](../../../../wiki/spec/terminal-psi/boundary_calls.md#checked-providers),
and [Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

- Establishment: both ordinary records have the same geometry and unrestricted
  `u32` contents. Writable compatibility holds in both directions without a
  deferred validity window or a caller assertion.
- Identity and lifetime: the view keeps the original backing and full one-field
  footprint through fields, arguments, returned views, joins and transitions.
- Composition: every place performs the same store and checks the original
  backing afterward. The twin changes only the requested view access and its
  receiving types, never an unrelated caller guard.
- Soundness: readable access cannot arise from `&write`. The six
  `tests/omega/fail/recast/write_only_*` controls separately cover shared/readable
  access, stored-pointer observation, source validity, overlap and local escape.
  Checked provider bodies establish non-observation through their call closure;
  opaque alternatives still need their independently admitted judgment.
- Psi lowering: preserve write-only access in Terminal Psi and execute the
  store without observing the old contents.
- Layout, provider and calling plans: retain the one-field record geometry and
  pointer call arguments. Build selects four exact named implementations; the
  caller keeps the same requirement on each target.
- Instructions and frames: preserve backing addresses through calls and joins,
  then store to the original record. The runtime oracle detects a missing store
  or a write to temporary storage.
- Encoding and native execution: actual native and Terminal observations must
  both report exit 0. Target bindings alone do not establish execution.

Historical validation before this target migration ran all nine original places
and their twins on macOS ARM64 after repairing a refusal at `as &write Viewed`
on optimized main `05580b16504`. Those records do not validate the newly selected
provider route or the stronger match example. The first Linux replay ran seven places successfully, including the migrated
selected implementation, but the unchanged returned-view place and the stronger
match place refused because assignment checking lost their relative field
address. The local-escape control also refused before its intended lifetime
check: its caller tried to write through the already-invalid returned view.
That earlier backing obligation remains mandatory. The control now leaves the
result unused so it isolates the unchanged forbidden return of local storage;
its original source and baseline refusal remain in the validation records.
The address repair and this focused control are under validation; the first
replay is not row acceptance.

This row establishes ordinary write-only representation-view transport. It does
not close domain-policy retention under erasure, operation demands, helper grants,
qualification restoration, or opaque write-only provider admission. Those remain
on the owning task board.

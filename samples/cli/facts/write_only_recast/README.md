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

The returned-view and match examples exposed a safe-write refusal: the compiler
kept the borrow but lost the field address below the returned view. It now
retains the field or index chosen when the receiver is evaluated. A containing
storage region stays conservative when the new representation has different
field names, and a copied match subject keeps its own storage. Missing backing
or borrow permission still refuses; no source spelling grants a new address.

| Place | Positive | Reject twin | How the view travels |
| --- | --- | --- | --- |
| Root | [main.omg](main.omg) | — | Direct recast and write |
| Local | [local/main.omg](local/main.omg) | [invalid](invalid_local/main.omg) | Local binding |
| Field | [field/main.omg](field/main.omg) | [invalid](invalid_field/main.omg) | Record field |
| Parameter | [parameter/main.omg](parameter/main.omg) | [invalid](invalid_parameter/main.omg) | Helper argument |
| Call result | [call_result/main.omg](call_result/main.omg) | [invalid](invalid_call_result/main.omg) | Returned view |
| Match join | [match_join/main.omg](match_join/main.omg) | [invalid](invalid_match_join/main.omg) | Two branches, one with a marked helper call |
| Const name | [const_name/main.omg](const_name/main.omg) | [invalid](invalid_const_name/main.omg) | Store uses the same named value 73 |
| Nested machine | [other_state/main.omg](other_state/main.omg) | [invalid](invalid_other_state/main.omg) | Transition argument |
| Other machine | [other_machine/main.omg](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) | Forwarding helper |
| Selected implementation | [target_body/main.omg](target_body/main.omg) | [invalid](invalid_target_body/main.omg) | Build-selected checked body |

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
  Checked provider bodies prove that their calls never observe the write-only
  data. An implementation whose body is unavailable still needs a separately
  admitted proof tied to the selected implementation.
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

[Fresh Linux x86-64 validation](https://github.com/CathedralOS/Omega/actions/runs/41543965070)
observed source `1d21edf56bc4916bfc33876810fc9ff4f5eaab86` with optimized
CLI and corpus producers (`opt_level = 1`). All nine places and their twins
passed their intended runtime or refusal checks. The root plus all 18 place
projects also passed the ordinary public sample commands: ten native/Terminal
successes and nine intended rejections.

The full focused replay checked 27 positives and rejected 32 controls for their
intended reasons; all 27 positives executed with native exit 0, Terminal exit 0,
and agreement. This includes the previous reference-store row and controls for
active child borrows, intermediate pointer reads, opaque boundaries, captured
copies and slice-window coordinates. These are actual observations on Linux;
execution of this repair on the other three hosted targets remains unverified.

The local-escape control keeps its forbidden return and expected lifetime
refusal, but leaves the returned view unused. Its earlier caller also wrote
through that invalid result, correctly failing a preceding backing obligation.
The isolated control now reports that local `original` does not outlive the
call. The original source, refusal and exact producer records remain in the
owning claim evidence; the compiler check order is unchanged.

This row establishes ordinary write-only representation-view transport. It does
not close domain-policy retention under erasure, operation demands, helper grants,
qualification restoration, or opaque write-only provider admission. Those remain
on the owning task board.

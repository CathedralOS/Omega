# Write-only representation views

Fact: a write-only representation view writes 73 to the original record without
granting observation. Parent: `DOMAIN-ACCESS-AUTHORITY` in `TASKS.md`, whose
controlled writer needs an explicitly erased write-only reference view before
delegating to a bare helper. This row establishes ordinary view formation and
transport, not domain-policy retention or routed-membership preservation.

`Original` and `Viewed` have the same one-field `u32` representation. Every
positive writes 73 through the view and then observes 73 in the original backing.
The access-escalation twin changes only the target-view access from `&write`
to `&mut`, consistently at the recast and receiving types; its source remains
`&write Original`. No caller guard supplies access or representation evidence.

Acceptance is `omega run --both <place>/main.omg` with native/Terminal exit 0,
plus refusal of each `invalid/main.omg` for attempted readable access. All four
hosted `ProgramEntry` bindings are present. Places: local, field, parameter,
call_result, match_join, const_name, other_state, other_machine, target_body.
The const-name place supplies the same 73 from `WRITTEN`; runtime-exclusive
references are not fabricated as constants.

## Evidence and remaining scope

Optimized main `05580b16504` refused the formed positive at `as &write Viewed`.
The repair keeps the reference-access axis through the common recast operation,
retains write-only local referent types, binds returned pointers through the
ordinary stored-loan classifier, and keeps arm-private offset operands out of
the scalar join roster. All nine unchanged positives execute with native and
Terminal exit 0 on macOS ARM64; their nine readable-view twins reject. Exact
producer identities and raw runs are retained in the owning qualification.

The row is pinned under
`tests/omega/{pass,fail}/facts/write_only_recast_through_<place>_{exit,rejected}`.
Additional controls under `tests/omega/fail/recast/write_only_*` refuse shared
access escalation, direct observation, stored-pointer observation, loss of
source validity, overlapping exclusive loans and escape of local backing.
Package codec units pin distinct access tags and bounded recovery. No execution
on the other three hosted targets is claimed.

This closes ordinary write-only representation-view transport, not the parent
domain-access policy. Policy retention under erasure, actual operation demands,
scoped helper grants and qualification restoration under the existing
[invariant-window rules](../../../../wiki/spec/language/dependent_values.md#invariant-windows)
remain separate implementation work.

## Required evidence

- Establishment: source and target ordinary validity, equal geometry and both
  directions of writable representation compatibility; no unchecked conversion.
- Identity/lifetime: retain the original backing and complete footprint across
  constructed fields, helper arguments/results, joins and state transitions.
- Composition: use the same write and observed result through all nine places;
  preserve exact reference access rather than collapsing it to exclusivity.
- Soundness: refuse read/escalation, invalid source restoration, overlapping
  loans and escaping returned views; retain atomic/Placed restrictions.
- Psi lowering: preserve `WriteOnlyBorrow` through canonical Terminal Psi and
  execute its actual store without loading old content.
- Layout/provider/ABI: ordinary one-field record geometry and pointer calls;
  four hosted entry bindings, with execution claims only for observed hosts.
- Instructions/frames: ordinary store, calls and preserved backing addresses.
- Encoding/native: actual native exit 0 agrees with Terminal; package-review
  encoding/recovery must distinguish write-only from readable mutable access.

Return to `domain_access_authority` and Cathedral after this prerequisite.

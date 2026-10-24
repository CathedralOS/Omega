# Tagged scalar-field loans

Parent: **REFERENCE-LOCAL-BINDING-ROUTES** in [TASKS.md](../../../../TASKS.md).
This also supplies the ordinary-loan prerequisite exposed while implementing
**DOMAIN-ACCESS-AUTHORITY** for Cathedral; it does not implement access policy.

Fact: a shared tagged scalar-field loan keeps its original backing and value 73
while computing `value ^ 0`.

The row exercises the loan through [a local](local/main.omg),
[a field](field/main.omg), [a parameter](parameter/main.omg),
[a call result](call_result/main.omg), [a match join](match_join/main.omg),
[a constant name](const_name/main.omg), [another state](other_state/main.omg),
[another machine](other_machine/main.omg), and
[target-scoped bodies](target_body/main.omg).
Each sibling `invalid/main.omg` adds only a write to the borrowed backing before
the loan's last use. It must reject for that active loan, not an unrelated error.
The constant-name place obtains the value 73 from `VALUE`, constructs ordinary
local backing and then borrows its field. It does not declare a constant
reference descriptor or constant mutable storage.

On content-backed clean main `bbc416b7192`, the original customer reduction
checks but refuses its qualified field loan at local-data lowering; removing
only its two qualifications gives native/Terminal exit 0. The original source
and exit oracle are pinned unchanged as
`pass/borrows/tagged_scalar_field_live_copy_exit`. Timestamp-backed historical
comparisons are diagnostic only; the retained content-backed producers establish
the baseline and repaired behavior.

The repair belongs to storage identity: positively classified scalar tags do not
create a second unrestricted scalar declaration. The exact typed qualification
and normalized application remain semantic identities. Target-family membership
also follows its published formal's structural/scalar namespace; a scalar
observation through a reference cannot select a nonexistent scalar formal slot.
Complete-graph retention also follows ordinary moves after a source descriptor's
membership roster grows. That query selects the exact current value occurrence,
not a descriptor's backing or another value with equal storage geometry.
Copying a reference descriptor also uses that reference's referent carrier and
result-relative `Referent` path for checked memberships, not the pointer carrier.
The chained two-tag reference control retains independent sibling backings.

## Requirement evidence

- Establishment: `Controlled` is a predicate-free, route-free declared tag.
  Ordinary formation checks backing, access and lifetime independently; the
  runtime observation does not establish membership retroactively.
- Identity and lifetime: each reference denotes the original selected field;
  its reject twin attempts to mutate that same field while the loan is live.
  Match alternatives retain their own backing. No source copy substitutes for
  a reference descriptor.
- Composition: qualification survives each of the nine places into the same
  observation operation. Call-result lifetimes and state transfers retain the
  receiving reference. Indexed tags, tag aliases and real receiving predicates
  have additional focused fixtures in `tests/omega/{pass,fail}/domains`.
- Soundness: physical identity does not merge domain identities or introduce
  routed evidence. Predicates, ranges, unknown and routed constraints keep their
  receiving storage identity. Neither access-policy admission fence changes.
- Psi 05–07: ordinary reference establishment, descriptor transport and cleanup
  lower into Terminal Psi; acceptance requires actual interpreter/native
  agreement, not source checking alone.
- Omega 00–02: every positive binds all four hosted `ProgramEntry` targets;
  borrowed argument and result placement use the ordinary calling-plan route.
- Omega 03–06: scalar observation and calls preserve the value across live
  reference homes, activation storage and the selected target's ABI.
- Omega 07–09: native emission and execution must report exit 0, agreeing with
  Terminal Psi. Matching-host execution establishes only that host's row;
  bindings and cross-compilation do not qualify another host.

Acceptance: `omega run --both <place>/main.omg`, exit 0 in both engines, and
the corresponding `<place>/invalid/main.omg` rejected for the active loan.
The same sources and twins are pinned under
`tests/omega/{pass,fail}/facts/scalar_tag_field_loan_through_*`.

This row does not close the parent's other host/authority gates, domain access
policy, stronger predicate introduction onto bare storage, or Cathedral's next
mapping-custody milestone. Keep producer/target-bound run records with the
qualification and commit rather than interpreting these declarations as a
release-matrix claim.

An independent record-owner probe exposed existing stale source-roster seeds
around tagged aggregate moves after mutation. The additive move-retention repair
does not remove such earlier seeds or establish their correctness. Source
rejection of a stale issuer use is a separate boundary, not proof that every
retained Terminal row is current. Likewise, nested routed facts across a newly
tagged owner cast are not qualified by this row.

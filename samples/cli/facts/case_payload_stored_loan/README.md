# Stored case-payload loan writeback

A returned variant carries the exclusive loan to its original backing; writing 41 through the active payload changes only that backing.

Parent: **STATE-LOCAL-VALUE-FRONTIER**, aggregate construction, replacement and
reads. This is stored-reference payload transport, not the neighbouring
`case_payload_exclusive_loan` row's borrow of an ordinary scalar payload.
[Carried-loan transport](../../../../wiki/spec/language/lifetimes.md#carried-loan-transport)
retains exact reference leaves through active sum payloads; extracting a leaf
and reborrowing it for an ordinary helper must retain its backing lifetime.
Lifetime evidence supplies neither a new loan nor an address.

## Acceptance

Run `omega run --both samples/cli/facts/case_payload_stored_loan/main.omg`.
The root must write 41 into the caller's original Cell, preserve the sibling's
19, and exit 0 in both Terminal Psi and native execution. The consuming store
requires its input equal 41. Each place's twin changes only the later
`store_code(&sibling)` operand to `store_code(&backing)`: the same code still
returns 41, but reading the original backing conflicts with the first operand's
captured exclusive loan. No caller equality guard re-establishes that loan.

| Place | Positive | Overlap reject |
| --- | --- | --- |
| Local | [program](local/main.omg) | [twin](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [twin](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| A nested machine | [program](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| Another target body | [program](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

The nine programs and twins are pinned under
[`tests/omega/pass/facts`](../../../../tests/omega/pass/facts) and
[`tests/omega/fail/facts`](../../../../tests/omega/fail/facts), named
`case_payload_stored_loan_through_<place>_{exit,rejected}`.
Run `python3 tools/corpus_gate.py --both --filter case_payload_stored_loan`.

The const-name place supplies the operation's value 41 through `WRITE_VALUE`;
it does not store an activation-relative exclusive loan in a constant.
[Constants](../../../../wiki/spec/language/constants.md) are copyable
compile-time values without runtime storage or authority, so the loan still
belongs to the runtime backing. This is named-operand composition, as in the
reference length row's named extent, not evidence of const loan storage.

## Baseline and mechanism

Fresh optimized main `60453171ca5f23df9feff9c236a4667d47f4456f` rejects the
unchanged root during checking:

    cannot plan contextual call cleanup against the executable hook roster:
    machine `consume`, state `entry`, statement 1, call 0:
    temporary operand has incomplete backing lifetime coverage

Passing the payload loan by value instead of reborrowing it checks; adding a
common scalar field to the sum does not repair the original reborrow. These
are diagnostic probes, not replacement acceptance programs. The next action
was to repair the shared captured-lifetime operation, not change the program.

An admitted input owns its complete lifetime frontier. Lifetime capture queries
each selected leaf's reaching store: a replacement keeps its completed capture,
an unknown remains unknown, and an unchanged leaf keeps the input dependency.
This lifetime-only fallback supplies neither an address nor borrow authority.

Another-state forwarding also exposed a distinct result-publication gap: the
finite state relation preserves an opaque sum as a whole moved value, while
the publisher requested flattened reference origins. It now carries both
formed references and whole-value moves. Exact completed operands determine
payload presence and source substitution; returned-loan rows still authenticate
access, owner paths and backing. A move on one return arm cannot mask another
arm's constructed or unresolved origin.

## Premise controls

- [Reordered state inputs](controls/reordered_state/main.omg) returns the
  second argument after permuting formal positions. It exercises both an active
  payload and an empty result whose unused sibling carries a live loan.
- [Mixed return routes](controls/mixed_state/main.omg) selects between a moved
  sum and a freshly constructed payload backed by another parameter. It runs
  both routes and distinguishes their original backings. A draft that masked
  references beneath a moved result passed native execution but failed Terminal
  backing correspondence; this control exposed that defect before publication.
- [Reference replacement](controls/replaced_leaves/main.omg) replaces one
  nested record/array input descriptor and preserves its sibling. The prior
  incoming descriptor is not a loan formed at statement zero. Its retirement
  needs exact input identity, descriptor—not referent—geometry, and a current
  version after preceding and RHS writes. Later installed descriptors retain
  their ordinary formation/weakening route.
- [Repeated replacement and retained copies](controls/repeated_replacement/main.omg)
  keeps a copy of the incoming descriptor, replaces the slot twice, and checks
  the original 17, intermediate 31, final 41 and untouched sibling 19. Ending
  the selected carrier must not end these independent shared copies.
- [Selective guard execution](controls/guard_order/main.omg) observes a skipped
  mutating RHS and a selected RHS exactly once. This already passed the baseline
  and is a regression control, not evidence of a necessary guard repair.
- [Mutable owned operands](controls/mutable_owned_arguments/main.omg) rebinds
  an owned formal, captures its copy as the first computed-call argument, and
  overwrites the source in the second argument. The receiving mutable binding
  still sees 7, not 9. Its [twin](controls/mutable_owned_arguments/invalid/main.omg)
  removes only copy permission and rejects the use after transfer. Binding
  mutability selects neither ownership nor borrowing access.

## Requirements established and boundaries

- **Establishment:** `lend` constructs the active payload from the caller's
  explicit exclusive loan. `store` consumes the carried reference and its
  independent value-41 requirement.
- **Identity and lifetime:** the same backing changes from 17 to 41; the
  sibling remains 19. The rejects access that exact original backing while its
  exclusive loan is live, rather than failing an unrelated value contract.
- **Composition:** fields, calls, joins, state arrivals and target-scoped bodies
  preserve the payload's stored loan. The state controls challenge formal
  reordering, absent cases and unions of distinct return routes.
- **Soundness:** lifetime roots never stand in for addresses, loans or ownership
  permission. Descriptor replacement retires its own current carrier, not its
  referent or independent copies. No erased mathematics or new proof rule is
  introduced.
- **Psi 05–07:** the admitted programs must publish canonical Terminal Psi and
  execute its actual loan, call, state and cleanup operations through `--both`.
  A source check or native-only result is not acceptance.
- **Omega 00–02:** Build entry settlement, tagged payload/record/array layouts
  and ordinary borrowed/owned call placements are exercised by execution.
  This does not qualify arbitrary layouts, foreign calls or stack-argument lanes.
- **Omega 03–06:** actual arm64 selection, register/frame homes and call
  preservation must realize these programs. This row makes no claim about
  register pressure, bounded stack usage or other instruction families.
- **Omega 07–09 and emission:** emitted macOS arm64 images must exit 0 and
  agree with Terminal Psi. Every positive binds all four hosted targets, but
  bindings alone are not execution evidence on Windows or Linux.

Actual producer revisions, optimized executable digests, raw engine results and
reject diagnostics are retained in the qualification/claim record for
`bb45840a447b4899b4e5cc8f868fa31b`. Earlier failed drafts and package-setup
attempts are not passing evidence. The parent outcome remains open for its
other aggregate, opaque-reference and expression-composition requirements.
RHS self-replacement is now exercised by
[`descriptor_replacement_order`](../descriptor_replacement_order/README.md).
Its local witness runs with native and Terminal exit zero on producer `ac9b6d8d9762`;
the earlier structural-destination-window refusal is historical evidence.


## Selected guard completion precedes payload operands

On optimized producer `ac9b6d8d97625b890927868616fbec23cce11560`, all nine
unchanged positives refuse the payload reborrow: a shared temporary used to test
the case remains lexically live through the missed path's later guard. The
selected path has already completed that observation and does not use its
carrier again. Calls and moves recognized the checked release boundary, while
ordinary operand observations did not.

The shared occurrence inventory now retains whether an access belongs to the
guard or an exact selected target. Observations, calls and moves use the same
retirement query. It requires the completed guard read, exact checked release,
no target use and no surviving descendant dependency. The captured payload still
refers to its original exclusive backing; it does not borrow through the shared
guard temporary. Lowering retains its existing full-graph verification before
moving the actual release onto that continuation. No source-name exception or
new cleanup representation is introduced.

All nine positives again return zero in native and Terminal execution on macOS
ARM64; all nine conflicting-backing twins still reject. The new
[`guard_mutation_keeps_observation`](controls/guard_mutation_keeps_observation/main.omg)
control attempts mutation during the guard and must keep rejecting. The existing
`guard_subject_alias_held_through_call` control keeps the independent live alias.
The fact sentence, original sources and their corpus pins remain unchanged.
Other-host execution and the parent capability's remaining runtime/case
construction, replacement and saved-observation acceptance remain open.

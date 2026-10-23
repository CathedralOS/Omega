# Stored exclusive byte-view observation

An exclusive byte view stored in a fixed array preserves its one-byte extent
and byte 19 through owned transport.

Parent: **STATE-LOCAL-VALUE-FRONTIER**, aggregate construction, replacement
and reads: nested byte observations below exclusive descriptor arrays.
This is the missing stored fat-descriptor observation mechanism, not another
array-size row or permission to copy exclusive access.

The root witness reads the exact stored view's length and first byte before
testing its authored exit-zero oracle. It does not use a caller equality guard
to establish an indexing proof. Canonical contracts retain the complete
descriptor source path and access for [byte observations](../../../../wiki/spec/terminal-psi/byte_views.md)
and preserve original referents across [structural projections](../../../../wiki/spec/terminal-psi/structural_access.md).

## Witnesses and refusal controls

Each place retains the sentence above, the one-byte bound and byte observation.
Its reject twin changes only the constructed view to an empty slice: the
operation must fail its own bound or receiving contract, not a caller equality
guard. Each pair is pinned under [pass fixtures](../../../../tests/omega/pass/facts/)
and [fail fixtures](../../../../tests/omega/fail/facts/) as
`stored_exclusive_byte_view_through_<place>_{exit,rejected}`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [empty view](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [empty view](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [empty view](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [empty view](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [empty view](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [empty view](const_name/invalid/main.omg) |
| Another state | [program](other_state/main.omg) | [empty view](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [empty view](other_machine/invalid/main.omg) |
| Another target body | [program](target_body/main.omg) | [empty view](target_body/invalid/main.omg) |

The [root](main.omg) and [root reject](invalid/main.omg) retain the initial
observation. After ordinary project preparation, run each positive with
`omega run --both <place>/main.omg`. Its authored result is exit 0 in both engines.

## Requirements established by this row

- **Establishment:** construction carries the completed element's actual extent.
  Parameter/state requirements and the relay's result equation transport it into
  indexing; an element's capacity or a sibling's length is not evidence.
- **Identity and lifetime:** descriptor homes remain distinct from original
  backing. [Window/writeback](controls/window/main.omg),
  [nested arrays](controls/nested_array/main.omg) and
  [callee writeback](controls/backing_write/main.omg) retain offset, extent and
  access through owned moves. [Replacement](controls/replacement/main.omg)
  preserves a saved scalar extent while retiring obsolete live-slot facts;
  [stale](controls/stale_extent/main.omg) and
  [sibling](controls/sibling_extent/main.omg) extents reject.
- **Composition:** fields, actual/formal substitution, completed results,
  selected arrivals, state edges and target-scoped bodies carry the same fact.
  [Owned-record contracts](controls/owned_record/main.omg) retain projected
  metadata without granting borrowed referent-content observation authority.
- **Soundness boundaries:** exclusive and write-only carriers cannot become
  copyable because their referents are copyable. Their
  [exclusive](controls/owned_copy/main.omg) and
  [write-only](controls/write_only_copy/main.omg) duplicate moves reject;
  [shared copying](controls/shared_copy/main.omg) and
  [direct reborrowing](controls/direct_reborrow/main.omg) remain valid. This
  grants no new loan authority, mathematical equality or termination rule.
- **Checked-to-Terminal Psi (`Psi 05–07`):** byte observations retain complete
  array/reference paths and independently proved bounds. Canonical artifact
  inspection admits the portable product; `run --both` executes its semantics.
  Selected ownership follows ordinary ordered evaluation rather than a
  subject-purity bypass. Controls exercise [effects](controls/match_effectful_subject/main.omg),
  [borrows](controls/match_borrowed_subject/main.omg),
  [replacement claims](controls/match_parameter_replacement/main.omg), and
  [skipped-source residual cleanup](controls/match_skipped_replacement/main.omg).
  The last control observes cleanup through both old and replacement backings.
  [Projected claims](controls/match_projected_claims/main.omg) swap two linear
  children while an untouched sibling's cleanup runs exactly once. A
  [duplicate move](controls/match_duplicated_projected_claim/main.omg) rejects,
  and an [unretired sibling](controls/match_unretired_projected_sibling/main.omg)
  still owes its own exact cleanup prerequisite.
- **Provider, layout and ABI (`Omega 00–02`):** authored Build entries use the
  normal provider route. Fixed-array storage and stored descriptors survive
  calls/results and states through ordinary layout and calling plans. This is
  not evidence for every ABI placement lane.
- **Instructions, register homes and frames (`Omega 03–06`):** native execution
  performs the actual length/byte observations, calls and writes. Agreement and
  original-backing controls exercise realized storage, not exhaustive register
  pressure, frame-bound or instruction coverage.
- **Encoding and native images (`Omega 07–09` and emission):** emitted host images
  execute the authored oracle and agree with Terminal. No codec format changes;
  not every relaxation or relocation boundary is exercised by this row.

## Evidence and remaining scope

Fresh qualification records retain producer revision/digest, optimized build
profile, source/oracle identities, actual engine exits, canonical artifact
decodes and intended reject diagnostics. Baseline and candidate corpus records
distinguish the repaired contract/lowering/runtime failures from controls that
already passed; checking success alone does not establish execution.

Execution evidence here is **macOS AArch64**. Every positive binds all four
hosted targets, and Psi checks every target-scoped body; those declarations are
not Windows or Linux execution evidence. Windows native execution is explicitly
deferred by the owner to a later Windows agent. Other hosted native runs remain
unverified.

The parent capability remains open: opaque incoming referents still need retained
inner-loan evidence. This row does not close arbitrary partial-owner/cyclic
composition or the whole compiler release matrix.

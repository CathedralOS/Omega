# Stored exclusive byte-view observation

An exclusive byte view stored in a fixed array preserves its one-byte extent
and byte 19 through owned transport.

Parent: **STATE-LOCAL-VALUE-FRONTIER**, aggregate construction, replacement
and reads: nested byte observations below exclusive descriptor arrays.
This is the missing stored fat-descriptor observation mechanism, not another
array-size row or permission to copy exclusive access.

The holder production adds a borrowed record field around the descriptor array.
Its callee observes the original view through both pointer layers. The same
one-byte extent and byte-19 operation survive each transport place, including
a mutable match destination and a returned holder. Calls retain the storage
cut after all operands complete; lifetimes and element types alone establish
no descriptor count.

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
| Local | [program](local/main.omg), [holder](local/borrowed_holder/main.omg) | [empty view](local/invalid/main.omg), [empty held view](local/borrowed_holder/invalid/main.omg) |
| Field | [program](field/main.omg), [holder](field/borrowed_holder/main.omg) | [empty view](field/invalid/main.omg), [empty held view](field/borrowed_holder/invalid/main.omg) |
| Parameter | [program](parameter/main.omg), [holder](parameter/borrowed_holder/main.omg) | [empty view](parameter/invalid/main.omg), [empty held view](parameter/borrowed_holder/invalid/main.omg) |
| Call result | [program](call_result/main.omg), [holder](call_result/borrowed_holder/main.omg) | [empty view](call_result/invalid/main.omg), [empty held view](call_result/borrowed_holder/invalid/main.omg) |
| Match join | [program](match_join/main.omg), [holder](match_join/borrowed_holder/main.omg) | [empty view](match_join/invalid/main.omg), [empty held view](match_join/borrowed_holder/invalid/main.omg) |
| Const name | [program](const_name/main.omg), [holder](const_name/borrowed_holder/main.omg) | [empty view](const_name/invalid/main.omg), [empty held view](const_name/borrowed_holder/invalid/main.omg) |
| A nested machine | [program](other_state/main.omg), [holder](other_state/borrowed_holder/main.omg) | [empty view](other_state/invalid/main.omg), [empty held view](other_state/borrowed_holder/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg), [holder](other_machine/borrowed_holder/main.omg) | [empty view](other_machine/invalid/main.omg), [empty held view](other_machine/borrowed_holder/invalid/main.omg) |
| Another target body | [program](target_body/main.omg), [holder](target_body/borrowed_holder/main.omg) | [empty view](target_body/invalid/main.omg), [empty held view](target_body/borrowed_holder/invalid/main.omg) |

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
  [Unknown incoming extents](controls/borrowed_holder_unknown_extent/main.omg),
  [replaced extents](controls/borrowed_holder_stale_extent/main.omg), and
  [selected replacement extents](controls/borrowed_holder_selected_stale_extent/main.omg)
  still reject. [Shared holder observation](controls/borrowed_holder_shared/main.omg)
  and [selected-holder replacement](controls/borrowed_holder_selected_replacement/main.omg)
  exercise access attenuation and the current receiving generation.
- **Checked-to-Terminal Psi (`Psi 05–07`):** byte observations retain complete
  array/reference paths and independently proved bounds. Canonical artifact
  inspection admits the portable product; `run --both` executes its semantics.
  Selected ownership follows ordinary ordered evaluation rather than a
  subject-purity bypass. Controls exercise [effects](controls/match_effectful_subject/main.omg),
  [borrows](controls/match_borrowed_subject/main.omg),
  [replacement claims](controls/match_parameter_replacement/main.omg), and
  [skipped-source residual cleanup](controls/match_skipped_replacement/main.omg).
  The last control observes cleanup through both old and replacement backings.
  Its conditional call transports the incoming descriptor's own supplier into
  the replaced field; resolving that descriptor home to its backing before
  transfer would incorrectly create a competing loan on the skipped route. Its
  [reuse control](controls/match_skipped_replacement/invalid/main.omg) rejects
  reading the incoming value after that possible transfer.
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
  execute the authored oracle and agree with Terminal. Scalar-read projection
  tag `6` retains a descriptor extent after explicit reference crossings; the
  receiver validates the complete declared path and readable carrier. Neither
  the projection nor its type supplies truth, bounds or loan authority;
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

The parent capability remains open for additional inner-loan capture and escape
through opaque incoming referents. Observing a retained descriptor does not grant
those operations. This row does not close arbitrary partial-owner/cyclic
composition or the whole compiler release matrix.

## Shared incoming record continuation

The same fact is also carried by an owned wrapper whose field is a shared
reference to the record containing the exclusive descriptor array. This is
another production of this row: the bound, byte, operation and fact sentence
stay the same. Each `<place>/nested_reference/main.omg` has its own empty-view
twin and matching `stored_exclusive_byte_view_nested_reference_through_<place>`
pass/fail fixtures.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/nested_reference/main.omg) | [empty view](local/nested_reference/invalid/main.omg) |
| Field | [program](field/nested_reference/main.omg) | [empty view](field/nested_reference/invalid/main.omg) |
| Parameter | [program](parameter/nested_reference/main.omg) | [empty view](parameter/nested_reference/invalid/main.omg) |
| Call result | [program](call_result/nested_reference/main.omg) | [empty view](call_result/nested_reference/invalid/main.omg) |
| Match join | [program](match_join/nested_reference/main.omg) | [empty view](match_join/nested_reference/invalid/main.omg) |
| Const name | [program](const_name/nested_reference/main.omg) | [empty view](const_name/nested_reference/invalid/main.omg) |
| Nested machine | [program](other_state/nested_reference/main.omg) | [empty view](other_state/nested_reference/invalid/main.omg) |
| Another machine | [program](other_machine/nested_reference/main.omg) | [empty view](other_machine/nested_reference/invalid/main.omg) |
| Another target body | [program](target_body/nested_reference/main.omg) | [empty view](target_body/nested_reference/invalid/main.omg) |

The [direct incoming record](controls/opaque_incoming_record/main.omg) separates
ordinary incoming inline storage from the additional shared-reference crossing
in the [nested control](controls/opaque_incoming_record/nested/main.omg).
On optimized Windows producer `7df8b4d92070d83d357849fb2eb77e05c61f36a4`,
the direct control exits zero in both engines. All nine nested positives wrongly
refuse the receiving `len == 1` requirement; their empty-view twins refuse the
same requirement correctly. These are frozen baseline observations, not
candidate acceptance. The call-result place publishes the selected descriptor's
length through its authored result equation; [removing that guarantee](controls/opaque_incoming_record/nested/without_extent_guarantee/main.omg)
must still refuse the receiving requirement.

A checked shared borrow records the exact referent's inline cells. Contained
references remain permission carriers and require independent observations
before another crossing. Selected result contracts publish descriptor metadata
without publishing backing bytes. Terminal Psi retains each explicit reference
crossing in the scalar observation path; its `Extent` projection selects only
the readable descriptor's live `u64` length. This extends the scalar-read wire
vocabulary, independently of the earlier canonical `ViewExtent` encoding.
Native reference arguments continue to address their original backing storage.

The [opaque result's owned field](call_result/nested_reference/projected_record/main.omg)
retains the same length equation after projecting its wrapper. Its
[empty-view twin](call_result/nested_reference/projected_record/invalid/main.omg)
and [missing-guarantee control](call_result/nested_reference/projected_record/without_extent_guarantee/main.omg)
still owe the receiving length requirement. A captured result's authenticated
owned projections select their declaration-bound carrier; crossing a stored
reference cannot manufacture another owned result root.

The [opaque result's fixed array element](call_result/nested_reference/projected_array/main.omg)
retains the same length equation after indexing an owned container's array of
wrappers. Its [empty-view twin](call_result/nested_reference/projected_array/invalid/main.omg)
rejects the receiving length requirement. Fixed index projection segments on
call-argument reference origins preserve coarse collections across formal selection,
and completed value equations accept constructors carrying observed shared references.

Current candidate validation and parent acceptance remain open until the
corresponding execution and refusal records establish them. Retained macOS
observations above remain attached to their historical producer. Windows
observations do not establish other hosts' native execution or authorize the
unrelated pending production-package decisions.

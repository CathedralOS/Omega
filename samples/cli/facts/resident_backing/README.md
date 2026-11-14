# Resident backing survives transport

Current identity migration: **PROOF-KERNEL-CORE**. The existing backing and
allocator acceptance remains owned by **BUMP-ALLOCATOR-CANARY** in `TASKS.md`.

Fact: **A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.**

The complete Granted/Vacant content equations now name
`core::Identity<IntervalSet<Nat>>`, using the exact projections declared by
core::extent. This compares canonical sets; it is not fieldwise descriptor
identity and grants no backing, vacancy, placement or access authority. All
existing allocation, profile, geometry, qualification, lifecycle, cleanup and
neighbor-canary checks remain separate.

This source checkpoint has preservation audits only. Integrated optimized
checking, native/interpreter agreement, intended control refusals and clean
corpus recording remain required. All earlier execution observations below
retain their original source identity and do not qualify this migrated source.

Each place obtains 64 actual zeroed bytes from the admitted host allocator,
establishes Granted and Vacant independently, partitions into two 32-byte
windows, and transports both windows. Each window initializes a 24-byte record
with values `101, 17, 103`, writes the middle field to 29, and extracts it. Exit
0 requires the original value 17, new value 29, unchanged neighbors 101/103,
and both returned lengths 32. The match place exercises both branches with
different marker values. In `const_name`, only `CAPACITY = 64` is constant;
the qualified extents and their lifecycle still run at runtime. The existing
program and oracle remain runtime coverage. Transport of this same backing fact
through a constant initializer remains open, so the row is unfinished.

Completing the constant place needs admitted local backing for the same
initialization, write and extraction, exporting only owned observations.
[Semantic evaluation](../../../../wiki/spec/language/evaluation.md#invocation-admission)
permits local allocation and temporary borrowing under the complete invocation
contract; [constant results](../../../../wiki/spec/language/constants.md) cannot
export references, storage identity or owned claims.
[Ownership](../../../../wiki/spec/language/ownership.md) still requires exact
claim provenance.

The nearest rows are `placed_stable_write` and `content_reunion`. The former
uses program-local structural storage rather than actual provider allocation
bytes. The latter establishes numeric content conservation, explicitly leaving
physical backing open. This row joins those independent requirements: numeric
geometry, source qualification, actual storage, and its admitted profile are
not interchangeable evidence. Its twins omit Vacant establishment and must
reject the attempted annotation, rather than merely denying a field operation.

| Place | Positive | Missing vacancy |
| --- | --- | --- |
| Local | [main](local/main.omg) | [invalid](invalid_local/main.omg) |
| Field | [main](field/main.omg) | [invalid](invalid_field/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| Const name | [main](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| Nested machine | [main](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| Other machine | [main](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| Selected target implementation | [main](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

## Requirements and evidence

- Establishment: admitted root issuance and a separate vacancy transfer precede
  initialization. The receiving annotation never proves itself.
- Identity and lifetime: both windows refer to one real allocation; split,
  transport, initialization and extraction preserve their exact ranges.
- Composition: owned fields, ordinary calls and returns, joins, nested machines
  and a Build-selected implementation carry the same qualified extents.
- Soundness: the host supplies existing bytes only. It does not establish guest
  qualifications or ownership; the source must already prove those. Field
  authority is bounded by the host's actual stable-RAM profile. No resident
  backing transfer across independent component instances is claimed here.
- Psi: actual host bytes are joined at the selected resource-profile crossing;
  writes and extraction observe those bytes, not a detached initialized copy.
- Provider/layout/ABI: checked three-field geometry and selected providers feed
  the same resident lifecycle on each engine. All four hosted entries and
  allocation imports are bound. Runtime observations identify the host tested;
  having target bindings alone does not establish cross-host execution.
- Selection/frames: simultaneous owned windows, projected field operations and
  aggregate extraction exercise real retained storage and their native homes.
- Encoding/runtime: `omega run --both` must return 0 on both engines. The middle
  update and neighbor canaries make incorrect offsets observable.

Run each place with `omega run --both samples/cli/facts/resident_backing/<place>/main.omg`.
Check each corresponding `invalid/main.omg`; its diagnostic must identify
missing `Extent::Vacant` establishment. Historical Windows x86-64 corpus
records retain all nine runtime programs at native/Terminal exit 0 with
agreement under producer `9f1ea2b3a8e1`. Missing-vacancy rejection records retain
`b3876048c0a2`, except the field twin at `e7c12b127bc8+wt`. These observations keep
their original producer identities; they do not establish constant transport or
current-source acceptance. The programs remain under `facts/resident_backing_*`.

The [coexisting-allocation control](coexisting/main.omg) initializes both
residents before extracting either. It writes the first, observes that the
second remains unchanged, then writes and extracts the second. It recomposes
the two vacant claims through an ordinary checked machine, initializes the
whole range again with 97, and extracts 97. Its address-distance check declares
the 64-bit address-model premise satisfied by the four bound hosted targets;
it does not assume counts and addresses have the same width in general.
Both engines return 0. Host-adapter tests separately check that retirement invalidates every
retained handle and that unknown or out-of-bounds ranges supply no storage.

The [package allocator customer](../../../../tests/omega/pass/memory/bump_allocator_canary/README.md)
extends this lifecycle through the ordinary Bump package: exhausted allocation,
release without cursor rewind, checked recomposition, full reset, and reuse.
Generic Vec element ownership and cleanup remain the parent outcome's acceptance.

The [generic-move production](generic_move/README.md) extracts and reinstalls a
complete element through generic helpers and returns both storage claims in one
record. Its affine composition observes exactly-once cleanup; its controls reject
a different placement identity and reuse of the consumed element.

The [library-growth production](library_growth/README.md) runs that lifecycle
through the alloc package's runtime-sized u32 vector. The application provides
backing and its selected profile; the library preserves the initialized prefix
and returns old storage for explicit release. Its twin omits vacancy issuance.

The [runtime-cell production](runtime_cells/README.md) partitions at runtime,
updates actual backing through a qualification-erasing view, and rejoins the
original descriptor. Its bounds twin rejects a cell extending past the end.
The accompanying ordinary-RAM control preserves an untouched neighbor but
rejects a stale fact about the written field; content-only writes must not
accidentally become the rule for every access view.

The [runtime-loop production](runtime_loop/README.md) forwards two distinct
allocation descriptors through a terminating loop before rejoining and using
their backing. Its twin swaps the outputs and must reject the identity claims.

The [nested helper-loop production](runtime_helper_loop/README.md) carries both
backings through helper-call results on a backedge. It checks their distinct
contents after zero and repeated iterations; its twin repeats a consumed input
and must reject the duplicate transfer. The reconstruction production below
also checks recursive descriptor guarantees; the Vec copy/reallocation lifecycle
remains the parent outcome's acceptance.

The [reconstruction-loop production](runtime_reconstruction_loop/README.md)
rebuilds both descriptors through checked content-conserving helpers on the
backedge. Its direct and recursive exits have different claim origins; each
completed call must still return its complete owned result. Its twin changes
the reconstructed length and must reject the unchanged conservation contract.

The [runtime-growth customer](runtime_growth/README.md) uses the Bump package
to copy live elements into larger backing and append another element. The
recursive descriptor guarantees directly establish the grown vector's invariant;
its twin swaps the recursive backing arguments and must reject. It then releases
both allocations, resets from their recomposed backing, and reuses the whole
48-byte allocation with readback before releasing and resetting again.

The field place captures its qualified extent through an ordinary call both
before reconstructing `Held` and while restoring a borrowed `Held`. Its default
domain consumes the returned length guarantee and the saved entry count. The
typed projection of an authenticated referent snapshot must agree with ordinary
owned projection; neither a reference descriptor nor an equal constructor can
supply missing Granted or Vacant membership. The
[unrestored control](field/invalid_restoration/main.omg) replaces the saved count
with zero and must reject at invariant-window closure. Together these exercise
the owned-record transfer and borrowed restoration needed by the allocator and
Vec, using the same real backing and neighbor-canary oracle as the field place.

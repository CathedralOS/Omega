# Resident backing survives transport

Owning outcome: **BUMP-ALLOCATOR-CANARY** in `TASKS.md`.

Fact: **A granted vacant extent retains actual backing through transport,
initialization, permitted writes, and extraction.**

Each place obtains 64 actual zeroed bytes from the admitted host allocator,
establishes Granted and Vacant independently, partitions into two 32-byte
windows, and transports both windows. Each window initializes a 24-byte record
with values `101, 17, 103`, writes the middle field to 29, and extracts it. Exit
0 requires the original value 17, new value 29, unchanged neighbors 101/103,
and both returned lengths 32. The match place exercises both branches with
different marker values. The const place names the allocation bound; it does
not pretend a runtime-owned allocation can be a compile-time constant.

The nearest rows are `placed_stable_write` and `content_reunion`. The former
uses program-local structural storage rather than actual provider allocation
bytes. The latter establishes numeric content conservation, explicitly leaving
physical backing open. This row joins those independent requirements: numeric
geometry, source qualification, actual storage, and its admitted profile are
not interchangeable evidence. Its twins omit Vacant establishment and must
reject the attempted annotation, rather than merely denying a field operation.

| Place | Positive | Missing vacancy |
| --- | --- | --- |
| Local | [main](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [main](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Selected target implementation | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

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
missing `Extent::Vacant` establishment. The nine positives execute with exit 0
on both engines on Windows x86-64, and all nine missing-vacancy twins reject.
The corpus retains those same programs under `facts/resident_backing_*`.

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
Vec growth/reallocation/drop remains the parent outcome's acceptance.

The [runtime-cell production](runtime_cells/README.md) partitions at runtime,
updates actual backing through a qualification-erasing view, and rejoins the
original descriptor. Its bounds twin rejects a cell extending past the end.
The accompanying ordinary-RAM control preserves an untouched neighbor but
rejects a stale fact about the written field; content-only writes must not
accidentally become the rule for every access view.

The field place also captures its qualified extent through an ordinary call
before reconstructing `Held`. Its default domain consumes the returned length
guarantee. The new occurrence must retain its existing Granted and Vacant
memberships: an equal constructor or receiving annotation supplies neither.
This exercises the owned-record transfer needed by the allocator and Vec.

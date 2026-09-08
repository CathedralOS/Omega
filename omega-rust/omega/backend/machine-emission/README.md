# Machine emission

This backend joins current allocation and machine facts to frame, encoding,
layout, and exit evidence, then emits replayable fragments and placed text.
Start at [lib.rs](src/lib.rs). The public contracts are
[optimization validation](../../../../wiki/spec/build/optimizations.md),
[machine-state evidence](../../../../wiki/spec/build/machine_state_evidence.md),
and [stack provisioning](../../../../wiki/spec/resources/storage.md).

## Function realization

[Fixed-frame realization](src/function_realization/routes/fixed_frame.rs) is the
common entrance for admitted allocation histories, including identity execution.
It replays allocation, rejoins the current machine, derives frame evidence,
performs selected-form encoding and baseline layout, executes the phase-local
layout selection, and validates the whole-function exit contract. The retained
result owns all replay inputs. Its validator reconstructs their complete join;
it does not select another machine or rerun allocation.

Encoding and layout are separate pipeline transforms. Target-owned encoders
produce bytes; independent validation uses target-owned decoders and exact
instruction effects. Layout binds labels, canonical order, spans, offsets,
fixups, predicates, and taken/fallthrough successors. An x86 relative branch uses
its instruction end as displacement origin; AArch64 uses the instruction
address. Relaxation consumes a complete validated baseline and retains both
baseline and selected layout evidence. An instruction rule must not hand-assemble
bytes or derive semantic authority from a size estimate.

## Frame responsibilities

[Frame assembly](src/function_realization/frame.rs) joins allocation-visible
callee-save requirements, target preservation-storage groups, exact frame
geometry, and separately encoded frame protocol. Durable geometry and protocol
plans live in `machine-code`; these backend modules own computation and sealed
independent replay.

[Frame layout](src/frame_layout/mod.rs) binds the selected machine, requirements,
storage, target, register environment, ABI, and policy. It accounts for outgoing
ABI storage before preservation storage, call alignment, and the target's
return-address location. The current geometry includes Microsoft x64 shadow and
stack-argument storage and AArch64 link-register preservation. This does not
establish all native program shapes or publication profiles. ABI red-zone
capacity is a fact, not a decision to use it.

The separate abstract spill-requirement calculation exposes extent/alignment,
not integration of executable spill accesses into the common frame. Callee-save
group selection similarly starts with abstract storage: complete target storage
views may cover more preserved units than the particular modified subset.
Neither calculation establishes SP/FP coordinates, actual loads/stores, unwind,
probing, faults, or execution authority by itself.

[Frame protocol](src/frame_protocol/mod.rs) uses target encoders and independent
replay over a packed byte arena with per-function spans. It excludes the selected
return instruction. [Frame application](src/frame_application/mod.rs) inserts
one entry prologue and the required epilogue at each retained return, reflows
block/row/fixup coordinates, and re-encodes affected branches. Its independent
checker validates sites and branch bytes. Callers still admit the source
fragments and protocol: successful byte projection alone is not publication.

## Fragments, placement, and reports

[Fragment emission](src/fragment_emission/mod.rs) consumes validated current
function realization. [Text placement](src/text_placement/mod.rs) retains the
source and applied-frame identities while resolving placement. Child manifests
are rebound and replayed; changing a rule does not require copying its custody
fields into parallel object, artifact, and callable schemas.

Object/container construction, final-image validation, executable installation,
and callable admission remain later owners. Compiler-generated source/byte
correspondence and one physical receipt for each surviving settled boundary
occurrence must be checked there. A receipt count alone cannot prove complete
coverage; omission requires validated elimination of that exact occurrence.
Neither ordinary frame geometry nor optimization selection supplies an otherwise
missing firmware adapter or executable-region proof.

The [image reporting projection](../images/image-emission/src/function_fragments/reporting.rs)
uses replayed fragment and frame data. Instruction counts mean selected spans,
including zero-byte spans, not ISA instruction counts: one materialization may
encode several instructions. Inserted prologue/epilogue bytes are counted
separately; the selected return is not part of inserted epilogue bytes, and
semantic fallthrough attribution is not an additional selected instruction.
Mechanical image APIs without retained compiler source cannot manufacture that
compiler-function report. Reports never replace source, provider, boundary,
entry, physical, or publication validation.

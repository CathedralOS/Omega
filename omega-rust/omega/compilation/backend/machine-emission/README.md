# Machine emission

This backend combines allocation and machine facts with frame, encoding,
layout and exit evidence, then emits replayable fragments and placed text. Start at [lib.rs](src/lib.rs): it lists the four stages in
the order native realization runs them, the three evidence modules
realization combines, and the x86-64 byte recipes outside those stages. The public contracts are
[optimization validation](../../../../../wiki/spec/build/optimizations.md),
[machine-state evidence](../../../../../wiki/spec/build/machine_state_evidence.md),
and [stack provisioning](../../../../../wiki/spec/resources/storage.md).

## Function realization

[Fixed-frame realization](src/function_realization.rs) consumes the current
physical program. Allocation carries its settled preservation requirements and
publication facts forward; frame planning adds storage, geometry and protocol
rows. Stages 07–09 retain those rows with the current encoding and layout.
Assembly reads stage 09's output alone to publish the whole-function exit
contract and manifest.

Current-artifact validation checks bytes, frame geometry, exit effects and
publication roots without retaining allocation or earlier layout graphs.
`FixedFrameCorrespondence` additionally compares the result with original
inputs, retained only when requested. Missing originals refuse that audit;
they never suppress the mandatory final-artifact checks.

Layout and exit-contract code computes each result once. Their separate
validators serve explicit checks, retained-artifact readers and tests;
assembling a function does not replay the layout or exit record it just made.
ABI, frame and exact-input identity matching stays part of construction. The final artifact must still pass machine-state admission.

Exit validation uses the current function contracts and physical choices.
Encoding is checked against the actual frame, and layout against the retained
phase selection. Original-input audits also compare full publication and
preservation records, not just their claimed hashes.

Encoding and layout are separate pipeline stages. Each target's encoder
produces bytes; independent validation uses that target's decoder and exact
instruction effects. Layout fixes labels, canonical order, spans, offsets,
fixups, predicates, and taken/fallthrough successors. An x86 relative branch uses
its instruction end as displacement origin; AArch64 uses the instruction
address. Branch relaxation starts from a complete baseline layout and keeps
the baseline's exact identity joins and evidence for the chosen layout. An instruction rule
must not hand-assemble bytes or decide meaning from a size estimate.

## Frame responsibilities

[Frame assembly](src/function_realization.rs) combines allocation-visible
callee-save requirements, target preservation-storage groups, exact frame geometry, and the separately encoded frame protocol. The
stored geometry and protocol plans live in `machine-code`; these modules
compute and independently replay them.

[Frame layout](src/frame_layout.rs) takes the selected machine,
requirements, storage, target, register environment, ABI and policy. It
places outgoing ABI storage before preservation storage and accounts for
call alignment and the target's return-address location. Geometry today
covers Microsoft x64 shadow and stack-argument storage and AArch64
link-register preservation; that does not cover every native program shape
or publication profile. The ABI's red-zone size is a fact, not a decision
to use it.

Preservation storage starts at its required alignment even when no local
storage follows a narrow outgoing argument. Alignment padding does not widen
the outgoing argument's payload or the bytes written for it.

On AArch64, over-aligned local values and indirect outgoing copies reserve
their payload plus `value_alignment - abi_alignment` bytes. Address formation
rounds the absolute address inside that interval, then adds the member offset.
The native entry bridge uses the same instruction recipe for its owned
snapshots, copy homes and result storage. Rounding only the frame offset would
leave a 32-aligned value only 16-aligned for some callers. SP stays fixed in
the ordinary protocol; realigning the whole frame at run time would needlessly
change incoming argument offsets, probing and unwind. x86-64 still
rejects over-aligned homes until its address and store effects support them.

The separate abstract spill-requirement calculation reports size and
alignment; it does not place real spill loads and stores in the common
frame. Callee-save group selection likewise starts from abstract storage: a
target's full storage view may cover more preserved units than those
modified. Neither calculation alone fixes SP/FP offsets, actual
loads and stores, unwind, probing, faults or execution authority.

[Frame protocol](src/frame_protocol.rs) uses target encoders and independent
replay over one packed byte buffer with a span per function. It leaves out
the selected return instruction. [Frame application](src/frame_application.rs)
admits the emitted fragments, adds one entry prologue and the required
epilogue at each kept return, recomputes block, row and fixup offsets, and
re-encodes affected branches. Its independent checker validates sites and
branch bytes for retained-artifact readers and tests, not on bytes the stage
just inserted. Inserting bytes is not publication.

## Fragments, placement, and reports

[Fragment emission](src/fragment_emission.rs) takes validated function
realization and turns resolved machine bytes into unplaced function
fragments. [Text placement](src/text_placement.rs) keeps the source and
applied-frame identities while resolving internal calls and section offsets.
Each stage has an independent checker in its module. The checkers serve
retained-artifact readers and tests; no stage replays output the same
compile just produced
([verification placement](../../../../pipeline.md#verification-placement)).
Child manifests are rebound, so changing a rule does not mean copying its
fields into parallel object, artifact and callable schemas.

[Private callback materialization](src/private_callback.rs) turns a placed
single-function section into its compiler-private machine-code record. It
checks the settled boundary ABI against the target plan the section keeps,
and rejects calls outside the private function. The coordinator supplies
the callback identity and boundary entry plan; it neither extracts machine
bytes nor rebuilds their ABI metadata. It reads the text-placement result and
is not another pipeline stage.

An x86 interrupt handler needs to return with the interrupted program's registers
and flags intact. The [native entry adapter](src/native_entry_bridge.rs) saves
all fifteen ordinary integer registers, clears the direction flag before calling
the compiled body, restores those registers, and returns with `iretq`. The CPU's
five-word return frame restores the original flags, instruction pointer and stack
pointer. Native stack arguments begin after that frame; its words are never
interpreted as handler arguments.

Before emitting this adapter, image emission checks the handler and its
reached helpers against the complete entry state plan. It uses the kept
instruction and frame effects, keyed by function and instruction, so one
helper cannot hide another's writes. The supported interrupt protocol rejects
vector or control-state use it cannot restore. A handler promised to stay
masked also rejects enabling interrupts, restoring unknown flags, and
opaque callees that lack that promise.

This adapter requires an admitted, normalized arrival frame. Installation
still owns the actual stack, any arguments, kept state and interrupt
controller duties. Emitting an interrupt entry does not install a root or
acknowledge the interrupt.

Object and container construction, final-image validation, executable
installation and callable admission happen later. Those steps must
check the compiler's source-to-byte map and one physical receipt for each
settled boundary occurrence that survives. A receipt count alone cannot
prove full coverage; leaving one out requires validated removal of that
exact occurrence. Neither frame geometry nor optimization supplies a
missing firmware adapter or executable-region proof.

The [image reporting projection](../images/image-emission/src/function_fragments.rs)
uses replayed fragment and frame data. Instruction counts mean selected spans,
including zero-byte spans, not ISA instruction counts: one materialization may
encode several instructions. Inserted prologue/epilogue bytes are counted
separately; the selected return is not part of them, and a fallthrough
attributed to source is not an extra selected instruction. Image APIs without
the compiler's source cannot produce this per-function report. Reports
never replace source, provider, boundary, entry, physical or publication
validation.

When unattributed spill instructions split one operation's code, ordinary
fragment publication keeps each maximal byte interval separately. Image and
installation replay check every interval and the unique one holding each
borrowed Unit call; they never stretch attribution across gaps. This source
map adds no extra logical cost. Specialized boundary realization profiles
keep their own stricter span rules.

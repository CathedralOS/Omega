# Machine emission

Placement writes the prologue, instruction rows and return epilogues into each
function, then patches branches and joins the functions into one text section.
The driver passes the resolved layout to [placement](src/lib.rs). The public contracts are
[optimization validation](../../../../../../wiki/spec/build/optimizations.md),
[machine-state evidence](../../../../../../wiki/spec/build/machine_state_evidence.md),
and [stack provisioning](../../../../../../wiki/spec/resources/storage.md).

## Function realization

[Exit checking](src/function_realization.rs) joins the selected program,
allocation, current layout and frame protocol. An audit retains that layout
and its checked exits beside the placed body. It reads the optimization
selections from the allocation that produced the body.

Current-artifact validation checks bytes, frame geometry and exit effects
without retaining allocation or earlier layout graphs.
`FixedFrameCorrespondence` additionally compares the result with original
inputs, retained only when requested. Missing originals refuse that audit;
they never suppress the mandatory final-artifact checks. The same retained
replay can carry stage 06's completed rewrite run as an opaque
`RetainedSelectedOptimizationRun`; `SelectedInstructionRewriteReplay` asks it
to rebuild itself against the record the allocated program carries. Emission
never reads it. Rebuilding uses the producer itself, so the replay checks
repeatability and record custody, not independent rewrite correctness.

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

[Frame layout](../09_post-allocation-machine-to-framed-machine/src/frame_layout.rs) takes the selected machine,
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

[Frame planning](../09_post-allocation-machine-to-framed-machine/src/frame_protocol.rs)
selects the protocol. [Frame encoding](../11_bound-framed-machine-to-selected-form-encoding/src/frame_protocol.rs)
uses target encoders and produces protocol bytes over one packed byte buffer with a span per function. It leaves out
the selected return instruction.

[Fragment emission](src/fragment_emission.rs) writes each block, instruction
and fixup at its framed offset. A return gets an epilogue immediately before
its instruction bytes. Once every block has an offset, [branch patching](src/frame_application.rs)
widens branches that exceed their encoding's reach and writes the final
displacements. [Text placement](src/text_placement.rs) resolves internal calls
and records external relocations in section coordinates.

On an explicit audit request, placement also projects the unframed instruction
rows. Replay compares them with the resolved input, checks the least branch
widths and frame insertions, and verifies the bytes and relocations in the
placed section. Ordinary placement releases the pre-layout encoding before
object construction.

[Private callback materialization](../../driver/image/callback_thunks.rs)
uses this stage to place the callable body. The driver then asks object and
image construction to bind the callback entry to that placed body.

An x86 interrupt handler needs to return with the interrupted program's registers
and flags intact. The [native entry adapter](../09_post-allocation-machine-to-framed-machine/entry-planning/src/native_entry_bridge.rs) saves
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

The [image reporting projection](../15_objects-to-image/image-emission/src/function_fragments.rs)
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

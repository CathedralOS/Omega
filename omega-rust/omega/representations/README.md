# Native representation ownership

These are current program representations, not a substitute portable boundary.
[Terminal Psi](../../../wiki/spec/terminal-psi/product.md) is the portable
boundary. Pipeline crates transform these programs; target backends own ISA,
ABI, object-format, and relocation details.

## Shared facts and real lowering boundaries

Represent a semantic concept once at each resolution level. Share vocabulary
across stages that do not change its meaning; move genuinely shared concepts to
their lowest appropriate owner, not merely to whichever stage first needed them.
Do not merge distinct lowering levels into a single representation.

An annotation-only stage retains its current input and adds annotations rather
than cloning its program or redeclaring every operation. The
[allocated program](register-homes/src/register_homes.rs) shares selected
instructions and register homes through separate immutable artifacts; its
[borrowed view](register-homes/src/register_homes/view.rs) exposes those same
artifacts. The [physical program](physical-instructions/src/physical_instructions.rs)
is a genuinely different resolution level with chosen machine alternatives and
physical operands. Neither representation embeds a chain of transformation
objects. Replay evidence is not another executable program.

Unchanged structural declarations live in one shared
[type catalog](abstract-operations/src/abstract_operations/structural_type_catalog.rs).
Function records and later native representations retain that catalog instead of
deep-copying its declarations. Editing a revision uses explicit copy-on-write;
replay and encoded identity still compare contents, never allocation identity.

Keep addressing compositional: operations consume places and value sources,
not a Cartesian family of source/destination-specific opcodes.
[Target structural values](target-operations/src/target_operations/values/structural.rs)
retain place identity, complete projection paths, access, qualifications, shapes,
and placements. [Scalar homes](target-operations/src/target_operations/storage.rs)
retain their scalar types and source identities. Resolve these semantic facts
from the checked representation and carry them; native consumers must not infer
them from source spelling, pointer shape, or a guessed storage slot.

Independent replay must still reconstruct and check the retained facts. This is
not the redundant semantic discovery that the ownership rule prohibits.
Unsupported lowering fails explicitly; a missing selection must not discard a
write, substitute a slot, or fabricate a result. The
[lowering owner](../pipeline/abstract-operations-to-target-operations/README.md)
describes current structural access and argument-replay boundaries.

## Encoding and artifact scope

Encoder output is authoritative for final instruction size. Lay out executable
bytes and relocations from the sequence actually emitted, not a separately
maintained instruction-width mirror. The
[text-section representation](machine-code/src/machine_code/layout/text_section.rs)
retains actual bytes, function/block/instruction spans, and resolved internal-call
coordinates. The [object handoff](../backend/images/image-emission/src/function_fragments/mod.rs)
projects admitted fragments into the image publisher and independently validates
that projection. Its relocation-free lane does not imply arbitrary external
symbol resolution or independent component publication.

Representation migrations must preserve exact places, widths, call results,
argument transfers, control flow, and byte/relocation correspondence. Use the
affected interpreter/native differential cases and x86-64/AArch64 encoder checks
as acceptance evidence. Extending calls or frames requires exact stack ownership
and disjointness evidence, not an assumption that whole-image addresses remain
valid across a component boundary. Existing frame and spill work owns wider
physical support; historical selection failures do not by themselves establish
frame overlap.

[Component publication](../backend/runtime/component-publication/README.md)
records current artifact and deployment limits separately from language meaning.

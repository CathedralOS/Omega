# Abstract operations to target operations

This stage derives target operations and call placement from abstract operations.
Start at [lib.rs](src/lib.rs). The public reference contract is
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md).

## Structural ABI derivation

[structural_signature.rs](src/lowering/structural_signature.rs) constructs
structural call signatures from declarations and resolved referent layouts.
The exhaustive access classifier in
[structural_layout.rs](src/lowering/structural_layout.rs) selects value placement
only for owned inputs. Shared, mutable, and write-only borrows use
`BorrowedReference`, preserving referent size/alignment and passing its pointer
without a caller-side value copy. Layout caches retain referent shapes, not
access-specific parameter shapes. Scalar and erased descriptor ABIs retain their
existing generic construction.

The native byte-observation path admits one unqualified, unrestricted shared
`BorrowedView` parameter, `u64` scalar inputs, and a `u64` result. It uses
`BorrowedReference(16, 8)`:
one pointer to the original two-word descriptor, with the length at byte offset
eight. The selected `Load64` retains the logical source place and independently
replayed read footprint. A guarded `ByteSequenceRead` loads the backing pointer
at offset zero, then performs an indexed, zero-extending byte load. It retains
the exact index, same-view length witness, and accepted bounds obligation.
The ordinary conditional graph preserves the non-reading branch; shared
[byte observations](src/lowering/scalar/byte_views.rs) serve both straight-line
and integer-result conditional lowering. Scalar inputs precede the descriptor
pointer in the derived call signature.

Scalar-result helpers can forward one whole shared descriptor and return the
callee's `u64` result, including through nested helper calls. The existing
`ReturnStructuralScalarCall` target form enters ordinary scalar graph legalization;
[call input checks](../target-operations-to-selected-instructions/src/legalization/scalar_graph_input/structural_call.rs)
rejoin the exact source argument, callee declaration, reference ABI, and placement.
Selection snapshots only the pointer, never the descriptor's contents, and
independent replay checks its incoming and outgoing homes. No separate call IR
or byte-view calling convention is introduced.

Mixed scalar/view calls also use ordinary `StructuralCall` integer expressions
in straight-line and conditional graphs. Each occurrence retains the exact
callee plan, ordered scalar actuals, whole-reference structural actuals, and
call contracts. Repeated calls remain ordered operations even when an earlier
result is unused. This route admits unqualified, unrestricted shared byte-view
parameters; it does not infer projection or mutable-reference support.
Standalone selection and replay also rejoin the caller's complete scalar roster
and placements to its ABI before forwarding the descriptor pointer. An extra or
missing scalar declaration cannot be inferred from an otherwise plausible ABI.

Checked subslices retain their exact source, structural result, endpoints, and
two-leg bounds obligation. Acyclic scalar-result graphs can measure, read, and
derive nested views without copying the original descriptor or backing bytes.
Target expressions retain the derivation, and legalization independently rejoins
it to the verified operations. Derived views need an addressable descriptor
before they can be passed through the reference ABI; that call path is not yet
implemented.

Selection retains the original backing pointer with an integer byte offset and
length in ordinary value homes. For root length `R`, each derived view preserves
`offset + length <= R`; the two bounds legs justify nested offset addition and
length subtraction. Guarded reads add the relative index to that offset before
the existing indexed load. No arithmetic proof is asserted about the pointer,
and an empty suffix need not form a one-past address. Independent replay checks
the same source chain and every contributing home.

This does not complete literal descriptor materialization, general Unit calls,
non-`u64` mixed helper calls, derived-view calls/block transfers, or ranked control. The
[native regression](../../../../tests/native-differential/tests/terminal_byte_views.rs)
starts from encoded, verified Terminal, cross-lowers four hosted targets, and
executes caller-owned descriptors, derived views, and framed helper chains
on supported hosts; it does not establish Omega-source writer closure or
standalone executable publication.

[Structural-header validation](src/validation/structural_signatures.rs) rejoins
retained Unit and mixed scalar ABI parameters to the source declarations and
reconstructs scalar-prefix, result, and structural placement. A coherently
recomputed value ABI cannot replace a source borrow. This check does not claim
full function-body translation coverage or native caller-visible behavior.

Finish caller preparation and independent native receiving/replay checks under
`STRUCTURAL-BORROW-IDENTITY` on the [execution board](../../../../TASKS.md).
Embedded callee plans and argument homes require their own reconciliation;
standalone native consumers cannot assume a producer ran header validation.
Existing owned-copy structural-call forms remain closed to borrowed execution;
the byte observations and scalar helper forwarding above preserve their exact
reference input contract.

A staged pointer and staged value bytes are different. A direct-home test is
valid for owned semantics, not evidence that a borrowed caller sees a write.
Test substituted access/shape/placement pairs as rejections and execute genuine
reference controls against caller-owned backing. Unsupported native forms remain
fenced until all producing and receiving boundaries enforce their contract.

## Store and argument replay

Lowering retains complete projected paths and scalar value sources. Physical
assignment and later replay must reconstruct offsets from declarations, match
incoming and outgoing placement, and preserve exact referent widths. A physical
pointer cannot erase write-only access or collapse Boolean/IEEE/integer stores.

Parallel argument transfers must preserve every original source despite register
cycles, duplicate sources, or register/stack crossings. Any private snapshots
store scalar sources or reference pointers, not borrowed referents. Call-result
sources use their exact durable homes; they are not fabricated constants.
Frame rebasing, transient call storage, relocation, and code intervals remain
independently replayed downstream.

Field-store/dynamic-call fixtures alone do not establish general borrow
writeback. Admission of a new native shape needs caller observation after return,
unchanged surrounding bytes, and suspension checks where applicable, in addition
to layout and byte replay.

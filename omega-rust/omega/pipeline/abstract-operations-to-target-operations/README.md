# Abstract operations to target operations

This stage derives target operations and call placement from abstract operations.
Start at [lib.rs](src/lib.rs). The public reference contract is
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md).

## Structural ABI derivation

[structural_layout.rs](src/lowering/structural_layout.rs) derives referent layout
and parameter shape separately. Its parameter classifier selects
`BorrowedReference` for mutable/write-only access but currently passes shared
access through as a value shape. [scalar/setup.rs](src/lowering/scalar/setup.rs)
has another access-to-shape decision. These are implementation gaps, not a
shared-reference snapshot contract.

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

This does not complete the general shared-reference classifier below, literal
descriptor materialization, structural helper calls, subslices, or ranked control. The
[native regression](../../../../tests/native-differential/tests/terminal_byte_views.rs)
starts from encoded, verified Terminal, cross-lowers four hosted targets, and
executes caller-owned descriptors on supported hosts; it does not establish
Omega-source helper closure or standalone executable publication.

Consolidate structural-signature producers under one mandatory derivation from
declarations and referent shapes. Audit caller preparation and independent
receiving validation/replay as well; fixing the classifier does not prevent a
hand-built downstream plan from bypassing it. Generic scalar ABI construction
remains legitimate. The owning work is `STRUCTURAL-BORROW-IDENTITY` on the
[execution board](../../../../TASKS.md).

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

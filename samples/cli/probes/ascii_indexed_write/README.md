# ascii_indexed_write

Indexed write through a qualified byte owner:
`bytes[at] = value` on `&mut [u8; 8] in Ascii`
succeeds because `value < 128` dominates the store and
re-proves `ascii_only` per element. Also: named states
declare `-> bool` results like machines, and a state
re-enters itself on a `repeat` edge.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Native realization

Measured on macOS arm64 on 2026-10-13. Copying a qualified byte owner into
an unqualified fixed array (`let copy: [u8; 8] = tagged.label;`, or
`let plain: [u8; 8] = bytes;` from a `[u8; 8] in Domain` local) has no
correct Terminal Psi meaning yet because the compiler incorrectly lowers the
qualified array to `BoundedOwned`. Both source types are exact-length
`FixedArray` carriers under
[domains](../../../../wiki/spec/language/domains.md). The field copy fails Omega lowering with
`InvalidStructuralLeafCopy`; the local copy fails Terminal validation with
"structural qualification has the wrong carrier". **NOMINAL-FIELD-FLOW** owns
removing that classifier and realizing this probe without a carrier conversion.

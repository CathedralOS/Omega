# ascii_indexed_write

Indexed write through a qualified byte owner:
`bytes[at] = value` on `&mut [u8; 8] in Ascii`
succeeds because `value < 128` dominates the store and
re-proves `ascii_only` per element. Also: named states
declare `-> bool` results like machines, and a state
re-enters itself on a `repeat` edge.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.

## Native realization waits on a design decision

Measured on macOS arm64 on 2026-10-13. Copying a qualified byte owner into
an unqualified fixed array (`let copy: [u8; 8] = tagged.label;`, or
`let plain: [u8; 8] = bytes;` from a `[u8; 8] in Domain` local) has no
Terminal Psi meaning yet. `[u8; N] in Domain` lowers to a `BoundedOwned`
byte carrier with a live length, while `[u8; N]` is a `FixedArray` whose
length is exactly `N` (spec/language/counts_and_addresses.md: "A fixed array
`[T; N]` has length exactly `N`"). The field copy fails Omega lowering with
`InvalidStructuralLeafCopy`; the local copy fails Terminal validation with
"structural qualification has the wrong carrier". Neither
spec/terminal-psi/structural_access.md nor byte_views.md says whether the
erased array holds the owner's whole backing or only its live bytes, nor
whether `.len` of a `[u8; N] in Domain` value is its live length or `N` (this
probe's guard `at < bytes.len` assumes `N`).

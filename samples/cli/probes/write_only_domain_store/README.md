# write_only_domain_store

Whole-array store of a domain-qualified leaf through a
`&write` loan: `tagged.label = label` where `label:
[u8; 8] in Utf8` discharges `valid_utf8` membership on
the stored value — same write-side check as `&mut`.

## Current implementation frontier

Indexing a domain-qualified array member reads as an
unknown-length slice (`tagged.label[0]` fails "cannot
prove index within unknown slice length"). This is compiler migration debt:
`[u8; 8] in Utf8` is the same exact-length carrier as `[u8; 8]`, so its `.len`
is 8 and the qualified-to-plain copy changes no representation.

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

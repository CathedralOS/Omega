# Recast validation implementation

The [recast contract](../../../../wiki/spec/layouts/recasts.md) defines the
representation judgment. `src/value_custody/recasts.rs` and its child modules implement the
supported source subset; their restrictions are not new language semantics.

Shared scalar, bounded byte-region, and recursively nested record/array views
are supported. Mutable typed aliases require equal geometry and equivalent
leaf representation sets. Integer range leaves normalize to exact
two's-complement bit sets; same-carrier float ranges use interval inclusion
for shared views and equality for mutable views. Float interval endpoints are
exact IEEE values at the leaf's carrier, never a rounded integer. Equal-looking
cross-carrier predicates do not establish equivalence.

A record's data `where` facts take part when every conjunct bounds one of its
direct scalar fields by a literal (`value >= 0.0 && value <= 1.0`). Such a
conjunction constrains each field independently, so it narrows that field's
leaf exactly as the revoked bracketed range did, and the leaf-wise comparison
decides the whole judgment. Float bounds read the literal at the field's
format, require the builtin or sealed IEEE comparison, and admit only closed
comparisons. Writes through a view re-prove the view's own declaration, which
the comparison showed equivalent (mutable) or implied (shared). A relational
fact, a bound on a nested aggregate or a membership fact keeps the record
fenced, and byte-region views still require recursively fact-free targets.

One direct erased-lifetime shell around an eligible synthesized record may
use its exact instance layout when lifetime arity is exact, no runtime
arguments remain, and stored fields do not recursively carry lifetime
applications. This applies only at the target root; array or nested-record
descent cannot erase more shells into layout authority.

Complete-source and proved interior unsized slices use exact tiling and
compiler-derived element stride, including padding. Runtime multi-byte
offsets need proved congruence. Source recasts are currently validated as direct
reference-typed `let` initializers restating the target type. Other positions
remain fenced; a mismatched bare borrow cannot bypass the recast judgment.

Every recast-path diagnostic attaches the authored source span of the
offending `as` expression (the stray cast itself for the positional sweep, the
let's initializer for the unspelled reference pun), so fenced positions and
judgment refusals name their exact site.

Checked trees retain a whole-place scalar re-view as
`CheckedScalarExpression::RepresentationView`: the source place's read at its
declared carrier beneath the view carrier. A same-carrier recast changes only
facts and reads the place directly. Stage 05 lowers the source read with its
declared-type checks intact and replays the carrier relation
(`expression_preparation/representation_view.rs`): equal-width integer
carriers read the two's-complement modular image, and a wider integer view of
a runtime-selected `u8` element reads the byte region little-endian, one
element read (and one Terminal index obligation) per byte. Boolean and IEEE float
cross-carrier views (`bool` as `u8`, `f32` as `u32`) reject there because
Terminal Psi has no bit-reinterpretation or 0/1-landing re-view operation yet;
Boolean view targets still retag their guard leaf until that form exists.
Reads through an immutable `&mut` place-loan local re-evaluate the loaned place
like shared loan locals do.

A shared record-as-record view erases at Terminal: stage 05 reads each view
field as the source leaf at the same offset under the source's declaration
(`recast_record_source_leaf`), which the identical-geometry judgment makes
exact. A leaf whose carrier differs (`i16` seen as `u16`) and mutable record
views do not lower yet.

Native and interpreter projection/state forwarding preserve backing identity.
`Placed<P, T>` and its accessors are excluded: representation equivalence
cannot replace the placement access plan. Focused source cases live in
`tests/omega/{pass,fail}/recast/`. Representation judgments are tested beside
`src/value_custody/recasts/`; checked-tree and lowering coverage is in
`04_typed-trees-to-checked-trees/src/tests/{borrow/recasts.rs,flow/terminal_unit/representation_views.rs}`
and `05_checked-trees-to-lowered-psi/src/tests/representation_view_guards.rs`.

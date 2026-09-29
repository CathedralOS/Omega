# Recast validation implementation

The [recast contract](../../../../wiki/spec/layouts/recasts.md) defines the
representation judgment. `src/value_custody/recasts.rs` and its child modules implement the
supported source subset; their restrictions are not new language semantics.

Shared scalar, bounded byte-region, and recursively nested record/array views
are supported. Mutable typed aliases require equal geometry and equivalent
leaf representation sets. Integer range leaves normalize to exact
two's-complement bit sets; same-carrier float ranges use interval inclusion
for shared views and equality for mutable views. Equal-looking cross-carrier
predicates do not establish equivalence.

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

Native and interpreter projection/state forwarding preserve backing identity.
`Placed<P, T>` and its accessors are excluded: representation equivalence
cannot replace the placement access plan. Focused source cases live in
`tests/omega/{pass,fail}/recast/`; compiler integration coverage is in
`omega-rust/omega/compilation/driver/tests/recast_views.rs`.

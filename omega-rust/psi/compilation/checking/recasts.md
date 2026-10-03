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
offsets need proved congruence. Source recasts are validated in direct
reference-typed `let` initializers and statement-reachable guards, call
arguments and nested operands. Inline casts use their stated target type;
contract-only positions remain fenced because they establish no runtime
borrow. A mismatched bare borrow cannot bypass the recast judgment.

Slice construction retains its exact formation occurrence. A local rejoins
its original loan at the declaration and destination slot; later replacement
loans cannot substitute for that origin. An inline state argument instead
rejoins the named transition's canonical call ordinal, exact argument and
complete ordered borrow-access roster. Both use ordinary structural value
construction. The selected edge evaluates an inline descriptor before lending
it to the successor; a mutable descriptor retains its affine disposal debt
while the receiving reference formal remains unrestricted. Writable arguments
use the same canonical backing place as borrow admission, including validated
indexed recast footprints. Indexed record-field observations replay the exact
view root, field path and selectors independently of the storage-read inventory;
a literal selector does not make the root or field identity redundant. Slice
bounds remain separate proof obligations derived from the live descriptor.

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
element read (and one Terminal index obligation) per byte. Boolean and IEEE
float cross-carrier views use `ScalarRepresentationView`: independent
verification reconstructs the equal-width carrier pair and requires a proved
0/1 landing when an integer becomes Boolean. Native selection preserves the
stored bits and applies only the result carrier's register normalization.
Reads through an immutable `&mut` place-loan local re-evaluate the loaned place
like shared loan locals do.

Shared primitive loans passed to helpers retain an `EstablishReference` carrier
and an explicit `Referent` call projection. The backing declaration and viewed
referent are separate catalog roots, retained through both checked planning and
lowering. Immutable scalar ingress receives one original storage home reused by
its views; mutable local storage already has a home. Declaration-level total
views use this route. A view justified only by contextual facts, such as a
`u8` constrained to 0/1 and observed as `bool`, keeps its proved scalar
observation instead of claiming that every source-carrier representation is
valid at the target. The source recast fact owner makes that distinction;
Terminal independently reconstructs the total carrier relation.

A record-as-record view retains its source leaf at Terminal: stage 05 reads
each view field at the same offset under the source's declaration
(`recast_record_source_leaf`), which the identical-geometry judgment makes
exact. A mutable view's direct field store rejoins the same leaf in stage 04
(`structural_scalar_store/destination.rs`); both store replays recompute it
(`representation_view_store::record_view_leaf_destination`). An equal-width
integer leaf of the other sign (`i16` seen as `u16`) reads its declared
carrier and takes the modular image, and a store writes the image of the
view's value. A leaf of another carrier kind (`f32` seen as `u32`) uses the
same scalar representation-view replay. Nested view paths remain unsupported.

Byte-backed record observations and stores compose nested record and fixed-array
paths with runtime selectors. The shared normalized layout retains a constant
leaf offset plus each selector's stride and declared extent. Lowering proves
each dimension before flattening the coordinate, then independently proves
each emitted byte's backing bound. A loan's outer runtime offset is captured
at its declaration boundary in a private primitive home; later statements read
that home rather than reloading the authored offset field. Current inner array
selectors remain ordinary observation or assignment operands. Geometry uses
bounds at the exact selector occurrence: entry requirements and incoming facts
survive only the effects before formation. Earlier writes and calls invalidate
those facts; later effects do not retarget the captured coordinate. The shared
arrival analysis supplies these bounds and call postconditions; recast checking
does not maintain a separate incoming-edge guard walker. Standing field bounds
still require store-enforced declarations. Exact source replay, loan access and
scalar carrier checks remain independent of geometry.

Native and interpreter projection/state forwarding preserve backing identity.
The interpreter retains the same reference-recast descriptor for shared and
mutable locals and arguments: original cells and captured coordinates stay
separate from the checked reference access. Shared record views therefore read
the original backing rather than an assembled copy.
`Placed<P, T>` and its accessors are excluded: representation equivalence
cannot replace the placement access plan. Focused source cases live in
`tests/omega/{pass,fail}/recast/`. Representation judgments are tested beside
`src/value_custody/recasts/`; checked-tree and lowering coverage is in
`04_typed-trees-to-checked-trees/src/tests/{borrow/recasts.rs,flow/terminal_unit/representation_views.rs}`
and `05_checked-trees-to-lowered-psi/src/tests/representation_view_guards.rs`.

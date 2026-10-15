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

Fixed-size scalar and aggregate views may start inside a borrowed byte slice.
The occurrence-level integer judgment proves `0 <= offset` and
`offset + footprint <= bytes.len` against the actual descriptor after selector
effects. This proves containment, not an exact descriptor length: unsized
target views still require their independent exact-suffix tiling judgment.
Mutable raw views also preserve any source byte-element predicates. Integer
and IEEE scalar reads share byte assembly, followed by bit interpretation;
no numeric integer-to-float conversion occurs.

An `IntegerAt` leaf compares its valid encoded bit patterns, not the width of
its semantic integer. Geometry comes from the selected layout; authored field
facts come from the retained schema even when that layout synthesized a new
record. A mutable canonical physical view must preserve the source's validity
in both directions. Semantic writes through the wider encoded view still prove
fit. Foreign thin pointers may carry this explicitly checked physical aggregate
backing, never an internal scalar pointer-and-codec descriptor.

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
while the receiving reference formal remains unrestricted. Scalar-returning
helper operands retain the same descriptor value. Scalar
operand scheduling captures the region before the call, uses the same view
emitter as Unit construction, and retires a mutable temporary on completion;
an owned-result contract is not descriptor admission. Writable arguments
use the same canonical backing place as borrow admission, including validated
indexed recast footprints. Indexed record-field observations replay the exact
view root, field path and selectors independently of the storage-read inventory;
a literal selector does not make the root or field identity redundant. Slice
bounds remain separate proof obligations derived from the live descriptor.

A captured interior suffix is a loan footprint, not a declared element place.
Value production first rejoins that footprint to the checked byte-region
formation and its backing collection, then queries the backing's declaration.
Querying a literal suffix before this join loses its descriptor; a runtime
suffix's conservative whole-backing loan happens to conceal the same mistake.
Both retain the captured offset and derived extent through the common producer,
without replacing the exact borrow-access or tiling judgment.

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
float cross-carrier views use `ScalarRepresentationView`: lowering classifies
the equal-width carrier pair and carries the source recast judgment's 0/1
landing decision when an integer becomes Boolean. It does not search for a
second proof of that observation. Raw byte-region views still reject Boolean
leaves, and a mutable view must preserve the source and target representation
sets in both directions. Native selection preserves the
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
A record field's restriction stays on its Terminal declaration, including
bounds or predicates from its type and its owner's `where` facts. Shared
scalar reference formation retains the original source and evaluated
selectors, rather than manufacturing an owned bounded scalar. Mutable loans
and views keep the place observation: reads transport the field invariant and
writes or normal-return handback re-prove it. Atomic cells retain their
separate access protocol and cannot supply an ordinary scalar reference.

A shared scalar call argument keeps the same original owner and field path,
with shared access and the formal's exact scalar carrier. Terminal, native
lowering and independent call replay resolve this as a shared observation,
not an owned bounded-integer type or a copied scalar home. The restriction
stays with the field. A bare writable scalar formal cannot use this observation
to erase the receiving field's invariant; atomic fields keep their distinct
access protocol. Qualifiers, bounds, overlapping loans and reference crossings
remain independently checked.

Writable scalar field loans whose standing predicates are independent literal
integer bounds use the selected callee's checked normal-return guarantees to
preserve those bounds. Arrival requirements alone do not suffice. A local
capture retains its original field for the same handback check. Coupled and
unprojected predicates remain fenced. Native and Terminal loan presentation
share the original field address and the receiving scalar carrier; neither
manufactures an owned unrestricted field or a predicate-free store authority.
An exact restricted referee retains its complete declaration instead.

Record re-views passed to ordinary or boundary calls retain their supplying
storage beside the logical referee. Locals and anonymous argument formations
use the same source capture and storage publisher; tuple neighbors do not choose
another preparation route. Callee field observations and writes project through
the original backing, including mutable-to-shared attenuation. The caller's
write summary uses that backing namespace, not the view's field spelling.
Public postconditions come from the exact bound requirement and establish
scalar bounds only when every captured source agrees and its interpretation
preserves the value. Writes need only matching geometry to invalidate those
bounds. Unknown writes remain opaque, and a signed reinterpretation cannot
transport an unsigned bound.

A record-as-record view retains its source leaf at Terminal: stage 05 reads
each view field at the same offset under the source's declaration
(`recast_record_source_leaf`), which the identical-geometry judgment makes
exact. A mutable view's direct field store rejoins the same leaf in stage 04
(`structural_scalar_store/destination.rs`); both store replays recompute it
(`representation_view_store::record_view_leaf_destination`). An equal-width
integer leaf of the other sign (`i16` seen as `u16`) reads its declared
carrier and takes the modular image, and a store writes the image of the
view's value. A leaf of another carrier kind (`f32` seen as `u32`) uses the
same scalar representation-view replay. Terminal interpretation composes nested
record and fixed-array selections as byte windows of the original backing;
equal geometry does not require equal field ordinals or nesting. A selected
scalar region must identify exactly one original cell, retaining both its
storage codec and the view interpretation. Calls, successor states and returned
references with an unchanged presentation carry that association instead of
copying or relabelling the source. Publishing a new presentation from a callee
still requires the result-origin contract to retain its admitted formation
separately from the original source coordinates.
Host replacement checks the original field predicate against the inverse byte
image. A foreign thin pointer discards the codec only when the shared scalar
export-width judgment establishes its entire footprint for every admitted
backing; logical width alone does not authorize exposure of adjacent bytes.

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
cannot replace the placement access plan. Source, lowering and execution
coverage lives in `tests/omega/{pass,fail}/recast/` through the corpus runner.

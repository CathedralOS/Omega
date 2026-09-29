# Structural access and stores

Structural parameters and call arguments retain referent type, logical place,
access, multiplicity, and qualifications independently. Physical pointer
equivalence does not make their permissions interchangeable.

[Loan resources and compatibility](loans.md) specifies formation, reborrow
lineage, and restoration separately from these permitted operations.

## Referent identity

Every borrowed argument denotes the original referent throughout its valid
loan, including across projections, calls, returns, and backedges. A small
referent or register ABI does not authorize passing a snapshot.

| Access | Meaning | Native parameter contract |
| --- | --- | --- |
| `Owned` | Ownership under the retained multiplicity. | Select the value ABI from the referent shape. ABI indirection does not create a borrow. |
| `SharedBorrow` | Shared observation under the loan's restrictions. | Reference to the original storage. |
| `MutableBorrow` | Exclusive mutable access. | Reference to the original storage. |
| `WriteOnlyBorrow` | Exclusive mutation without observation. | Reference to the original storage. |

Reference ABI placement uses the existing `ValueClass::BorrowedReference`:
size and alignment describe the referent; placement carries its pointer without
allocating a caller-side value copy. Semantic access remains separately retained.
Aggregate layout determines a type's shape; parameter placement additionally
consumes access.

A local home containing the pointer preserves reference identity. A local home
containing copied referent bytes does not. Copy-based optimization requires an
occurrence-specific equivalence proof covering permitted observations and
relevant exits, including surviving observers on a crash path. A callee seeing
its own staged write does not establish caller-visible writeback.

## Construction and receiving checks

A first-class `Reference { referent, access }` structural value owns a permission
carrier, not its referent. `EstablishReference` captures the exact source place;
the explicit `Referent` path segment crosses the borrowed boundary. Owned
transfer, claims, and referent cleanup cannot cross that segment. Exclusive
carriers have affine custody even when their referent is unrestricted.

Structural result signatures bind each returned reference leaf to its exact
formal ingress source. Independent verification reconstructs actual origins
from establishments and call substitution, rejects local escapes, and checks
the result mapping against the returned carrier. Returning or moving a carrier
does not end its loan. `ReleaseReference` ends it only after its descendants
have ended; same-block restoration therefore precedes any renewed parent use.
Formation, release, and successful return commit only after their fuel charge.

The current executable slice retains whole mutable primitive references and
record-contained reference leaves through ordinary calls and returns.
Reference-valued entry parameters or host-entry results, reference projections
into host-boundary calls, shared/write-only carriers, and native realization remain explicit
implementation limits. Ordinary calls may temporarily attenuate a mutable
carrier to shared or write-only access without creating such a carrier. None
of the unsupported forms gains authority merely because its shape or source
lifetime resembles an admitted reference.

Structural signatures derive from parameter declarations and resolved referent
shapes through one exhaustive access classifier. All structural-call producers
must use that construction; they cannot attach access to an independently
assembled ABI signature. Generic ABI signature construction remains available
for non-structural uses without depending on Terminal declarations.

Caller preparation preserves the actual referent, projection, lifetime, and
access against the callee declaration. Receiving validation and physical replay
independently check access, derived shape, placement, and argument/home identity.
A correctly derived callee signature does not validate an unrelated caller
pointer, copied argument, or substituted plan.

An ordinary call may borrow a whole `BoundedOwned` byte owner or an inline
bounded-owned field through its original root and complete static path. The
callee's carrier capacity must equal the selected owner's capacity. Shared
access may require fewer primitive byte predicates than the owner guarantees;
mutable or write-only access requires implication in both directions, so the
callee cannot return bytes that violate the caller's field invariant. These
predicate checks neither establish disjointness nor create loan authority:
ordinary access attenuation, overlap, lifetime and restoration checks still
apply. In particular, a write-only parent cannot supply a readable child, and
passing a restricted field to a bare mutable byte view is rejected.

## Write-only authority

`&write T` borrows an existing valid `T`; it is not uninitialized output storage.
Loan compatibility and permitted operations are separate. Content-independent
place projection and metadata observation are permitted. Reading stored content,
readable reborrowing, taking, swapping, and read-modify-write are not.

A field or index projection must locate its target without reading content.
A sum payload needing a tag read is unavailable unless an existing refinement
already fixes the case. Whole-value replacement writes tag and payload together.
Partial writes still owe ordinary invariant-window validity; written inputs,
static structure and caller-supplied facts may prove it, never a load through
the write-only loan. Freely discardable old content needs no content-dependent
cleanup; conserved or linear displaced custody cannot silently disappear.

Outcome contracts may identify a modified prefix `[0..count)` and unchanged
suffix `[count..len)`, preserving the caller's suffix facts. The count describes
an effect, not a construction of previously vacant storage. The loan begins
and ends with a valid referent.

Attenuating mutable access to write-only retains the root's declared access
and records write-only access on the argument and callee. It creates neither
ownership transfer nor reusable reborrow authority. Projection from an
unrestricted root does not make the referent linear. Each receiving check
reconstructs the path, referent type, multiplicity, qualifications, and access;
shape or pointer equivalence cannot authorize widening.

Each write event retains its loan occurrence, logical place, physical write
footprint, and outcome guard. Verification invalidates facts on written paths
and preserves facts on explicitly unchanged paths. Discardability of displaced
contents and validity after writing derive from static structure, written
inputs, and supplied premises, never from observing the referent. Checked
realizations establish non-observation through their call closure. Opaque
realizations require an admitted judgment bound to the selected implementation
and receipt, unless installation supplies physical isolation evidence.

## Case observation

`StructuralCaseMembership` retains an established root and a canonical path to
the sum whose current discriminator it reads, producing an unqualified Boolean.
An empty path observes the whole root. Relevant record fields and in-range fixed
array indices select the original backing without copying or borrowing a child.
Its case identity must belong to the exact selected sum type;
matching case spellings in another type supply no authority. The source must be
readable and established on every incoming path. An affine source must also be
live and whole at the observation, not merely present in the declaration table.
Receiving checks reconstruct the selected type and native tag offset from every
retained path segment; another element, a same-spelled case in another nominal
type, or an asserted byte offset cannot replace that evidence.

The path may also select an array element at run time: `self.items[self.w] is
Color::Red` is one `StructuralCaseMembership` whose path is `items`,
`RuntimeIndex`. The observation then owns that element's bound, the obligation
every runtime segment owns (`index < extent`, plus `0 <= index` for a signed
selector), reconstructed against the array its prefix resolves to. It reads
"some element", so it names no canonical sum and keys no case fact; a
`Case` step or leaf copy that needs one must still find a membership of a
canonical place. The Boolean result still selects exactly one dispatch arm.
Symbol resolution hoists such a dispatch subject into one shared
`let __hoist = &self.items[self.w];` so the arms stay one exhaustive
dispatch; the Unit plan erases that borrow while nothing but the dispatch's
transitions follows it, and each arm observes the element itself. Omega
rebuilds the tag's address from declarations: the carrier's static offset
plus each selector times its element stride.

The operation neither consumes the owner nor projects its payload. Its result
captures that occurrence's observation; it grants no reusable equality with
mutable storage and no payload-access refinement after intervening mutation.
Write-only access cannot read the discriminator, even with a known-case premise.
Each observation costs one logical operation unit, charged before execution.

## Complete record construction

`EstablishRecord` produces one exact `OperationResult` record from a complete
roster of declaration-identified field operands. Scalar operands are dominating
SSA values of the exact field carrier. Bounded integer fields require an
independently reconstructed inclusive-range obligation. Nested fields consume
already completed whole owned records or reference carriers of the exact type;
affine children transfer once, while unrestricted children remain available. An unrestricted
parent cannot absorb an affine child.

The admitted records contain relevant scalar, mutable primitive reference, or
recursive record fields and have no claim or qualification rosters. Reference
children transfer their existing permission without copying their referents;
the complete returned-leaf source map uses canonical path order, independently
of constructor and cleanup order. Projected owned child
construction remains unsupported: its residual custody cannot be inferred from
a whole-value operand. Ordinary borrowed projections retain their access rules.

Each operand is evaluated in authored order before this operation. Establishment
is atomic after all operands are available, with one logical fuel charge before
any child transfer or parent publication. A paused or failing operand computation
therefore retains the previously completed temporaries under their ordinary
cleanup rules, rather than exposing a partially initialized destination. Every
execution creates an independent referent, including nested machine activations.

## Store vocabulary

An initialized primitive local uses an unrestricted, unqualified, claim-free
`OperationResult` place. `EstablishPrimitiveLocal` consumes an exactly typed SSA
initializer and establishes the referent; `PrimitiveScalarRead` produces a
fresh scalar observation of an established local or readable primitive borrow.
Establishment must dominate every local use. A read is not equality with the
initializer or a previous read: intervening stores and calls can change storage.

Primitive reads and stores also retain an optional canonical field/index path
from the original root to a primitive structural leaf. Empty paths keep the
whole-primitive contract. Nonempty paths select relevant structural record fields
and fixed-array indices; each index must be below its declared extent, including
every dimension of a nested array. Cases, reference crossings and nonprimitive
leaves reject.

A read, and only a read, may also end at a scalar field of a record element it
selects at run time: `self.cells[self.i].hp` is one `PrimitiveScalarRead` whose
path is `cells`, `RuntimeIndex`, `hp`. The final field must be a relevant,
non-address integer or Boolean `Scalar` field of the record the carrier
resolves to; erased, bounded, IEEE, byte and structural fields reject, and so
does a carrier with no runtime element, which keeps the canonical
`IntegerStructuralField` or `BooleanStructuralField` read. Each runtime element
owns the obligation every runtime segment owns, `index < extent` plus
`0 <= index` for a signed selector, reconstructed against the array its prefix
resolves to. The read observes "some element", so it names no canonical leaf
and keys no field fact; a bounded field's declared range is supplied only by
the canonical read, which is why bounded fields stay outside this route.
This route was chosen over a runtime carrier on the field observations because
the primitive read already carries runtime elements through verification, the
codec, the interpreter and every Omega stage, while the field observations'
canonical carrier is the key every field fact uses; the field store keeps its
own operation because only it owns a bounded field's range obligation. Omega
rebuilds the leaf from declarations: the carrier's static offset and
`(index, stride)` runs, then the field's declared offset inside the record. This is direct storage access, not a synthetic record field or an
introduced reference lifetime. Root access, whole-value availability, claims,
qualifications and conflicting loans remain independently checked. An affine
root stays affine; observing or replacing an unrestricted leaf does not consume
the root or weaken its cleanup obligations.

An exact reaching-store check may equate the copied SSA result with a stored SSA
value only when every actual arrival reaches that same stored SSA value and the
definition-to-read interval excludes potentially aliasing writes. An unknown
arrival, intervening effect, or unproved cyclic arrival supplies no equality.
The reaching store must name the identical root and complete path: a store to a
sibling element cannot supply a read's value. Statically disjoint sibling writes
may preserve that exact reaching definition.
This is storage-version availability, not evaluation of a symbolic expression.
The resulting equality relates immutable values; writes after the capture do
not invalidate it. Floating-point copies do not license mathematical reflexive
equality, since NaN remains unequal to itself.

Each execution of establishment creates a fresh referent, including loop reentry
and nested activations of the same machine. Locals remain live through borrowed
calls and suspension; dead activation-local backing is reclaimed without affine
cleanup. This admitted local form cannot escape by owned transfer or structural
return. Shared, mutable, and write-only loans retain their usual compatibility
and access restrictions. Establishment and reads each cost one logical operation
unit, charged before execution.

| Operation | Retained subject |
| --- | --- |
| `WriteOnlyPrimitiveStore` | Original destination root, path to a primitive structural leaf (empty for a whole primitive), and already-defined, exactly typed SSA value. Path elements may be `RuntimeIndex` segments, each with the obligation the store owns. The referent is not represented as a synthetic record. |
| `StructuralScalarFieldStore` | Destination structural home, ordered path to the carrier record, final relevant scalar field identity, and already-defined, exactly typed SSA value. The carrier composes record fields and fixed-array elements, literal or `RuntimeIndex`, in any order; each runtime element's bound is an obligation the store owns. A whole, unqualified, claim-free owned record home (entry, block parameter, or completed establishment/call result) supplies write authority directly; mutable and write-only borrowed parameters retain their existing authority. An empty carrier path denotes a field directly on the root record. |
| `StructuralByteSequenceFieldStore` | Destination parameter, carrier path, final bounded-owned byte field, whole immutable source (borrowed view or established owned concatenation), exact dominating source-length observation, and capacity obligation. |
| `ByteSequenceStore` | Whole bounded-owned destination parameter, whole immutable source, exact dominating source-length observation, and capacity obligation. The destination's primitive byte qualifications remain required after replacement. |
| `StructuralByteSequenceFieldByteStore` | Destination parameter, carrier path, bounded-owned field, exact runtime `u64` index, `u8` value, current field-length observation, and index obligation. |
| `ByteSequenceWrite` | Whole mutable borrowed view, exact runtime `u64` index, `u8` value, current same-view length observation, and index obligation. It needs no synthetic record or field identity. |

These are non-observing Unit operations. Their names describe effects: a mutable
borrow may perform a non-observing store without first discarding read authority.
The admitted whole-primitive store has an unrestricted, unqualified, claim-free
mutable or write-only parameter, or an established owned primitive local.
A projected primitive store retains the corresponding mutable/write-only borrow
or whole established owned root; unrestricted and affine roots are admitted,
while linear, qualified and claim-bearing roots require further support.
Field-store admission independently checks the
complete home declaration, path, field, access, qualifications/claims,
scalar type, and dominating definition. Owned destinations must be established
and live as whole values, with no conflicting reference custody. A bounded
integer field store additionally retains its own `range_obligation`; unbounded
fields must not carry one. Independent reconstruction derives the conjunction
of the destination declaration's lower and upper bounds on the exact stored
SSA value, using only pre-write facts. The completed write's new field equality
cannot prove its own validity. The canonical certificate is checked before
execution, and Omega's current-IR validation binds the retained obligation to
the current destination, value and bounds before native lowering. Accepting
only the underlying scalar carrier would invalidate later range snapshots.
Integer and Boolean field observations
remain distinct operations and require readable access. A field or primitive
store whose path crosses a runtime element names no canonical leaf: it
publishes no field equation, forgets the current facts observed beneath the
static prefix before its first runtime element (the array it selects in),
and overlaps every element of that array. Facts of disjoint siblings, such as
the selector's own field, stay current; a prefix that cannot be scoped
forgets the whole destination root.

`IntegerStructuralField` and `BooleanStructuralField` retain the whole source
place, an ordered canonical carrier path, and the final declaration-local field
ID. The carrier path excludes the final field; an empty path is a direct read.
Each record step selects a relevant structural field of a record by its exact
parent-local ID. One literal fixed index may end the carrier when its element
record owns the field. A runtime-selected element's field is read by
`PrimitiveScalarRead` instead (see the store vocabulary above). One `Case` step may end the carrier when the preceding
record steps reach a sum. The final field is then a relevant scalar payload
field of that exact declared case. A case payload is overlaid at the sum's own
place, so the carrier's shape is no evidence that the overlay is live. Each
`Case` step must hold as a `StructuralCaseMembership` fact for the same root and
prefix in the read's incoming facts. A dominating membership test or dispatch
supplies that fact. Otherwise the case holds when every other declared case of
that sum is refuted. A refutation is a surviving membership observation of the
same place and case whose Boolean result is known false. A sum holds exactly
one declared case, so an exhaustive test chain's untested final arm may read
its payload. A write to the sum or any prefix retires these facts, as it does
for `StructuralCaseLeafCopy`. The rule is the same for a `Case` step directly on
the root, though producers read a root sum's payload as parameters of its
dispatch block. Unknown, erased, truncated, and non-record carriers reject;
consumers must not infer a path from names or layout offsets.

Reading a nested field observes the original backing below that root, not a
copied child or a new borrow. Independent validation retains the root's readable
access, type, multiplicity, qualification/claim custody and live whole affine
ownership. The scalar equation retains every carrier ID and the final field ID.
A bounded integer leaf supplies its declaration-derived range on the fresh read
SSA value, not on a mutable field alias; construction, entry and store checks
remain independently responsible for the declared invariant.

Before a write or mutating/consuming call invalidates current field facts, a
live exact equality between a Boolean/integer field and an immutable SSA value
may capture the affected scalar occurrences in those facts. The equality must
match the canonical root, complete path and scalar type; it may come from a
validated read, completed store or imported guarantee. Conditional equalities
cannot supply unconditional substitutions. Correlations keep their whole
connective, and any remaining affected storage observation still invalidates
the fact. This preserves a saved value's proof without transferring it to a
new read. It adds neither an entry-origin claim for private crash checking nor
an invariant for an unchecked cyclic arrival. Capture replaces facts rather
than enumerating combinations of old and new observations.

Consumers reconstruct projections from retained field/index identities and
types, rather than trusting an accumulated byte offset. Runtime scalar inputs
retain value identity; integer, Boolean, and IEEE literals retain their exact
type and value or raw bits. Fuel is consumed before mutation; suspension and
resumption must not repeat a committed store. Referent backing survives callee
frames so a completed write remains visible to the caller.

`EstablishByteSequenceConcatenation` establishes an ordinary structural operation
result with a `BoundedOwned` byte carrier. Each operand retains its exact live
length and either a readable whole byte source or a readable bounded byte field
identified by root, complete carrier path, and final field. A whole owner may be
read through shared or mutable access; literals and completed owned results
remain immutable. A field operand needs shared read authority; write-only access
cannot supply its contents. Its length
observation dominates concatenation, and no overlapping content mutation may
intervene, including length-preserving writes or mutation on a backedge.

The operation owns an obligation proving the mathematical sum of the two observed
lengths is at most the result declaration's capacity. The result's live length is
that sum, never its capacity. Both sources are read before the independent result
is published; replacing an aliasing destination afterward cannot change either
operand. Each execution establishes fresh activation-owned backing. The admitted
result is unrestricted, unqualified and claim-free, remains immutable, and enters
ordinary structural result availability checks. Length reads, byte reads and
whole-owner or whole-field replacement may consume this exact owned result
without inventing a borrowed view or granting mutable access. Its backing follows
activation lifetime and needs no affine disposal frontier entry.

Concatenation costs one logical operation unit, charged before execution. Neither
capacity nor owned custody establishes `Utf8`. Primitive byte predicates replay
independently from exact literal octets, declared field invariants, qualified
formal parameters, and the predicates preserved by concatenating both inputs.
Literal qualification rosters must agree with their actual octets. A qualified
formal's domain must name that exact byte carrier and a recognized primitive
predicate, with no content projection or establishment route; arbitrary semantic
domains do not become byte predicates. A subsequent replacement proves its own
receiving bound against the completed result's live length; larger operand carrier
capacities do not by themselves refute that bound.

Bounded byte replacement writes the source's live bytes and live length together,
without observing displaced contents. The verifier resolves capacity from the
exact destination declaration and reconstructs `length <= capacity`; neither an
asserted capacity nor another source's length supplies that evidence. Empty and
shorter replacements are legal when the destination predicates permit them, and
unused capacity is not live content. This is not raw fixed-array assignment,
which retains its exact-length rule.

`StructuralByteSequenceFieldStore` retains a static carrier path below an
unqualified, claim-free mutable or write-only parameter with unrestricted or
affine multiplicity. `ByteSequenceStore` instead replaces a whole `BoundedOwned`
formal with mutable or write-only access and unrestricted multiplicity. That
formal may carry exact primitive byte qualifications; projected qualifications
and claims remain unsupported. Both stores require a distinct immutable source,
its current dominating length observation, and independently reconstructed source
predicates implying every destination predicate. A readable mutable owner may
feed concatenation but cannot directly serve as an immutable replacement source.
Immutable source backing may be shared only while writes cannot mutate it or its
aliases. The store costs one logical operation unit, invalidates destination
observations, and creates no new ownership frontier entry.

A `PredicateByteSequence` field retains a nonempty set of primitive byte
predicates alongside its bounded-owned carrier or immutable shared byte view.
Equality contracts observe either carrier without discharging or erasing those
predicates. Qualified shared-view declarations do not grant record construction,
owned-return, or byte-mutation authority; those operations retain their separate
custody and predicate checks. Qualified mutable, write-only, and access-unspecified
view fields remain unsupported. The supported predicates are
valid UTF-8, ASCII-only, no NUL, and nonempty. Receiving validation checks these
invariants independently of source checking and repeats the check after
optimization. Zero-initialized attachment storage requires every contained byte
field predicate to hold for the empty sequence. Every write keeps every declared
predicate true: Terminal Psi opens no invariant window on a field, so reads and
calls may rely on the declaration at any point.

An indexed store into a bounded-owned predicate-bearing field owes more than
its bound; a bound alone cannot preserve the contents' invariants. The verifier
reconstructs the store's goal from the exact destination declaration:
`index < length`, then `value <= 127` when the field declares ASCII-only or
valid UTF-8, then `1 <= value` when it declares no NUL. Nonempty adds no leg
because the store keeps the extent. A producer supplies the certificate; it
cannot weaken the goal, and checked-source preservation facts are not carried
as authority.

Valid UTF-8 is not a per-byte class: an ASCII byte written over part of a
multi-byte scalar breaks it. Unless the field also declares ASCII-only, every
path into the store must reach a whole replacement of the identical root, path,
and field whose source predicates, replayed from literal octets, qualified
formals, or concatenation inputs, include ASCII-only. Between that replacement
and the store, only stores into the identical field, each proving its own byte
ASCII, and operations disjoint from the field may intervene. A replacement
from a source not proved ASCII, a projected move, an overlapping mutable or
write-only call argument, and dynamic dispatch each reject. The machine entry
supplies only the declaration, never ASCII contents. The Omega consumer
requires the accepted obligation to be exactly this goal and replays the
carrier rule on its current graph after every rewrite.

`ByteSequenceLength` on a whole bounded owner observes current live-length
metadata through shared, mutable, or write-only access. Metadata access grants
no readable payload authority through a write-only loan. A replacement or
potentially overlapping mutable call invalidates that observation, including
when it occurs on a backedge.

Indexed replacement changes one byte without changing live length.
`StructuralByteSequenceFieldLength` observes the selected field's current live
length as `u64`, not capacity or displaced contents. It permits shared, mutable,
or write-only access with the same claim-free, unqualified static field custody.
The byte store requires mutable or write-only access and reconstructs
`index < length` from a dominating observation of the identical root, path,
and field. Any intervening overlapping length-changing replacement or mutable
call, including one on a backedge, invalidates that observation. Disjoint sibling
writes and length-preserving byte stores do not.

A bounded-owned field-length observation also establishes `length <= capacity`
from the exact field declaration. This upper bound describes the observed scalar
snapshot; it neither makes capacity the live length nor gives an unbounded
borrowed view a declaration-owned limit.

A field-length observation may derive its extent from a still-current earlier
whole-field replacement. An established literal gives its exact octet count;
other replacements retain their source-length scalar only while that scalar
cannot reexecute between replacement and observation. These facts neither
substitute capacity for live length nor discharge source encoding predicates.
Both operations cost one logical operation unit. Indexed writes detach shared
immutable backing before mutation, preserving the source and sibling values.

Receiver writes require an actual mutable or write-only receiver. An attached
namespace alone grants no storage authority. Source `Self`, receiver position,
and the `self` write-frame root refer to the exact attachment, not an ordinary
parameter relabeled as a receiver. Executable entry provisioning is separate:
a receiver entry requires a generated bridge supplying initialized, root-backed
storage and its activation loan. A receiver-free attached machine needs no
hidden pointer.

## Conformance checks

Controls must cover caller-visible writes, forwarded/projected references,
register- and stack-passed pointers, address identity where exposed, and
write-only stores that do not read the destination. Shared-reference controls
use legal synchronized mutation through another permitted path, not a data race
or illegal mutable alias. Negative controls reject copied borrowed arguments,
access/shape mismatches, overlapping exclusive arguments, and stale or
substituted receiving plans. Interpreter-only or encoding checks do not replace
native caller-observation controls on both Linux architectures.

Current source and native limits belong beside
[Terminal production](../../../omega-rust/omega/compilation/driver/terminal-production/README.md#structural-access-and-stores)
and [ABI lowering](../../../omega-rust/omega/compilation/pipeline/02_abstract-operations-to-target-operations/README.md).
Unsupported forms reject; bounded implementation support does not redefine
reference semantics.

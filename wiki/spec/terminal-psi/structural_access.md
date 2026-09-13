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

`StructuralCaseMembership` reads a whole sum's current discriminator and produces
an unqualified Boolean. Its case identity must belong to the exact source type;
matching case spellings in another type supply no authority. The source must be
readable and established on every incoming path. An affine source must also be
live and whole at the observation, not merely present in the declaration table.

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

An exact reaching-store check may equate the copied SSA result with a stored SSA
value only when every actual arrival reaches that same stored SSA value and the
definition-to-read interval excludes potentially aliasing writes. An unknown
arrival, intervening effect, or unproved cyclic arrival supplies no equality.
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
| `WriteOnlyPrimitiveStore` | Destination primitive parameter or established primitive local and already-defined, exactly typed SSA value. The referent is not represented as a synthetic record. |
| `StructuralScalarFieldStore` | Destination parameter, ordered path to the carrier record, final relevant scalar field identity, and already-defined, exactly typed SSA value. An empty carrier path denotes a field directly on the root record. |
| `StructuralByteSequenceFieldStore` | Destination parameter, carrier path, final bounded-owned byte field, whole immutable source view, exact dominating source-length observation, and capacity obligation. |
| `StructuralByteSequenceFieldByteStore` | Destination parameter, carrier path, bounded-owned field, exact runtime `u64` index, `u8` value, current field-length observation, and index obligation. |
| `ByteSequenceWrite` | Whole mutable borrowed view, exact runtime `u64` index, `u8` value, current same-view length observation, and index obligation. It needs no synthetic record or field identity. |

These are non-observing Unit operations. Their names describe effects: a mutable
borrow may perform a non-observing store without first discarding read authority.
The admitted primitive-store form has an unrestricted, unqualified, claim-free
mutable or write-only parameter, or an established owned primitive local.
Field-store admission independently checks the
complete parameter declaration, path, field, access, qualifications/claims,
scalar type, and dominating definition. Integer and Boolean field observations
remain distinct operations and require readable access.

Consumers reconstruct projections from retained field/index identities and
types, rather than trusting an accumulated byte offset. Runtime scalar inputs
retain value identity; integer, Boolean, and IEEE literals retain their exact
type and value or raw bits. Fuel is consumed before mutation; suspension and
resumption must not repeat a committed store. Referent backing survives callee
frames so a completed write remains visible to the caller.

Bounded byte replacement writes the source's live bytes and live length together,
without observing displaced contents. The verifier resolves capacity from the
exact destination field declaration and reconstructs `length <= capacity`;
neither an asserted capacity nor another view's length supplies that evidence.
Empty and shorter replacements are legal, and unused capacity is not live content.
This is not raw fixed-array assignment, which retains its exact-length rule.
The admitted destination is an unqualified, claim-free mutable or write-only
parameter with unrestricted or affine multiplicity and a static carrier path.
Source encoding predicates still require source-level proof. Immutable source
backing may be shared, provided later writes cannot mutate that source or aliases.
The store costs one logical operation unit and invalidates destination observations;
it creates no new ownership frontier entry.

Indexed replacement changes one byte without changing live length.
`StructuralByteSequenceFieldLength` observes the selected field's current live
length as `u64`, not capacity or displaced contents. It permits shared, mutable,
or write-only access with the same claim-free, unqualified static field custody.
The byte store requires mutable or write-only access and reconstructs
`index < length` from a dominating observation of the identical root, path,
and field. Any intervening overlapping length-changing replacement or mutable
call, including one on a backedge, invalidates that observation. Disjoint sibling
writes and length-preserving byte stores do not.

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
[Terminal production](../../../omega-rust/psi/compiler/terminal-production/README.md#structural-access-and-stores)
and [ABI lowering](../../../omega-rust/omega/pipeline/abstract-operations-to-target-operations/README.md).
Unsupported forms reject; bounded implementation support does not redefine
reference semantics.

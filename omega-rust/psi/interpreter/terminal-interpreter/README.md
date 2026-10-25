# Terminal interpreter

Start at [terminal_interpreter.rs](src/terminal_interpreter.rs). Execution follows verified
[calls and outcomes](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
and [structural ownership](../../../../wiki/spec/terminal-psi/ownership.md).

The domain entrance owns artifact loading and startup: external bytes decode
once, while in-process callers borrow a sealed Terminal artifact view. Both
construct instances of the same execution engine. Loading does not perform
proof verification. [execution.rs](src/execution.rs) is the engine —
the live state and the resumable dispatch loop. [Call frames](src/calls/frames.rs)
bind invocations and save live caller state; [custody](src/custody.rs) checks and
transfers structural ownership and claims.
[structural_identities.rs](src/structural_identities.rs) owns execution-wide local
identity allocation and host reservations; record copies and block transitions
stage its allocation cursor until their payloads commit. Storage-specific
operations remain under their named reference, record, array, primitive, and byte-storage owners
([byte_sequences.rs](src/byte_sequences.rs) and its subordinates).

The program retains one immutable canonical module, owned or borrowed.
Machine, block and declaration lookups use its already-sorted tables without
rebuilding trees or copying an executable projection. Borrowed storage must
outlive every program handle and paused instance. External loading retains the
whole decoded module, including its non-executable catalogs.
Frames retain machine/block identities, not cloned block graphs. A resume borrows
operations and terminators from that owner while mutating live execution state.
One shared owner preserves the existing owned execution API without introducing
self-referential borrows; it does not allocate shared owners per operation.
Runtime payload copies, effect observations, and retained return/cleanup metadata
remain separate from executable code. This removes code copying, not all runtime
allocation, and does not establish a measured speedup.

## Embedding contract and probe limits

The accepted [embedding contract](../../../../wiki/spec/build/embedding.md)
now has its public surface here rather than only the raw hook. `TerminalProgram::load`
decodes canonical semantic bytes; `load_envelope` consumes the portable envelope;
`borrow` uses an existing canonical artifact view. All retain immutable shared
code; `instantiate` starts an independent instance without re-reading bytes.
Its `TerminalInstantiation` request carries scalar/structural inputs, optional
admitted installation and optional Terminal invocation account; there is no
separate installed-instantiation route. The account is selected before entry
backing allocation and follows nested component instances.
`TerminalInstance::bind_import` establishes each demanded import slot against a
checked adapter before `invoke` runs any guest operation — a missing, unknown,
provider-realized, duplicate, or signature-mismatched supply rejects without
effects — and `close` releases the instance's runtime state once the activation
settles. `TerminalImportAdapter` is the contract a host object declares;
`TerminalScalarImport` and `TerminalUnitImport` wrap the common leaf signatures.

After normal return, `TerminalInstance::observe_borrowed_entry` reads a semantic
leaf of an original host-lent structural argument by dense argument index and
canonical declaration path. The original home survives retired helper/entry
frames; owned inputs are not retained for readback. Observation returns plain
scalar/case contents or borrowed byte readers, never a qualified guest value or
opaque host capability. Paused, crashed, externally terminated and closed
instances cannot report a normal borrowed result. Source-produced ordinary and
Build helpers exercise this route in `compiler`'s `terminal_entry_writeback`
integration test, with retained and externally loaded artifacts. This supplies
Build settings observation; it does not migrate Build effects or activation
scheduling from the legacy evaluator.

Byte readback retains a Rust borrow of the instance, not another payload copy.
`TerminalByteRead::len` lets the host validate size and reserve live storage and
result custody before allocating its output; `copy_to` fills that caller-owned
buffer. A scalar-backed byte window stays scalar-backed during observation.
This is a publication prerequisite, not completed Terminal storage accounting.

Owned byte backing now reserves its full logical length before allocation.
Aliases share the backing's lease; even a small window keeps the full backing
reserved. Copy-on-write reserves the overlapping replacement before copying.
Boundary byte buffers own the same storage type: capture and growth reserve
before copying, and whole-owner writeback moves the reservation with the bytes.
Retirement releases live bytes, not consumed fuel.
[Runtime rows](src/runtime_rows.rs) own one cell per live scalar or structural
table entry, including staged call/edge bindings and cleanup copies. New entries
and overlapping copies reserve first; replacements retain their cell, frame moves
carry the lease, and table joins transfer reservations without charging again.
Removal and teardown release only retired rows. The complete source-produced
Build control checks exact and one-cell-short budgets across fuel pauses, retaining
faults without replaying effects. Root activations and suspended call headers
each reserve one cell before creation. The shared frame capture in
[frames.rs](src/calls/frames.rs) handles scalar, structural and cleanup calls;
return restores the caller and releases its saved header. The root activation
remains reserved through result readback until close. A refused capture retains
its outgoing cleanup continuation in that failed activation, including byte
backing and returned claims; it allocates no unpaid saved frame and does not
advance the caller's instruction position. Packed scalar arrays retain one cell
per element with their backing: guest snapshots reserve independent copies,
inspection clones share read-only backing, and writes reserve any overlapping
replacement. Plain projection copies use that same snapshot operation. Returned
payloads remain charged after instance close until the last inspection drops.
Frame-local array, byte, element-view, primitive-loan and aggregate-backing
descriptor tables use the same paid row owner through argument staging, saved
frames, joins and return installation. Even an empty array needs its descriptor
cell. A refused return retains its output, caller continuation and loan roster
beside any partially installed rows until the failed activation is discarded.
Global descriptor/placed/case storage and result publication are not yet completely accounted;
these partial snapshots must not become production Build usage receipts.

A failed execution retains its exact host fault and unfinished custody. Calling
it again returns that fault without charging or reexecuting the site, even if
resource capacity later becomes available. Only an explicit pause is resumable.
Failure does not create normal-return readback or make a busy instance closable.

Retained checked-provider candidates describe availability, not the receiving
host's selection. `TerminalProgram::admit_runtime_supplies` joins explicit normal
host-import choices with an existing sealed checked installation. The returned
program-bound selection is shared by preflight, instance construction and core
dispatch. Duplicate or competing choices, unknown slots, wrong program identity
and external-completion imports reject before effects; selection grants no
adapter or ProcessExit authority. `TerminalInstance::import` describes the
instance's actual demanded imports, including explicitly selected slots with
retained checked alternatives. `TerminalProgram::imports` remains the default
catalog without those explicit choices. Unselected candidate-bearing demands
still require a checked installation rather than falling back to a host import.

Remaining gaps toward the full contract: an instance exposes only the module's
entry invocation (no exported secondary operations yet), adapters above the
scalar/unit leaves are hand-implemented `TerminalImportAdapter`s, and public
entry descriptions wired from product compilation (component publication)
remain a named dependency outside this crate. The raw `TerminalEffectHandler`
stays the low-level control the embedding layer is built over — it is not an
admitted binding API.

Checked assembly follows the [instruction interpretation contract](../../../../wiki/spec/language/assembly.md#interpreted-execution):
reuse ordinary value/place/transition operations, keep scratch state block-local,
and preserve explicit memory and environment effects across calls and fuel stops.
The selected target environment must supply every demanded operation before
execution. A generic effect hook or successful Unit response alone establishes
neither instruction semantics nor environment correspondence. End-to-end
instruction coverage remains **INTERPRETED-ASSEMBLY** on the execution board;
this contract is not a claim that all catalog entries already run here.

The source-produced [embedding tests](../../../omega/compilation/driver/tests/terminal_entry_writeback.rs)
cover retained/loaded artifacts, borrowed-entry readback, once-only Build effects,
resource refusal and frame retirement across fuel pauses. Two sequential empty
call trees complete with exactly three live header cells; a two-cell allowance
refuses the nested call, and an occupied allowance refuses root instantiation.
Normal return retains one root header until close; fault retries cannot resume.
These controls do not establish pending host completion or component replacement.
Run them with:

```text
mbx nextest run -p compiler --test terminal_entry_writeback --test-threads 1
```

## Cyclic execution

Ordinary interpretation accepts verified natural-ranked scalar, plain-owned-input,
immutable-view, receiver, and primitive-local
graphs under the [control contract](../../../../wiki/spec/terminal-psi/control_flow.md).
The legacy one-machine structural Unit countdown uses its own verifier entrance:
it reconstructs `0 < remaining` as the unsigned `1 <= remaining` premise and
checks exact subtraction before creating resumable state. This interpreter
acceptance grants no native, fixed-fuel, provider-installation, or mixed-work
authority.

[block_bindings.rs](src/terminators/block_bindings.rs) captures all selected operands before
cleanup and simultaneous successor installation. Affine inputs move once;
Unrestricted descriptors remain reusable. Rebinding changes the descriptor place,
not its referent or field backing. Missing/duplicate owners and transfer/discard
overlap reject before mutation. Fuel suspension preserves the uncommitted edge.
Projected affine operands retain the original subtree backing and leave their
maximal live complement for the selected cleanup schedule. Case paths check the
current discriminator before moving, select only active payload siblings, and
retain mixed common fields in their independent namespace. They do not create
fresh affine copies. Qualification requirements are checked against captured
runtime memberships by the common argument binder, not copied from destination
declarations. Claim-bearing projected owners still need their separate transport. Exact
unqualified mutable byte-view block parameters retain an existing field loan.
Plain aggregate mutable and write-only block parameters retain their original
subtree backing, with source declarations governing access attenuation. State
edges carry a root and projection path, not a manufactured stored reference.
Qualified bounded owners retain independent immutable byte snapshots across
states. Reference-bearing exclusive aggregate transport remains separate.

## Calls and work

Unit, scalar and structural results share one call-frame transition after their
exact signatures and arguments are prepared. A structural result imposes no
additional input-path or unqualified-shape restriction. Returned reference
backings are captured after owned argument staging and checked at completion;
the returned carrier, byte contents and memberships enter the saved caller only
on successful return.

The interpreter charges a call before entering its owned frame. Exhaustion inside
the callee resumes without paying or invoking that call again. A callee crash
retains its original site and edge charge, with no fabricated caller crash.
Ordinary recursive call graphs remain fenced until their tail/ranking evidence
is admitted; cyclic block execution has its separate verified route above.
Installed checked Unit providers forward the boundary's ordered scalar arguments
through the ordinary call binder, alongside independently prepared structural
arguments. A Unit result does not imply an empty parameter list. Installation
replays the exact signatures; fuel suspension retains the same selected frame.
Fixed-work composition distinguishes normal return bounds from crash bounds:
only a normal return composes the caller's remaining work.

Checked-provider structural returns transport the actual returning edge's claims
through the ordinary path-based result binder, including linear children of
affine records. They do not mint replacement claims from result declarations.
Only the exact admitted crossing can add its authorized root qualifications;
calling an adapter directly retains its ordinary declared result contract.

## Primitive array results

[scalar_array.rs](src/scalar_array.rs) owns complete primitive-array construction
and return. Ordinary and installed checked-provider structural calls return the
exact unrestricted payload into
their saved caller's fresh result place. Earlier caller arrays survive alongside
that result. Return preflight precedes fuel charging and frame restoration, so
exhaustion cannot publish a partial result or repeat a completed invocation.
Nested and empty dimensions, Boolean values, and IEEE payload bits are retained.
Ordinary Unit, scalar, and structural calls copy whole owned unrestricted arrays
into the exact callee parameter places. Repeated arguments preserve the caller's
payload, and parameter returns select the actual input contents. State transfers
capture all incoming array payloads before committing successor bindings, so
parallel transfers cannot overwrite a later operand. Installed providers use
the same saved-frame return transport after exact conformance and installation
replay. Opaque host identities cannot fabricate array payloads: host result
preflight rejects them before invoking the handler. Borrowed/projected array
result payloads remain unsupported; existing borrowed byte-array backing is a
separate route. Native realization belongs to Omega's result-home and target
transport owners.

## Primitive local storage

[primitive_storage.rs](src/primitive_storage.rs) establishes fresh referents on
each activation or loop reentry. Borrowed calls share their backing; reads copy
the current scalar, so earlier snapshots survive later writes. Suspension retains
the storage, and departure reclaims it without affine cleanup. Local identities
cannot collide with declared places or supplied host identities. This support
does not provide native allocation or permit owned local escape; see
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md).

## References stored in records

[record.rs](src/record.rs) stages descriptor relocation together with ordinary
record payload construction. Nested packing changes the carrier's owner and
field path, never the original primitive backing. [reference.rs](src/reference.rs)
resolves typed field paths ending in `Referent`; whole-owner cleanup removes
descriptor subtrees without disposing the borrowed storage. The verifier checks
loan ancestry and cleanup order independently. Language fixtures, including
[projected reference returns](../../../../tests/omega/pass/borrows/runtime_projected_reference_call_return_exit/main.omg),
exercise original storage mutation and reborrowing through ordinary calls.
Return preflight compares every
leaf's captured backing against its bound formal before transferring the same
record identity into the caller. Whole owned affine record arguments preserve
that same descriptor custody. Structural call preparation retains only the
expected returned-leaf backings in its existing suspended result, so return
preflight remains valid after a formal is moved, repacked, or passed through
another call. These expectations are not additional owners or loan authority.
An opaque entry or host-result identity cannot supply nested reference carriers.
Host storage loans below borrow established backing; they cannot manufacture a
record's references from native descriptor bytes.

## External storage loans

Ordinary primitive arguments also retain their original storage endpoints.
In particular, a local array element stays in its suspended owner's frame;
callee parameters, successor bindings and stored references carry a loan
association rather than a copied scalar. Shared representation views and
qualifiers retain their ordinary rules. The stricter host admission described
below applies only when an external adapter receives a payload buffer.

`TerminalEffectHandler::storage_arguments` requests observation of selected
structural arguments. `handle_effect_with_buffers` receives those semantic
storage buffers beside byte buffers. `TerminalBoundaryStorageBuffer` separates
a primitive endpoint from a complete active aggregate; both identify the
structural argument index and admitted access without exposing interpreter
storage handles. Mutable replacements stage exactly typed contents; shared
borrows cannot replace them. The core captures original local, field and array
element homes before dispatch, including suspended callers' backing. It
validates every staged payload and original restriction before publishing
results or input writes. It never resolves a scalar source again after host
code. A per-buffer token prevents substitution from another invocation.

Aggregate contents include nominal cases, not native discriminator integers.
Exclusive replacement can change the active case only with complete valid new
contents. Completion removes stale payload storage and restores the original
referent identity, including byte-array backing. Common mixed fields and payload
fields keep separate namespaces. Opaque children must retain their original
storage: host bytes cannot manufacture permissions or pointer descriptors.
Changing opaque children or inline byte fields remains an unsupported semantic
codec, not a language restriction on case replacement. Native adapters own
layout, tag encoding and accessed-prefix coverage. Padding, descriptor bytes,
and unread inactive payload bytes supply no semantic contents.

This protocol admits only unqualified initialized shared/mutable storage
loans. Write-only input and qualified owner storage reject before dispatch;
observing a plain formal does not erase the caller's stronger restrictions.
Exclusive aggregate loans can inspect an original atomic resident without
forming an ordinary scalar reference. Shared loans omit that resident; a native
adapter can observe a disjoint ordinary prefix, but cannot snapshot its bytes.
Adapters that implement only the old byte callback refuse storage loans before
performing effects. Checked provider bodies still borrow the ordinary live
storage and may forward it into an external call; they do not receive a staged
copy that could overwrite nested changes on return.

## Bounded byte fields

Whole-field replacement supplies live backing associated with the original
structural referent, carrier path, and field. Field-length observations and
indexed byte stores retain that backing across calls and fuel suspension.
Indexed stores validate the current extent before mutation and detach shared
immutable backing; they cannot change source literals or sibling fields.
An opaque incoming structural identity does not supply initialized field bytes:
measuring or indexing an unprovided field fails rather than assuming empty
or zero-initialized storage. This execution support does not establish native
byte-field realization.

The current migration path lets an initialized bounded byte field supply an
unqualified mutable boundary parameter. The boundary-specific resolver
retains the original referent, complete record/array path, and capacity; it does
not make bounded storage generally interchangeable with views. The exact
mutable field-to-byte-view presentation is also available to ordinary Unit
helpers on the verifier's admitted field-only paths; it retains the same field
loan rather than copying or owning bytes. Missing backing still rejects.
`TerminalEffectHandler::handle_effect_with_byte_buffers` receives pre-call
bytes and staged writable storage. An ordinary mutable view uses
`TerminalBoundaryByteBuffer::bytes_mut`: its slice cannot resize storage or
expose unused capacity. `replace` belongs only to a whole bounded owner and
rejects ordinary views, including equally sized replacements. All captured
loans, buffer extents and the declared response validate before writeback or
completion commits. The effect trace records pre-call bytes;
`structural_byte_sequence_field` observes committed writes.
Handlers without this callback reject mutable buffers before performing an
effect. Missing backing remains an error, including for an opaque entry object.

Installed checked providers receive a frame-owned mutable field binding, not a
copy of the incoming bytes. Whole mutable-view arguments can forward that binding
through ordinary helpers or reborrow it at another boundary. Each external call
observes the current backing and commits to the original field; returning from
the provider does not restore its entry snapshot. Calls and fuel suspension
retain the binding's exact referent, record/array path, and capacity. An equal
opaque identity without that binding supplies no mutable loan. Byte reads and
subslice operations still require immutable bindings. Native byte-field forwarding and
`read_line` realization remain separate dependencies.

Fixed-view length and indexed writes use the exact existing mutable field loan
at a machine or block parameter. Length means the current live extent, never
capacity. A write validates the typed index, byte, and saved length against that
extent before changing one byte; it cannot resize storage or alter owner length.
State transfers capture the original referent binding before fuel and commit,
reject aliasing mutable arguments, and preserve writes through calls and suspension.
Shared immutable backing detaches on mutation, preserving sibling values and
the destination's untouched suffix. External boundary replacement remains its
separate existing contract.

The settled [bounded-input contract](../../../../wiki/spec/resources/bounded_input.md)
does not use this owner-replacement behavior: it writes within a supplied slice
and returns a count plus line/EOF/full outcome, with ZII Invalid reserved for no
result. Migrate line-input callers, checked providers, and interpreter/native
consumers together. Existing replacement/forwarding tests pin their current
binding behavior, not new line-input acceptance. Ordinary whole-field stores
remain independently supported. Encoding qualification needs checked library
validation of the written prefix; buffer writeback itself grants none.

## Fixed byte arrays

`TerminalExecution::start_artifact` -- the only start entry, since `8e2dc8b8ade91`
collapsed the two named variants -- supplies explicit initialized fixed-array
contents through `TerminalStructuralInputs.byte_arrays`, by structural argument
and relative field path.
Every supplied value must match a real unqualified `FixedArray(u8, N)` and contain
exactly `N` bytes. Duplicate referents, mistyped paths, and fabricated storage
reject; an opaque root does not imply initialization. `structural_byte_array`
observes the original backing by referent identity and path.

Ordinary helpers and installed checked providers may borrow the whole array or
an admitted record-field array as a mutable byte view. Argument preparation
shares the exact initialized-array binding rather than staging a field replacement.
Calls and block transfers retain that binding; length remains `N` and indexed
writes preserve all other elements through fuel
suspension. External calls stage the selected initialized range, including
array storage in suspended caller frames. `bytes_mut` writes that fixed extent;
successful completion commits it to the captured array and offset, never a
newly resolved equal-looking place. Short reads preserve the unread suffix.
A fieldless array binding is distinct from the bounded-field loan, so the
external `replace` callback cannot resize an array. This input establishment
route does not itself implement source-owned construction or native storage.

## Boundary responses

[effects/results.rs](src/effects/results.rs) distinguishes Unit, scalar, and opaque
structural values from an explicit `Crash(Trap | Abort)` response. A crash is
admitted only against the published same-cause route with the invocation's
already evaluated scalar inputs. A true guard permits normal completion too.
Closed IEEE comparisons use those exact landed operand bits, including unordered
NaN comparisons; the host's report cannot manufacture a permitted crash route.
Mathematical-integer guards reuse proof admission's exact closed denotation,
including intermediates beyond the fixed-width input carrier. Input traversal
and cumulative arithmetic growth/work are bounded before allocation; exhausting
those private service limits rejects the reported crash distinctly from a false
or unsupported guard. This neither changes mathematical integer semantics nor
adds authored mathematical-guard lowering. The verifier's outcome validator
reuses the existing positional substitution and closed scalar arithmetic semantics.

A Trapping primitive whose exact value does not exist crashes with cause `Trap`
at `TerminalCrashSite::Operation` (machine, block, operation) before any
result is published; its frontier is the machine-local live claim set, as for
a boundary crash, and no wrapped or saturated value is substituted.

A valid boundary crash retains a tagged machine/block/operation/boundary site,
not a fabricated edge. The attempted call remains in the effects list: those
records describe invocation inputs and required normal-completion receipts,
not proof that completion occurred. No result, receipt, owned-input disposal,
staged byte-buffer writeback, later operation, or cleanup commits on crash.
Previously performed host effects cannot be rolled back. Repeated resume returns
the same crash without charging or invoking again. Ordinary operand crashes
retain their original edge and prevent the enclosing boundary invocation.
Installed-provider refinement and native support remain separate; the edge-only
`TerminalTraceV1` observer explicitly rejects boundary crash routes until its
complete observation representation supports them.

The current structural response has exact type,
qualifications, opaque identity, and an empty path. Linear results, result claims,
projected qualifications, and sum discriminator/payload inspection remain
unsupported. Preflight rejects unsupported result requirements before the host
effect; validate the response before committing result/claim custody.

Structural entry records and fixed arrays containing bounded integer fields
require explicit contents for every bounded field, including unread fields.
Startup validates each exact integer carrier and range before establishing
custody; missing, duplicate, or conflicting aliased fields reject. Unbounded
integer fields keep their existing deferred initialization check. Arrays are
visited incrementally, so missing contents do not require expanding their shape.
Opaque entry inputs and host results recursively containing bounded integer
fields still reject, as do bounded sum or mixed entry shapes without a selected
case discriminator. Their type identities do not supply complete field or
selected-payload values with which to validate the restrictions. Result preflight
rejects before the handler runs; this fence does not prohibit independently
valid selected scalar-case construction or supported exact internal transfers.

`EstablishScalarCase` materializes the declared selected payload from existing
typed scalar values, including independently proved restricted integers. The
interpreter publishes one placed home holding the discriminator and scalar
payload cells, just as `EstablishStructuralCase` does. This storage survives
internal-call suspension; case inspection stages the selected scalar parameters
before whole-result disposal. The public scalar-case host/result payload is an
import/export snapshot, not a second frame-local storage representation.
Affine case results support ordinary whole-root cleanup on returns and edges.
Opaque host values still cannot supply case observations. Ordinary calls pass
complete internally established cases through ordinary placed storage: owned
affine cases move once, unrestricted owned arguments copy their actual subtree,
and borrowing preserves the caller's original home. Mutable whole-case lending
can change the selected case under the same declaration and custody constraints
as every other structural value. Scalar-case host results validate their contents
before publication into that storage.

`EstablishStructuralCase` builds scalar or structural payloads, or a mixed
shape's case beside its common fields, as runtime cells under a fresh
root: the discriminator in the root's case contents, common fields as member
cells, and scalar payload members as case-payload cells. Terminal numbers common
and payload fields in separate namespaces (verification matches the roster by
position), so a cell key carries its namespace and every subtree copy or move
keeps it. A `StructuralCase` dispatch over such a root reads the stored
discriminator and binds the selected payload's scalars from those cells.
An atomic field's completed scalar initializer establishes its original cell,
not an atomic event or an immutable snapshot. Shared calls retain that backing.
Atomic accesses to record members and mixed common members use the same member
namespace, preserving the checked event's observed prior and exact location;
native realization independently checks the common member's post-tag offset
and alignment. Direct atomic payload access still needs its selected-case
location and atomic-event lowering; scalar-carrier equality does not grant it.

Host response validation is not allocation or freshness verification. The host
must return a legitimate owned value. Rejection preserves interpreter bookkeeping,
not effects the host already performed. Completion followed by budget suspension
must not repeat the effect. This carrier does not extend native provider support.

Residual disposal validates and charges the owning edge before mutation. On a
jump, scalar arguments materialize before disposal and successor binding. An
exhausted budget leaves residual ownership live. The exact producer/result root
and claim map survive suspension; no partially moved ancestor becomes whole again.

Structural storage copies and constructor staging use identity-bounded
[`structural_storage`](src/structural_storage.rs) ranges. Keys order by carrier
identity before path and field; subtree selection still checks the complete path.
Each transaction stages all seven storage families before pruning either region,
so overlapping copies/moves preserve the original snapshot behavior. This avoids
scanning unrelated historical carriers; it grants no access to borrowed cells.
At settled block boundaries scalar-only roots without executable handles may be
reclaimed. Current and suspended frames, captured reference backings and every
non-scalar storage family protect their roots. Loans, views, byte bindings and
nominal cleanup transactions conservatively defer collection.

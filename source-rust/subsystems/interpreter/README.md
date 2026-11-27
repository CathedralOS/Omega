# Psi interpreter

This crate runs Terminal Psi. Start at [interpreter.rs](interpreter.rs). Execution follows the verified
[calls and outcomes](../../../wiki/spec/terminal-psi/calls_and_outcomes.md)
and [structural ownership](../../../wiki/spec/terminal-psi/ownership.md) rules.

The crate also owns the live storage ledgers under `memory/`: the
[extent ledger](memory/extents/) (`extents`), the
[live access checks](memory/storage-access/) (`storage_access`) and the
[layout materializer](memory/materialization/mod.rs) (`storage_materialization`).
The host runtime reaches the first two as `terminal_interpreter::extents` and
`terminal_interpreter::storage_access`.

Three crates under `host/` and `numeric-imports/` stay separate because
`mbx xtask check-boundary` requires it:
- `retained-image-runtime` is the only runtime package allowed to read the
  completed native image (`native-artifact`); the host runtime may not.
- `native-provider-sponsor` uses native image emission. The host runtime may
  not depend on a native stage, and the source compiler, which depends on
  this crate, may not reach native code.
- `interpreter-numeric-imports` combines this crate with the native
  `target-vocabulary`. The source compiler depends on this crate and may not
  reach native code, and native stage 00 uses the numeric imports without
  the host runtime.

The product compiles a program before running it. The engine depends on its
input representation and semantic checks, not on the compiler coordinator, so
compile-time evaluation creates no dependency cycle.

[Compile-time admission and elaboration](../compiler/build/README.md)
belong to compilation. That service calls the Terminal engine and installs the
results into compiler representations; the engine does not own those
transformations. A build-time machine that would have to run before the program
is checked has no engine, and the compiler refuses it.

Native code generation stays in Omega. The interpreter is not a fallback when
native lowering does not support a construct. See the
[pipeline contract](../compiler/README.md).

The interpreter installs the bindings it is given and dispatches calls. Host
adapters that know the target live outside Psi, in Omega's host integration.
They reuse the independent backend ABI libraries and keep exact foreign-binding
checks, so Psi gains no Omega dependency. Ordinary Omega provider bodies can
run interpreted; only external mechanisms need host adapters. See
[target environments and engines](../../../wiki/spec/build/embedding.md#target-environments-and-execution-engines).

The crate's public entry points own artifact loading and startup. External
bytes are decoded once; in-process callers borrow a sealed Terminal artifact
view. Both build instances of the same execution engine. Loading does not
verify proofs.

- [execution.rs](instructions/execution.rs) is the engine: the live state
  and the resumable dispatch loop.
- [Call frames](instructions/calls.rs) bind calls and save the caller's live
  state.
- [Custody](memory/custody.rs) checks and transfers structural ownership
  and claims.
- [primitives.rs](memory/primitives.rs) allocates local
  identities for the whole execution and holds host reservations. Record copies
  and block transitions stage its allocation cursor until their payloads
  commit.
- Reference, record, array, primitive and byte storage each have their own
  files ([bytes_and_views.rs](memory/bytes_and_views.rs) and those
  it uses).

The program keeps one immutable canonical module, owned or borrowed. Machine,
block and declaration lookups use the module's already-sorted tables; they do
not rebuild trees or copy an executable projection. Borrowed storage must
outlive every program handle and paused instance. External loading keeps the
whole decoded module, including its non-executable catalogs.

Frames keep machine and block identities, not cloned block graphs. A resume
borrows operations and terminators from the module while it mutates live
state. One shared owner keeps the existing owned execution API without
self-referential borrows, and it is not allocated per operation. Runtime
payload copies, effect observations and saved return/cleanup metadata stay
separate from executable code. This removes code copying, not all runtime
allocation, and no speedup has been measured.

The [native provider sponsor](host/native-sponsor/sponsor.rs) starts the isolated
worker and installs the compiler's completed adapter images. The caller supplies
the published roster. The sponsor matches each request against its application,
locator, calling convention and image digest before sending bytes to the worker.

## Admission and preparation work

Until containment of untrusted artifacts is audited, the
[admission contract](../../../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
requires the host to explicitly trust an exact producer artifact before running
it. The byte-loading APIs below do not take that trust witness yet. Canonical
decoding and safe Rust do not isolate the guest. Inspection works without
execution trust, under shared input and work limits.

The [implementation plan](../../../wiki/drafts/designs/psi_claim_admission.md)
tracks the API gate, splitting the inspection and preparation decoders, the
host's optional encoded input size limit and the total loading budget. It also moves
declaration-dependent restrictions and descriptor lookups out of repeated
dispatch: today primitive stores, reference paths and boundary qualifications
reach static entailment from the execution layer. Prepared plans must keep
the checks of predicates on actual values and the safety of concrete
operations. Acceptance measures startup and steady-state costs; it does not
assume closed predicate evaluation is always cheap.

## Which execution refusals admission already covers

The interpreter still refuses malformed programs while it runs them: 2,087
production calls to `malformed()` and seven `VerifiedOperationContractInvalid`
refusals check, one operation at a time, properties that a checked program
cannot violate. This table records, for each family of refusals, whether
loading already guarantees the property, so that later changes can move each
check into admission once or delete it as covered. A check that depends on
values only known while running stays in execution. Measured on `main`
`47996c86b`.

The table depends on what loading does today. A program reaches execution by
one of two routes:

- **Bytes** (`TerminalProgram::load`, `load_envelope`,
  `TerminalExecution::start_artifact`). `decode_closed_parts`
  (`psi/artifact/wire/src/semantic_module.rs`) decodes canonically, then runs
  `validate_storage_foundation` and `validate_structural_foundation`
  (`foundation_validation.rs`). The latter calls
  `machine_foundations::validate_machine` and `validate_operation_foundation`
  on every machine and operation. Decoding does **not** run the lowered-form
  checker. These routes also take no host trust witness yet
  ([admission plan](../../../wiki/drafts/designs/psi_claim_admission.md),
  step 1), so they can receive bytes that no checker accepted.
- **In process** (`TerminalProgram::borrow`, `retain`). The module comes from
  this process's compiler after the lowered-form checker accepted it.
  `ClosedTerminalModule::new` only seals it: `encode_closed_execution_module`
  checks scalar qualifications and canonical order, not the foundation
  validators above.

So "covered" below means covered on both routes. A property that only the
checker establishes is not covered on the bytes route until execution entries
require trust. Until then, removing its refusal would let an unchecked artifact
reach code that assumes it. Under AGENTS.md rule 2 such a check moves into a
preparation pass that runs on every route. It is never just deleted. The
[admission contract](../../../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption)
asks for exactly this split: preparation computes declaration-dependent check
plans once, and dispatch checks only actual values ("actual bounds, storage
liveness, access permissions, operand safety and admitted resource limits").

### The six per-operation semantic re-checks

| Site | What it checks | Already guaranteed? | Depends on run-time values? | Verdict |
| --- | --- | --- | --- | --- |
| `embedding/boundary_inputs.rs:78` `validate_access_view_referent` | A host-supplied access-view establishment names a referent type, path and canonical qualifications that the module declares. | No. The referent comes from the host's entry inputs, not from the program. | Yes: host input. It runs once per instantiation in `establish_access_view_inputs`, not per operation. | Keep. It is already admission of host input. |
| `instructions/structural.rs:981` `validate_vacant_structural_local` | An `EstablishStructuralLocal` result names one declared storage type, carries no claims or establishments, and declares only memberships its shape allows. | Bytes route: yes, `validate_establish_structural_local` (`foundation_validation.rs:10429`) runs the same function against the base type catalog. In-process route: only the checker. Execution checks against `structural_types()`, which may substitute target-layout or scalar-geometry records. | No. It reads only the operation and the type catalog. | Move to preparation: run once per operation against the selected catalog, then delete the per-operation call. |
| `instructions/structural.rs:1247` `content_transformations::validate_memberships` | A `TransformContent` result's memberships follow from its derivations and from the memberships the source value actually carries. | No. | Yes: the inherited memberships come from the staged runtime value. | Keep the comparison with the value's memberships. The derivation-only part could move to preparation if a later change splits the function. |
| `memory/custody.rs:48` `validate_complete_claim_frontier` | A whole-value claim is complete over the live linear claims and stored cases under one place. | No. | Yes: live claims, the live affine frontier and stored cases. | Keep. Its `ClaimTypes::new` (line 27) moves to preparation (see below). |
| `memory/custody.rs:554` `ClaimTypes::new` + `validate_owned_claim_frontier` | `ClaimTypes::new` sorts and classifies every structural type on every owned argument. The validator checks that the live claims under an owned argument form an exact frontier. | `ClaimTypes::new` on the base catalog: yes, `validate_intrinsic_claim_roots` (`psi/semantics/src/structural/owned_structure.rs:141`) builds it during `validate_storage_foundation`, after `StorageTypeGraph::validate_acyclic`. | `ClaimTypes::new`: no. The frontier: yes, live claims. | Build `ClaimTypes` once per admitted program over `structural_types()` and refuse there. Keep the frontier check. The same rebuild sits in `records_and_arrays.rs:445` (`stage_constructor_claims`). |
| `memory/references.rs:1047` `validate_access_view_content` | A view lifecycle plan's content agrees with the module's declarations. | Bytes route: yes, `validate_access_lifecycle_plans` (`psi/semantics/src/access/access_lifecycles.rs:242`, called from `foundation_validation.rs:11758`) runs it on every plan. `admit_target_layouts` (`psi/execution-inputs/target-layouts/admission.rs:155`) only filters and clones those plans. In-process route: only the checker. | No. | Move to preparation once per plan, then delete. The neighbouring `access_result_contract` call is static in the same way. |

### `VerifiedOperationContractInvalid`

| Site | What it checks | Verdict |
| --- | --- | --- |
| `instructions/calls.rs:3096`, `:3122` | `CallStructural` and `CallStructuralWithScalarArguments` have a structural result. | Bytes route: yes, `validate_call_structural` (`foundation_validation.rs:576`) refuses a structural call without one. In-process route: only the checker. Static, so it moves to the preparation pass for family 2 below, then both sites go. |
| `instructions/structural.rs:985` | The vacant-local re-check above. | Move to preparation. |
| `memory/custody.rs:29`, `:64` | `ClaimTypes::new` failure, and the complete-claim frontier. | Prepared `ClaimTypes`; keep the frontier check. |
| `memory/records_and_arrays.rs:449` | `ClaimTypes::new` failure. | Prepared `ClaimTypes`. |

The `VerifiedOperationContractInvalid` variant itself is still used by the
frontier check; deleting it comes after the other six sites move.

### `malformed()` refusal families

Counts come from classifying the five lines before each call by the first
keyword family they match. They show where the work is, not a verdict per
site. Each later change confirms the verdict for the sites it moves.

| Family (sites) | What the refusals check | Already guaranteed? | Run-time? | Verdict |
| --- | --- | --- | --- | --- |
| 1. Operation kind (37) | `operation_kind!` and `let OperationKind::X = &operation.kind else` refuse when a helper receives a different operation. | Dispatch already chose the handler by the same tag; `dispatched_operation!` (`execution.rs:2791`) treats a mismatch as `unreachable!`. | No. | Delete where only the dispatcher calls the helper. Where several kinds share a helper, keep the match but make the other arm unreachable. |
| 2. Result and operand kind (72) | An operation's result is structural or scalar as its kind requires. A scalar operand has the type the operation names. | Partly: per-kind validators in `validate_operation_foundation` (for example `validate_establish_structural_local`) check some kinds on the bytes route. The checker covers in-process programs. No general result-kind pass exists. | No. | Add one result-kind and operand-type pass to preparation, then make execution accessors infallible. |
| 3. Declaration lookup (298) | A machine, block, boundary machine, structural type, descriptor, domain, identity or lifecycle plan named by an operation exists. | Partly: the foundation validators resolve many references (callees, attachment types, descriptors, boundary declarations, services). I found no decode check that every terminator's target block resolves, nor one general pass over every named declaration. | No. | Prepare dense tables for the referenced IDs once, refusing a missing one at admission. Execution then indexes them. |
| 4. Frame value and place lookup (96) | A value or place an operation reads was defined earlier in the frame; the call stack has the expected depth. | No. Only the checker's dataflow proves definition before use; decoding has no such pass. | Mostly no. Definition before use is static. The call-stack depth checks belong to execution state. | Bytes route: keep until execution requires a trust witness, or add a definition-before-use pass to preparation. Then delete. |
| 5. Static numeric conversion (part of 6) | An operation index, argument index or dispatch slot fits `u32` or `usize`. | Mostly: decoders bound counts by the remaining input, but no single function states the bound. | No. | Move to preparation with the result-kind pass, or fold into the table indexes of family 3. |
| 6. Run-time bounds and arithmetic (368, includes 5) | Byte offsets, lengths, extents, element indexes and checked arithmetic on actual values. | No. | Yes. | Keep. The spec requires the interpreter to enforce actual bounds and operand safety. |
| 7. Live storage, claims and loans (140) | A result place is not already in use, claims and loans are live, restorations match, reservations exist. | No. | Yes. | Keep: storage liveness and access permissions. |
| 8. Host and provider input (625) | Values, storage, effects and results that the host or a provider supplies (`embedding/boundary_*.rs`, `memory/providers.rs`, `embedding/instance/storage_observation`). | No. | Yes. | Keep. Host adapters must enforce what they rely on. Declaration-dependent parts of a boundary's signature can move to preparation with family 3. |
| 9. Operand and stored type agreement (449) | Operand types agree with each other and with the result; a stored field cell has the scalar type the operation names; element counts match. | Not on the bytes route; the checker types every operation. | Mostly no. Stored cell type is static for checked programs. | Same as family 2: one typing pass in preparation, then delete. A few of these sites compare actual stored contents and stay; each change names them. |

Order of work: prepared `ClaimTypes` and the two re-checks the decoder
already runs come first, because they need no new admission logic. Families 1
and 3 follow, then the typing pass for families 2 and 9. Family 4 waits for
the trust witness or a definition-before-use pass.

## Embedding contract and probe limits

This crate exposes the public API of the accepted
[embedding contract](../../../wiki/spec/build/embedding.md), not only the raw
hook. `TerminalProgram::load` decodes canonical semantic bytes, `load_envelope`
reads the portable envelope, and `borrow` uses an existing canonical artifact
view. All three keep immutable shared code. `instantiate` starts an independent
instance without reading the bytes again. Its `TerminalInstantiation` request
carries scalar and structural inputs, an optional admitted installation and an
optional Terminal invocation account; there is no separate route for installed
programs. The account is chosen before entry storage is allocated and applies
to nested component instances too.

`TerminalStructuralInputs::contents` fills in the complete scalar and case
contents of the supplied arguments. Exact declaration paths keep nested
records, active case payloads, fixed arrays, booleans and primitive storage
widths. A missing, duplicate, inactive or extra leaf is refused before
execution. Fixed byte arrays, including initialized empty ones, use ordinary
compact storage; borrowed descriptors and opaque contents are never invented.
Erased fields have no runtime cell but still must be established explicitly.
Contents give no claims, qualifications or reference authority. The shared
[storage contents](memory/records_and_arrays.rs) walker and constructor
transaction own this operation, including sponsored reservations and cleanup
on failure. Complete initializers cannot overlap each other or a partial
input. Cases with and without payloads use this same route.

`TerminalInstance::bind_import` binds each import the program needs to a
checked adapter before `invoke` runs any guest operation. A missing, unknown,
provider-realized, duplicate or wrongly typed supply is rejected with no
effects. `close` releases the instance's runtime state once the activation
settles. `TerminalImportAdapter` is the contract a host object declares;
`TerminalScalarImport` and `TerminalUnitImport` wrap the common leaf signatures.

`TerminalProgram::defer_component` reserves an exact component route in the
supply plan before entry. It supplies no code and no host adapter, so calling
it early fails. `bind_component` can fill that reservation while the original
receiver waits at a checked host-readiness crossing. It uses the same sealed
import/export admission and fully prepared component closure as installation
before entry. Receiving code may stay private to one invocation: the selection
and the execution plan both keep its full code-owner identity, so equal bytes
from another private owner cannot reuse either, and native consumers need a
portable receiver identity. Work pauses and failed activations cannot authorize
the change, and a bound slot cannot be rebound. (Its test,
`terminal_effect_readiness`, no longer exists.) Acquiring build dependencies
and preparing exact backing storage are application-coordinator work, not
interpreter work.

An independent component may borrow an original closed Inert host atom under
its exact shared or mutable parameter contract. The pending call keeps the
original prepared tuple and caller activation through the child's run,
including host computation pauses, until normal return is acknowledged. The
parent stays paused with the original home and loan lineage, and nested
components keep that authenticated link through the same private carrier.
Owned inputs follow their normal transfer rules. An opaque identity cannot
supply materialized storage, reference companions, write-only access or a
borrowed result; those need their own carriers and are refused. A bare
borrowed entry annotation supplies no host storage either. See
[component arguments](instructions/calls.rs).

Lending an opaque object gives the guest no field cells. Local scalar and
atomic stores and subtree replacement under those borrowed homes are refused
before any detached payload is made; host effects still act on the original
object. Owned materialized inputs use complete checked contents in the child's
declaration space. Borrowed materialized storage still needs its own transfer
carrier.

`poll_host_computation` returns `HostComputationPending` when a synchronous
host operation needs executor work before it can return. The invocation, its
prepared arguments and borrowed storage stay live; polling neither repeats
effects nor charges the guest operation again. This is an executor pause, like
a work slice, not a source-language suspension, and it cannot authorize
component installation. External provider readiness goes through
`poll_effect_readiness` and needs a checked suspension crossing. The Build
application, not either interpreter hook, schedules product queries.

The host reads storage through `TerminalInstance::observe_storage`. It selects
a root and a canonical declaration path. On normal return the root may be the
result or an original borrowed argument. While a synchronous host computation
is pending, the host can observe an original borrowed argument to answer that
call. External provider waits and interrupted invocations cannot supply that
result. Owned inputs expire with their entry frame.

The Build driver uses the returned plan to select its product. A declaration
query reads the live plan at its host call. Reading an issued value inside a
sum also checks the active case before it reads the field.

Byte readback holds a Rust borrow of the instance instead of copying the
payload. `TerminalByteRead::len` lets the host check the size and reserve live
storage and result custody before allocating its output; `copy_to` fills that
caller-owned buffer. A scalar-backed byte window stays scalar-backed while it
is read. Publication needs this, but it does not complete Terminal storage
accounting.

- Owned byte storage reserves its full logical length before
  allocation. Aliases share its lease, so even a small window keeps the whole
  storage reserved. Copy-on-write reserves the overlapping replacement before
  copying. Boundary byte buffers use the same storage type: capture and growth
  reserve before copying, and whole-owner writeback moves the reservation with
  the bytes. Retirement releases live bytes, not consumed work.
- [Runtime rows](memory/primitives.rs) own one cell per live
  scalar or structural table entry, including staged call/edge bindings and
  cleanup copies. New entries and overlapping copies reserve first;
  replacements keep their cell, frame moves carry the lease, and table joins
  transfer reservations without charging again.
- Root activations and suspended call headers each reserve one
  cell before creation. The shared frame capture in
  [calls.rs](instructions/calls.rs) handles scalar, structural and
  cleanup calls; return restores the caller and releases its saved header. The
  root activation stays reserved through result readback until close. A
  refused capture keeps its outgoing cleanup continuation, including byte
  storage and returned claims, in the failed activation; it allocates no unpaid
  saved frame and does not advance the caller's instruction position.
- Packed scalar arrays keep one cell per element with their
  storage. Guest snapshots, including plain projection copies, reserve
  independent copies; inspection clones share read-only storage; writes
  reserve any overlapping replacement. Returned payloads stay charged after
  close until the last inspection is dropped.
- Frame-local array, byte, element-view,
  primitive-loan and aggregate-backing descriptor tables use the same paid row
  owner through argument staging, saved frames, joins and return installation.
  Even an empty array needs its descriptor cell. A refused return keeps its
  output, caller continuation and loan roster beside any partly installed rows
  until the failed activation is discarded.
- Shared scalar fields, primitive backing cells, primitive
  entry lookups, references, byte fields, byte/view descriptors, backing, cases
  and host atoms use the same owner in execution and in constructor/transplant
  staging. Constructor and subtree snapshots reserve before copying and
  transfer the reservations at publication; pruning retires the selected rows.
  Updating a primitive keeps its cell; first initialization reserves before
  invalidating stored provenance. Publication checks every paid table before
  retiring source storage, and moving the prepared rows asks for no further
  budget.
- Activation frame ledgers, live claims, grants, backing
  owners, split associations and claim descriptors are charged. Split/merge
  pays for its result case and binding, and stages every possible result
  descriptor, before consuming source claims, then drops the unused case's
  rows. Constructor transport carries descriptor reservations. Ledger growth is
  paid before the foundation's authority operation; release/merge retires the
  removed cells.

Source-produced controls check exact and one-cell-short budgets across
work-budget pauses, for borrowed and loaded artifacts, local and borrowed-entry
mutation, and readback through the original entry reference. Faults are kept
without repeating effects; refusal is sticky and teardown releases storage.
Provider ledgers and descriptors and cumulative result custody are not yet
accounted, so these partial snapshots must not become production Build usage
receipts.

A failed execution keeps its exact host fault and unfinished custody. Calling
it again returns that fault without charging or running the site again, even if
capacity later becomes available. Only an explicit pause can resume. Failure
gives no normal-return readback and does not make a busy instance closable.

The checked-provider candidates a program keeps say what is available, not
what the receiving host selected. `TerminalProgram::admit_runtime_supplies`
combines the host's explicit normal host-import choices with an existing sealed
checked installation. Preflight, instance construction and core dispatch all
use the returned selection, which is bound to that program. Duplicate or
competing choices, unknown slots, the wrong program identity and
external-completion imports are rejected before any effect; selection grants
no adapter or ProcessExit authority. `TerminalInstance::import` describes the
instance's actual demanded imports, including explicitly selected slots with
retained checked alternatives. `TerminalProgram::imports` remains the default
catalog without those explicit choices. Unselected candidate-bearing demands
still require a checked installation rather than falling back to a host import.

Still missing from the full contract: an instance exposes only the module's
entry invocation, with no exported secondary operations; adapters beyond the
scalar and unit leaves are hand-written `TerminalImportAdapter`s; and public
entry descriptions from product compilation (component publication) depend on
work outside this crate. The raw `TerminalEffectHandler` is the low-level hook
the embedding layer is built on, not an admitted binding API.

Checked assembly follows the [instruction interpretation contract](../../../wiki/spec/language/assembly.md#interpreted-execution):
reuse ordinary value, place and transition operations, keep scratch state local
to the block, and keep explicit memory and environment effects across calls and
work stops. The selected target environment must supply every operation the
program needs before execution. A generic effect hook or a successful Unit
response does not by itself define instruction semantics or match the
environment. End-to-end instruction coverage is tracked as
**INTERPRETED-ASSEMBLY**; not every catalog entry runs here yet.

No test currently covers these embedding guarantees end to end; the former
`terminal_entry_writeback` test no longer exists.

## Cyclic execution

The interpreter runs verified natural-ranked scalar, plain-owned-input,
immutable-view, receiver and primitive-local graphs under the [control contract](../../../wiki/spec/terminal-psi/control_flow.md).
The legacy one-machine structural Unit countdown has its own verifier entry:
it rewrites `0 < remaining` as the unsigned premise `1 <= remaining` and checks
exact subtraction before creating resumable state. Running here grants no
native, fixed-work, provider-installation or mixed-work authority.

[terminators.rs](instructions/terminators.rs) captures every selected
operand before cleanup and before installing all successor bindings at once.
Affine inputs move once; Unrestricted descriptors stay reusable. Rebinding changes the descriptor place,
not its referent or field backing. Missing or duplicate owners and overlapping transfer
and discard are rejected before mutation. Work suspension keeps the uncommitted
edge.
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

Once exact signatures and arguments are prepared, Unit, scalar and structural
results share one call-frame transition; a structural result adds no input-path
or unqualified-shape restriction. Returned reference storage is captured after
owned arguments are staged and checked at completion; the returned carrier,
byte contents and memberships reach the saved caller only on a successful
return.

The interpreter charges a call before entering the callee's frame. If the
budget runs out inside the callee, resuming neither pays for nor repeats that
call. A callee crash keeps its original site and edge charge; no caller crash
is invented. Ordinary recursive call graphs are refused until their tail or
ranking evidence is admitted; cyclic blocks use the separate verified route
above.
Installed checked Unit providers pass the boundary's ordered scalar arguments
through the ordinary call binder, beside separately prepared structural
arguments; a Unit result does not mean an empty parameter list. Installation
replays the exact signatures, and work suspension keeps the same selected
frame. Fixed-work composition keeps normal-return bounds apart from crash
bounds: only a normal return adds the caller's remaining work.

Checked-provider structural returns carry the returning edge's actual claims
through the ordinary path-based result binder, including linear children of
affine records, instead of creating new claims from result declarations. Only
the exact admitted crossing can add its authorized root qualifications; calling
an adapter directly keeps its declared result contract.

## Primitive array results

[records_and_arrays.rs](memory/records_and_arrays.rs) builds and
returns complete primitive arrays. Ordinary and installed checked-provider
structural calls return the exact unrestricted payload into a fresh result
place in the saved caller, beside the caller's earlier arrays. Return preflight
runs before work is charged and the frame restored, so running out of budget
cannot publish a partial result or repeat a finished call. Nested and empty
dimensions, Boolean values and IEEE payload bits are kept.
Ordinary Unit, scalar, and structural calls copy whole owned unrestricted arrays
into the exact callee parameter places. Repeated arguments preserve the caller's
payload, and parameter returns select the actual input contents. State transfers
capture all incoming array payloads before committing successor bindings, so
parallel transfers cannot overwrite a later operand. Installed providers use
the same saved-frame return transport after exact conformance and installation
replay. Host result preflight rejects opaque host identities as array payloads
before calling the handler. Borrowed or projected array results are not
supported; borrowed byte-array storage is a separate route. Omega's result-home
and target transport code owns native realization.

## Primitive local storage

[primitives.rs](memory/primitives.rs) creates fresh referents on
each activation or loop reentry. Borrowed calls share the storage; reads copy
the current scalar, so earlier snapshots survive later writes. Suspension keeps
the storage, and leaving the block reclaims it without affine cleanup. Local identities
cannot collide with declared places or supplied host identities. This support
does not provide native allocation or permit owned local escape; see
[structural access](../../../wiki/spec/terminal-psi/structural_access.md).

## References stored in records

[records_and_arrays.rs](memory/records_and_arrays.rs) stages descriptor relocation together with ordinary
record payload construction. Nested packing changes the carrier's owner and
field path, never the original primitive backing. [references.rs](memory/references.rs)
resolves typed field paths ending in `Referent`; whole-owner cleanup removes
descriptor subtrees without disposing the borrowed storage. The verifier checks
loan ancestry and cleanup order on its own. Return preflight compares every
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

Ordinary primitive arguments keep their original storage. A local array
element stays in its suspended owner's frame; callee parameters, successor
bindings and stored references carry a loan to it, not a copied scalar. Shared representation views and
qualifiers retain their ordinary rules. The stricter host admission described
below applies only when an external adapter receives a payload buffer.

`TerminalEffectHandler::storage_arguments` asks to observe selected
structural arguments, and `handle_effect_with_buffers` receives them as
storage buffers beside byte buffers. `TerminalBoundaryStorageBuffer` separates
a primitive endpoint from a complete active aggregate; both identify the
structural argument index and admitted access without exposing interpreter
storage handles. Mutable replacements stage exactly typed contents; shared
borrows cannot replace them. The core captures original local, field and array
element homes before dispatch, including suspended callers' backing. It
validates every staged payload and original restriction before publishing
results or input writes. It never resolves a scalar source again after host
code. A per-buffer token prevents substitution from another invocation.

Aggregate contents name cases, not native discriminator integers. Exclusive
replacement can change the active case only with complete, valid new contents.
Completion removes stale payload storage and restores the original referent
identity, including byte-array storage. Common mixed fields and payload
fields keep separate namespaces. Opaque children must retain their original
storage: host bytes cannot manufacture permissions or pointer descriptors.
Changing opaque children or inline byte fields remains an unsupported semantic
codec, not a language restriction on case replacement. Native adapters own
layout, tag encoding and accessed-prefix coverage. Padding, descriptor bytes,
and unread inactive payload bytes supply no semantic contents.

This protocol accepts only unqualified, initialized shared or mutable storage
loans. Write-only input and qualified owner storage are rejected before
dispatch; a plain formal does not erase the caller's stronger restrictions.
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
indexed byte stores retain that backing across calls and work suspension.
Indexed stores validate the current extent before mutation and detach shared
immutable backing; they cannot change source literals or sibling fields.
An opaque incoming structural identity does not supply initialized field bytes:
measuring or indexing an unprovided field fails rather than assuming empty
or zero-initialized storage. This execution support does not establish native
byte-field realization.

For now, an initialized bounded byte field can supply an unqualified mutable
boundary parameter. The boundary resolver keeps the original referent, full
record/array path and capacity; bounded storage does not become generally
interchangeable with views. Ordinary Unit helpers can also see the field as a
mutable byte view on the verifier's admitted field-only paths, holding the
same field loan rather than copying or owning bytes. Missing storage is still
rejected.
`TerminalEffectHandler::handle_effect_with_byte_buffers` receives pre-call
bytes and staged writable storage. An ordinary mutable view uses
`TerminalBoundaryByteBuffer::bytes_mut`: its slice cannot resize storage or
expose unused capacity. `replace` belongs only to a whole bounded owner and
rejects ordinary views, including equally sized replacements. All captured
loans, buffer extents and the declared response validate before writeback or
completion commits. The effect trace records pre-call bytes;
`structural_byte_sequence_field` observes committed writes.
Handlers without this callback reject mutable buffers before any effect.
Missing storage is an error, even for an opaque entry object.

Installed checked providers get a frame-owned mutable field binding, not a
copy of the bytes. Whole mutable-view arguments can pass that binding through
ordinary helpers or reborrow it at another boundary. Each external call sees
the current storage and commits to the original field; returning from the
provider does not restore its entry snapshot. Calls and work suspension keep
the binding's exact referent, record/array path and capacity. An equal opaque
identity without the binding gives no mutable loan. Byte reads and
subslice operations still require immutable bindings. Native byte-field forwarding and
`read_line` realization remain separate dependencies.

Fixed-view length and indexed writes at a machine or block parameter use the
existing mutable field loan. Length is the current live extent, never the
capacity. A write checks the typed index, byte and saved length against that
extent before changing one byte; it cannot resize storage or change the
owner's length. State transfers capture the original referent binding before
work and commit, reject aliased mutable arguments, and keep writes through
calls and suspension. Shared immutable storage is copied on mutation, keeping
sibling values and the destination's untouched suffix. External boundary
replacement has its own contract.

The settled [bounded-input contract](../../../wiki/spec/resources/bounded_input.md)
does not replace the owner: it writes within a supplied slice and returns a
count plus a line/EOF/full outcome, with ZII Invalid meaning no result.
Line-input callers, checked providers and interpreter/native consumers must
move to it together. Existing replacement and forwarding tests pin today's
binding behavior, not the new line input. Ordinary whole-field stores stay
supported. An encoding qualification needs checked library validation of the
written prefix; buffer writeback grants none.

## Fixed byte arrays

`TerminalExecution::start_artifact`, the only start entry since `8e2dc8b8ade91`,
supplies initialized fixed-array contents through
`TerminalStructuralInputs.byte_arrays`, by structural argument and relative
field path. Each value must match a real unqualified `FixedArray(u8, N)` and
hold exactly `N` bytes. Duplicate referents, mistyped paths, and fabricated storage
reject; an opaque root does not imply initialization. `structural_byte_array`
observes the original backing by referent identity and path.

Ordinary helpers and installed checked providers may borrow the whole array,
or an admitted record-field array, as a mutable byte view. Argument preparation
shares the initialized-array binding instead of staging a field replacement.
Calls and block transfers keep that binding; length stays `N`, and indexed
writes keep all other elements through work suspension. External calls stage the selected initialized range, including
array storage in suspended caller frames. `bytes_mut` writes that fixed extent;
successful completion commits it to the captured array and offset, never a
newly resolved equal-looking place. Short reads preserve the unread suffix.
A fieldless array binding is distinct from the bounded-field loan, so the
external `replace` callback cannot resize an array. This input establishment
route does not itself implement source-owned construction or native storage.

## Boundary responses

[boundary_effects.rs](embedding/boundary_effects.rs) tells Unit, scalar and opaque
structural values apart from an explicit `Crash(Trap | Abort)` response. A
host crash is accepted only if the program published a crash route with the
same cause, judged against the invocation's already evaluated scalar inputs; a
true guard also permits normal completion. Closed IEEE comparisons use the
exact operand bits, including unordered NaN, so the host's report cannot
invent a permitted crash route. Mathematical-integer guards reuse proof
admission's exact closed meaning, including intermediates wider than the
fixed-width input. Input traversal and total arithmetic growth and work are
bounded before allocation; hitting those private limits rejects the crash
report with a different error from a false or unsupported guard. This changes
neither mathematical integer semantics nor guard lowering. The verifier's
outcome validator reuses the existing positional substitution and closed
scalar arithmetic.

A Trapping primitive whose exact value does not exist crashes with cause `Trap`
at `TerminalCrashSite::Operation` (machine, block, operation) before any
result is published; its frontier is the machine-local live claim set, as for
a boundary crash, and no wrapped or saturated value is substituted.

A valid boundary crash records a tagged machine/block/operation/boundary
site, not an invented edge. The attempted call stays in the effects list; those
records describe the call's inputs and the receipts a normal completion needs,
not proof that it completed. On a crash, no result, receipt, owned-input
disposal, staged byte-buffer writeback, later operation or cleanup commits.
Host effects already performed cannot be undone. Resuming again returns the
same crash without charging or calling again. An ordinary operand crash keeps
its original edge and stops the enclosing boundary call. Installed-provider
refinement and native support are separate work. No `TerminalTraceV1`
observer exists yet (**OBSERVATION-PROFILE** in `TASKS.md`).

A structural response today has an exact type, qualifications, an opaque
identity and an empty path. Linear results, result claims, projected
qualifications and inspecting a sum's discriminator or payload are not
supported. Preflight rejects unsupported result requirements before the host
effect; the response is validated before result and claim custody commit.

Structural entry records and fixed arrays with bounded integer fields need
explicit contents for every bounded field, even unread ones. Startup checks
each integer's exact type and range before establishing custody; missing,
duplicate or conflicting aliased fields are rejected. Unbounded integer fields
keep their deferred initialization check. Arrays are visited incrementally, so
missing contents are found without expanding their shape. Opaque entry inputs
and host results that contain bounded integer fields at any depth are still
rejected, as are bounded sum or mixed entry shapes without a selected case
discriminator, because their type identities give no values to check the
restrictions against. Result preflight rejects before the handler runs. This
does not forbid valid selected scalar-case construction or supported exact
internal transfers.

`EstablishScalarCase` builds the declared selected payload from existing
typed scalar values, including independently proved restricted integers. Like
`EstablishStructuralCase`, it publishes one placed home holding the
discriminator and scalar payload cells. This storage survives internal-call
suspension; case inspection stages the selected scalar parameters before the
whole result is disposed of. The public scalar-case host/result payload is an
import/export snapshot, not a second frame-local storage form. Affine case
results get ordinary whole-root cleanup on returns and edges. Opaque host
values still cannot supply case observations. Ordinary calls pass complete
cases through ordinary placed storage: owned affine cases move once,
unrestricted owned arguments copy their actual subtree, and borrowing keeps
the caller's original home. Lending a whole case mutably can change the
selected case under the same rules as any structural value. Scalar-case host
results are validated before they are stored.

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

Validating a host response does not check allocation or freshness; the host
must return a legitimate owned value. A rejection restores interpreter
bookkeeping, not effects the host already performed. A completion followed by
a budget pause must not repeat the effect. None of this extends native
provider support.

Disposing of leftover owned values validates and charges the owning edge
before mutation. On a jump, scalar arguments are materialized before disposal
and successor binding. When the budget runs out, the leftover ownership stays
live. The exact producer/result root and claim map survive suspension; no
partly moved ancestor becomes whole again.

Structural storage copies and constructor staging use
[`records_and_arrays`](memory/records_and_arrays.rs) ranges bounded
by carrier identity. Keys sort by carrier identity before path and field, so a
copy does not scan unrelated old carriers; subtree selection still checks the
full path. Each transaction stages its shared storage families before pruning
either region, so overlapping copies and moves keep the original snapshot. This
grants no access to borrowed cells. At settled block boundaries, scalar-only
roots with no executable handles may be reclaimed. Current and suspended
frames, captured reference storage and every non-scalar storage family keep
their roots alive. Loans, views, byte bindings and nominal cleanup
transactions postpone collection.

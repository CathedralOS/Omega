# Abstract operations to target operations

This stage turns abstract operations into target operations, places call
values, and publishes the execution plan instruction selection reads. Start at
[lib.rs](src/lib.rs). The target representation holds the resulting control
flow, values, storage, calls and boundary data; this stage assigns no
registers or frame offsets.

## The stage's output is one execution plan

`lower_optimized_to_target_operations` ends by calling
[`prepare_execution_plan`](native-preparation/src/legalization.rs). That joins
the target operations to what this stage already holds: the optimized
program's accepted obligation and ownership-frontier facts, the admitted
boundary origins and interrupt installations, and the integer types the
sealed module retains. Preparation closes every scalar type it writes into
the plan against those module types, including the leaf scalars of
structural declarations it reads, so the plan carries only closed types. The
result is a sealed
[`PreparedExecutionPlan`](../../representations/target-operations/legalized-operations/src/lib.rs):

| The plan carries | Where |
| --- | --- |
| Concrete scalar types, with sign and width | Value definitions, parameters, block parameters and typed operations |
| Calling plans and argument and result placements | Function and call records |
| The identity of each accepted proof | The operation, edge or terminator it licenses |
| Ownership-frontier facts | Each function |
| Interrupt installations that survived lowering | A plan-level list |
| Each function's target calling contract (scalar, mixed or parameter ABI) | A plan-level table sorted by machine, `function_abis`, which image emission publishes with the object |
| Who performs each hosted byte write (provider or built-in) | The `HostedWriteByteI32` instruction |
| Native geometry of borrowed boundary inputs | A plan-level list, `borrowed_storage_layouts`, which the host bridge reads |

Instruction selection reads a scalar's closed type from the value, parameter
or operation it types. Where it checks a structural declaration's leaf
scalar, it compares the declaration with that closed type or with a fixed
type; it never asks which integer types exist.

A fact, type or installation the plan needs and cannot find refuses the
program here, as `LoweringError::Preparation`. Instruction selection then
needs the plan, the installations the driver binds to it, and the register
environment; `ValidatedOptimizedTargetOperations::execution_plan` hands the
plan over. The target operations remain this stage's working form. Only
validators read them, through `working_program_for_validation`: the validator
below and image emission's replay validators, which check the emitted object
against what this stage lowered. Those validators see this stage's output only
when a validator request retained it; the native driver otherwise drops it
after instruction selection and hands emission the few facts it needs
(`EmissionFacts`). `mbx xtask check-boundary` refuses any other caller outside
this stage and tests.

[`verify_execution_plan`](native-preparation/src/legalization.rs) is the
stage's validator. It derives the plan's functions a second way, from the
abstract program and the optimization unit, and requires the published plan
to equal that derivation, to match its sources' header, to state only closed
types the module retains, and to still hash to its seal. It runs on `omega --validator
TargetOperationCorrespondence` and in tests, never in an ordinary compile.

[Omega 00](../00_terminal-psi-to-abstract-operations/src/native_execution_scope.rs)
admits providers, callbacks and authority. This stage checks its lowering of
those admitted settlements; it neither selects providers nor authorizes
execution.

## Functions have one route

Every function goes through [control-flow lowering](src/control_flow.rs). It
requires explicit source blocks and keeps definitions, calls, branches and
returns in `TargetFunction.graph`. Unit, scalar and aggregate returns are graph
terminators, not different function representations. Missing block structure
rejects. There is no flat-input adapter, expression-tree function planner,
special-form fallback, or test-only entry point to one.

Each computed scalar has one result home; uses refer to it instead of copying
its producer or repeating a call. Constants and operation-local expressions
keep their source identities and arithmetic obligations; they are not a second
whole-function representation. Block parameters belong to their destination
block, and edges keep complete ordered transfers.

[Function contracts](src/function_contract.rs) keep the semantic structural
parameters, place declarations, entry claims, service ceiling and result beside
each function's target ABI. Preparation reads that contract without fetching
the earlier body; this stage's validator checks it against the source.

Each ordinary target instruction also records its input operation, scalar
definition, work charge, effect order and custody events (who owns which
storage). These are attached once to the settled block, after arrival
inference; revisiting a block does not copy them again. Compiler-inserted
cleanup calls record their cleanup owner instead, with no input operation.

Control exits keep their work, effect and ownership annotations. Each successor
also keeps its edge charge and bound structural selector operands from before
the edge commits transfers and cleanup, so selection never reopens an earlier
body for them.

Signature preparation, dominance, operation lowering, terminators and edge
bindings each have their own code in the common graph. An unsupported
operation, qualification transport, cleanup or ABI relationship rejects at the
check that owns it. Supporting it means extending the ordinary operation and
transfer contract, not restoring a whole-body recognizer. Native scalar kernels
may cover fewer operations than target operations.

Removed whole-function-family tests are no compatibility promise. Rebuild
useful arithmetic, cleanup, crash and reference cases on the common graph as
support arrives; do not revive retired target forms to keep old test counts.

## Independent checking

The stage checks inputs while lowering:
[coordination](src/coordination.rs) binds settlement bindings, the
installation join, the selected call index and each function's ABIs as it
produces the program, and rejects a raw fused-multiply-add plan. The same
compile does not recheck the output; output breaking this stage's contract is
a bug here, for focused tests and explicit validator requests to find.

The [independent audit](native-preparation/src/legalization_target.rs)
in this stage's validator matches every graph to its source and
optimized operations. Direct-native tests run it on the saved artifact, outside
graph construction; a default compile does not. It checks source order, value
identity, exact ABI, and proof, call and cleanup obligations. Root and roster
custody alone never proves a body correct.

Natural-ranked and unranked cycles share the graph and native route, each with
its own verification obligations. The older unsigned-countdown native custody
is unsupported and rejects at admission; it cannot be dropped or reinterpreted
as ordinary proof. No dedicated countdown target or
machine-code carrier remains. This is an implementation limit, not a change to
termination or resource semantics.

## References, calls, and storage

Every ordinary direct call is one `TargetUnitOperation::Call` record, for any
result (Unit, scalar, structural) and any admitted argument mix. Its `TargetCallResult` names the exact scalar
home or structural result, declaration, storage and returned custody. A bare
reference keeps its referent, with no invented result home; an aggregate keeps
its exact storage. Only Unit has no result. The
independent audit rebuilds the result from custody before the call; production
keeps the resulting origins for later storage joins.
One [direct-call lowering](src/control_flow_calls.rs) produces every such row:
it resolves the callee contract and each operand's storage, then commits moves
and publishes the result. The result kind does not decide argument passing.
Published scalar operations also keep their standalone ABI contract. The audit
replays these rows through one `direct_call` replay with a tail per result kind.
A runtime selector keeps its scalar source, stride, fixed extent if any, and
exact obligation key through geometry and argument construction. Calls pass
those rows forward; the next stage looks the keys up in the admitted proof
catalog instead of rebuilding the selector from an earlier body.
Source-specific restrictions are still checked separately. Foreign calls
keep their own carriers; not every call uses this record yet.

Operation dispatch has no up-front borrow check for the whole operation. An
ordinary call selects the existing descriptor metadata under each owned
operand's exact carrier prefix, keeps it while lowering addresses, then commits
its relocation. It neither rebuilds a list of the type's leaves nor
rechecks pairwise argument aliasing and suspension. This keeps case namespaces
apart without inventing metadata for an inactive payload; source legality is
Psi's job. On entry, `Case`/`Field` guards name the possible
descriptor slots. Construction supplies only the selected payload's
descriptors; cleanup disposes only metadata present. The guards survive branch
joins and describe conditional storage, not permission to use all alternatives
at once. Moving a descriptor needs its referent's identity, not a transitive
catalog of what it points to; projecting into the pointee still needs
declarations and concrete layout.
At a join, descriptor metadata follows the carrier's SSA storage definition.
A branch-local observation does not force another arm to recreate its home,
but surviving carriers must agree on custody across the edges taken.
Formation, release, edges and returns still have local native custody
restrictions. Removing them means replacing the storage relationship, not
skipping the independent audit.

[Structural signatures](src/function_signature.rs) derive placement
from structural declarations and referent layout. Every borrowed access uses
`BorrowedReference`; only owned values receive shape-selected value placement.
Passing a pointer never passes a copy of its referent. See the
[reference contract](../../../../../wiki/spec/terminal-psi/structural_access.md).
Each call argument is checked against the source place, path and access; the
callee declaration's access and type; the rebuilt shape; the callee plan row's
placement; and, for caller-parameter roots with field-only paths, the root type
and projected byte offset. The embedded call plan proves nothing alone: it must
equal the plan derived from the callee's declared signature, and each scalar
argument must bind its source value, ordinal and the verified row's placement.
Each structural argument's source must name the referent's own caller-side
home: the incoming parameter placement, a non-entry block parameter, or the
operation that established it. A staged destination, a sibling's home or a
copied referent cannot replace the original storage.
A function's standalone entry points (its graph's scalar parameter rows and
the published `scalar_abi`/`mixed_structural_scalar_abi` for callers outside
the plan) are checked against the same derived signature: each scalar and
result row must bind the declared value, type and plan placement, and a
published ABI is accepted only for the family its producer derives. A valid
plan on an ineligible function, or both ABI forms together, is a substituted
entry point.

For a mutable fixed-byte-array view, the projected storage type is the array
and the callee receives a view descriptor. Validation rebuilds the exact byte
element, array extent and unit stride with the original root and offset;
matching descriptor types or a producer-supplied length is not enough.

Graph operations keep the original invocation places, projections and widths.
Byte views keep their backing pointer, length, checked slice derivation and
bounds obligations. Mutable stores address the original referent. Calls keep
the selected callee, ordered arguments, result home, exact ABI and contract.
Parallel transfers must handle duplicate sources and cycles before overwriting
destinations. Allocation owns the actual snapshots and frame locations.

A shared byte-view argument may read storage of a bounded owner without taking
over its capacity or predicates. [Byte arguments](src/control_flow_calls.rs)
resolve the exact entry placement, state arrival or operation home and keep its
backing identity. Whole owners and inline owner fields both store the live
length word followed by the bytes; the descriptor built for the call holds the
backing address and current length, never the capacity as length. The verified
source call stays responsible for qualifications: an unqualified read-only view
does not strip the owner's qualifications or copy its predicates to the formal
parameter. Later stages rebuild geometry, source identity, dominance and call
placement themselves. Borrowing an established home does not require
concatenation.

A case observation finds its root through the live storage inventory and the
entry or state declaration. A descriptor carried into a state can still be
observed; a declaration that is not available cannot revive it. Tag layout and
runtime selectors stay separate from that lookup, so observing a case through a
slice does not confuse its descriptor with the element addressed.

A shared view field stores a two-word descriptor (backing pointer, then
extent) in its record. `StructuralByteViewFieldStore` overwrites it with an
established literal's pointer and length. A literal is immutable image
storage: selection loads its address with `ImageAddress`, a PC-relative address
(`adrp`/`add` on AArch64, `lea [rip]` on x86-64) that object construction binds
to the literal's bytes in the image's read-only byte-literal pool, and only the
two-word descriptor sits in the frame that sets it. The stored view therefore
outlives every root, so lowering accepts the store in any activation and across
a `Referent` crossing, exactly where Terminal Psi accepts it. Reading a stored
view field takes its extent from the second word; passing it to a shared view
parameter lends its own descriptor, addressed from the root. A zero-filled
shared view field is the empty view.

Plain record construction uses the existing aggregate result home. Each field
keeps its declaration identity and its evaluated scalar operand or completed
owned child. The target and legalized checks rebuild the field list, scalar
carriers, child types, range obligations, shape and storage offsets. Selection
fills all the storage before publishing its address; nested copies use each
child's exact extent, and incoming owned values use their captured ABI storage.
A call borrowing a local record keeps the original home. Direct and
hidden-pointer aggregate results share calling-plan custody. Record block
arrivals, nested ones included, use the existing aggregate edge transport and
their own destination storage; there is no literal-only constructor or separate
record storage graph.

Case construction uses `EstablishCase` with the same scalar or structural field
operands as records. Lowering checks the selected case's exact field list and
reuses the record source and affine-consumption checks; nested completed
children keep their declared types and whole owned source places. The existing
recursive sum layout decides storage. Descriptor relocation puts the selected
case prefix on payload fields and none on a mixed shape's common fields;
copying descriptor bytes never copies what they point to. Static projection
computes pointer-slot geometry, case-qualified paths included, so calls need no
path grammar of their own.

A shared call can borrow a constructed plain record's existing home, including
one produced by a call or chosen at a block join. A shared field loan from a
joined record keeps the exact block home and declaration-derived offset. The borrow passes an address, not copied field values. An
affine owner stays affine while its shared loan is unrestricted; cleanup and
transfer obligations stay with the owner.
Reading an integer or Boolean field uses the common graph's explicit field-read
operation. The checks derive the field offset and scalar type from the
declaration; selection then reuses the primitive load kernel and independent
replay. Record locals and owned block arrivals are read the same way: the real
source place is kept, not relabeled as an incoming parameter. Availability and
field identity are checked apart from physical layout, and each load produces a
fresh scalar. Bounded integer fields use the same load with their exact signed
or unsigned carrier. Construction range obligations and the original bounded
field declaration stay in the input and checks; equal layout cannot erase the
range or allow an unproved store. Four-target
field-read controls test bounded carriers alongside wrong paths, fields, result
types, homes and physical offsets.
A record passed by value as an entry parameter keeps its owned-value ABI. When
a field read or borrowed call needs an address, selection gathers its register
pieces into one input home for the activation. Whole-record returns and nested
copies use that home, so entry registers cannot overwrite bytes a finished
mutable borrow changed. Inline stack inputs use their existing
incoming storage. An owned record passed indirectly keeps the copy the ABI
prepared, through its register- or stack-passed pointer; it is still an owned
value, not a borrow of the caller's record. Field reads, nested construction and
whole-parameter returns share that storage across calls. The complete call plan
selects input and result placement separately; a stack-passed value may return
through hidden result storage.
Whole owned record and primitive-array arguments share
[owned argument lowering](src/control_flow_calls.rs). It keeps the current
source storage and exact callee placement, including indirect copies;
pointer-slot and payload-copy offsets stay distinct.
Source-produced integer getter controls cover direct and call-produced locals,
tail completion and nested scalar arguments. Mutable local receiver storage is
still a separate dependency; shared record-field receiver projections keep their
original pointer and typed path.

Integer XOR is an ordinary scalar definition with legalized operands and a
selected bitwise instruction. Its x86-64 and AArch64 realizations each check the
opcode, operand registers and flag effects; AND cannot stand in for XOR even
with the same register constraints.

This stage does not lower dynamic dispatch. The named-implementation selector
route is legacy; the descriptor migration on [TASKS.md](../../../../../TASKS.md)
tracks replacing it with machine values and `some` packages. A selection never permits
substituting a direct call or inventing an implementation.

## Acceptance

The crate's Rust tests cover
[placed container geometry](src/structural_layout.rs).
The end-to-end `tests/omega/{pass,fail,run}` corpus was deleted on 2026-11-13;
programs Rust tests read live under `tests/fixtures/`. Cross-emission does not
show host execution.

Tests of borrowed storage, descriptors, boundaries and cleanup must check the
caller-visible result and include invalid provenance and ownership controls. A
target-only fixture does not prove native code works.
The [execution board](../../../../../TASKS_OPTIMIZER.md) tracks the remaining
native operation and independent replay work.

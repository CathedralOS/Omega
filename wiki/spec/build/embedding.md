# Embedding and interpreted components

Embedding executes ordinary Omega programs through an ordinary interpreter
library consuming [Terminal Psi](../terminal-psi/product.md). It is not another
source dialect, authority system, module system, or proof relation. This contract
specifies the destination; [implementation tasks](../../../TASKS.md#embedding-and-interpreted-components)
and the [interpreter implementation](../../../omega-rust/psi/semantics/terminal-interpreter/README.md)
identify what is actually connected. Library method names below describe roles,
not a frozen SDK spelling or new source keywords.

## Products and composition

A host depends on the interpreter and the shared interface packages. It may
separately depend on the source compiler. Loading Psi does not run `build.omg`,
fetch packages, compile source, or obtain filesystem or installation authority.
Source compilation uses the ordinary build request, root-selected backing,
lock agreement, and attenuated authority, including selected nested build files.
Build permissions do not become runtime bindings.

Source modules and packages are not automatically separate runtime artifacts.
Ordinary dependencies may be fused. An independently replaceable component needs
the checked closure and stable slots of [component publication](component_publication.md).
Its interpreted realization can be a separate Psi artifact. Building a selected
component uses its required compilation context; no whole-program rebuild or
incremental-compilation speedup is implied. Fused dependencies can enlarge the
required rebuild/replacement cohort.

Artifact acquisition is separate from execution. Bytes may arrive from supplied
memory, a filesystem, a network, or another authorized source. A virtual filesystem
is neither required for loading nor an implicit module resolver. Native and Psi
component realizations share requirement and lifecycle contracts, not necessarily
container formats, physical representations, or provider implementations. A native
plugin product still needs its selected ABI/container; a Psi artifact is not a DLL.

## Imports, exports, and Binding

`Binding<R>` is the compiler-known opaque affine core carrier for an established
boundary requirement binding, as defined by
[binding validity](component_publication.md#bindings-and-era-entry). It is a core
type name, not a keyword, network-service marker, proof of provider correctness,
or new domain modifier. The compile-time foreign locator is separately named
[`ForeignBinding`](foreign_bindings.md); locator data cannot establish a runtime
binding.

The host and guest use exact shared interface declarations. Builds select exported
entries and attached receiver establishment under [entry roots](entry_roots.md).
Neither `pub` nor `satisfies` alone exports a callable ABI. No `main`, `run`, or
`Run` name is privileged by embedding.

An instance's import roster identifies each exact slot/occurrence, closed
requirement, and full contract. Two fields of the same requirement type may bind
different host objects. Establish every demanded import before running guest
code; missing, duplicate, wrong-instance, incompatible, or unauthorized bindings
reject without an ambient fallback. Supplying an instance transfers the required
authority. Reusing a provider for another instance requires an authorized sharing
or establishment route, not copying an affine binding.

The library supplies typed import/export adapters and descriptions, rather than
requiring each embedder to implement a dispatch system. Generation derives from
checked interfaces and may be untrusted: admission rejoins exact declarations,
applications, contracts, representations, and selected adapters. Display-name
lookup resolves to that exact entry; a string, ordinal, or address alone supplies
neither identity nor call authority.

A guest boundary call dispatches through the installed adapter and its host
context. A compiled adapter can make an ordinary native host call; the interpreter
does not need to generate machine code per call. Runtime-discovered native
signatures need a supported checked ABI adapter, not an arbitrary function-pointer
cast. Host-to-guest calls satisfy the complete entry contract too. Well-shaped
scalars do not establish preconditions, domain membership, ownership, or grants.
Static proofs, admitted evidence, or checked entry operations establish those
obligations; arbitrary mathematical contracts are not automatically executable
validators. Native provider behavior remains an explicit trusted premise unless
independently checked.

## Ownership and lifetime

| Library role | Responsibility |
| --- | --- |
| Program | Immutable admitted Psi, selected public descriptions, and execution metadata; shareable across instances. |
| Runtime | Host/provider context, bindings, guest instances and storage, activations, and outstanding resource custody. |
| Invocation | A control handle for one unfinished call and its continuation, with the access needed to resume it. |

Instances have independently owned guest state and retain their program owner.
Runtime-scoped instance and entry handles cannot become valid for an unrelated
instance after storage reuse. A host context may own application state or borrow
it with an explicit lifetime; neither arrangement grants unrestricted access while
guest loans exclude it. A long-lived runtime can reclaim a closed instance without
destroying all other instances.

Crossings preserve ordinary value rules:

- Scalars are checked against their exact carriers. Records, sums, and arrays
  use checked representations, not reinterpretation of arbitrary host structs.
- Borrowed views retain the backing, access mode, and loan for their complete
  lifetime, including a permitted suspension. Owned transfer and compatible
  borrowing need not serialize or copy every value.
- Host resources retain exact owner, lifetime, and authority. A numeric identifier
  supplied by a script cannot fabricate them. Guest values retained by the host
  keep their necessary instance/component custody alive.
- Erasure does not license reconstructing proofs, grants, linear resources, or
  qualifiers from metadata or bytes.

## Execution and interference

Starting and resuming use one execution mechanism. A synchronous convenience call
drives that same mechanism. Outcomes distinguish:

| Outcome | Meaning |
| --- | --- |
| Returned | Normal result and contracted disposition; returned values may still retain loans or component pins. |
| Paused | Execution budget exhausted with a resumable activation and its obligations intact. |
| Waiting | An admitted asynchronous boundary operation owns an exact pending request and completion obligation. |
| Faulted | Failure with its actual effects and custody, not successful return or implicit rollback. |

Execution runs on the caller's thread unless the host supplies an appropriate
executor. No automatic worker, global engine, event loop, JIT, or async runtime
is required. Interpreter fuel cannot preempt a synchronous native call or bound
its wall time. Decoding, checking, and storage provision require their own bounds.

A fuel pause is an interpreter-safe stop, not source `suspends`, a call return,
cancellation, or an invariant-restoring observation point. Live loans, facts,
partially moved storage, invariant windows, and component pins remain attached to
the activation. An external operation cannot inspect or mutate affected live
values while those obligations exclude it. Even a write preserving the type's
invariant can invalidate an activation's stronger flow facts. Metadata is not a
borrow permit. A queued edit acquires valid access and establishes its contract
when applied, not merely when queued.

Access exclusion follows actual storage and obligations. A conservative wrapper
may keep a runtime exclusively borrowed across an unfinished call, but this is
not the general execution model: a long-running guest must be able to invoke
authorized component-management operations and permit independently admissible
work without returning from its top-level machine. Management operations work
through scoped authority; ordinary host callbacks receive no unrestricted mutable
runtime. Re-entry obeys the existing [callback contract](private_callbacks.md)
and must preserve all retained access and custody constraints.

Invocation arguments and binding identities cannot be substituted by passing a
different handler on resume. For independent slots, each entry pins its selected
implementation until leave; a subsequent entry may select a newly published
implementation. Returned sessions or other retained dependencies may extend the
pin. Holding an outer guest invocation does not freeze every component's provider
for that outer invocation's lifetime.

Pending host operations complete against their exact request identity once.
Completed effects are not repeated on resumption. Rejection after a possible
host effect neither rolls it back nor permits automatic retry. Dropping an
invocation handle does not discard its unfinished activation or release loans;
the runtime/custodian retains the obligations until a valid disposition.

## Messages and callbacks

Both directions use ordinary contracted calls and values. Applications can choose
direct queries or updates, host-to-guest event calls, bounded command/event queues,
and admitted asynchronous completion. Queuing an event reports queue acceptance,
not that the event's requested action occurred. Delivery, capacity, backpressure,
ordering, and completion are explicit library contracts, not language-wide event
semantics. An idle guest executes only when started/resumed by its selected
execution mechanism; no hidden polling or automatic event thread is implied.

## Reflection and debugging

The SDK exposes retained exact import/export descriptions and requested
[semantic metadata](../language/reflection.md#runtime-descriptions-and-placed-access).
Selected schemas and checked adapters support dynamic invocation, inspectors,
serialization, and editors. Dynamic lookup chooses an already retained entry; it
cannot instantiate arbitrary new generic code or acquire private access.

There is no safe embedding operation for arbitrary writes to live fields, locals,
or instructions. Read and update live objects through exposed checked operations
at valid access points. A detached editable document is not yet an instance of its
described type. Constructors and update operations establish invariants and
resource custody before returning established values. Ordinary internal Omega
mutation still uses machines and borrows; it need not introduce a boundary trait
for every setter.

Optional debugger descriptions may report source locations, execution positions,
and retained local/storage snapshots. They must distinguish unavailable, erased,
uninitialized, and currently unestablished data. Such diagnostic data grants no
typed value, callable handle, or resource authority and executes no getter merely
to display it. A debugger write-and-resume escape hatch is not part of this safe
contract. Names, schemas, and diagnostic metadata have explicit retention costs;
typed calls do not require a universal object header or registry of all types.

## Closing and failure

Closing an instance prevents new entry and accounts for activations, returned
handles, callbacks, child work, loans, claims, and required cleanup. Rejection or
Busy preserves custody. Shutdown releases host state only after its outstanding
obligations settle; cleanup may itself need calls and execution provision.

Cancellation and restart require their actual disposition/containment contract.
A faulted contained instance with only private disposable storage differs from one
holding a host lock, transaction, DMA loan, or registered callback. If the selected
embedding cannot safely contain abandonment, reject that mode before granting
such resources or require an appropriate wider custodian. Memory leakage alone
does not settle external protocol debt. Neither a paused call nor a host-language
destructor establishes safe cancellation.

## Replacement and native equivalence

An interpreted component provider implements the same installation, publication,
entry, and retirement obligations as a native provider, with admitted Psi and
interpreter instances instead of native mappings where appropriate. It does not
invent another component manager or weaker authority model.

```text
build source/dependency tree -> replacement artifact
artifact source -> authorized updater -> stage/admit
updater -> state disposition and publication
new entries -> new implementation
old activations/objects -> retained implementation until valid retirement
```

The application or explicitly appointed supervisor owns update policy, migration,
reset/coexistence, and continuity promises. Loading creates a candidate; only the
designated update authority may publish. An external editor submits a candidate
through that authority rather than overwriting interpreter state. A guest may
itself hold update authority and manage its subcomponents without ending its own
main invocation. Continuity-free publication may precede old-version drain;
stronger state continuity requires the selected coordination/migration contract.

Editing source and recompiling is the ordinary code-change route. Direct Psi
transformation is another producer whose new artifact must be admitted with
evidence for its changed contents. Neither invalid old certificates nor an
unvalidated byte patch can preserve an admitted Program's identity. This does not
require physically copying every unchanged byte. Automatic migration of active
continuations into edited code and retroactive discovery of safe replacement cuts
are not supplied by reflection or recompilation.

## Authority, evidence, and ownership of implementation

The receiving host admits providers and resources under its own policy. There is
no ambient filesystem, network, process, or loader capability. Descendants may
attenuate supply, not turn virtual backing into genuine host access. A binding's
type does not constrain malicious native code by itself; containment and provider
trust remain explicit. Separate instances are not automatically OS sandboxes.

Ordinary structure, schema, input, authority, and execution checks always apply.
[PCC](../proofs/publication.md) is separately requested/required by policy; its
companion is an adjacent `.proof` file. A receiver requiring it independently
checks the exact artifact, contracts, dependencies, and assumptions. An accepted
producer-trust route without that evidence must not be reported as independent
verification.

Psi owns source meaning, descriptions, and portable obligations. Interpreter
semantics owns execution against those obligations. Host/native adapters and
component providers own realization, calling/storage correspondence, and
installation under the existing Psi/Omega firewall. The CLI is a library client,
not the embedding API. Test harnesses reuse this lifecycle with fresh instances
and selected providers; semantic constant evaluation retains its separate
admission regime. Exact SDK packaging and method spellings remain engineering.

## Interpreted Cathedral acceptance and open assembly question

The integration goal is the same Cathedral algorithms and state machines under
native or interpreted execution, with selected providers/build composition rather
than execution-mode branches throughout shared application code. Host providers
may realize/model memory, devices, time, external entry, and component execution;
they must not implement Cathedral's boot, scheduler, migration, or update policy
as a substitute for running that code.

Required demonstrations are actual startup and memory establishment with visible
device interaction, event delivery while the guest remains running, and a
guest-driven independent-component replacement with an old activation retained
and later retired. Preserve exact contract/custody and reject incompatible or
unauthorized replacement. Simulation establishes behavior under its stated model,
not native timing or actual hardware correctness.

Interpreted target-specific inline assembly is **undetermined**, tracked as
`interpreted-inline-assembly` in [OWNER_QUESTIONS.md](../../../OWNER_QUESTIONS.md).
Checked assembly is not revoked, and OS source may legitimately need it. This
contract neither requires moving every assembly operation behind a provider nor
promises universal instruction emulation. Its support, target-state model, and
evidence need a concrete decision before claiming unchanged assembly-bearing
Cathedral boot. Generic embedding and component work do not wait on that decision.

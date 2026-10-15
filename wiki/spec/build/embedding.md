# Embedding and interpreted components

Embedding executes ordinary Omega programs through an ordinary interpreter
library consuming [Terminal Psi](../terminal-psi/product.md). It is not another
source dialect, authority system, module system, or proof relation. This contract
specifies the destination; [implementation tasks](../../../TASKS.md#embedding-and-interpreted-components)
and the [interpreter implementation](../../../omega-rust/psi/interpreter/terminal-interpreter/README.md)
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

## Target environments and execution engines

Target environment and execution engine are separate selections. Windows,
macOS, Linux and Cathedral describe environments; native and interpreted
describe execution. Interpretation is not another OS target. An interpreter
may execute Linux-targeted code when its admitted host environment supplies
the required Linux contracts and supported instruction semantics.

Ordinary Omega provider bodies execute through the same language semantics in
either engine. Most libraries need no interpreter-specific implementation.
Adapters are needed at external mechanisms, not at every machine or package.
A portable filesystem binding may select a virtual implementation; an exact
Linux syscall binding must preserve its selected Linux contract, representation
and foreign identity. A convenient host operation is not a silent substitute.
Native/interpreter comparison retains those exact selections rather than
replacing them with mocks. Separately configured virtual-service tests exercise
their declared service contracts, not the unselected real OS implementation.

Under `hosted-interpreter-abi-library-dependency`, Psi owns Terminal execution
and the interface for installing and dispatching explicit host bindings.
Target-aware host adapters live outside Psi in Omega-owned host integration;
they reuse independent Omega ABI, locator and mechanism libraries, not compiler
drivers or producing stages. The hosting application installs the admitted
adapters. Generic loading and execution require neither those optional adapters
nor source recompilation. Exact SDK packaging remains an engineering choice.

Establish every demanded provider slot in the selected executable closure before
execution, including possible dynamic calls. A branch merely expected not to
run cannot excuse missing supply; code outside that closure need not demand
one. Missing adapter or target-environment support prevents that interpretation
and names the missing requirement and its demand. Do not run native code as an
implicit fallback, replace selected services, or increase authority. The
[test runner](testing.md#execution-plan) may explicitly schedule both engines;
their availability and outcomes remain separate.

## Imports, exports, and provider slots

An exact boundary-trait application is a static import/provider slot under
[provider-slot realization](component_publication.md#provider-slots-and-era-entry).
Guest source calls its requirements directly; it carries no opaque routing
value. The compile-time foreign locator remains separately named
[`ForeignBinding`](foreign_bindings.md); locator data selects a foreign leaf but
does not replace provider admission or runtime authority checks.

The host and guest use exact shared interface declarations. Builds select exported
static entries and establish their schema-designated ordinary context parameters
under [entry roots](entry_roots.md). A qualified machine name introduces no hidden
receiver. Requirement and implementation signatures retain the same explicit
parameter correspondence; the admitted entry adapter supplies any provisioned
context rather than changing the machine's signature.
Neither `pub` nor `satisfies` alone exports a callable ABI. No `main`, `run`, or
`Run` name is privileged by embedding.

An instance's import roster identifies each exact slot/occurrence, closed
requirement application, and full contract. Multiple static roles use distinct
applications or wrapper requirements, not same-typed source fields. Establish
every demanded import before running guest code; missing, duplicate,
wrong-instance, incompatible, ambiguous, or unauthorized supply rejects without
an ambient fallback. Provider supply does not transfer authority; operation
arguments and admitted receiver policy carry their own obligations.

The library supplies typed import/export adapters and descriptions, rather than
requiring each embedder to implement a dispatch system. Generation derives from
checked interfaces and may be untrusted: admission rejoins exact declarations,
applications, contracts, representations, and selected adapters. Display-name
lookup resolves to that exact entry; a string, ordinal, or address alone
establishes neither interface compatibility nor call authority.

A guest boundary call dispatches through the exact installed slot and its host
adapter/context. A compiled adapter can make an ordinary native host call; the interpreter
does not need to generate machine code per call. Runtime-discovered native
signatures need a supported checked ABI adapter, not an arbitrary function-pointer
cast. Host-to-guest calls satisfy the complete entry contract too. Well-shaped
scalars do not establish preconditions, domain membership, ownership, or grants.
Static proofs, admitted evidence, or checked entry operations establish those
obligations; arbitrary mathematical contracts are not automatically executable
validators. Native provider behavior remains an explicit trusted premise unless
independently checked.

## Address observations

The ordinary [address queries](../language/counts_and_addresses.md#address-exposure)
also apply to interpreted components. A selected executable machine yields a
stable entry identity; a borrowed storage location yields its runtime location
identity. Code and storage have disjoint encodings, including across instances
whose address values can meet. Existing interpreter identities may supply these
values; a second address registry is not a language requirement. Proof-only
entities with no executable realization reject instead of receiving fake entries
or zero. Address demand retains an otherwise uncalled executable body.

These numbers are neither host pointers nor native trampolines. The query creates
no host ABI adapter, indirect-call permission, byte-addressed memory model, or
reflective edit operation. Native interoperation still uses the selected
import/export adapters. The identities do not make native address arithmetic or
physical memory accesses meaningful by themselves. Interpreted assembly follows
the [instruction execution contract](../language/assembly.md#interpreted-execution),
including explicit mappings for operations requiring physical geometry.
`code_layout<Machine>()` cannot describe a purely interpreted body's native
regions: those do not exist. Data
`size_of<T>()` and `align_of<T>()` instead observe the selected target layout,
not the interpreter's host storage.

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

Psi owns source meaning, descriptions, and portable obligations. Interpreter
semantics owns execution against those obligations and installation of supplied
bindings. Host/native adapters and component providers own realization and
calling/storage correspondence under the existing Psi/Omega firewall; target-aware
adapters remain outside Psi as described above. The CLI is a library client,
not the embedding API. Test harnesses reuse this lifecycle with fresh instances
and selected providers; semantic constant evaluation retains its separate
admission regime. Exact SDK packaging and method spellings remain engineering.

## Checked assembly and target environments

Interpreted components execute supported checked instruction semantics, not
arbitrary machine-code bytes. Ordinary moves, computation and transitions reuse
the interpreter's existing operations. Block-local scratch registers/flags do
not communicate implicitly with another block or surrounding generated code.
Explicit values, authorized memory and contracted target state do communicate;
those effects persist across blocks and pauses.

The root selects and the receiving host admits the environment required by the
selected executable closure. External mechanisms use ordinary scoped bindings
and state; no per-block alternate implementation or wrapper is required. A
modeled device, interrupt mask or processor is not the host's device or processor.
Live access requires actual host authority. Keep device mechanisms in environment
providers and OS policy in the interpreted application, not the compiler.

Instruction coverage and environment compatibility are checked before execution
and at component installation. Unknown/unsupported effects reject without native
fallback. Halt and external entry obey the target model, not fuel-pause behavior;
the existing invocation/resource lifecycle retains pending effects and prevents
re-execution on resume. The full [assembly contract](../language/assembly.md#interpreted-execution)
also governs ordering, state observations, faults and realization evidence.

## Interpreted Cathedral acceptance

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

Assembly-bearing startup additionally exercises a device-interaction sequence
and a processor idle/external-entry sequence against the selected environment,
with authority, persistent state and once-only effects preserved across fuel
pauses. This is required implementation work, not a remaining owner decision.
Do not replace the exercised OS sequence with a host routine, demand assembly
removal, or report support for unimplemented instructions. Generic embedding and
component work need not wait for complete Cathedral instruction coverage.

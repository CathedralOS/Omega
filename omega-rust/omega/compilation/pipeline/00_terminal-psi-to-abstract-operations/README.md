# Terminal Psi to abstract operations

Public contracts: [Terminal product](../../../../../wiki/spec/terminal-psi/product.md)
and [boundary realization](../../../../../wiki/spec/terminal-psi/boundary_calls.md).

This stage lowers checked Psi for native execution. The build's checked
selection names the functions that need target support; a raw artifact request
without it checks the whole module. Unused library helpers thus demand no
unavailable target operations.

Public entry needs host-authorized producer trust bound to the complete
canonical artifact. Decoding bytes establishes their identity, not permission
to run them. The authorization travels with the native input through lowering
and optimization; reloading a saved product needs a fresh receiving-host
decision. There is no native component disk cache; [compiler storage](../../../../../wiki/spec/build/compiler_storage.md)
retires it.

The build's starting function, implementations and callbacks are joined before
lowering. The [shared input owner](../../representations/abstract-operations/verified-optimization-input/src/lib.rs)
keeps that checked record with the lowered program, so later stages read it
without depending on this stage. Start at [lib.rs](src/lib.rs), which re-exports the
established public names.

- The [shared proposal owner](../../representations/abstract-operations/verified-optimization-input/src/native_proposal.rs)
  checks the chosen target, program entry, provider rows and callback
  occurrences against the canonical artifact, then publishes an immutable native
  proposal. It compiles no source and grants no native authority. The target
  comes from the validated entry; no Build evaluator object or duplicate entry
  receipt crosses over. Providers, occurrences and boundary
  applications stay complete groups. Application metadata belongs to the
  backend target owner; package permissions and behavior exclusions are
  separate. Selected exclusions map to the artifact's local service IDs
  without reopening a typed or checked program or a source resolver.
- The [shared execution owner](../../representations/abstract-operations/verified-optimization-input/src/native_execution.rs)
  follows calls from the proposal's entry, implementations and callbacks. It
  validates component replacements before excluding their local implementations. The [scope walk](../../representations/abstract-operations/verified-optimization-input/src/native_execution_scope.rs)
  also keeps cleanup, installed-interrupt and code-layout requirements, and
  functions that dynamic dispatch tables reference. Reached calls can need more
  direct or Build-selected provider implementations; demand completion checks
  them, then binds the final choices. Original build choices stay
  separate from it.
- [retained_terminal.rs](src/retained_terminal.rs) runs lowering and native
  provider preparation from the shared proposal and execution records. It joins
  package permissions and prepares callbacks, imports and builtins for
  admission. Derived import evidence stays invocation-owned through admission
  and emission; the compiler forwards this stage's request rather than copying
  its fields. The [codec](../../representations/abstract-operations/verified-optimization-input/src/retained_terminal_codec.rs)
  that saves derivation inputs, which contain no source, lives beside
  `RetainedTerminalArtifact` in the shared input owner, so a saved product is
  recovered without running this stage. Recovery runs them through the same
  shared proposal operation without source. The in-memory retained product
  encodes no separate native authority record.
- [native_execution_scope.rs](src/native_execution_scope.rs) is the complete
  native entry point. Canonical input preparation, provider settlement and
  installation, and mechanism and exclusion review produce one admitted input
  before Omega 01.
  [Provider admission](../../realization/provider-admission/src/provider_admission.rs)
  derives each demanded leaf's mechanism and admits builtins through the
  receiving policy and target catalog; it reads the bindings the proposal
  derived instead of joining them again. Omega 02 consumes these facts; neither
  it nor the compiler repeats admission.
- [artifact_admission.rs](src/artifact_admission.rs) does canonical
  preparation, optional obligation-ledger replay, and separate ordinary,
  optimizer and native admission. Every admitted result keeps the exact
  access-view input roster. Extracting into a consumer without the roster
  rejects nonempty inputs, after verification and lowering, not at decode.
  [Native preparation](src/artifact_admission.rs) receives the complete checked
  selection from the shared owner, verifies its artifact, entry, target and
  original choices, applies the selected geometry, then lowers those functions.
  Raw requests without it check the whole module and keep no selected scope. No partly lowered native input is pruned by a later public step.
  Ordinary and no-normal-return calls, cleanup, callbacks, native metadata and
  dynamic-table address roots keep their existing demand and per-block
  diagnostic order. Explicit artifact inspection and independent ownership
  validation cover the full catalog.
- The admitted handoff to Omega 01 is `VerifiedPsiOptimizationInput` in [verified-optimization-input](../../representations/abstract-operations/verified-optimization-input/src/lib.rs); Omega 01 builds its optimization unit from it.
- `provider_installation` holds the exact selected adapter and installation.
- [shared callback settlement](../../representations/abstract-operations/verified-optimization-input/src/callback_settlement.rs)
  joins a produced callback's Terminal interface to its exact selected calling
  application. Native placement reads each reached boundary's canonical calling
  parameter list: semantic and native-only argument identities and order,
  nominal callback binders, static-machine positions and synchronous-invocation
  flags. Target calling conventions supply direct function-pointer geometry. A
  proposed application's duplicate list must agree with that declaration, not
  override it.
  Ordinary and private inline-slot layouts still arrive prepared, a separate
  source-free preparation gap. Placement replay,
  requirement identity, scalar and structural ABI matching, and the private
  channel's import exclusion live here, beside program-entry settlement. The
  coordinator supplies the artifact, placement and calling-plan roster, with no
  checked-compilation wrapper or new stage.
  Its [registrar-import settlement](../../representations/abstract-operations/verified-optimization-input/src/callback_settlement.rs)
  joins each ordered callback occurrence to the registrar's separate outbound
  plan and selected import row. Retained-Terminal and direct-native routes
  share it; exact operation counts and check-only import absence stay explicit.
- [Shared boundary settlement](../../representations/abstract-operations/verified-optimization-input/src/settlement.rs)
  binds calls and comparisons to implementations the build already chose. It
  selects from canonical requirements and checked body candidates using the
  target's Build rows. Provider review already establishes intrinsic
  executions. It does no checked application scoping, source declaration
  lookup, const or type reconstruction, or intrinsic replay. The proposal keeps the selected coverage and opaque ABI applications;
  native admission uses them without a second source join. It also walks
  Terminal comparisons, using the executions boundary coverage selected. A sorted index of plan digests joins the target plans without source
  uses, checked handles or Lowered Psi occurrence records. Integer comparisons
  keep their selected semantic operation, not a separate lowering triple.
  Builtin integer comparisons need no provider row.
- `lowering` walks operations in order for scalar, Unit and structural
  results. Optimization selection does not change this projection.

[Module lowering](src/lowering.rs) checks the entry and structural block
bindings, then calls [machine lowering](src/lowering.rs) directly. It
prepares block and value rosters, visits blocks in order, dispatches
[operations](src/lowering.rs) and [terminators](src/lowering.rs), and assembles
the abstract function. Each abstract block owns its operations and its final
control transfer; there is no flat body with a separate table of block offsets.
Scalar, Unit and structural results share this walk; there is no separate
forwarding or ordinary-machine route.

No earlier program, StateGraph or caller-built module replaces the canonical
artifact. Unsupported vocabulary is rejected at admission or lowering;
installing a provider does not permit dropping an unsupported result.

Native admission returns `AdmittedNativeArtifact`: one abstract program and its
checked optimizer context. It borrows the authorized immutable artifact, lowers
it with ordinary native authority, then checks optimizer eligibility as it
moves the retained state. Explicit obligation replay is a separate validation
request. The gates stay separate; optimizer-only input cannot become native
authority or pair with an unrelated plan. Optimizer and
native holders borrow the roster from their retained canonical module, with no
second mutable roster. A native consumer without placement support extracts
`VerifiedNativeArtifactInput` only after checking that the roster is empty.

Image-writer storage must not be relabeled as compiler-written object data. For example, Mach-O replay derives aligned eight-byte
lazy-binding pointer slots from the exact imports and relocations.
Installation's text and immutable-data projection covers compiler-written
prefixes; the complete image is bound and replayed separately. A
projection does not claim every image byte came from an object.

Installation replay keeps candidate and caller result places apart, with
complete argument paths, claim sources and residual complements.
Provider scalar signatures are compared with Terminal declarations, not copied
into a second wire schema. Admitting results and conformances
does not implement native structural-result storage.

The next stages use the same representations whether optimization is empty or
selected. The old direct-assignment and one-fragment publication routes are not
alternatives. Machine-effect facts must still
reach final call placement, frame and return-address preservation,
relocations, bytes and independently validated publication.

## Structural results and residual cleanup

Primitive array construction keeps its complete structural result and ordered
scalar leaves in `EstablishScalarArray`. Ordinary, optimizer and native
artifact admission all use this projection, including empty nested dimensions.
Current-IR validation separately checks shape, leaf type and availability, and
call and return ownership; scalar rewrites keep every
leaf occurrence. Native storage and direct result fragments use the later
aggregate graph; its
[instruction-selection owner](../03_target-operations-to-selected-instructions/README.md#ordinary-selected-control-flow)
lists the remaining limits on owned arguments, empty values and indirect results.

Verified branches, calls and crash exits keep operation identities and
ownership transfers for every result category. Local establishments are
ordinary operations, not hidden in a structural return. The
[ownership contract](../../../../../wiki/spec/terminal-psi/ownership.md) requires
replay of actual moves and maximal residual subtrees, independent of producer
rows. Parameter and result sources stay apart from unrelated live roots.

Keeping an operation here does not give it a native implementation. The next stages accept Unit, scalar and structural functions
through one graph. Executable cleanup, continuations that carry claims,
projected qualifications and installed-provider calls need their complete
ordinary operation and proof joins; otherwise the program is rejected. No
flat whole-function or no-code structural-return route provides that support.

## Native calls and storage

Each callee has its own ABI. Argument passing must keep source values intact
until registers or outgoing-stack slots are filled. Physical replay
checks the outgoing area, alignment, return-link preservation, source places,
field widths, calls and relocations. Padding is not observable payload,
and a borrowed referent must not become a private copy.

The [target stage](../02_abstract-operations-to-target-operations/README.md) and
[instruction-selection stage](../03_target-operations-to-selected-instructions/README.md)
own their operation coverage. Admission here, source production and
successful optimization do not claim native publication.

## Ranked native admission

Contract: [control flow and ranking](../../../../../wiki/spec/terminal-psi/control_flow.md).
[artifact_admission.rs](src/artifact_admission.rs) sends natural-ranked
modules through ordinary Terminal verification and abstract lowering.
That verifier checks the exact grouped control-cycle evidence, including
slice-decrease proofs; the route neither requires nor invents a fixed-work
ceiling. The abstract graph keeps its cyclic edges and structural bindings, and
the separately verified optimizer input keeps the canonical Terminal module and
checked producer facts. Admission alone does not show that later
optimization, target lowering or publication support these cycles.

Whole plain owned structural block arrivals keep exact source and destination
types, access, multiplicity and ordered bindings.
Independent current-ownership replay consumes all old owned roots before
establishing destinations, including swaps and self-loops. Whole Linear arrivals
move each live claim to its exact destination with its identity and typed path,
without new claims or copied obligations. An
ordinary call transfer joins that current home, including terminal calls on
conditional and guarded arms. Alternative arms inherit the same claim identity;
only the selected call transfers it, and each path accounts for its owner. Affine cleanup runs in reverse establishment order, derived from
dominance, not from block ID order. Kept frontier snapshots join cleanup and
rebinding independently. This bounded admission excludes conditional
claim-lineage changes, claim-bearing result joins, and broader qualified or
projected Linear transfers. Cleanup of an exact owner-designated scalar
record keeps its consumed claim roster through an ordinary hook call; discarding
without code is Affine-only. The parent hook does not cover erased or nested
Linear children. Plain owned payloads use the ordinary native graph.

Reads of Integer and Boolean record fields keep their actual parameter,
construction-result or block-arrival place; current ownership and dominance
checks reject sources that are unavailable. Field-only carrier paths keep
canonical declaration-local IDs through the abstract graph. Independent native
and image replay rebuild each child declaration and offset from the original
root; the same leaf ID in another record cannot stand in for that path. Direct
reads use an empty path. Native local and block-arrival reads use the existing structural
home and primitive load path, including bounded integer leaves.
Declarations and independently rebuilt construction obligations are kept; a read's physical width and sign come from that exact carrier. Bounded
stores are still rejected without a proof that establishes the invariant.
Indexed and case carriers and broader structural call passing remain separate
consumer dependencies.

Initialized primitive locals keep their exact operation-result place and typed
initializer. Local stores stay separate from stores to incoming parameters;
fresh reads keep their source place and a new scalar result. Current-IR checking
rebuilds primitive type, access, claim-free unrestricted storage, dominating
establishment and non-escape. Optimization cannot treat a read as
its initializer or as an earlier observation across a mutation. The ordinary
native graph implements fixed 8/16/32/64-bit integer, Boolean, and IEEE
binary32/binary64 locals and their borrowed calls, with exact-width reads and
unchanged payload bits.

Unranked modules take the same ordinary verification and abstract route without
claiming progress. Scalar cycles go through the shared
[target control graph](../02_abstract-operations-to-target-operations/README.md)
and the native physical pipeline. Exact verified-source custody, not ranking
metadata, authorizes later cyclic safety checking.

The older native handling of unsigned countdowns is unsupported. Admission
rejects it explicitly instead of erasing its proof requirements or falling back
to ordinary verification. There is no dedicated countdown target, selected or
machine-code form. Natural ranking and its grouped certificates are still
checked on the ordinary path.

The [projected-receiver contract](../../../../../wiki/spec/language/termination.md#ranked-callees-on-projected-receivers)
still requires composed argument references, call and return, cleanup, checking
of the callee's measure, and resource evidence. A graph alone does not provide
them; unsupported transfers are rejected until their operation and proof joins
exist.

## Dynamic dispatch

This is the legacy selector implementation, to be replaced by
[runtime machine values and some packages](../../../../../wiki/drafts/designs/machine_values_some.md). Source `dyn` and whole-trait runtime tables are retired. Keep the
argument, contract, result and ownership behavior below while migrating; these
notes do not authorize the old source form.

The [dynamic-dispatch contract](../../../../../wiki/spec/terminal-psi/dynamic_dispatch.md)
owns the replacement descriptor and package evidence. The legacy implementation
selects named implementations, with data arguments supplied separately.
`SelectImplementation` keeps its exact table and a scalar result home;
`CallDynamic` becomes an ordinary abstract call with a dynamic target, keeping
the selector, interface, slot and value arguments separate. The u64 is only the
opaque physical carrier, not permission to substitute an integer literal or
direct machine. There is no instance or table descriptor, synthetic receiver,
or native fallback through a transparent hop.

The abstract plan keeps the complete interface and table catalog. The
invoked slot's requirement owns the argument ABI; no implementation supplies a
representative signature. Ordinary scalar flow carries forwarding,
rebinding, storage and joins, separately from ownership of borrowed data.
Executable table rows stay roots that keep addresses alive, while only the
reachability of invoked slots contributes authority and effects. Changing an
unselected row still changes identity.

The current public slot vocabulary carries Unit or scalar results and ordinary
structural parameters. It does not carry structural result origins,
qualification preconditions or ingress claim schemas. Missing metadata is
rejected, not inferred from private bodies. Native lowering,
register allocation and image publication use these same targets through their
ordinary operation owners; table rows without executable callables keep their
zero trap slots.

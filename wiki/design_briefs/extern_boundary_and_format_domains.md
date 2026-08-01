# Design Brief: Extern Boundaries And Foreign Formats

Current as of 2026-07-28. This brief defines the durable extern model. Concrete
binding/layout grammar remains subject to the referenced subsystem briefs.

## Abstract API, target binding

Application code calls an abstract `boundary trait`. Target/toolchain packages
provide implementations; application code never imports a DLL, syscall number,
or firmware table as if it were an ordinary module.

```omega
boundary trait WindowSystem {
    machine create(spec: WindowSpec) -> CreateWindowResult;
    machine present(window: &mut Window, pixels: &[Pixel]) -> PresentResult;
}
```

A `ProviderPlan` maps requirements to their checked realizations, but it is a
**derived normalized artifact**, not a value assembled row by row by user
code. Authors provide three inputs:

1. boundary-trait requirements;
2. ordinary checked machines that explicitly `satisfy` those requirements;
   and
3. irreducible external leaves declared with `satisfies ... via <Binding>`.

The binding vocabulary is an ordinary closed sum, not a growing family of
keywords:

```omega
data Binding {
    case DllImport(library: LibraryId, symbol: SymbolId, plan: CallingPlanId);
    case Syscall(number: u64, plan: CallingPlanId);
    case Firmware(table: TableId, slot: u32, plan: CallingPlanId);
    case CompilerIntrinsic(name: IntrinsicId);
}
```

Exact cases may grow only when a genuinely different irreducible binding
mechanism exists. Host-specific flags and `host:` mini-languages are not part
of Omega. Foreign struct offsets and bit positions belong to programmable
layout/format declarations, not a generic `Binding::Value` escape hatch.
Privileged target instructions belong to parsed, contract-emitting `asm {}`;
`Binding::Instruction` is retired rather than preserving two ways to state the
same operation with different visibility to effect and authority analysis.

```omega
machine Kernel32::write_file(handle: WinHandle, bytes: &[u8]) -> WriteResult
    satisfies Kernel32Requirements::write_file
    via Binding::DllImport {
        library: kernel32_lib,
        symbol: write_file_symbol,
        plan: MsX64,
    };
```

The `via` expression must be compile-time evaluable to a normalized `Binding`
value. It is the external-realization variant of the machine supply slot, not
an executable body and not a self-authored trust assertion. Its identity feeds
the derived plan. Structural validation checks the declaration; admission
assigns the trust class and receipt.

Composite behavior is checked Omega code rather than plan-shaped call
sequences. A Console adapter that gets a standard handle, appends a newline,
and performs one or more writes is an ordinary machine satisfying the Console
requirement. This permits caching, batching, policy, and stronger contracts
without extending a call-shape DSL. Constants and foreign formats similarly
stay in their existing semantic homes.

The toolchain computes the selected provider type's conformance closure and
derives plan coverage, signatures, effect summaries, dependencies, normalized
identity, and admission inputs. Only explicit `satisfies` edges participate;
structural coincidence never makes a provider. Build-time machines may select
among declared candidates or compute a leaf `Binding` value, but they never
imperatively append plan rows.

`Binding` values themselves may be constructed freely; construction grants no
authority and makes no provider selectable. Provider handling then has four
distinct artifact stages: derive a candidate deterministically from
declarations; validate structural coverage, signatures, calling/layout
plans, and normalized identity; admit its semantic claims under boundary grant
authority and issue receipts; then select it for a slot under that slot owner's
capability. A target package supplies ordinary provider-type defaults,
`build.omg` selects a default target profile, and explicit
build/test/component configuration may override individual slots. Defaults
are package declarations/data, not compiler magic. Slot selection changes the
selected provider; it does not reconstruct its rows. Every authored slot path
must resolve to exactly one canonical boundary-trait identity in the loaded
closure. An exact canonical name wins; a short leaf fallback is accepted only
when unique, and qualified/unqualified aliases cannot be used to select the
same slot twice.

The selected plans survive typed-to-checked lowering as one canonical checked
fact set. Every retained plan is revalidated as fully covering, selected names
must resolve exactly once, and duplicate or identity-colliding selections
reject. Later backend, generated-machine, and provider-execution work consumes
that immutable normalized carrier rather than scanning `satisfies`
declarations again. The carrier publishes both each plan's normalized identity
and a deterministic identity for the complete selected set. External-root
construction resolves its boundary slot against this carrier and copies the
resulting plan identity into the root candidate before validation; an absent or
ambiguous retained selection rejects.

The normalized service schema also retains each linear routed parameter
qualification as a structured entry claim. Its carrier-aware semantic-domain
identity, `accepts` authority-flow verb, and born-strict compiler carry policy
participate in provider-plan identity. The external-root selection bridge
copies those rows beside that identity, and the qualification artifact reports
them with the selected-plan receipt. The row records what an admitted external
entry may supply; only the matching concrete entry receipt establishes a source
fact for one invocation.

Checked-adapter dispatch consumes that retained carrier as well. Only an exact
`CheckedAdapter` row in the selected plan may rewrite the corresponding
boundary call; an unrelated or unselected adapter cannot overlay the selection.
Every checked adapter belongs to a nominal provider type. The rewrite retains
the selected entry-state symbol and complete nominal machine name for both
statement and value calls. Standard Console publishes one complete nominal
provider closure per hosted target, with checked
`write`/`write_line` adapters and compiler-intrinsic rows selecting the
target's existing `read_line`, `read_byte`, `write_byte`, and process-exit
lowerings.

The static build-root spelling is
`b.select_provider<BoundaryTrait, ProviderType>();`. Both arguments are types,
not static machine parameters. The declaration is harvested only from the
authoritative `build.omg` machine, and selection succeeds only when that
provider's own derived candidate exists in the loaded dependency closure,
applies to the selected target, and covers the complete slot schema. Thus the
marker grants neither rows nor trust; it spends the build root's slot-selection
authority over an already-derived and independently admitted candidate.

The satisfied requirement supplies the public contract, including service-
reach, suspension, and blocking ceilings. The external realization's behavior
is derived from the binding/provider contract and must refine every ceiling at
validation/admission. A `via` machine does not repeat those clauses.

This is one boundary-contract shape, not FFI-only ceremony. A checked Omega
provider derives facts from its body. An opaque provider supplies admitted
facts through its binding. Trust is classified per fact, and a composite
guarantee reports the weakest input together with the exact provider premise
that made it so.

```text
StackPlan
  class: admitted
  input: Firmware.foreign_stack_ceiling

callback_acyclic
  class: derived
  input: checked invokes graph
```

## Reach, authority, and trust

Decision 22 applies without an extern exception:

- the boundary-trait identity contributes service reach;
- capability/evidence values carry authority;
- the selected external provider produces a trust receipt; and
- `suspends`/`blocks` are independent operation/provider ceilings when
    applicable.

A checked wrapper may refine operational behavior or reduce trust expenditure;
it does not erase the abstract service reach from callers compiled against that
trait.

Reach and executable trust remain separate. Checked bodies infer complete
service reach, bodyless surfaces publish it, and callers receive the transitive
closure. Static import is not a runtime operation: a call through a statically
selected Windows provider reaches `WindowSystem`, while an explicit runtime
loader call additionally reaches `DynamicLibraryLoading`. Deployment policy
decides which reach entries warrant refusal or a loud report; capability
authors do not opt into propagation.

TCB expansion is a selected-provider property. The same source requirement may
select checked Omega, an opaque in-process binary, or an isolated endpoint.
Selection therefore contributes a normalized executable entry to the artifact
without changing the source reach contract. Each known entry retains:

- exact provider, provider-plan, and executable/artifact identity;
- implementation evidence class and admission provenance;
- origin as static selection or Omega-mediated runtime admission;
- execution scope; and
- scoped containment guarantees with their own trust evidence.

Containment guarantees are named by what they establish: memory isolation
outside explicitly shared authority, forcible termination, fault containment,
and bounded resource use. Mechanism names do not imply the complete set. A
process needs explicit quotas before it supplies resource containment; a
same-address-space mechanism supplies only the guarantees its admitted
enforcement actually establishes. Implementation evidence and containment
remain independent axes: an admitted hardware or instruction fact is not an
opaque executable in the caller's address space.

The manifest separately reports whether its known entry list is complete for
one execution scope:

```text
Complete(scope, evidence)
Incomplete(scope, attributed uncontained providers)
```

An uncontained opaque in-process binary makes the caller-address-space
manifest incomplete. It may load or generate executable code without using
Omega's loader, so the runtime ledger can report only entries Omega admitted,
not every executable actually present. A constrained dynamic-loading envelope
is enforceable only inside a containment regime that controls executable
admission. A checked adapter cannot remove this provenance.

Build and deployment profiles evaluate the selected entry set, manifest
completeness and evidence, required containment guarantees, and approved
platform or third-party identities. Platform baselines are policy allowlists,
not different language semantics. Development profiles may admit and mark an
incomplete artifact; safety profiles fail before artifact installation when
their requirements are not met. An isolated provider is an endpoint in the
caller's manifest and receives its own executable manifest for its execution
scope.

Binding authors publish the widest contract they can honestly support.
Over-approximation may cost usability: an unconstrained synchronous invocation
ceiling rejects from an acyclic context, and a blocking edge without finite
wait evidence yields `NoFiniteGuarantee(edge)`. Under-approximating an opaque
provider is an unsound admitted claim. The compiler checks the consequences and
internal coherence of a declaration; it cannot establish its truth from a DLL.

## Boundary declaration coherence

The checker reads existing contract axes rather than a separate foreign-use
plan:

- bodyful checked providers infer operational facts; bodyless surfaces author
  their ceilings;
- `blocks` must fit the caller's blocking ceiling and carry the source
  acknowledgement at the call site; without selected finite wait evidence the
  response report is `NoFiniteGuarantee(edge)`;
- `invokes` contributes direct synchronous edges and rejects a realized
  component-boundary cycle;
- a reference grants use only before return; a result claim retaining storage
  after return must receive that authority from a consumed input through the
  ordinary conservation mapping;
- the selected executor must satisfy the operation's thread or apartment
  affinity; and
- `addr` and `Ptr<T>` remain inert ABI data and cannot substitute for an
  established storage claim.

## Calling plans

Boundary-entry behavior is one normalized artifact with independent `CallPlan`
and `StatePlan` facets. The first owns parameter/result placement and ordinary
ABI clobbers; the second owns initial machine regime, interrupted state,
save/restore commitments, and permitted transitive machine-state use. It is not
inferred from library or symbol strings.

Bindings cite a plan identity. Provider admission verifies that the binding and
entry stub implement the pinned boundary-machine contract. See
[`calling_plans.md`](calling_plans.md).

The evaluated plan belongs to the satisfied requirement through ordinary
`Calling<C>` policy composition. The old `boundary(<Plan>)` marker is retired;
`boundary` identifies the trust/supply edge and does not carry deployment data.

### Floating control state

`f32` and `f64` requirements assume Omega's canonical semantic floating-control
configuration. A native boundary must therefore state how the relevant control
bits cross it:

- a preserving binding proves that the foreign call leaves the masked
  MXCSR/FPCR semantic controls unchanged;
- a general binding saves and restores those controls in its trampoline; and
- an inbound callback establishes the canonical Omega controls before checked
  code runs, then restores the foreign controls on exit.

Sticky floating status flags are not part of this semantic invariant.
Directed-rounding operations do not alter ambient control state, and
`Trapping` does not unmask hardware exceptions. A library or callback that
silently enables FTZ/DAZ cannot leave behind a valid Omega hardware-float
realization.

## Foreign execution and stack accounting

Execution placement is selected through ordinary providers and runtime
executors. It is not a language-level foreign-call disposition. A bodyless
binding declares blocking and affinity; a checked provider derives them. The
selected execution context must permit blocking and satisfy the required
thread or apartment affinity. Thus a Windows message loop may call a blocking
`GetMessage` directly on its dedicated pinned UI executor, while a codec-style
opaque call may be wrapped by an ordinary blocking-executor package.

Hosted direct calls use the host-managed stack and its guard according to the
selected calling plan. Callback entry preflights the exact Omega WCSU when the
host profile requires it. A fixed-stack or freestanding provider instead needs
an admitted foreign contribution or a separately provisioned provider stack.
An isolated provider crosses the existing process/component boundary and
exposes an endpoint rather than a special FFI call kind.

A boundary requirement's resource ceiling is not evidence that an opaque
implementation fits it. Checked Omega realizations derive WCSU. A native
binding needs admitted foreign demand, or an enforced guarded capacity whose
overflow remains an abnormal-exit route rather than proof of successful
completion. Trust composes by the weakest input and reports the exact foreign
premise.

A hosted blocking executor is an ordinary package assembled from activations,
bounded queues, moved custody, linear completion claims, suspension, and
provider selection. It keeps a blocking call off a no-block scheduler worker
but does not change the foreign contract. An in-process worker cannot be killed
safely; a detached call pins its worker, storage, and provider era until native
return. Bounded recovery from a genuine hang requires process isolation.

## Registered callbacks

A callback protocol is declared by an ordinary boundary requirement carrying
its `Calling<C>` policy. A named static `boundary machine` explicitly satisfies
that requirement. Passing the machine to a registration operation selects the
conformance, validates its `CallPlan + StatePlan`, and lets the compiler
materialize the foreign ABI thunk and relocation inside that exact binding. The
source surface does not need a general function-pointer value.

Durable registration returns an ordinary linear package value. It owns the
foreign registration and any code/component lease needed to keep the entry
valid; its explicit terminal operation unregisters before releasing those
obligations. Call-scoped callback parameters remain borrowed for the call.
Foreign context storage carries an inert protocol token or generational handle,
while the owning state remains in an Omega registry or another ordinary
package-owned value.

Synchronous entry and deferred registration are separate contracts. A bodyful
machine infers its `invokes` set from the body, including forwarding through
local helpers. A bodyless requirement declares every binding it may invoke
before returning:

```omega
boundary trait EventSource {
    machine register_and_fire(handler: Handler) -> Registration
    invokes handler;
}
```

`invokes handler` contributes the handler trait and the selected conformance's
operational envelope to the current invocation's normalized reach. The returned
linear registration separately establishes a future external root carrying
that same concrete conformance and envelope. A registration operation without
`invokes handler` cannot enter the handler synchronously on its current call
chain. A separately activated root may run according to the registration
contract, including concurrently with registration.
Root establishment requires the selected root policy to admit the concrete
handler envelope; the sealed registration establishment route records that
fact. It is not a freely assertable postcondition.

Cycle checking uses the direct synchronous `invokes` graph, never the
transitive service-reach closure. The realized synchronous graph across Omega
component boundaries must be acyclic. A protocol that needs a cycle moves one
edge within an artifact or breaks it structurally through a mailbox, queue,
scheduler handoff, or other new-activation boundary. Deferred roots may form
reach cycles in the final program graph without creating nested component
stacks.

Hosted callback entry may continue on the provider stack, preflight its
remaining capacity against the exact Omega WCSU and target reserve, or enter a
target-supported owned stack. Preflight proves the predicted segment fits; an
owned hard-limited stack additionally detects underestimation at its own
boundary. Foreign calls made by a separated-stack callback return to the
provider stack domain before entering opaque code.

Native protocols may synchronously re-enter application callbacks. A platform
adapter exposes exact `invokes` ceilings, checks each ordinary Omega handler's
realized envelope, answers synchronous platform queries through restricted
handlers, and queues ordinary application events until the outermost native
dispatch returns. This package-local construction does not require inferring
the opaque provider's internal call graph.

A raw opaque callback remains trust-relative. Its binding may enforce a
chain-scoped active/depth limit only when the protocol supplies a valid
unavailable result. Otherwise finite mixed-chain admission requires a checked
provider contract or structural isolation; a handwritten native header does
not become proof of non-re-entry.

## Foreign data and formats

Foreign layout is expressed by authored programmable layout policies built from
compiler-known placement primitives. Plain `data` supplies the semantic shape;
layout policy supplies the foreign byte representation. Format packages publish
their selected plan, codec requirements, realizations, and trust evidence.

Inbound paths are explicit:

1. receive raw bytes/pointers under a boundary contract;
2. validate or materialize according to the layout/format policy;
3. establish predicate facts and any authorized semantic qualification; and
4. expose ordinary Omega values or checked borrowed views.

Decision 19 governs the transitions. `as` may prove a refinement or declare an
authorized representation-identical semantic commitment; executable conversion
is an ordinary contracted call. A recast may expose the same storage under a
weaker/alternate stated layout only when the representation and lifetime laws
permit it. No cast fabricates stronger foreign validity.

Outbound paths forget semantic facts or execute an explicit encoding/conversion
before crossing the boundary. The foreign vocabulary does not leak into normal
program types merely because one provider uses it.

The filesystem open-flag migration is the first concrete instance. Application
and portable standard-library code author `OpenOptions`; selected target-package
machines encode those semantics into Darwin, Linux, or MSVCRT flag words. The
bit positions are checked target-format implementation facts, not provider-plan
`Value` rows and not portable constants. Foreign record offsets remain on the
retirement path until placed/recast views can consume the validated layout plan
directly; exposing a public raw-offset accessor is not an acceptable bridge.

## Foreign addresses and storage lifetime

`addr` is numerical address data. `Ptr<T>` is a sealed, inert foreign-ABI
carrier whose parameter supplies representation and pointee-shape information
to boundary lowering. Neither is authority. Ordinary Omega code cannot
dereference, index, or manufacture a reference from either carrier. A binding
materializes a `Ptr<T>` only from an established storage claim after validating
the selected marshaling and calling policies; inbound carriers become checked
views only through an authorized establishment route.

Foreign storage use has three outbound ownership shapes:

1. **Call-scoped:** an ordinary `&T`, `&[T]`, `&mut T`, or `&mut [T]` permits
   only access before the call returns.
2. **Retained after return:** storage authority moves into an ordinary linear
   protocol value such as `PendingRead`; a terminal completion redeems it.
3. **Process-lifetime:** the authority moves into an already-established static
   or process-lifetime root. Omega has no general permanent-custodian spelling;
   other permanent retention remains unsupported until a concrete customer
   justifies one.

Post-return retention is not a long borrow. The linear claim owns the keepalive
and reclamation authority for its backing place, not necessarily the bytes
inline. It may lend ordinary lexical views over rights the foreign side does
not hold. A read-only foreign operation can therefore preserve semantic facts
and lend Omega read views; a writing operation invalidates facts over exactly
the writable extent and re-establishes them from terminal completion evidence.
Separated partial release is an ordinary split in the claim-content algebra:
the returned subextent leaves flight while the disjoint remainder stays under
the same protocol claim.

The compiler learns that use survives return from ownership conservation. A
consumed `Buffer` may map into the content retained by `PendingWrite`;
`submit(&buffer) -> PendingWrite` rejects because a borrow supplies no owned
claim that can establish the result.
Unambiguous consumed-input-to-produced-claim mappings are inferred; ambiguous
or unsupported mappings reject unless an ordinary postcondition pins the
correspondence. A content-bearing exact qualification supplies its projection
through its owner-unique core `Content<A>` conformance; the binding does not
invent a separate foreign-extent algebra or projection annotation.

The reverse direction uses the same types. A provider-owned view whose
invalidators require exclusive access to one receiver is an ordinary borrow
from that receiver. More precise or nonlexical protocols return a linear view
claim and require every invalidating operation to consume the claims it kills.
Global, thread-local, or asynchronously invalidated foreign storage must be
copied, mediated by such a protocol claim, or accepted under an admitted
stability promise. A claim cannot prevent an opaque provider from invalidating
storage through an unmodeled route.

Completion is an establishment point. Its contract correlates one event with
one live claim through a unique/nonreused identity, a generation-checked
identity, or an exclusively ordered channel. A progress event releases nothing
unless its contract returns an exact separated subclaim; cancellation requests
release nothing until a terminal acknowledgement. Reused tokens require
generations wherever stale foreign copies can survive.

The selected provider era enters the compiler-tracked set of live claims for a
value (its claim frontier) only when that value's meaning depends on state owned
by the exact era. A
provider-created handle or pending operation pins that era; a rebindable service
binding names a slot and does not. Pins block reclamation rather than teardown
execution: an old era remains callable while it discharges roots it owns, then
waits for application-held claims, establishes quiescence, and unloads. Static
custodians discharge their outlives relationships at build time and create no
runtime ledger noise.

The safe parameter and result types carry access and lifetime behavior.
Calling/marshaling plans describe representation only: for example, that
`BoundarySignature` parameters 0 and 1 encode one contiguous slice, or that one
validated descriptor denotes several separated extents. A selected policy that
defines a native slice ABI derives the ordinary reference case. Raw
pointer/count pairs and descriptor graphs require an authored binding policy;
the compiler never guesses their association.

Omega presently has shared and exclusive read/write references but no precise
provider-writes-only view. Until a core write-only claim/view lands, a binding
must not silently widen write-only access to read/write when doing so would
disclose existing bytes, and it cannot expose uninitialized receive storage
under a contract that permits foreign reads. Identity-only retention is an
ordinary stable keepalive claim that lends no memory view.

The native leaf declares the foreign signature's actual parameter structure.
Separate pointer and length parameters are not interchangeable with a record
containing the same fields: the selected calling policy may place them
differently. Safe slice/text carriers remain private Omega representations and
are rejected as bare native leaves unless an explicit custom `Calling<C>`
policy publishes their ABI. Fixed arrays and records, by contrast, may be
structurally classified because their public normalized shape determines the
aggregate facts the policy consumes. Omega never performs C array decay.

Every reclaimable installed callback/interrupt entry is also an external
artifact root. Because no Omega call edge reaches it, the dynamic root ledger
retains its reach, authority/trust receipts, state footprint, stack domain,
nesting relation, and version pins until its linear registration proves
unregistration and required quiescence. A process-lifetime statically linked
callback needs the same build report but no live replacement ledger. This
reuses provider admission rather than creating an entry-specific trust system.

## Process entry

`main` is an exported boundary callable with a typed handoff shape. A
target-specific inbound stub translates the OS/firmware startup convention into
that shape and records the accepted trust/calling-plan contract.

```omega
boundary machine Main::run(
    &self,
    handoff: ProcessHandoff
) -> ExitStatus;
```

On Windows the stub may read the native command-line/environment surfaces; on
ELF it may read the initial stack; on firmware it may consume a firmware
handoff. Those details stay in providers. Image/subsystem selection belongs in
`build.omg`, not a target-specific source dialect.

## Engineering order

1. Normalize boundary-machine contracts and calling-plan identities.
2. Represent `Binding` as resolved target/provider data.
3. Validate provider admission and emit the transitive executable-entry,
   containment-guarantee, and scope-completeness manifest; distinguish static
   selection from Omega-mediated runtime admission and retain exact
   incompleteness attribution.
4. Lower imported calls and inbound stubs from checked plans only.
5. Integrate programmable layout validation/materialization.
6. Add final-artifact state-footprint validation and external-root reporting.
7. Add boundary-coherence rejection canaries: retained-after-return custody
   sourced only from a borrow, blocking under a no-block root, incompatible
   affinity, and undeclared or cyclic synchronous invocation.
8. Implement the narrow Windows `user32` acceptance slice in `TASKS.md`.
9. Add foreign-retention and provider-view canaries.
10. Delete host-string special cases and legacy target blocks.

## Still open

Target-specific launch/exit details not covered by existing calling plans.

Exact `Build` library method names for choosing a target profile remain
ordinary library/API engineering. Per-requirement provider override has settled on
`select_provider<BoundaryTrait, ProviderType>()`; equivalent scoped APIs for
tests and replaceable-realization owners remain engineering work, not an open
grammar question. "Binding" here is build/artifact state, not a source `slot`
construct.

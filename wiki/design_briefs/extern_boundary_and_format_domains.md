# Design Brief: Extern Boundaries And Foreign Formats

Current as of 2026-08-24. This brief defines the durable extern model. Concrete
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
data DllImport<
    const ObjectLength: u64,
    const SymbolLength: u64,
    const VersionLength: u64,
> {
    case PeByName(
        library: [u8; ObjectLength],
        export: [u8; SymbolLength],
    );
    case PeByOrdinal(library: [u8; ObjectLength], ordinal: u16);
    case ElfVersioned(
        object: [u8; ObjectLength],
        symbol: [u8; SymbolLength],
        version: [u8; VersionLength],
    );
}

data Binding<
    const ObjectLength: u64,
    const SymbolLength: u64,
    const VersionLength: u64,
> {
    case DllImport(
        import: DllImport<ObjectLength, SymbolLength, VersionLength>
    );
    case Syscall(number: u64);
    case Firmware(table: FirmwareTable, slot: u32);
    case CompilerIntrinsic;
}
```

Exact cases may grow only when a genuinely different irreducible binding
mechanism exists. Host-specific flags and `host:` mini-languages are not part
of Omega. Foreign struct offsets and bit positions belong to programmable
layout/format declarations, not a generic `Binding::Value` escape hatch.
Privileged target instructions belong to parsed, contract-emitting `asm {}`;
`Binding::Instruction` is retired rather than preserving two ways to state the
same operation with different visibility to effect and authority analysis.

Target packages use ordinary target-scoped machines to compute these values.
The locator is one typed variant, so its object-format coordinates cannot drift
apart, while raw foreign bytes remain honest data rather than Omega names:

```omega
windows_x64 machine WindowsBindings::write_file() -> Binding<12, 9, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "WriteFile",
        },
    }
}

machine Kernel32::write_file(handle: WinHandle, bytes: &[u8]) -> WriteResult
    satisfies Kernel32Requirements::write_file
    via WindowsBindings::write_file();
```

The `via` expression must be compile-time evaluable to a normalized closed
`Binding<ObjectLength, SymbolLength, VersionLength>` application. The const
arguments are ordinary type identity and preserve the exact coordinate widths;
unused coordinates normalize to zero. A quoted literal in one of these
fixed-array positions copies exactly its source bytes, and a width mismatch
rejects. No evaluator reference or dynamically sized byte primitive crosses
the boundary. The binding value is the external-realization variant of the
machine supply slot, not an executable body and not a self-authored trust
assertion. Its identity feeds the derived plan. Structural validation checks
the declaration; admission assigns the trust class and receipt.

The satisfied boundary requirement independently owns its `Calling<C, Policy>`
relationship. Evaluating that policy against the normalized signature produces
the `CallPlan`; the binding mechanism must refine it but never carries or
reselects a duplicate plan.

Binding identity is never reconstructed by looking up text. The complete
evaluated `Binding` is normalized and fingerprinted together with its producer
closure and selected target. For `Binding::CompilerIntrinsic`, the exact
resolved realization-machine symbol, normalized signature, and selected target
key the sealed compiler catalog, so the variant needs no duplicate payload.
For a DLL import, the typed locator variant owns all physical coordinates as one
value; a raw library, export, version, or ordinal is neither an Omega symbol nor
a requirement/provider-selection key. `build.omg` may select the target package
or provider but cannot replace fields inside its evaluated binding.

Validation is variant- and target-specific: it checks required nonempty fields,
forbidden terminators/bytes, ordinal ranges, object-format encoding, target
applicability, unused-coordinate zeros, and any versioning rules before the
binding enters a provider plan. Fixed byte arrays supply ordinary owned values,
not an ambient text interpretation; the locator case supplies their physical
meaning.

The Rust comparator now has the first dependency-light representation rung: a
sealed target-bound normalized locator for atomic PE-by-name, PE-by-ordinal,
and versioned-ELF coordinates. It validates target applicability and basic
coordinate shape and derives a domain-separated, length-prefixed compatibility
identity. Provider import rows and opaque executable-TCB projections now retain
that whole normalized locator, and package-review/manifest output preserves its
target, case, identity, and raw coordinates without rebuilding strings. The
current source evaluator is visibly segregated as a temporary string-backed
bootstrap. Trust artifacts now carry the atomic locator and render exact raw
coordinates without text reconstruction, rejecting target drift before report
installation. The calling bridge, ordinary authored machine validation, object
locator side table, relocation replay, and PE name/ordinal emission retain that
same atomic value. Versioned ELF rows now reach a canonical final-image request
with exact raw object/symbol/version coordinates and relocation sites. The
first dependency-light loader-plan input rung only seals one exact
target/deployment-supplied interpreter pathname for a Linux x86-64 or AArch64
profile. It preserves raw non-UTF-8 bytes, rejects paths that are empty,
relative, or contain NUL bytes, and fingerprints the exact profile and
length-framed bytes.
The first ELF-owner join consumes one exact final image beside that input and
accepts only its nonempty canonical referenced `ElfVersioned` row set under the
same Linux profile. The non-clone carrier privately retains every symbol
handle, raw locator, normalized identity, and relocation site. Target drift,
string-backed or unused interpreter input, and canonical-request failure return
the original image and interpreter unchanged. These carriers grant no loader,
section, publication, or admission authority. Runnable dynamic emission stays
fail closed. The first complete address-free table plan now consumes that
preflight and independently replays an exact NUL-terminated `PT_INTERP`
payload, canonical raw-byte `.dynstr`, the reserved undefined `.dynsym` row
plus one sorted undefined global function row per import, one concrete System V
`.hash`, parallel `.gnu.version`, grouped `.gnu.version_r`, private import-to-
symbol/version indexes, and the exact `DT_NEEDED` string-index roster. Shared
strings, objects, and object/version requirements deduplicate by exact bytes;
permuted import insertion cannot change the table contents or their
deterministic identity. The selected System V hash is sufficient for this
first table plan; a GNU-hash bloom/bucket policy remains separate.

The table invariants follow the primary [System V ABI program-header
rules](https://gabi.xinuos.com/elf/07-pheader.html), [string-table
rules](https://gabi.xinuos.com/elf/04-strtab.html), [symbol-table
rules](https://gabi.xinuos.com/elf/05-symtab.html), and [dynamic hash
rules](https://gabi.xinuos.com/elf/08-dynamic.html#hash-table), together with
the [LSB symbol-version requirement
format](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html).
The plan still grants no loader, layout, publication, or runnable-image
authority. Serialization and section-header placement of the validated
contents, `PT_INTERP` program-header placement, `PT_DYNAMIC`, `.dynamic`
addresses/tags, optional `.gnu.hash`, the selected GOT/PLT arrangement,
`.rela.dyn`/`.rela.plt`, architecture-specific relocation lowering, complete
load/program-header layout, image mutation, and independent final-byte replay
remain open. An owned direct `[u8; N]` destination now contextually
copies a quoted literal into an ordinary raw-byte array only when `N` is a
resolved integer literal and the source byte count matches exactly; non-byte
or unresolved/mismatched widths reject, and hermetic evaluation observes the
array value. Producer closure, evaluator receipt, source `via` evaluation,
specialized string-only adapters, and complete versioned-ELF emission remain
to migrate.

Changing raw foreign bytes changes the normalized binding, forces every final
artifact whose reachable closure contains it to relink, and requires fresh
admission. No parallel endpoint registry or sealed metadata language exists.
Audit reports enumerate the actual evaluated locator rather than a nominal name
that could map elsewhere.

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

There is no parallel source-level primitive-provider registry. The retired
top-level `provider Name : Category;` declaration and operator-local
`provider Name` clause are bootstrap artifacts; requirement declarations do not
select their implementations. Checked satisfiers and `via` leaves declare
candidates, while target defaults, `build.omg`, or installation choose admitted
provider plans through owned slots.

The normalized service schema also retains each linear routed parameter
qualification as a structured entry claim. Its carrier-aware semantic-domain
identity, `accepts` authority-flow verb, and born-strict compiler carry policy
participate in provider-plan identity. The external-root selection bridge
copies those rows beside that identity, and the qualification artifact reports
them with the selected-plan receipt. The row records what an admitted external
entry may supply; only the matching concrete entry receipt establishes a source
fact for one invocation. The durable trust report copies the same normalized
provider-schema claims rather than parsing type displays: exact plan
fingerprint, requirement, parameter/result subject, authority flow, semantic
domain, carry policy, predicate-discharge requirement, and grant provenance.

Checked-adapter dispatch consumes that retained carrier as well. Only an exact
`CheckedAdapter` row in the selected plan may rewrite the corresponding
boundary call; an unrelated or unselected adapter cannot overlay the selection.
Every selected schema method and row carries a nonempty canonical overload
identity. Name-only singleton matching is not a compatibility form: the
readable method name is only a drift check beside exact identity.
Every checked adapter belongs to a nominal provider type. The rewrite retains
the selected entry-state symbol and complete nominal machine name for both
statement and value calls. Standard Console publishes one complete nominal
provider closure per hosted target, with checked
`write`/`write_line` adapters and compiler-intrinsic rows selecting the
target's existing `read_line`, `read_byte`, `write_byte`, and process-exit
lowerings.

The static build root names a boundary service and one exact nominal provider
type, for example
`builder.select_provider<target::Console, SerialConsole>();`. The selection is
harvested only from the authoritative `build.omg` machine, and succeeds only
when that provider's derived candidate exists in the loaded dependency closure,
applies to the selected target, and covers the complete slot schema. Thus the
selection grants neither rows nor trust; it spends the build root's slot-
selection authority over an already-derived and independently admitted
candidate.

The satisfied requirement supplies the public contract, including service
reach, suspension, blocking, and guarded-crash ceilings. The external
realization's behavior is derived from the binding/provider contract and must
refine every ceiling at validation/admission. A `via` machine does not repeat
those clauses.

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
- `suspends`/`blocks` and guarded crash routes are independent
  operation/provider ceilings when applicable.

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

The implemented runtime ledger is an append-only snapshot scoped to one exact
execution domain. Only its Omega-mediation boundary can add an entry, and that
boundary requires pinned executable, provider-plan, implementation-evidence,
and admission-receipt identities; it rejects receipt replay and has no path or
loader-name identity input. Union marks each such entry as a runtime admission.
Without separate executable-closure evidence the entry remains known but adds
an attributed incompleteness cause. With that evidence, the union retains a
complete static scope as complete; evidence remains visible beside unrelated
causes, and repeated union is idempotent.

Build and deployment profiles evaluate the selected entry set, manifest
completeness and evidence, required containment guarantees, and approved
platform or third-party identities. Platform baselines are policy allowlists,
not different language semantics. Development profiles may admit and mark an
incomplete artifact; safety profiles fail before artifact installation when
their requirements are not met. An isolated provider is an endpoint in the
caller's manifest and receives its own executable manifest for its execution
scope.

The implemented isolated-scope carrier makes that separation structural. A
selected closure is assigned a nonzero isolated scope before opaque admissions;
the manifest-set admission then binds the child's exact manifest and admission
receipt to one exact endpoint entry in the parent. Endpoint containment stays
on the parent entry, while every child entry and its completeness result remain
under the child scope. Scope drift, duplicate child scope identity, and mixed-
scope child entries reject. Parent and child profiles are evaluated separately.

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

The first concrete outbound realization is conservative by mechanism. Every
returning import or indirect vtable/table call receives an aligned trampoline
that saves and restores the caller's complete MXCSR/FPCR around the existing
call sequence. Direct syscalls do not execute a returning user-space
counterparty and receive no envelope. The target call layout remains unchanged
inside the trampoline, and relocation planning rebases its existing fixups by
the exact target prefix width. An admitted per-binding preservation proof may
later select a zero-envelope optimization.

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
that requirement. The registration operation declares
`where machine Selected satisfies Trait::requirement`; the nominal requirement
supplies the complete signature and contract without structural repetition.
Passing the selected machine chooses its explicit satisfaction row, validates
the published and actual refining envelopes plus their `CallPlan + StatePlan`,
and lets the compiler materialize the foreign ABI thunk and relocation inside
that exact binding. The registrar's evaluated outbound `CallPlan` carries one
normalized callback-materialization row per nominal callback binder. Each row
maps the registrar's binder-slot identity, not its later selected-machine
argument, to one declared `NativePlace`: either a direct native parameter or a
field projected through a validated native layout. The plan fingerprint is
therefore fixed across callback selections; the per-use row separately retains
the selected machine, satisfaction, entry plan, and private thunk identity, and
lowering joins those identities only when emitting the private relocation.
Signature coincidence and unique visibility are not
selection rules. A signature-free requirement path must resolve uniquely or
reject, consistently with domain `established by` clauses. The source surface
does not need
a general function-pointer value.

A projected native callback field is a typed private-materialization demand in
the normalized layout plan, not a field of the source-visible specification.
The target package declares its stable identity as an explicitly named
`Layout satisfies PrivateCallbackSlot<Trait::requirement>` conformance, and the
layout policy must cite that exact conformance in its private placement entry.
The declaration alone is inert: layout evaluation never enumerates visible
conformances, and ordinary third-party evidence cannot inject a demand into an
existing plan. The subject supplies exact layout identity while the static
argument supplies one signature-free callback-requirement path; overload
ambiguity rejects.

Layout validation records the conformance-owned slot identity, exact callback
requirement, and target-closed placement independently. Complete outbound-plan
validation requires every such demand to be supplied exactly once by a
compatible callback-materialization row. Missing, duplicate, wrong-layout,
wrong-requirement, overlapping, shape-incompatible, or unresolved demands
reject. Source cannot read, write, serialize, or address the field. The
authoritative layout may author or compute its physical offset, but neither
binder order nor a repeated byte offset is a calling-plan placement rule.
Changing only the selected callback changes per-use/thunk identity; changing
the evaluated offset changes target-realization and artifact identity while
the target-neutral requirement declaration remains stable.

Callback placement does not own native-argument storage lifetime. Direct
arguments, call-scoped staging, retained pointees, snapshots, and stable roots
remain ordinary outbound calling-plan and foreign-storage dispositions. A
common copying registrar needs only call-scoped staging; an API that retains a
caller-supplied native object must satisfy the general retained-storage rules
below. The callback row records only binder slot and destination.

Durable registration returns an ordinary linear package value. It owns the
foreign registration and any code/component lease needed to keep the entry
valid; its explicit terminal operation unregisters before releasing those
obligations. Call-scoped callback parameters remain borrowed for the call.
Foreign context storage carries an inert protocol token or generational handle,
while the owning state remains in an Omega registry or another ordinary
package-owned value. The registration occurrence retains the selected machine
in provenance, but possession alone imports no narrower implementation facts;
an API forwards any caller-visible guarantee explicitly.

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

That selected execution stack is only one part of root provisioning. Each
installed callback root also carries every admissible arrival context and its
finite entry/body/exit epoch sequence. Epochs retain the active domain,
per-domain occupancy, and phase-specific nesting allowance, so a software
stack switch divides the sequence while an atomic hardware switch does not.
Terminal-Psi WCSU joins only the body execution domain. Architectural arrival
comes from a sealed target rule applied to installed facts; emitted adapters
are derived from their bytes; opaque adapters require admitted evidence.

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

1. **Call-scoped:** an ordinary `&T`, `&[T]`, `&write T`, `&write [T]`,
   `&mut T`, or `&mut [T]` permits only its exact access set before the call
   returns.
2. **Retained after return:** the public requirement states the caller-visible
   lifetime/custody contract. An ordinary linear protocol value such as
   `PendingRead` or `Registration<Storage>` may own stable storage; a
   lifetime-parameterized `Registration<'a>` may retain a checked borrow; or a
   realization may hide stricter native retention behind a private stable
   snapshot when a semantic snapshot contract permits copying. A terminal
   completion redeems public custody and releases private backing.
3. **Process-lifetime:** the authority moves into an already-established static
   or process-lifetime root. Omega has no general permanent-custodian spelling;
   other permanent retention remains unsupported until a concrete customer
   justifies one.

Post-return retention is never an untracked extension of a call-scoped borrow.
A lifetime-parameterized protocol value may carry an explicit checked loan;
otherwise its linear claim owns the keepalive and reclamation authority for its
backing place, not necessarily the bytes inline. It may lend ordinary lexical
views over rights the foreign side does not hold. A read-only foreign operation
can therefore preserve semantic facts and lend Omega read views; a writing
operation invalidates facts over exactly the writable extent and re-establishes
them from terminal completion evidence.
Separated partial release is an ordinary split in the claim-content algebra:
the returned subextent leaves flight while the disjoint remainder stays under
the same protocol claim.

The compiler learns that use survives return from the published result contract
and ownership conservation. A consumed `Buffer` may map into the content
retained by `PendingWrite`; `submit(&buffer) -> PendingWrite` rejects when the
unparameterized result claims owned retention, while a result explicitly
parameterized by the borrow lifetime may carry that checked loan.
Unambiguous consumed-input-to-produced-claim mappings are inferred; ambiguous
or unsupported mappings reject unless an ordinary postcondition pins the
correspondence. A content-bearing exact qualification supplies its projection
through its owner-unique core `Content<A>` conformance; the binding does not
invent a separate foreign-extent algebra or projection annotation.

Every pointer-valued native slot retained after return carries checked
provenance to an exact stable root, range, access mode, lifetime, and any
revision or lease. Unknown provenance rejects until an admitted provider route
establishes it. Embedded nested layouts have a finite structural closure;
recursive or dynamically sized pointer graphs instead retain one arena/extent
root covering the graph rather than asking the compiler to traverse runtime
pointers. A private snapshot is legal only under an explicit semantic contract
that permits an independent value copy and requires neither identity
preservation nor unchecked write-back. Concurrent foreign and Omega access is
External placed backing; exclusive foreign mutation may instead move storage
into the protocol value and return it under the requirement's declared
preserved, invalidated, or outcome-dependent content qualification.

Provider-specific backing never changes a separately compiled public result
type. Requirements publish unavoidable caller-visible lifetimes and custody;
realizations record and validate their concrete backing recipes. Private
snapshot bytes count as persistent demand per live protocol occurrence. A
static aggregate bound therefore also requires a finite live-occurrence
capacity authority: success moves the exact authority into the registration,
rejection returns it unchanged, and successful unregister returns the same
occurrence. A consumable lifetime budget is a different authority. Static thunk
code is bounded separately by distinct admitted callback identities, not by the
number of simultaneously live registrations.

The executable checker admits the unique compatible consumed input. When
several compatible owned inputs exist, one exact authored equality may select
the source by relating the whole entry projection of that parameter directly to
the whole current result projection in the same content algebra. Partition
equations and structural subplaces do not select custody. Borrow-only retention
still rejects even if a content equality names the borrow.

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

Terminal Psi retains that correlation on successful bodyless boundary calls as
an exact completion-receipt row `(operation, boundary, argument position,
claim)`. Verification reconstructs the complete live-claim set for every
consumed argument and rejects missing, extra, duplicate, reordered, or
cross-argument receipts. Interpretation and native realization bind the same
rows to the admitted provider execution. A rejected provider effect records no
receipt and consumes no custody.

The compiler canaries pin both halves of this ownership split. A synchronous
fixed-array pointer import releases its ordinary source loan before the next
owner mutation. Retained custody cannot originate from a borrow; the accepted
round trip consumes an owned buffer into one linear pending claim and permits
only a terminal completion consuming that claim to re-establish buffer custody
under the same `Content<A>` algebra. Provider-owned storage uses the ordinary
receiver-borrow path when all invalidators require exclusive receiver access:
invalidation after the view's last use passes, while invalidation before a
later use rejects. Providers with independent invalidation instead return the
view from an explicit linear validity claim; the invalidator consumes that
claim, with the same last-use acceptance and live-view rejection.

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

`&write T` is the call-scoped provider-writes-only form. It borrows one existing
valid `T` exclusively and permits mutation without observation. It is not an
output/construction slot, never covers `Vacant` storage, and creates no durable
custody transfer. A mutable borrow may attenuate explicitly to it; the callee
cannot derive `&T` or `&mut T`, take or swap old content, perform
read-modify-write, or call a helper with broader access.

For a byte-producing operation, the outcome contract names the exact modified
prefix or other write footprint. The untouched suffix is unchanged, so caller
facts over that suffix survive; the returned count does not establish a value
that was absent at entry. Each replacement separately requires freely
discardable displaced content and preservation of the referent's validity.
Partial writes through structured `T` are therefore accepted only when validity
follows from static structure, written inputs, and deliberately supplied facts
without loading the referent.

Checked Omega providers enforce non-observation transitively through their
entire call closure. An opaque provider physically receiving an address may
still read it; the selected provider evidence admits compliance unless target
isolation enforces it. Artifacts retain the write-only mode and exact outcome
write frame rather than widening the call to read/write. Identity-only
retention remains an ordinary stable keepalive claim that lends no memory view.
Storage with no live `T` and typed foreign construction are separate future
features rather than alternate meanings of `&write`.

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
context-indexed stack epochs, nesting relation, and version pins until its
linear registration proves
unregistration and required quiescence. A process-lifetime statically linked
callback needs the same build report but no live replacement ledger. This
reuses provider admission rather than creating an entry-specific trust system.

## Process entry

Process entry is one required environment-to-program root slot in a target
profile. The program binds an exact semantic source machine while the target
fixes the separate physical arrival contract, calling policy, bootstrap
adapter, provider setup, physical-result map, and source-visible entry shape.

```omega
machine start() {
    Console::write_line("Hello, Omega.");
}

machine build(builder: &mut Build) {
    builder.target = windows_x86_64;
    builder.roots.bind(
        windows_x86_64::ProgramEntry,
        start
    );
}
```

On Windows the generated stub may read native command-line/environment
surfaces; on ELF it may read the initial stack; on firmware its exact physical
requirement may receive a firmware handoff. Those details stay in scoped
providers and the target-authored bootstrap behind a generated ABI shell.
Native handles remain typed physical inputs rather than semantic storage roots.
Target selection and semantic slot binding belong in `build.omg`, not a
target-specific source dialect or a `main` naming convention.

A hosted schema normally hides raw image and storage roots. If the bound entry
has one `&mut self` receiver, the bridge provisions exactly one ZII-valid
instance beneath an admitted storage root and lends it only to that activation.
A freestanding schema may deliberately expose `image: Extent in Granted` and
`initial_storage: Extent in Granted` because provisioning is then the
application's responsibility. A separate semantic installation edge introduces
those exact occurrences after the bootstrap establishes their evidence. The
combined shell and adapter remain the installed external root in both cases and
contribute their complete derived contract.

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
ordinary library/API engineering. Provider override binds one target-owned
typed slot to the exact satisfier or complete named conformance demanded by
that slot; equivalent scoped APIs for tests and replaceable-realization owners
remain engineering work, not an open grammar question. "Binding" here is
build/artifact state, not a source `slot` construct.

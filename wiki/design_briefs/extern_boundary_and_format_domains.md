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
selected provider; it does not reconstruct its rows.

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

Checked-adapter dispatch consumes that retained carrier as well. When a slot has
a selected plan, only an exact `CheckedAdapter` row in that plan may rewrite the
corresponding boundary call; an unrelated free adapter cannot overlay the
selection. The rewrite retains the selected entry-state symbol and complete
nominal machine name for both statement and value calls. A free adapter remains
a temporary compatibility fallback only when the slot has no selected source
provider; standard Console will remove that fallback when its target packages
publish complete provider closures over the native byte leaves.

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

## Effects, authority, and trust

Decision 22 applies without an extern exception:

- the boundary-trait identity contributes service reach;
- capability/evidence values carry authority;
- the selected external provider produces a trust receipt; and
- `suspends`/`blocks` are independent operation/provider ceilings when
    applicable.

A checked wrapper may refine operational behavior or reduce trust expenditure;
it does not erase the abstract service reach from callers compiled against that
trait.

An opaque in-process executable provider is part of the artifact's trusted
computing base. Checked ABI adaptation, lifetime accounting, and stack
containment do not prevent that binary from modifying arbitrary process memory.
The selected-provider manifest retains its exact identity and trust receipt.
Process- or hardware-isolated providers instead remain external endpoints.
The root declaration and build-profile rejection surface for these transitive
trust dependencies remains owner question #7.

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

## Foreign execution placement and stack accounting

The binding package selects where an opaque call executes; the compiler does
not guess from a DLL name or signature.

```text
direct
    foreign frames continue the current activation's stack chain
    an admitted foreign ceiling enters that StackPlan

gateway/component
    the caller accounts for its checked local stub
    the provider owns a separately provisioned native stack

isolated
    the call crosses a process or hardware protection boundary
```

A boundary requirement's resource ceiling is not evidence that an opaque
implementation fits it. Checked Omega realizations derive WCSU. A native
binding needs admitted foreign demand, or an enforced guarded capacity whose
overflow remains an abnormal-exit route rather than proof of successful
completion. Trust composes by the weakest input and reports the exact foreign
premise.

A hosted gateway is an ordinary boundary provider backed by a bounded native
worker resource. Reaching its submission safe point does not bound native
completion, cancellation finalization, retained-loan release, or later gateway
admission. Pool/queue/backpressure and failure-domain semantics remain owner
question #5. Retained pointer custody remains owner question #1.

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

Hosted callback entry may continue on the provider stack, preflight its
remaining capacity against the exact Omega WCSU and target reserve, or enter a
target-supported owned stack. Preflight proves the predicted segment fits; an
owned hard-limited stack additionally detects underestimation at its own
boundary. Foreign calls made by a separated-stack callback return to the
provider stack domain before entering opaque code.

Native protocols may synchronously re-enter application callbacks. A platform
adapter defines a safer handler requirement, classifies which of its own
operations can re-enter, and checks each ordinary Omega handler's inferred
reach locally. It may answer synchronous platform queries through restricted
handlers and queue ordinary application events until the outermost native
dispatch returns. This package-local construction does not require inferring
the provider's internal call graph or a general higher-order callback-cycle
analysis.

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

## Foreign pointer cases

Foreign pointers fit four contract shapes:

1. **Borrowed out:** Omega-owned storage is lent to a foreign call for the
   declared call duration.
2. **Borrowed in:** foreign-owned storage is exposed through a lifetime- and
   provenance-bounded view supplied by the boundary.
3. **Callback entry:** a compiler-generated entry stub validates the calling
   plan, reconstructs typed context, and enters an Omega boundary machine.
4. **Opaque handle:** foreign identity remains an uninspectable capability whose
   operations stay behind the boundary trait.

Raw address arithmetic is not a fifth user-facing escape hatch. Pointer access
must remain attributable to one of these ownership/provenance contracts.

Borrowed-out is specifically the synchronous, non-retaining case. A checked
adapter may derive the foreign pointer and length from a safe slice, but the
borrow ends with that native call. A foreign API that retains the pointer,
completes asynchronously, or stores it for later callbacks requires an explicit
pinned loan, ownership transfer, or registration protocol.

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
retains its effects, authority/trust receipts, state footprint, stack domain,
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
3. Validate provider admission and emit trust/boundary reports.
4. Lower imported calls and inbound stubs from checked plans only.
5. Integrate programmable layout validation/materialization.
6. Add final-artifact state-footprint validation and external-root reporting.
7. Add callback and foreign-pointer lifetime canaries.
8. Delete host-string special cases and legacy target blocks.

## Still open

- retained foreign borrows and their completion/revocation receipts
  (`OWNER_QUESTIONS.md` #1);
- dynamic-library loading/unloading under component versioning;
- transitive root visibility and profile rejection for opaque in-process
  executable providers (`OWNER_QUESTIONS.md` #7);
- contained execution failure with outstanding obligations
  (`OWNER_QUESTIONS.md` #8); and
- target-specific launch/exit details not covered by existing calling plans.

Exact `Build` library method names for choosing a target profile remain
ordinary library/API engineering. Per-requirement provider override has settled on
`select_provider<BoundaryTrait, ProviderType>()`; equivalent scoped APIs for
tests and replaceable-realization owners remain engineering work, not an open
grammar question. "Binding" here is build/artifact state, not a source `slot`
construct.

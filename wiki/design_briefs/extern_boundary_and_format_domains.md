# Design Brief: Extern Boundaries And Foreign Formats

Current as of 2026-07-18. This brief defines the durable extern model. Concrete
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

The static build-root spelling is
`b.select_provider<BoundaryTrait, ProviderType>();`. Both arguments are types,
not static machine parameters. The declaration is harvested only from the
authoritative `build.omg` machine, and selection succeeds only when that
provider's own derived candidate exists in the loaded dependency closure,
applies to the selected target, and covers the complete slot schema. Thus the
marker grants neither rows nor trust; it spends the build root's slot-selection
authority over an already-derived and independently admitted candidate.

The satisfied requirement supplies the public contract and effect ceiling.
The external realization's behavior is derived from the binding/provider
contract and must refine that ceiling at validation/admission. A `via` machine
does not repeat an authored `effects` row.

## Effects, authority, and trust

Decision 22 applies without an extern exception:

- the boundary-trait identity contributes service reach;
- capability/evidence values carry authority;
- the selected external provider produces a trust receipt; and
- `Suspend`/`Block` are operation/provider contract members when applicable.

A checked wrapper may refine operational behavior or reduce trust expenditure;
it does not erase the abstract service reach from callers compiled against that
trait.

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

## Foreign data and formats

Foreign layout is expressed by authored programmable layout policies built from
compiler-known placement primitives. Plain `data` supplies the semantic shape;
layout policy supplies the foreign byte representation. There is no separate
`wire data` species and no compiler-owned catalog of every external format.

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

Every installed callback/interrupt entry is also an external artifact root.
Because no Omega call edge reaches it, the root ledger must include its effects,
authority/trust receipts, state footprint, stack domain, nesting relation, and
version pins. Static build plans declare roots during image derivation; dynamic
admission records them at installation. This reuses provider admission rather
than creating an entry-specific trust system.

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

- callback registration/revocation and long-lived foreign borrows;
- dynamic-library loading/unloading under component versioning; and
- source-visible reified entry references (deferred until dynamic callbacks
  demonstrate a need beyond build/provider selection); and
- target-specific launch/exit details not covered by existing calling plans.

Exact `Build` library method names for choosing a target profile remain
ordinary library/API engineering. Per-slot provider override has settled on
`select_provider<BoundaryTrait, ProviderType>()`; equivalent scoped APIs for
test and component slot owners remain engineering work, not an open grammar
question.

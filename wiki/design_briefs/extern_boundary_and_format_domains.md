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

A boundary-provider policy maps each requirement to a binding description and
returns a candidate `ProviderPlan<Service>`. Provider definition is independent
of target/profile selection; see
[`provider_plans.md`](provider_plans.md). The binding mechanism is a closed sum,
not a growing family of keywords:

```omega
data Binding {
    case DllImport(library: LibraryId, symbol: SymbolId, plan: CallingPlanId);
    case Syscall(number: u64, plan: CallingPlanId);
    case Firmware(table: TableId, slot: u32, plan: CallingPlanId);
    case CompilerIntrinsic(name: IntrinsicId);
}
```

Exact cases may grow only when a genuinely different binding mechanism exists.
Host-specific flags and `host:` mini-languages are not part of Omega. The
implemented `<target> provides <Trait> { ... }` declaration is retired: it
fused provider authorship to selection and accumulated non-binding constants.
Ordinary provider policies plus build/profile selection replace it.

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

ABI behavior is a normalized calling-plan policy: parameter/result placement,
register classes, stack layout, clobbers, unwind/control behavior, and target
applicability. It is not inferred from library or symbol strings.

Bindings cite a plan identity. Provider admission verifies that the binding and
entry stub implement the pinned boundary-machine contract. See
[`calling_plans.md`](calling_plans.md).

Provider plans follow construct -> validate -> admit -> select. Structural
validation proves coverage and compatibility; authorized admission accepts the
foreign semantic claim and grants receipts; the owner of a service slot selects
only among admitted candidates. Authoring a plan can never self-grant trust.

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
2. Land source-expressible `BoundaryProvider<Service>` and `ProviderPlan`
   policy data with a closed `Binding` vocabulary.
3. Move target defaults into platform-profile packages and make `build.omg`
   selection/overrides normalize to explicit service-slot assignments.
4. Validate provider admission and emit trust/boundary reports.
5. Lower imported calls and inbound stubs from checked provider + calling plans
   only.
6. Integrate programmable layout/format validation and remove hand-numbered
   foreign representation constants from provider tables.
7. Add callback and foreign-pointer lifetime canaries.
8. Delete `provides`, host-string special cases, and legacy target blocks.

## Still open

- final `ProviderPlan`/`Binding` source data and profile-library ergonomics;
- callback registration/revocation and long-lived foreign borrows;
- the exact accepted-proof surface for hand-authored providers;
- dynamic-library loading/unloading under component versioning; and
- target-specific launch/exit details not covered by existing calling plans.

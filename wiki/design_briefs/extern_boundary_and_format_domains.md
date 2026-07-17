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

A `ProviderPlan` maps each requirement to a binding description. Provider
plans are ordinary build-time policy values; there is no `provides` keyword.
The binding vocabulary is a sum, not a growing family of keywords:

```omega
data Binding {
    case DllImport(library: LibraryId, symbol: SymbolId, plan: CallingPlanId);
    case Syscall(number: u64, plan: CallingPlanId);
    case Firmware(table: TableId, slot: u32, plan: CallingPlanId);
    case CompilerIntrinsic(name: IntrinsicId);
}
```

Exact cases may grow only when a genuinely different binding mechanism exists.
Host-specific flags and `host:` mini-languages are not part of Omega.

Provider handling has four distinct stages: construct a candidate freely;
validate structural coverage, signatures, calling/layout plans, and normalized
identity deterministically; admit its semantic claims under boundary grant
authority and issue receipts; then select it for a slot under that slot owner's
capability. A target package supplies ordinary defaults, `build.omg` selects a
default target profile, and explicit build/test/component configuration may
override individual slots. Defaults are package data, not compiler magic.

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
2. Represent `Binding` as resolved target/provider data.
3. Validate provider admission and emit trust/boundary reports.
4. Lower imported calls and inbound stubs from checked plans only.
5. Integrate programmable layout validation/materialization.
6. Add callback and foreign-pointer lifetime canaries.
7. Delete host-string special cases and legacy target blocks.

## Still open

- final `ProviderPlan` construction/selection grammar;
- callback registration/revocation and long-lived foreign borrows;
- dynamic-library loading/unloading under component versioning; and
- target-specific launch/exit details not covered by existing calling plans.

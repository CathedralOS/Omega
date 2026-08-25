# Whole-Program Assumptions

Omega today is a WHOLE-PROGRAM compiler: one entrypoint, one fused lowering,
one direct executable image. That is the right shape for the current phase.
But the language's first large consumer (`wiki/cathedral_alignment.md`) needs
provider realizations that can be compiled, signed, shipped, loaded, and
hot-swapped independently
([Versioned Data](../language_guide/chapter_22_versioned_data.md)).

This page exists so the whole-program assumption is a TRACKED decision per
backend layer instead of an ambient default that silently deepens. The rule:
when a new backend mechanism bakes in "I can see the whole program" or "I can
use an absolute address," note it here. Nothing needs to be component-ready
today; everything needs to be FINDABLE when component compilation becomes
real.

## Current whole-program dependencies (the rework inventory)

- **One global runtime frame region.** Every machine's locals/params/call
  scratch live in a single `omega_runtime_frame_storage` blob; slots get
  absolute region offsets at plan time (`stack_runtime_storage_by_call_context`
  in omega-backend-pipeline). Per-component compilation needs per-component
  regions or a base-register discipline.
- **One fused dispatch loop.** All dispatched states across all machines
  compile into a single dispatch loop with dense global case indices
  (`dispatch_index` = state arena index). A swapped-in machine cannot add
  cases to a fused loop.
- **Per-call-context monomorphization spans machine boundaries.** Dispatch
  specialization clones callee states per call context across the whole
  program; a component boundary would cut those clones.
- **Global relocation/data planning.** String/data addresses and the
  relocation set are computed against one final image; no per-component
  object format exists (`.o` emission is explicitly a debug bridge per the
  repository layout doc).
- **Symbol identity is arena-index-based per compilation.** Handles are not
  stable across separate compilations; cross-component references would need
  exported symbolic identity (the layering docs already point this
  direction: names as edge metadata).
- **Entry selection, host-provider wiring, and the boundary registry** are
  resolved once for the whole image.

## What stays true regardless

- Direct image construction (no external linker) remains the bet; component
  loading would extend the image/loader machinery, not abandon it.
- The machine/state graph as the unit of behavioral contract is unchanged.
  A replaceable component is not one arbitrary machine or one package: it is a
  selected provider realization plus the closed code/state/resource graph that
  the realization owns.
- Package-shaped component closures are a valid first implementation fence,
  not the definition of component. Removing that fence must only admit more
  closures without changing already-accepted programs.

## Settled component shape and implementation gaps

[^artifact]: A component capsule targets one exact closed requirement
application and contains canonical Terminal Psi, reconstructed obligation
evidence, symbolic imports/exports, lifecycle and resource demands,
target-semantics dependencies, and optional target-native realizations with
their refinement evidence. The concrete encoding, content addressing, mapping
lifetime cohorts, and loader representation remain implementation work.

[^abi]: Crossings name exact requirement identities with evaluated calling,
state, representation, entry, and observation plans. Runtime call authority is
an explicit routed `Service<R> in Bound`, not a bare trait value or public
vtable. The concrete entry-acquisition algorithm, stack ownership, dispatch
handoff, and binary encoding remain implementation work.

[^loader]: Loader/linker responsibilities (who patches what at load time, how
content-addressed code dedup works in memory) belong to the consumer OS's
design and are out of scope here. Omega owes the verified capsule, local
acceptance envelope, linear installation/era transitions, and replayable
deployment record; Cathedral chooses mappings, cohorts, scheduler/device
quiescence, rollback, and the irreducible update nucleus.

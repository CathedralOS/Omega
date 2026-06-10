# Whole-Program Assumptions

Omega today is a WHOLE-PROGRAM compiler: one entrypoint, one fused lowering,
one direct executable image. That is the right shape for the current phase.
But the language's first large consumer (`wiki/cathedral_alignment.md`) needs
components that are compiled, signed, shipped, loaded, and hot-swapped
INDEPENDENTLY, with machines as the swap boundary
([Versioned Data](../language_guide/chapter_21_versioned_data.md)).

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
- The machine/state graph as the unit of contract is unchanged -- machines
  are already the planned swap points; the work is making their COMPILED form
  self-contained enough to load.

## Footnotes / unknowns

[^artifact]: The component artifact format is undesigned: what a compiled,
signed, loadable Omega component contains (code, layout report, boundary
provider manifest, wire schemas, version/migration tables, authority-flow
report) and how it is content-addressed.

[^abi]: The cross-component ABI granularity is undesigned: presumably "machine
entry + versioned data layouts + wire schemas," but calling convention,
dispatch handoff, and frame ownership across a component edge are open.

[^loader]: Loader/linker responsibilities (who patches what at load time, how
content-addressed code dedup works in memory) belong to the consumer OS's
design and are out of scope here; the language side only owes a loadable
artifact.

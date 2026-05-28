# Repository Layout

This page is the architecture home for the workspace hierarchy currently
sketched in the root README.

The short version:

```text
Omega/
|-- apps/                 # user-facing CLI, language server, docs tooling
|-- compiler/
|   |-- foundation/       # small shared primitives, arenas, diagnostics, source ids
|   |-- frontend/         # concrete/lossless syntax-facing tooling
|   |-- packages/         # package manifests, loaders, dependency graphs
|   |-- semantics/        # name/type/effect/proof/borrow/domain validation
|   |-- representations/  # durable IR data structures
|   |-- pipeline/         # representation-to-representation transforms
|   |-- backend/          # target, layout, ABI, selection, object/image/linking
|   |-- runtime/          # runtime contracts, memory, startup, host shims
|   |-- orchestration/    # compiler session, artifacts, phase sequencing
|   `-- tool_support/     # IDE and documentation support
|-- omega/                # bundled Omega core/std/host packages
|-- target_runtime/       # future linkable runtime payloads
|-- samples/              # language pressure tests
|-- canaries/             # focused pass/fail/run compiler canaries
|-- tests/                # integration, target, bootstrap tests
`-- wiki/                 # language and architecture notes
```

## Placement Rules

- `representations/` owns durable arena-shaped data. It should not know the
  orchestration story.
- `pipeline/` owns transforms from one representation to the next. A pipeline
  crate may depend on input and output representations, but should not become
  the permanent home for shared concepts.
- `semantics/` owns language meaning: names, types, effects, proof facts,
  ownership, domains, contracts, validation, and diagnostics.
- `backend/` owns machine-facing meaning: layout, ABI, calling conventions,
  target operations, instruction selection, object/image emission, and linking.
- `orchestration/` sequences stages and writes artifacts. It should not absorb
  phase implementation logic.
- `omega/` contains source-visible language packages. Compiler-private lowering
  details should sit behind boundary surfaces, not leak into ordinary core APIs.

When a concept is needed by several stages, prefer a small shared semantic
representation over copy-pasted local structs. When a concept only exists during
one transform, keep it local to that transform until repetition proves otherwise.

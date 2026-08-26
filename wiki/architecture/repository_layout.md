# Repository Layout

This page is a map and a placement guide. It should help answer two questions:

- Where should a new crate or module live?
- Which layer is allowed to own a concept?

The pipeline-specific semantic rules live in
[Pipeline Architecture](pipeline/pipeline.md).

How Omega builds *itself*—the trust architecture and the exact
`Alpha → Beta → Gamma → Delta` language spine—is a separate ownership domain described by
[The Bootstrap Lattice](bootstrap_lattice/bootstrap_lattice.md) and its
[target repository structure](bootstrap_lattice/repository_structure.md).

## Design Bias

- Prefer feature-first crates with explicit names.
- Keep durable IR structs in `representations/`.
- Keep transforms in `pipeline/`.
- Keep language meaning in `semantics/`.
- Keep target, ABI, layout, object, linker, and image details in `backend/`.
- Keep orchestration boring: sequence phases, write artifacts, do not absorb phase logic.
- Do not add a crate until a module boundary has stopped moving.

Legend:

- `[CRATE]` means a Cargo workspace package.
- Unprefixed folders are ordinary source/module boundaries inside a crate.

This tree is a conceptual placement map anchored in the current Rust on-ramp;
it is not an exhaustive generated inventory of Cargo workspace members. Some
sub-areas named in the placement prose are not yet separate crates, while small
implementation crates may appear in the workspace before this map names them.
`packages/`, `runtime/startup`, `tool_support/`, and several backend
object/linker/image writers remain placement intent rather than current
packages.

> **Ownership boundary.** The current Cargo implementation is explicitly an
> external-language producer under `bootstrap/onramps/omega-rust/`. Its `psi/`
> half implements parsing and target-neutral semantics through terminal Psi;
> its `omega/` half implements provider, ABI, target, artifact, and execution
> machinery. `compiler/{psi,omega}/` owns Omega-written product source; the
> first Psi lexical checkpoint has landed while later phases remain open.
> Bootstrap gates resolve cross-owner locations through the
> role manifest in `bootstrap/paths.sh`; new cross-owner sibling-relative paths
> are rejected. The tree below documents the current Cargo/product structure;
> the canonical bootstrap inventory is documented in the
> [bootstrap repository structure](bootstrap_lattice/repository_structure.md),
> while active lattice work is tracked in
> [TASKS_BOOTSTRAP.md](../../TASKS_BOOTSTRAP.md).
>
> Human-facing reports, HTML visualizations, interpreters, REPLs, and debug
> viewers shown in the Rust development tree are optional tooling, not members
> of the hosted compiler closure unless the product executable imports them.
> Their existence here does not create bootstrap implementation work.

```text
Omega/
|-- Cargo.toml
|-- README.md
|-- bootstrap/onramps/omega-rust/
|   |-- apps/
|   |   `-- [CRATE] omega-cli/                            # Current Rust `omega` development command.
|   |
|   |-- psi/                                         # Psi owns target-neutral semantics through terminal Psi.
|   |   |-- foundation/
|   |   |   |-- [CRATE] psi-access-plans/               # Normalized placed-view access semantics.
|   |   |   |-- [CRATE] psi-arena/                      # Generic typed arena storage for Psi source representations.
|   |   |   |-- [CRATE] psi-diagnostics/                # Source diagnostics and phase-snapshot contracts.
|   |   |   |-- [CRATE] psi-extents/                    # Extent geometry, lineage, rights, and provider identity.
|   |   |   |-- [CRATE] psi-language-core/              # Target-neutral source-language vocabulary.
|   |   |   |-- [CRATE] psi-language-semantics/         # Resolved identities, content/value domains, tables, and plans.
|   |   |   |-- [CRATE] psi-layout-plans/                # Normalized layout geometry and materialization plans.
|   |   |   |-- [CRATE] psi-numerics/                   # Exact numerics, float semantics, and literal payloads.
|   |   |   |-- [CRATE] psi-source/                     # Loaded-source data and coordinates owned by the Psi frontend.
|   |   |   |-- [CRATE] psi-source-loader/              # Root-file loading into Psi-owned source maps.
|   |   |   |-- [CRATE] psi-symbols/                    # Stable symbol identities and hierarchy storage.
|   |   |   `-- [CRATE] psi-core/                       # Stable semantic/fuel ids and typed proposition vocabulary.
|   |   |-- representations/
|   |   |   |-- [CRATE] psi-tokens/                     # Omega spelling-level token streams.
|   |   |   |-- [CRATE] psi-syntax-trees/               # Parsed source shape before symbol resolution.
|   |   |   |-- [CRATE] psi-symbol-resolved-trees/      # Source trees with resolved symbol identity.
|   |   |   |-- [CRATE] psi-typed-trees/                # Typed source semantics without target realization state.
|   |   |   |-- [CRATE] psi-facts/                      # Durable checked places, contexts, and semantic fact plans.
|   |   |   |-- [CRATE] psi-effects/                    # Target-neutral operational, reach, invocation, and capability-flow facts.
|   |   |   |-- [CRATE] psi-checked-trees/              # Checked proof, borrow, flow, reach, value, and admissibility evidence.
|   |   |   `-- [CRATE] psi-terminal/                   # Self-contained terminal module and closed operation vocabulary.
|   |   |-- pipeline/
|   |   |   |-- [CRATE] psi-source-files-to-tokens/     # Psi-owned Omega source lexer.
|   |   |   |-- [CRATE] psi-tokens-to-syntax-trees/     # Psi-owned unresolved Omega parser.
|   |   |   |-- [CRATE] psi-syntax-trees-to-symbol-resolved-trees/ # Psi-owned name and symbol resolution.
|   |   |   |-- [CRATE] psi-symbol-resolved-trees-to-typed-trees/ # Psi-owned type/signature normalization.
|   |   |   |-- [CRATE] psi-typed-trees-to-checked-trees/ # Psi-owned semantic checking and checked-fact construction.
|   |   |   `-- [CRATE] psi-checked-trees-to-terminal/   # Fail-closed executable slice plus checked content-evidence production.
|   |   `-- semantics/
|   |       |-- [CRATE] psi-types/                      # Unresolved source type-surface analysis.
|   |       |-- [CRATE] psi-validation/                 # Cross-semantic source validation and diagnostics.
|   |       |-- [CRATE] psi-proof/                      # Source proof obligations, planning, and checking.
|   |       |-- [CRATE] psi-proof-kernel/               # Current name; product-local Psi admission, with rename tracked in TASKS.md.
|   |       |-- [CRATE] psi-checked-interpreter/        # Checked-tree build-time and transitional reference execution.
|   |       |-- [CRATE] psi-build-time-evaluation/      # Const/domain evaluation and programmable plan normalization.
|   |       |-- [CRATE] psi-terminal-semantics/         # Closed scalar, structural/effect, and call-composition policy rows.
|   |       |-- [CRATE] psi-terminal-verifier/          # Module validation and reconstructed-obligation checking.
|   |       `-- [CRATE] psi-terminal-interpreter/       # Fuel-bounded reference execution of verified terminal artifacts.
|   |
|   `-- omega/                                           # Omega owns target realization and native emission.
|       |-- foundation/
|       |   `-- [CRATE] omega-core/                         # Omega execution/build utilities.
|       |
|       |-- representations/
|       |   |-- [CRATE] omega-effects/                      # Omega provider bindings, selection, and admission.
|       |   |-- [CRATE] omega-state-graph/                  # Explicit machine/state graph for proof and scheduling.
|       |   |-- [CRATE] omega-control-flow/                 # Control-flow/data-flow graph.
|       |   |-- [CRATE] omega-abstract-operations/          # Target-independent abstract operations with virtual registers.
|       |   |-- [CRATE] omega-terminal-abstract-operations/ # Source-independent terminal-Psi lowering requirements.
|       |   |-- [CRATE] omega-terminal-installation-evidence/ # Read-only projections of orchestration-owned admission evidence.
|       |   |-- [CRATE] omega-terminal-target-operations/  # Target-aware operations for the clean terminal-Psi lane.
|       |   |-- [CRATE] omega-terminal-assigned-target-operations/ # Assigned homes for the clean terminal-Psi lane.
|       |   |-- [CRATE] omega-target-operations/            # Target-aware operations after legalization and selection.
|       |   |-- [CRATE] omega-assigned-target-operations/   # Target-aware operations after register and stack assignment.
|       |   |-- [CRATE] omega-machine-instructions/         # Final symbolic machine instructions before encoding.
|       |   |-- [CRATE] omega-machine-program/              # Machine-program artifact assembled from target operations.
|       |   |-- [CRATE] omega-machine-bytes/                # Encoded machine bytes.
|       |   `-- [CRATE] omega-backend-plan/                 # Backend planning data shared across lowering stages.
|       |
|       |-- pipeline/
|       |   |-- [CRATE] omega-checked-trees-to-state-graph/                      # Checked trees to explicit machine/state graph.
|       |   |-- [CRATE] omega-state-graph-to-control-flow/                       # State graph to control-flow/data-flow graph.
|       |   |-- [CRATE] omega-control-flow-to-abstract-operations/               # Lower control flow into target-independent abstract operations with virtual registers.
|       |   |-- [CRATE] omega-abstract-operations-to-target-operations/          # Normalize, legalize, and lower abstract operations into target-aware forms.
|       |   |-- [CRATE] omega-terminal-target-operations-to-assigned-target-operations/ # Assign clean terminal-Psi register and spill homes.
|       |   |-- [CRATE] omega-target-operations-to-assigned-target-operations/   # Assign registers, stack slots, and calling-convention homes to target-aware operations.
|       |   |-- [CRATE] omega-assigned-target-operations-to-machine-instructions/ # Convert assigned target-aware operations into final symbolic machine instructions.
|       |   `-- [CRATE] omega-target-operations-to-machine-program/              # Assemble target operations into a machine-program artifact.
|       |
|       |-- backend/
|       |   |-- [CRATE] omega-target/                       # Target triples, cpu/features, os/env/object format matrix.
|       |   |-- [CRATE] omega-runtime-abi/                  # Backend-ABI carrier shapes (fat descriptors, slices, text windows) and their accessors.
|       |   |-- [CRATE] omega-data-planning/                # Data/section planning for emitted program data.
|       |   |-- [CRATE] omega-platform-interface/           # ABI-facing OS/platform imports, host surfaces, loader facts.
|       |   |-- [CRATE] omega-state-calls/                  # State-machine call lowering surface.
|       |   |-- [CRATE] omega-state-storage/               # State storage layout and slot lowering.
|       |   |-- [CRATE] omega-state-values/               # State value lowering and materialization.
|       |   |-- [CRATE] omega-state-schedule/             # State scheduling into backend form.
|       |   |-- [CRATE] omega-state-dispatch/             # State dispatch lowering.
|       |   |-- [CRATE] omega-state-guards/               # State guard/condition lowering.
|       |   |-- [CRATE] omega-runtime-text/               # Runtime text-window carrier lowering.
|       |   |-- [CRATE] omega-runtime-bodies/             # Runtime body/state-body lowering.
|       |   |-- [CRATE] omega-runtime-storage/            # Runtime storage surfaces.
|       |   |-- [CRATE] omega-runtime-branching/          # Runtime branch lowering.
|       |   |-- [CRATE] omega-runtime-dispatch-loop/      # Runtime dispatch-loop lowering.
|       |   |-- [CRATE] omega-calling-conventions/        # ABI rules for registers, stack, parameter/return passing.
|       |   |-- [CRATE] omega-layout/                     # Type layout, alignments, field offsets, calling-convention records.
|       |   |-- [CRATE] omega-instruction-selection/      # Shared instruction selection framework.
|       |   |-- [CRATE] omega-emission-planning/          # Section/symbol plans before machine emission.
|       |   |-- [CRATE] omega-machine-emission/           # Final machine program to encoded bytes.
|       |   |-- [CRATE] omega-backend-report/             # Backend summaries and reports.
|       |   |
|       |   |-- instruction_set_architectures/
|       |   |   |-- [CRATE] omega-isa-aarch64/            # AArch64 instruction defs, encodings, lowering hooks.
|       |   |   `-- [CRATE] omega-isa-x86_64/             # x86_64 instruction defs, encodings, lowering hooks.
|       |   |
|       |   |-- object/
|       |   |   |-- [CRATE] omega-object-file/            # Shared object-file representation: sections, symbols, relocations.
|       |   |   |-- [CRATE] omega-object-file-planning/   # Builds section and symbol plans before object-file or image writing.
|       |   |   `-- [CRATE] omega-relocations/            # Builds relocation records over selected and machine instructions.
|       |   |
|       |   `-- images/
|       |       |-- [CRATE] omega-image/                  # Shared final image data model and fixup helpers.
|       |       |-- [CRATE] omega-image-emission/         # Selects the final executable image writer for a target.
|       |       |-- [CRATE] omega-terminal-image-emission/ # Clean terminal-Psi object/image emission and typed installation record.
|       |       |-- [CRATE] omega-image-elf/              # Final ELF image layout, program headers, loaders.
|       |       |-- [CRATE] omega-image-macho/            # Final Mach-O image layout, load commands, fixups.
|       |       `-- [CRATE] omega-image-pe/               # Final PE image layout, directories, imports, relocations.
|       |
|       `-- orchestration/
|           |-- [CRATE] omega-artifacts/                  # Phase artifact data and text/binary dumping.
|           |-- [CRATE] omega-backend-pipeline/           # Backend phase sequencing at the orchestration edge.
|           |-- [CRATE] omega-compiler/                   # Top-level check/build API used by cli/tests.
|           |-- [CRATE] omega-external-roots/             # Installed provider/root execution, receipts, and resource evidence.
|           |-- [CRATE] omega-native-differential-test/    # Cross-layer Psi-interpreter/native differential tests only.
|           `-- [CRATE] omega-visualizations/             # Visualization/dump views of pipeline artifacts.
|
|-- compiler/
|   |-- psi/                                               # Omega-written Psi source; lexical checkpoint landed.
|   |-- omega/                                             # Omega-written backend/optimizer owner; implementation open.
|   `-- source-checkpoints/                                # Exact product closures and provisional Ωself censuses.
|
|-- apps/
|   `-- omega-compiler/                                    # Hosted Omega-written product compiler entrypoint.
|
|-- omega/
|   `-- language/
|       |-- core/                                       # Always-available language package.
|       `-- std/                                        # Higher-level standard package surface.
|
|-- samples/
|   |-- cli_mvp/                                        # Smallest console program.
|   |-- dungeon_crawler_cli/                            # Console input/output and room navigation pressure test.
|   `-- README.md                                       # Notes for sample expectations and local build output.
|
|-- canaries/
|   |-- pass/                                           # Tiny feature canaries expected to check.
|   `-- fail/                                           # Tiny negative canaries with expected diagnostics.
|
|-- tests/
|   |-- integration/                                    # End-to-end compiler tests.
|   |-- target_corpus/                                  # Per-target calling convention, ABI, object, link, and image tests.
|   `-- bootstrap/                                      # Product/hosted-build integration tests; lattice gates stay under bootstrap/.
|
`-- wiki/                                               # Language design notes, target notes, and guide drafts.
```

## Planned Relocation

The tree above describes the repository as it stands. It is not the intended
shape. Three top-level directories split one concept — the compiler — by role
and implementation language, and the only one that builds anything is three
levels down under a name that means "temporary":

```text
apps/omega-compiler/                       Omega entrypoint          3 files
compiler/{psi,omega}/                      Omega implementation     17 files
bootstrap/onramps/omega-rust/              Rust implementation   2,547 files
```

`bootstrap/` is 63% working compiler by tracked file count. That is the
discoverability defect: someone opening this repository to read the compiler
finds four `.omg` files under `compiler/` and a stdlib under `omega/`.

Target shape:

```text
source/
  compiler/
    rust/            <- bootstrap/onramps/omega-rust
    omega/           <- compiler/{psi,omega} + apps/omega-compiler
  library/           <- omega/language
  assurance/         <- bootstrap/assurance
bootstrap/           <- lattice only
  {alpha,beta,gamma,delta}/   <- rungs/X merged with onramps/X-rust
  omega-bootstrap/
  gates/             <- corpus + lattice-cache-deps
tests/               <- canaries + fixtures
tools/               <- scripts
samples/  wiki/      unchanged
```

Ordered work:

- [ ] Rewrite this page against the current tree before moving anything. A map
      that is already wrong compounds every relocation.
- [ ] `bootstrap/onramps/omega-rust` -> `source/compiler/rust`. The Rust
      compiler is the current implementation, not bootstrapping material;
      its language is a fact about how it is written, not about its role.
- [ ] `compiler/{psi,omega}` + `apps/omega-compiler` -> `source/compiler/omega`.
      These are the implementation and entrypoint halves of one self-hosted
      compiler.
- [ ] `bootstrap/assurance` -> `source/assurance`. The proof kernel is a
      shipped cross-cutting service, explicitly "deliberately not a language
      rung"; it is product, currently filed under bootstrap.
- [ ] Merge `bootstrap/rungs/X` with `bootstrap/onramps/X-rust` into
      `bootstrap/X/`. Today one directory holds source written in a rung's
      language and another holds the Rust host that runs it — two layers
      splitting one concept.
- [ ] `bootstrap/corpus` + `bootstrap/lattice-cache-deps` -> `bootstrap/gates/`.
- [ ] `canaries` + `fixtures` -> `tests/`. Eleven package fixtures currently sit
      apart from 4,592 other test programs for no reason beyond being
      package-shaped.
- [ ] `scripts` -> `tools/`.
- [ ] **Blocked:** `omega/language` -> `source/library`. Gated on
      `TASKS_PACKAGE_MANAGER.md` P8 — the standard library must stop being
      reached by hardcoded path before its directory can move. Every other item
      above is unblocked.

Retired already: `docs/` (one orphaned file, no references), `omega/host/`
(tombstone README, now a design brief), `target_runtime/` (nine `.gitkeep`
files reserving a home for payloads that were never built).

Separately tracked: `bootstrap/corpus` is a hand-maintained fork of `samples/`.
All 74 corpus programs share a name with a sample and differ only by stripped
interactive I/O, with no drift check. Generate it or gate it.

## Placement Rules

### Front Door

- Product `apps/` stay thin. They parse user intent and call compiler services.
  `apps/omega-compiler/` is the hosted product entrypoint; the current Rust
  `omega-cli` stays with its producer under
  `bootstrap/onramps/omega-rust/apps/`. The language-server and docs-generator
  are not yet separate applications.
- `orchestration/` sequences phases, owns artifacts and the top-level
  check/build API (`omega-compiler`, `omega-backend-pipeline`, `omega-artifacts`,
  `omega-visualizations`), and keeps source loading coherent. Session/options and
  incremental-query engines are not yet separate crates.
- Checked-tree visualization keeps replaceable view production separate from
  its regression corpus. Shared fixture construction may live in a small test
  parent, but behavior, content, qualification, carry, and machine-contract
  manifest cases compile as responsibility-specific child modules rather than
  being embedded in the production view or recombined in one permutation file.
- Large integration suites follow the same rule. Corpus registries, shared
  compilation seams, and umbrella orchestration stay in a small test root;
  target, artifact, semantic, provider, ABI, proof, layout, and runtime cases
  compile in responsibility-specific modules with explicit cross-family imports.
- `orchestration/` must not become the home for semantic checks or backend
  lowering.

### Source And Packages

- `foundation/` stays dependency-light. If it needs semantic or target details,
  it is in the wrong layer. `omega-core` contains Omega execution/build
  utilities; source and target-neutral semantic foundations belong to Psi.
- Source-preserving syntax data belongs in `representations/`; source-to-syntax
  transforms belong in `pipeline/`.
- Package manifests, package graphs, and loading are placement intent for a
  future `packages/` layer; there are no `packages/` crates today, and that work
  must not absorb language semantics when it lands.
- Source discovery/loading should remain an orchestration subsystem, not parser
  responsibility.

### Semantic Ownership

- The Psi role owns Omega-file parsing and all target-neutral language meaning
  through terminal Psi. Its current Rust realization is
  `bootstrap/onramps/omega-rust/psi/`. Psi crates must not depend on Omega
  crates; the architecture test enforces that firewall.
- Existing target-neutral `omega-*` crates are migration inputs, not a second
  permanent frontend. Move or rename them under Psi ownership as terminal
  vertical slices replace their source-shaped handles.
- The Omega backend role begins its long-term semantic consumption at terminal
  Psi and owns provider installation, ABI/storage realization, optimization,
  target lowering, and native execution machinery. Its current Rust realization
  is `bootstrap/onramps/omega-rust/omega/`. Psi owns both transitional
  checked-tree reference execution and canonical terminal-Psi interpretation;
  Omega contains only the cross-layer native differential-test harness. That
  harness keeps shared artifact decoding, verified lowering, and native image
  execution separate from responsibility-specific source differential families;
  it does not recombine semantic verification cases in its test root.
- `semantics/` owns language meaning: names, types, effects, proofs, facts,
  invariants, and validation. Borrow, invariant, contract, and const-evaluation
  reasoning live chiefly in `psi-types`, `psi-facts`, `psi-validation`, and
  `psi-proof`.
- `psi-facts` carries checked facts, invariants, and refinement data: what
  remains true. Transitional backend consumers depend on the Psi owner directly
  until terminal-Psi slices replace them.
- `psi-checked-trees` owns the durable checked semantic representation and its
  proof, borrow, flow, reach, value-origin, and admissibility evidence. Legacy
  state/control representations and transforms, artifact/backend orchestration,
  the Psi-owned transitional checked-tree interpreter, and backend leaf
  consumers depend on the Psi owner directly.
- `psi-effects` carries target-neutral operational ceilings, service reach,
  synchronous invocation summaries, and capability-flow facts; target-neutral
  consumers depend on it directly. `omega-effects` retains provider
  declarations, target/provider bindings, approval, and installation-facing
  records—including the exact selected-plan carrier—but no longer re-exports
  the Psi vocabulary. Checked semantic trees do not retain that Omega
  realization sidecar.
- `psi-validation` answers target-neutral cross-semantic obligations, including
  who may read or mutate and what a callable requires or promises. Provider
  installation and approval remain in Omega.
- `psi-proof` plans and discharges source-level obligations.
- `omega-compiler` invokes the Psi-owned source-to-checked frontend directly.
  Its checked-tree handoff to the transitional Omega state-graph lane remains
  only until general terminal-Psi production replaces it.

### Representations And Pipeline

- `representations/` owns durable IR data structures and arena storage.
- `pipeline/` crates transform one representation into the next.
- Pipeline crates may depend on input and output representations, but should not
  become owners of shared helper structures.
- Long-lived representation boundaries remain explicit within their semantic
  owners. Psi owns tokens, syntax, symbol-resolved, typed, checked, and terminal
  Psi. Omega consumes terminal Psi and owns abstract operations, target
  operations, assigned target operations, machine instructions, machine
  program, and machine bytes.
- State graph and control flow are transitional bootstrap representations, not
  durable language or cross-owner boundaries.
- Do not skip from source-shaped trees to backend-specific structs.

### Backend

- `omega-control-flow-to-abstract-operations` is where backend lowering begins;
  it should produce target-independent operations with explicit values and
  effects.
- `omega-abstract-operations-to-target-operations` owns target legalization and
  instruction-shape lowering.
- `omega-target-operations-to-assigned-target-operations` owns register
  allocation, stack slots, spills, and calling-convention homes.
- The clean terminal-Psi lane mirrors that boundary in
  `omega-terminal-target-operations-to-assigned-target-operations`; it must
  assign concrete parameter and spill homes before terminal machine emission.
- `omega-assigned-target-operations-to-machine-instructions` owns symbolic
  machine instruction construction.
- `omega-target-operations-to-machine-program` assembles target operations into
  the machine-program artifact; `omega-machine-emission` produces encoded
  machine bytes, and `backend/object/*` owns sections, symbols, and relocations.
- Register allocation and machine-level optimization are not yet separate crates
  (`omega-regalloc` / `omega-machine-optimization` do not exist); that work lives
  in the assigned-target-operations stage and the relevant backend crates.
- `backend/instruction_set_architectures/*` owns ISA definitions and encodings.
  Only AArch64 and x86_64 exist today. Shared lowering policy belongs in shared
  backend crates.
- The backend also carries fine-grained state and runtime lowering crates
  (`omega-state-*`, `omega-runtime-*`) plus `omega-runtime-abi`, which owns the
  backend-ABI carrier shapes (fat descriptors, slices, text windows) and their
  field-offset/subslice accessors. `omega-layout` and instruction selection
  consume those shapes rather than re-deriving them.
- `omega-calling-conventions` owns ABI value passing rules.
- `omega-platform-interface` owns ABI-facing host/platform surfaces.
- `backend/object/*` and `backend/images/*` should stay separate. Object writing
  and final image layout are different jobs. Per-format object writers and the
  linker crates are not yet separate packages.

### Runtime And Boundaries

- Process entry and startup-runtime replacement are placement intent for a
  future `compiler/runtime/startup/*` layer; there are no compiler-side runtime
  or startup crates today, and no linkable startup payloads exist yet. The
  `target_runtime/` skeleton that once reserved a home for them held only
  `.gitkeep` files and was removed; reserve that placement here in prose rather
  than as empty directories on disk.
- The retired `omega/host` capability scaffold is not a second boundary model.
  Portable host requirements and checked adapters live under
  `omega/language/std`; target-owned implementations/defaults live under
  `omega/language/std/targets`. [Retired: Host Package Scaffold](../design_briefs/retired_host_package_scaffold.md)
  preserves the migration fence for any future dedicated provider package; the
  `omega/host/` directory itself is gone.
- Import tables, export tables, loader metadata, startup selection, and final
  fixups are compiler responsibilities because Omega does not assume native
  system linkers.
- `.o` emission is a compatibility/debug bridge. Direct image construction from
  machine program data remains the long-term pressure.

### Identity And Data Shape

- Internal identity is handle-first, not string-first.
- Source text is source-loading, diagnostic, and debug payload, not semantic
  identity after resolution.
- User string literals are program payload. Debug names and linker names are
  edge metadata.
- Repeated durable children should prefer arena `Handle<T>` and `HandleSpan<T>`
  over recursive boxes or scattered owned vectors.
- Symbol lookup should prefer scoped symbol-tree walks. Add maps only for
  measured pathological scopes.

### Hygiene

- Public crate roots should explain exports, not hide implementation.
- Tests should not live in giant `lib.rs` files.
- `mod.rs` and `lib.rs` declare boundaries; they are not junk drawers.

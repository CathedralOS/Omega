# Repository Layout

This page is a map and a placement guide. It should help answer two questions:

- Where should a new crate or module live?
- Which layer is allowed to own a concept?

The pipeline-specific semantic rules live in
[Pipeline Architecture](pipeline/pipeline.md).

How Omega reaches its hosted compiler—the trust architecture and the exact
`Alpha → Beta → Gamma → Delta` language spine—is a build-graph property
described by [The Bootstrap Lattice](bootstrap_lattice/bootstrap_lattice.md)
and its [target repository structure](bootstrap_lattice/repository_structure.md).
It is not a separate source ownership domain.

## Design Bias

- Prefer feature-first crates with explicit names.
- Keep durable IR structs in `representations/`.
- Keep transforms in `pipeline/`.
- Keep language meaning in `semantics/`.
- Keep target, ABI, layout, object, linker, and image details in `backend/`.
- Keep coordinators boring: sequence typed phases and stop. Artifact writing,
  package loading, build evaluation, deployment, and reports belong to their
  named subsystems.
- Do not add a crate until a module boundary has stopped moving.

Legend:

- `[CRATE]` means a Cargo workspace package.
- Unprefixed folders are ordinary source/module boundaries inside a crate.

This tree is a conceptual placement map anchored in the current Rust product
implementation;
it is not an exhaustive generated inventory of Cargo workspace members. Some
sub-areas named in the placement prose are not yet separate crates, while small
implementation crates may appear in the workspace before this map names them.
Some finer-grained backend object/linker/image writers remain placement intent;
the displayed Omega top-level directories are current and exhaustive.

> **Ownership boundary.** The current Cargo implementation is explicitly an
> external-language producer under `source/omega-rust/`. Its `psi/`
> half implements parsing and target-neutral semantics through terminal Psi;
> its `omega/` half implements provider, ABI, target, artifact, and execution
> machinery. The Omega-written product is split across sibling owners:
> `source/psi/` owns its target-neutral half and `source/omega/` consumes
> Terminal Psi for target realization and product composition; the
> live Psi lexical slice has landed while later phases remain open.
> Lattice runners resolve cross-owner locations through the role manifest in
> `tools/lattice/paths.sh`; lattice scripts may not hard-code sibling-relative
> paths. Package dependencies are declared by their `build.omg` package graph.
> The tree below documents the current Cargo/product structure;
> the canonical compiler-sequence inventory is documented in the
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
|-- source/omega-rust/
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
|   |       |-- [CRATE] psi-proof-admission/            # Product-local Psi judgment and admission checking.
|   |       |-- [CRATE] psi-checked-interpreter/        # Checked-tree build-time and transitional reference execution.
|   |       |-- [CRATE] psi-build-time-evaluation/      # Const/domain evaluation and programmable plan normalization.
|   |       |-- [CRATE] psi-terminal-semantics/         # Closed scalar, structural/effect, and call-composition policy rows.
|   |       |-- [CRATE] psi-terminal-verifier/          # Module validation and reconstructed-obligation checking.
|   |       `-- [CRATE] psi-terminal-interpreter/       # Fuel-bounded reference execution of verified terminal artifacts.
|   |
|   `-- [CRATE] omega/                                   # Current Rust `omega` product command.
|       |-- representations/                             # Durable, target-independent carriers and evidence.
|       |   |-- [CRATE] omega-core/
|       |   |-- [CRATE] omega-target/
|       |   |-- [CRATE] omega-calling-conventions/
|       |   |-- [CRATE] omega-{abstract,target,legalized,selected,assigned}-operations/
|       |   |-- [CRATE] omega-machine-code/
|       |   |-- [CRATE] omega-{register-model,optimization-core,optimization-unit}/
|       |   |-- [CRATE] omega-{effects,installation-evidence,task-plans}/
|       |   `-- [CRATE] omega-{function-identity,backend-report-types}/
|       |
|       |-- pipeline/                                    # Every checked representation-to-representation transform.
|       |   |-- [CRATE] omega-psi-to-abstract-operations/
|       |   |-- [CRATE] omega-optimization-run-to-abstract-operations/
|       |   |-- [CRATE] omega-abstract-operations-to-target-operations/
|       |   |-- [CRATE] omega-target-operations-to-{selected-instructions,assigned-target-operations}/
|       |   |-- [CRATE] omega-terminal-psi-to-native-artifact/
|       |   `-- optimization/
|       |       |-- [CRATE] omega-{psi-optimizer,regalloc,machine-optimizer}/
|       |       |-- [CRATE] omega-{optimization-policy,optimization-validation}/
|       |       `-- [CRATE] omega-optimization-pipeline/
|       |
|       |-- backend/                                     # Target/runtime primitives and backend-owned artifacts.
|       |   |-- [CRATE] omega-{layout,machine-emission}/
|       |   |-- instruction_set_architectures/
|       |   |   |-- [CRATE] omega-isa-{aarch64,x86_64}/
|       |   |   `-- [CRATE] omega-x86-encoding/
|       |   |-- object/                                  # Object-file ownership.
|       |   |-- images/                                  # Image models, emission, and formats.
|       |   |-- artifacts/
|       |   |   |-- [CRATE] omega-native-artifact/
|       |   |   `-- [CRATE] omega-component-candidate/
|       |   |-- plans/
|       |   |   |-- [CRATE] omega-backend-plan/
|       |   |   `-- [CRATE] omega-program-entry-plan/
|       |   `-- runtime/
|       |       |-- [CRATE] omega-{runtime-abi,executable-installation,external-roots}/
|       |       `-- [CRATE] omega-component-publication/
|       |
|       |-- build/                                       # Build evaluation, composition, policy, and trust decisions.
|       |   |-- [CRATE] omega-build-{declarations,evaluation,output,provenance}/
|       |   |-- [CRATE] omega-{package-compilation,provider-planning,selected-dispatch}/
|       |   |-- [CRATE] omega-component-deployment/
|       |   `-- [CRATE] omega-trust-ledger/
|       |-- compiler/                                    # Thin product coordinator and result surface.
|       |-- packages/                                    # Package graph, loading, and review.
|       |-- tooling/                                     # Auxiliary artifacts, profiles, visualizations, and host custody.
|       |-- src/                                         # Tiny `omega` product command.
|       `-- tests/                                       # Cargo integration tests for that product command.
||-- source/
|   |-- alpha/                                             # Alpha semantics, seeds, assembler, and root checker.
|   |   `-- checker/                                       # Universal derivation checker and checker tests.
|   |-- beta/                                              # Beta language, reference meaning, and gates.
|   |   `-- compiler/                                      # Compiler source, artifact, cold start, and validation.
|   |-- gamma/                                             # Gamma language, interpreter, and type checker.
|   |-- delta/                                             # Delta language, compiler, meaning, tests, and artifacts.
|   |   |-- compiler/                                     # Canonical compiler source, validation, and admitted artifacts.
|   |   |-- meaning/                                      # Lower-rung Delta-to-Gamma elaboration.
|   |   `-- tests/                                        # Delta language cases.
|   |-- library/                                           # Core, allocation, and standard library source.
|   |   |-- core/                                          # Always-available language package.
|   |   |-- alloc/                                         # Allocation facilities.
|   |   `-- std/                                           # Higher-level standard package surface.
|   |-- psi/                                               # Omega-written target-neutral phases through terminal Psi.
|   |-- omega/                                             # Omega-written Terminal-Psi consumer and product root.
|   |   |-- build.omg                                      # Product build/composition entrypoint.
|   |   |-- main.omg                                       # Product machine entrypoint.
|   `-- omega-rust/                                        # Current Rust product implementation and comparator.
|
|-- tools/lattice/                                         # Lattice orchestration and path gates.
|
|-- samples/
|   |-- cli_mvp/                                        # Smallest console program.
|   |-- dungeon_crawler_cli/                            # Console input/output and room navigation pressure test.
|   `-- README.md                                       # Notes for sample expectations and local build output.
|
|-- tests/
|   |-- omega/
|   |   |-- pass/                                       # Focused Omega cases expected to check.
|   |   `-- fail/                                       # Focused Omega cases expected to reject.
|   `-- fixtures/packages/                              # Package-shaped integration fixtures.
|
|-- tools/                                              # Repository maintenance tools.
|
`-- wiki/                                               # Language design notes, target notes, and guide drafts.
```

## Completed relocation

The displayed tree is the canonical ownership shape. The unblocked relocation
steps are complete:

```text
source/{alpha,beta,gamma,delta}/       canonical language rungs
source/library/                        core, allocation, and standard libraries
source/psi/                            Omega-written target-neutral phases through terminal Psi
source/omega/                          Terminal-Psi consumer and product root
source/alpha/checker/                  root derivation checking
source/beta/compiler/                  Beta compiler and its admission evidence
source/omega-rust/                     current Rust product implementation and comparator
tests/{omega,fixtures}/                language and package integration tests
tools/lattice/                         lattice orchestration and path gates
tools/                                 other repository maintenance scripts
```

Each rung remains the semantic owner of its language and lattice-built
artifacts. A Rust producer nested beneath that rung is tooling for the same
concept, not a second semantic owner. The root proof checker belongs to Alpha.
Validation belongs beside the artifact it admits, so the Beta compiler's
source/artifact reconstruction lives under `source/beta/compiler/validation/`
and Delta publication/custody lives under
`source/delta/compiler/validation/`.

The package library now lives at `source/library/`. The relocation deliberately
has no compatibility symlink. Package-manager P8 still owns removal of the
temporary physical-path readers that were updated to this location during the
move; the final implementation resolves std through the package graph.

## Placement Rules

### Front Door

- Product entrypoints stay thin. `source/omega/{build.omg,main.omg}` owns the
  hosted product entrypoint; the current Rust product package and command are
  rooted directly at `source/omega-rust/omega/`. The language-server and
  docs-generator are not separate products.
- `compiler/omega-compiler` owns the top-level typed check/build coordinator.
  Its coordinator forwards one stage result into the next; it does not own
  package loading, build evaluation, stage logic, artifact formatting, or
  visualization semantics.
- `pipeline/` owns every checked transformation, including optimization and
  Terminal-Psi-to-native realization. A target-closing stage may consume
  `backend/` primitives; that does not earn it a separate top-level kingdom.
- Checked-tree visualization keeps replaceable view production separate from
  its regression corpus. Shared fixture construction may live in a small test
  parent, but behavior, content, qualification, carry, and machine-contract
  manifest cases compile as responsibility-specific child modules rather than
  being embedded in the production view or recombined in one permutation file.
- Large integration suites follow the same rule. Corpus registries, shared
  compilation seams, and umbrella orchestration stay in a small test root;
  target, artifact, semantic, provider, ABI, proof, layout, and runtime cases
  compile in responsibility-specific modules with explicit cross-family imports.
- No general-purpose orchestration layer exists. Do not recreate one under a
  different name or use `compiler/` as a top-ranked dependency escape hatch.

### Source And Packages

- Omega has no separate `foundation/` bucket. Durable dependency-light
  identities and carriers live in `representations/`; target/runtime
  primitives live in `backend/`. Source and target-neutral semantic
  foundations belong to Psi.
- Source-preserving syntax data belongs in `representations/`; source-to-syntax
  transforms belong in `pipeline/`.
- Package manifests, package graphs, and loading live under `packages/`. They
  must not absorb language semantics.
- Source discovery/loading belongs to the package/compiler boundary, not parser
  responsibility and not a generic orchestration subsystem.

### Semantic Ownership

- The Psi role owns Omega-file parsing and all target-neutral language meaning
  through terminal Psi. Its current Rust realization is
  `source/omega-rust/psi/`. Psi crates must not depend on Omega
  crates; the architecture test enforces that firewall.
- Existing target-neutral `omega-*` crates are migration inputs, not a second
  permanent frontend. Move or rename them under Psi ownership as terminal
  vertical slices replace their source-shaped handles.
- The Omega backend role begins its long-term semantic consumption at terminal
  Psi and owns provider installation, ABI/storage realization, optimization,
  target lowering, and native execution machinery. Its current Rust realization
  is `source/omega-rust/omega/`. Psi owns both transitional
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
  proof, borrow, flow, reach, value-origin, and admissibility evidence. Terminal
  lowering consumes it once; Omega backend crates consume Terminal Psi rather
  than checked trees.
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
- `omega-compiler` invokes the Psi-owned frontend and canonical terminal-Psi
  producer directly. Production native realization begins from that immutable
  artifact. There is no StateGraph/control-flow fallback route.

### Representations And Pipeline

- `representations/` owns durable IR data structures and arena storage.
- `pipeline/` crates transform one representation into the next.
- Pipeline crates may depend on input and output representations, but should not
  become owners of shared helper structures.
- Long-lived representation boundaries remain explicit within their semantic
  owners. Psi owns tokens, syntax, symbol-resolved, typed, checked, and terminal
  Psi. Omega consumes terminal Psi and owns abstract operations, target
  operations, selected/legalized/assigned operations, machine code, objects,
  and final images.
- State graph and control flow are transitional implementation representations, not
  durable language or cross-owner boundaries.
- Do not skip from source-shaped trees to backend-specific structs.

### Backend

- Production backend lowering begins at `omega-psi-to-abstract-operations`,
  then proceeds through ordinary `omega-*` operation, selection, allocation,
  machine-code, object, and image owners.
- The bounded `omega-target-operations-to-assigned-target-operations` route is
  a temporary continuation for unsupported physical slices. It must converge
  into the selected-instruction continuation rather than become a second
  pipeline.
- The retired StateGraph/control-flow backend has been deleted. Reintroducing a
  source-shaped backend fallback is an architecture violation.
- `omega-machine-emission` produces production machine code, and
  `backend/object/*` owns sections, symbols, and relocations.
- `pipeline/optimization/*` owns reusable optimization, validation, register
  allocation, and machine-optimization logic. In particular, `omega-regalloc`
  and `omega-machine-optimizer` own their algorithms. The bounded target-to-assigned
  publication adapter is not a substitute for either owner and must disappear
  once the selected physical conveyor has complete operation coverage.
- `backend/instruction_set_architectures/*` owns ISA definitions and encodings.
  Only AArch64 and x86_64 exist today. Shared lowering policy belongs in shared
  backend crates.
- `omega-runtime-abi` owns backend-ABI carrier shapes (fat descriptors, slices,
  text windows) and their accessors. `omega-layout` and instruction selection
  consume those shapes rather than re-deriving them.
- `omega-calling-conventions` owns ABI value passing rules.
- Calling plans and selected provider settlements own ABI-facing host/platform
  surfaces; no parallel platform-interface backend exists.
- `backend/object/*` and `backend/images/*` should stay separate. Object writing
  and final image layout are different jobs. Per-format object writers and the
  linker crates are not yet separate packages.

### Runtime And Boundaries

- Process entry and startup-runtime replacement belong under
  `backend/runtime/startup/*` when they acquire a real implementation; there
  are no startup crates or linkable startup payloads today. The
  `target_runtime/` skeleton that once reserved a home for them held only
  `.gitkeep` files and was removed; reserve that placement here in prose rather
  than as empty directories on disk.
- The retired `omega/host` capability scaffold is not a second boundary model.
  Portable host requirements and checked adapters live under
  `source/library/std`; target-owned implementations/defaults live under
  `source/library/std/targets`. [Retired: Host Package Scaffold](../design_briefs/retired_host_package_scaffold.md)
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
- Omega language cases live only under `tests/omega/`; generic `canaries/`
  trees obscure the language owner and must not be recreated.
- `mod.rs` and `lib.rs` declare boundaries; they are not junk drawers.

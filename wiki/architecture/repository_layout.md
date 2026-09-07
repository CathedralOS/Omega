# Repository Layout

This page is a map and a placement guide. It should help answer two questions:

- Where should a new crate or module live?
- Which layer is allowed to own a concept?

The pipeline-specific semantic rules live in
[Pipeline Architecture](pipeline/pipeline.md).

How Omega reaches its hosted compiler—the selected
`Alpha -> Beta -> Gamma -> Delta -> Epsilon -> Omega` trust lattice is a build-graph property
described by [The Bootstrap Chain](bootstrap_chain/bootstrap_chain.md)
and its [target repository structure](bootstrap_chain/repository_structure.md).
It is not a separate source ownership domain. The completed alternatives audit
is retained in [Bootstrap chain alternatives](../design_briefs/bootstrap_chain_alternatives.md).

## Design Bias

- Prefer feature-first crates with explicit names.
- Keep durable IR structs in `representations/`.
- Keep transforms in `pipeline/`.
- Keep language meaning in `semantics/`.
- Keep target, ABI, layout, object, linker, and image details in `backend/`.
- Keep coordinators boring: sequence typed phases and stop. Artifact writing,
  package loading, build evaluation, deployment, and reports belong to their
  named subsystems.
- Compiler allocation counters and phase-report deltas belong to
  `omega/tooling/artifacts`, not a program representation or dependency-floor
  `core` crate. The product installs that owner's allocator wrapper.
- Do not add a crate until a module boundary has stopped moving.

## Representation entrances and ownership

A program representation has one named root file beside `lib.rs`. That file
defines the root program struct and maps its subordinate concepts. For example,
`selected-instructions/src/selected_instructions.rs` owns
`SelectedInstructionPlan`; `selected_instructions/` contains control flow,
virtual values, instructions, structural calls, constraints, provenance, and
effects, plus retained liveness and live-range facts. Their raw plans and
canonical identities describe selected instructions, not physical homes or
validation authority. Effect catalogs describe target mechanisms; effect program rows
describe a particular selected program and own their canonical encoding.

`register-homes/src/register_homes.rs` leads to the current allocated program,
allocation constraints, storage assignments, and recovery facts. `constraints/`
owns allocator availability, allocation legality, fixed-precolored intervals,
and split requirements; `storage/` owns segment homes; `recovery/` owns spill
choices and recovery classifications. These owners define the raw plans,
policies, identities, and codecs. Computation, independent replay, and sealed
validation receipts stay in the transforms. Decoding or hashing a plan does not
grant allocation authority.

The corresponding entrances are `abstract_operations.rs` for
`AbstractOperationPlan`, `target_operations.rs` for `TargetOperationPlan`,
`legalized_operations.rs` for `LegalizedOperationPlan`, and
`assigned_operations.rs` for `AssignedOperationPlan`. Each is beside its crate's
`lib.rs`, with subordinate areas beneath the matching directory. The areas
deliberately differ: abstract operations retain completion claims and ranked
semantic control; target operations add ABI requirements and selected boundary
mechanisms; legalization retains legality recipes; assigned operations contain
concrete register/frame locations.

Subfolders follow actual semantic areas rather than a universal template or a
file-count target. A concept such as ownership can change form or be discharged
between stages. Each representation must explain where the remaining facts live
and how their identities connect to the next representation. Empty placeholder
areas do not satisfy this requirement.

Pipeline stages consume and produce these representations. Their private
working state and admission results may remain local, but public program data
and its serialization belong to the representation owner. A stage result must
not become a second program representation by accumulating earlier stage
objects. Selected optimization passes preserve the representation vocabulary;
later analysis consumes the current program rather than matching the history of
which passes ran.

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
> external-language producer under `omega-rust/`. Its `psi/`
> half implements parsing and target-neutral semantics through terminal Psi;
> its `omega/` half implements provider, ABI, target, artifact, and execution
> machinery. The Omega-written product is split across sibling owners:
> `source/psi/` owns its target-neutral half and `source/omega/` consumes
> Terminal Psi for target realization and product composition; the
> live Psi lexical slice has landed while later phases remain open.
> Bootstrap runners resolve cross-owner locations through the role manifest in
> `tools/bootstrap/paths.sh`; bootstrap scripts may not hard-code sibling-relative
> paths. Package dependencies are declared by their `build.omg` package graph.
> The tree below documents the current Cargo/product structure;
> the canonical compiler-sequence inventory is documented in the
> [bootstrap repository structure](bootstrap_chain/repository_structure.md),
> while active bootstrap work is tracked in
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
|-- omega-rust/
|   |-- psi/                                         # Psi owns target-neutral semantics through terminal Psi.
|   |   |-- foundation/
|   |   |   |-- [CRATE] access-plans/               # Normalized placed-view access semantics.
|   |   |   |-- [CRATE] arena/                      # Generic typed arena storage for Psi source representations.
|   |   |   |-- [CRATE] diagnostics/                # Source diagnostics and phase-snapshot contracts.
|   |   |   |-- [CRATE] extents/                    # Extent geometry, lineage, rights, and provider identity.
|   |   |   |-- [CRATE] language-core/              # Target-neutral source-language vocabulary.
|   |   |   |-- [CRATE] language-semantics/         # Resolved identities, content/value domains, tables, and plans.
|   |   |   |-- [CRATE] layout-plans/                # Normalized layout geometry and materialization plans.
|   |   |   |-- [CRATE] numerics/                   # Exact numerics, float semantics, and literal payloads.
|   |   |   |-- [CRATE] source/                     # Loaded-source data and coordinates owned by the Psi frontend.
|   |   |   |-- [CRATE] symbols/                    # Stable symbol identities and hierarchy storage.
|   |   |   `-- [CRATE] semantic-vocabulary/                       # Stable semantic/fuel ids and typed proposition vocabulary.
|   |   |-- representations/
|   |   |   |-- [CRATE] tokens/                     # Omega spelling-level token streams.
|   |   |   |-- [CRATE] syntax-trees/               # Parsed source shape before symbol resolution.
|   |   |   |-- [CRATE] symbol-resolved-trees/      # Source trees with resolved symbol identity.
|   |   |   |-- [CRATE] typed-trees/                # Typed source semantics without target realization state.
|   |   |   |-- [CRATE] facts/                      # Durable checked places, contexts, and semantic fact plans.
|   |   |   |-- [CRATE] flow-effects/                    # Target-neutral operational, reach, invocation, and capability-flow facts.
|   |   |   |-- [CRATE] checked-trees/              # Checked proof, borrow, flow, reach, value, and admissibility evidence.
|   |   |   |-- [CRATE] optimization/               # Closed pre-Terminal pass names, selections, encoding, and identity.
|   |   |   |-- [CRATE] lowered-psi/                # Unsealed semantics and proof/debug/source companions.
|   |   |   `-- [CRATE] terminal-psi/               # Self-contained Terminal Psi module and closed operation vocabulary.
|   |   |-- pipeline/
|   |   |   |-- [CRATE] source-files-to-tokens/     # Psi-owned Omega source lexer.
|   |   |   |-- [CRATE] tokens-to-syntax-trees/     # Psi-owned unresolved Omega parser.
|   |   |   |-- [CRATE] syntax-trees-to-symbol-resolved-trees/ # Psi-owned name and symbol resolution.
|   |   |   |-- [CRATE] symbol-resolved-trees-to-typed-trees/ # Psi-owned type/signature normalization.
|   |   |   |-- [CRATE] typed-trees-to-checked-trees/ # Psi-owned semantic checking and checked-fact construction.
|   |   |   |-- [CRATE] checked-trees-to-lowered-psi/ # Checked vocabulary lowering and source joins.
|   |   |   |-- [CRATE] lowered-psi-to-lowered-psi/   # Selected pre-Terminal optimization.
|   |   |   `-- [CRATE] lowered-psi-to-terminal-psi/  # Canonical publication and checked source-scope custody.
|   |   |-- compiler/
|   |   |   `-- [CRATE] terminal-production/         # Sequences the three stages and retains product receipts.
|   |   `-- semantics/
|   |       |-- [CRATE] validation/                 # Cross-semantic source validation and diagnostics.
|   |       |-- [CRATE] proof/                      # Source proof obligations, planning, and checking.
|   |       |-- [CRATE] proof-admission/            # Product-local Psi judgment and admission checking.
|   |       |-- [CRATE] checked-interpreter/        # Checked-tree build-time and transitional reference execution.
|   |       |-- [CRATE] build-time-evaluation/      # Const/domain evaluation and programmable plan normalization.
|   |       |-- [CRATE] terminal-semantics/         # Closed scalar, structural/effect, and call-composition policy rows.
|   |       |-- [CRATE] terminal-verifier/          # Module validation and reconstructed-obligation checking.
|   |       `-- [CRATE] terminal-interpreter/       # Fuel-bounded reference execution of verified terminal artifacts.
|   |
|   `-- [CRATE] omega/                                   # Current Rust `omega` product command.
|       |-- representations/                             # Durable, target-independent carriers and evidence.
|       |   |-- [CRATE] target/
|       |   |-- [CRATE] calling-conventions/
|       |   |-- [CRATE] {abstract,target,legalized,assigned-target}-operations/
|       |   |-- [CRATE] selected-instructions/
|       |   |-- [CRATE] machine-code/
|       |   |-- [CRATE] {register-model,optimization-core,optimization-unit}/
|       |   |-- [CRATE] {effects,installation-evidence,task-plans}/
|       |   `-- [CRATE] function-identity/
|       |
|       |-- pipeline/                                    # Every checked representation-to-representation transform.
|       |   |-- [CRATE] terminal-psi-to-abstract-operations/
|       |   |-- [CRATE] abstract-operations-to-abstract-operations/
|       |   |-- [CRATE] abstract-operations-to-target-operations/
|       |   |-- [CRATE] target-operations-to-selected-instructions/
|       |   |-- [CRATE] selected-instructions-to-selected-instructions/
|       |   |-- [CRATE] selected-instructions-to-register-homes/
|       |   |-- [CRATE] register-homes-to-post-allocation-machine/
|       |   |-- [CRATE] post-allocation-machine-to-post-allocation-machine/
|       |   |-- [CRATE] post-allocation-machine-to-selected-form-encoding/
|       |   |-- [CRATE] selected-form-encoding-to-resolved-layout/
|       |   |-- [CRATE] resolved-layout-to-resolved-layout/
|       |   `-- [CRATE] target-operations-to-assigned-target-operations/ # Alternate route still to delete.
|       |
|       |-- semantics/
|       |   `-- [CRATE] optimization-unit-semantics/       # Independent unit and rewrite checks.
|       |
|       |-- backend/                                     # Target/runtime primitives and backend-owned artifacts.
|       |   |-- [CRATE] register-environment/              # Shared target/ABI setup and validation.
|       |   |-- [CRATE] {layout,machine-emission}/          # Emission owns frame geometry and protocol.
|       |   |-- instruction_set_architectures/
|       |   |   |-- [CRATE] isa-{aarch64,x86_64}/
|       |   |   `-- [CRATE] x86-encoding/
|       |   |-- object/                                  # Object-file ownership.
|       |   |-- images/                                  # Image models, emission, and formats.
|       |   |-- artifacts/
|       |   |   |-- [CRATE] native-artifact/
|       |   |   `-- [CRATE] component-candidate/
|       |   |-- plans/
|       |   |   |-- [CRATE] backend-plan/
|       |   |   `-- [CRATE] program-entry-plan/
|       |   `-- runtime/
|       |       |-- [CRATE] {runtime-abi,executable-installation,external-roots}/
|       |       `-- [CRATE] component-publication/
|       |
|       |-- build/                                       # Build evaluation, composition, policy, and trust decisions.
|       |   |-- [CRATE] build-{declarations,evaluation,output}/
|       |   |-- [CRATE] {package-compilation,provider-planning,selected-dispatch}/
|       |   |-- [CRATE] component-deployment/
|       |   `-- [CRATE] trust-ledger/
|       |-- compiler/                                    # Thin product coordinator and result surface.
|       |   |-- [CRATE] compiler/                         # Source product and multi-target orchestration.
|       |   |-- [CRATE] native-realization/               # Terminal product realization with separately supplied authority.
|       |   `-- [CRATE] compilation-report/               # Completed product reports.
|       |-- packages/                                    # Registry-free package service and trust boundaries.
|       |   |-- README.md                                # Human entrance and dependency-direction map.
|       |   |-- manager/                                 # Command workflows, graph, and local admission policy.
|       |   |-- source/                                  # Immutable acquisition boundary.
|       |   |   |-- [CRATE] acquisition/                 # Source identity, snapshots, and custody.
|       |   |   `-- [CRATE] execution/                   # Bounded process lifecycle and cleanup for source acquisition.
|       |   `-- review/                                  # Human and compiler review surfaces.
|       |       |-- [CRATE] evidence/                    # Compiler-owned non-admitting semantic projection.
|       |       `-- [CRATE] advisory/                    # Optional model-facing source review.
|       |-- tooling/                                     # Auxiliary artifacts, profiles, visualizations, and host custody.
|       |-- src/                                         # Tiny `omega` product command.
|       `-- tests/                                       # Cargo integration tests for that product command.
|-- bootstrap/
|   |-- alpha/                                             # Alpha semantics and native VM seeds.
|   |-- beta/                                              # Trusted tape-assembly language and compiler.
|   |-- gamma/                                             # Typed scalar/effect language and Beta-written evaluator.
|   |-- delta/                                             # Typed pure functional compiler language.
|   |-- epsilon/                                           # Fixed-storage compiler-host language and evaluator.
|   `-- omega/                                             # Epsilon-written first Omega compiler D.
|       |-- compiler/                                      # D source members.
|       `-- compiler.epsilon.sources                 # Canonical D source-member manifest.
|-- source/
|   |-- library/                                           # Core, allocation, and standard library source.
|   |   |-- core/                                          # Always-available language package.
|   |   |-- alloc/                                         # Allocation facilities.
|   |   `-- std/                                           # Higher-level standard package surface.
|   |-- psi/                                               # Omega-written target-neutral phases through terminal Psi.
|   |-- omega/                                             # Omega-written product compiler implementation C.
|   |   |-- build.omg                                      # Product build/composition entrypoint.
|   |   |-- main.omg                                       # Product machine entrypoint.
|
|-- tools/
|   `-- bootstrap/                                        # Alpha/Beta materialization and path gates.
|
|-- samples/
|   |-- cli_mvp/                                        # Smallest console program.
|   |-- dungeon_crawler_cli/                            # Console input/output and room navigation pressure test.
|   `-- README.md                                       # Notes for sample expectations and local build output.
|
|-- tests/
|   |-- alpha/                                         # Alpha conformance and reference differential.
|   |-- beta/                                          # Beta compiler reconstruction and differential.
|   |-- gamma/                                         # Gamma evaluator and compiler-customer gates.
|   |-- bootstrap/                                     # Tests whose subject spans multiple rungs.
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
bootstrap/{alpha,beta,gamma,delta,epsilon}/ canonical language rungs
bootstrap/omega/                       Epsilon-written first Omega compiler D
source/library/                        core, allocation, and standard libraries
source/psi/                            Omega-written target-neutral phases through terminal Psi
source/omega/                          Terminal-Psi consumer and product root
omega-rust/                            current Rust product implementation and comparator
tests/{alpha,beta,gamma,bootstrap,omega,fixtures}/
                                      executable validation by subject
tools/bootstrap/                       bootstrap invocation and artifact construction
tools/                                 other repository maintenance scripts
```

Each rung remains the semantic owner of its language and chain-built
artifacts. A Rust producer nested beneath that rung is tooling for the same
concept, not a second semantic owner. The derivation checker belongs to Gamma.
Bootstrap compiler sources, tapes, and closed wire tables remain together under
`bootstrap/`; final Omega-written source remains under `source/`. Executable
validation is grouped by subject under `tests/` and names the canonical
source/artifact explicitly. Host materialization and deliberate
artifact replacement live under `tools/bootstrap/`. The removed
Epsilon-to-Delta/native-publication tree is not a validation precedent.

The package library now lives at `source/library/`. The relocation deliberately
has no compatibility symlink. Compiler task
`OPTIONAL-STDLIB-SEMANTIC-BINDINGS` in `TASKS.md` owns removal of the
temporary physical-path readers that were updated to this location during the
move. The final implementation resolves std only through the package graph and
keeps the compiler-owned build protocol independent of whether std exists.

## Placement Rules

### Front Door

- Product entrypoints stay thin. `source/omega/{build.omg,main.omg}` owns the
  hosted product entrypoint; the current Rust product package and command are
  rooted directly at `omega-rust/omega/`. The language-server and
  docs-generator are not separate products.
- `compiler/compiler` owns the top-level typed check/build coordinator.
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
  `omega-rust/psi/`. Psi crates must not depend on Omega
  crates; the architecture test enforces that firewall.
- Existing target-neutral `omega-*` crates are migration inputs, not a second
  permanent frontend. Move or rename them under Psi ownership as terminal
  vertical slices replace their source-shaped handles.
- The Omega backend role begins its long-term semantic consumption at terminal
  Psi and owns provider installation, ABI/storage realization, optimization,
  target lowering, and native execution machinery. Its current Rust realization
  is `omega-rust/omega/`. Psi owns both transitional
  checked-tree reference execution and canonical terminal-Psi interpretation;
  Omega contains only the cross-layer native differential-test harness. That
  harness keeps shared artifact decoding, verified lowering, and native image
  execution separate from responsibility-specific source differential families;
  it does not recombine semantic verification cases in its test root.
- `semantics/` owns language meaning: names, types, effects, proofs, facts,
  invariants, and validation. Resolved and checked type reasoning belongs to
  the typed/checked pipeline and `validation`; durable facts and proof
  obligations belong to `facts` and `proof`.
- `facts` carries checked facts, invariants, and refinement data: what
  remains true. Its `fact_plan.rs` root owns the current arenas and leads into
  place, context and evidence records. `validation::build_definition_fact_plan`
  derives declaration facts; representation queries do not construct that plan.
  Transitional backend consumers depend on the Psi owner directly until
  terminal-Psi slices replace them.
- `checked-trees` owns the durable checked semantic representation and its
  proof, borrow, flow, reach, value-origin, and admissibility evidence. Terminal
  lowering consumes it once; Omega backend crates consume Terminal Psi rather
  than checked trees.
- `flow-effects` carries target-neutral operational ceilings, service reach,
  synchronous invocation summaries, and capability-flow facts; target-neutral
  consumers depend on it directly. `effects` retains provider
  declarations, target/provider bindings, approval, and installation-facing
  records—including the exact selected-plan carrier—but no longer re-exports
  the Psi vocabulary. Checked semantic trees do not retain that Omega
  realization sidecar.
- `validation` answers target-neutral cross-semantic obligations, including
  who may read or mutate and what a callable requires or promises. It derives
  operational, reach and invocation summaries with private fixed-point work;
  `flow-effects` stores those summaries without depending on typed trees.
  Omega `provider-planning` derives service schemas from the checked declaration
  surface. The `effects` representation stores the schemas and their identities;
  provider installation and approval remain in Omega.
- `proof` plans and discharges source-level obligations.
- `compiler` invokes the Psi-owned frontend and canonical terminal-Psi
  producer directly. Production native realization begins from that immutable
  artifact. There is no StateGraph/control-flow fallback route.

### Representations And Pipeline

- `representations/` owns durable IR data structures and arena storage.
  Each Psi crate has one named entry beside `lib.rs`. Whole-program entries
  define their current program, such as `token_stream.rs` and `fact_plan.rs`.
  Shared vocabulary uses the same navigation without an invented aggregate IR:
  `flow_effects.rs` exposes independent summaries, and
  `optimization_selections.rs` owns selections and their closed pass catalog.
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

- Production backend lowering begins at `terminal-psi-to-abstract-operations`,
  then proceeds through ordinary `omega-*` operation, selection, allocation,
  machine-code, object, and image owners.
- The bounded `target-operations-to-assigned-target-operations` route is
  a temporary continuation for unsupported physical slices. It must converge
  into the selected-instruction continuation rather than become a second
  pipeline.
- The retired StateGraph/control-flow backend has been deleted. Reintroducing a
  source-shaped backend fallback is an architecture violation.
- `machine-emission` produces production machine code, and
  `backend/object/*` owns sections, symbols, and relocations.
- `selected-instructions-to-selected-instructions` owns selected-lowering
  rewrites and reusable selected-program analysis, including machine effects.
  It publishes the same current selected-program carrier for empty and nonempty
  selections, with replay evidence retained separately.
  `selected-instructions-to-register-homes` consumes that result, assigns homes,
  and owns allocation-pressure recovery. Effect facts are not another program output;
  `register-homes-to-post-allocation-machine` constructs the machine plan;
  `post-allocation-machine-to-post-allocation-machine` owns its opt-in rewrites.
  No construction stage depends on a later optimizer. The bounded target-to-assigned
  publication adapter is not a substitute for either owner and must disappear
  once the selected physical conveyor has complete operation coverage.
- `backend/instruction_set_architectures/*` owns ISA definitions and encodings.
  Only AArch64 and x86_64 exist today. Shared lowering policy belongs in shared
  backend crates.
- `runtime-abi` owns backend-ABI carrier shapes (fat descriptors, slices,
  text windows) and their accessors. `layout` and instruction selection
  consume those shapes rather than re-deriving them.
- `calling-conventions` owns ABI value passing rules.
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
  The repository's current optional host requirements, checked adapters, and
  target implementations live under `source/library/std`, but that package and
  decomposition have no compiler privilege and may be split or retired.
  [Retired: Host Package Scaffold](../design_briefs/retired_host_package_scaffold.md)
  preserves the migration fence for future dedicated provider packages; the
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

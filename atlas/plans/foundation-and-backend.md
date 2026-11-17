# Foundation and backend: migration plan (owner rulings of 2026-11-17)

Foundation should keep generic building blocks, and native compilation should
expose its work as representation-to-representation stages. The six
[rulings](../rulings.md) set that direction. This plan proposes the contracts and
order of the moves; its package names, stage boundaries and answers to open
questions are **judgment**, not additional owner rulings.

At source revision `3f9ce43f0c9d179048911d18faa91ba93f60176d`, the two foundation
folders contain 96,223 Rust lines in 26 crates, and the backend contains 223,507
in 29 crates. Counts include tests. [Generated counts](../evidence/data/counts.json)
are the authority for sizes; they do not measure complexity or the amount of
code a move can delete.

Part A separates primitive bytes and storage from language vocabulary and
program facts. Part B gives native preparation, encoding, layout, objects and
images explicit owners. Provider settlement belongs in this account too: it
closes over reached requirements before native lowering, but installation
binding also needs facts produced much later. The [Omega map](../subsystems/omega.md)
traces those operations before proposing a new order.

The migration must preserve accepted programs, refusals, artifact identities and
output bytes. This documentation update has not implemented or tested that
migration. Each implementation PR must prove its own part before deleting the
old route.

## 0. Method and how much to trust it

This review reads source at `3f9ce43f0c9d179048911d18faa91ba93f60176d`.
The [evidence manifest](../evidence/data/manifest.json) records the source
revision, scanner versions and exclusions. Reproduction commands live in the
[atlas README](../README.md#reproduce-the-evidence).

- **Crate edges:** `cargo metadata --format-version 1 --no-deps` reports 139
  workspace packages and 1,479 workspace path dependency declarations: 1,318
  normal, 1 build and 160 dev. A manifest edge establishes permission to use a
  crate; it does not prove a production function call.
- **Module uses:** [modscan.py](../evidence/scripts/modscan.py) and
  [modscan_be.py](../evidence/scripts/modscan_be.py) scan non-test Rust paths.
  They are evidence of observed references, not a Rust resolver. Re-exports,
  macros, glob imports and conditional compilation limit their conclusions.
- **Counts:** `all_rs_lines` includes all Git-tracked Rust files under a crate;
  `src_rs_lines` counts those under `src/` only. Backend totals are respectively 223,507
  and 221,538. The former plan mixed an all-files total with a `src/`-only
  scanner. It also compared source from `6de27da2a36d86f6c3883cb89527d0d44dea646a`
  with later code. Historical measurements are not fresh verification. Module sizes and
  proposed split sizes retained below from the original `6de27da…` inventory
  are historical estimates, **unverified** at the reviewed source; generated
  package totals are identified separately.
- **Ownership:** trace the function that creates a value, its containing
  representation, and the consumer. A type's defining crate alone proves
  neither a side channel nor where its computation belongs.

### Corrections that change the plan

| Earlier claim | Checked result and consequence |
| --- | --- |
| Six `target` foundation dependencies are unused | The old scan skipped every directory named `target`, including the real backend crate. `compiler-storage`, `diagnostics`, `language-core`, `layout-plans`, `optimization-core` and `semantic-vocabulary` are live there. Do not delete them from this evidence. [Examples][target-profile] [float catalog][float-target] |
| The documented backend scan reproduces backend usage | Its old default selected foundation unless `SCAN_DIR` was set externally. The repaired script and manifest must be used for backend move decisions. |
| `float_operations` has one user | `terminal-semantics` and backend `target` read its language catalog. Put its definition in carried vocabulary, not a single producing stage. [Target read][float-target] |
| P02 writes `declaration_selection` into foundation and P05 skips typed trees | The type lives in foundation, but symbol-resolved trees own the value, P03 carries it into typed trees, and P05 reads typed trees. Move the definition; do not invent a missing handoff. [Storage][selection-storage] [transfer][selection-transfer] [read][selection-read] |
| Anything pure can be target description | `evaluate_call_plan(policy, signature)` derives argument/result placements for a particular signature. That is program work. [Producer][call-plan] |
| Image emission normally replays earlier stages | The cited replay functions require explicitly retained inputs and refuse when absent. Default compilation drops those inputs; `EmissionFacts` is a separate, real bypass. [Retention][native-pipeline] [audit][audit] |
| Every realization action can run before O00 | Provider closure is early; O00 admits providers; installed-handler stack requirements are derived from emitted child objects and bound before the installer's O03. [Closure][closure] [admission][native-admission] [handler stacks][handler-stacks] |

“Unobserved dependency” means investigate, not delete. Confirm feature and target
configurations and then type-check the removal. In particular, the absence of a
regular-expression match is not safe evidence for moving a type to one stage.

Some original brief corrections still matter: `mutation-matrix` has an optional
`test-support` production declaration in `optimization-core`; `source-audit`
has dev users; `extents` has interpreter and runtime users; Build accounting is
also used outside the pipeline. Use the refreshed package graph and observed
references for their exact users, then inspect construction before moving them.

# Part A: Foundation keeps only primitives

## 1. The rule and what decides each destination

### What the 2026-11-17 ruling says

Foundation keeps only **primitives**. A primitive is a generic building block
with no Omega-language or compiler meaning: arenas, handles and dense ID tables,
the record codec, wire bytes and digest encoding, plain diagnostics carriers,
source text positions, and big-integer arithmetic.

Foundation has no language layer. A language table (intrinsic catalogs,
spellings, the scalar and proposition term language, numeric and float rules,
the optimization catalog, access and layout rules) moves into the first
pipeline stage or representation that needs it. Later stages then carry it
forward. A crate that no pipeline stage uses leaves for its actual owner, even
if that splits the crate.

### The four verdicts

| Verdict | Test | Text that decides it |
| --- | --- | --- |
| **primitive** (stays in `psi/foundation/`) | Generic building block with no language or compiler meaning, used by the pipeline | The ruling |
| **move into `<stage or representation>`** | A language table, or a table, handle, plan or summary that a stage fills from the program. Goes to the first representation that needs it; a private helper used by one stage stays there; a value handed to its neighbor belongs to their representation contract. | The ruling. Also AGENTS.md "Representation-to-representation pipeline: … Carry needed semantic facts in that representation; do not bypass the pipeline with sidecars". |
| **move to `<non-pipeline owner>`** | No pipeline user | The ruling. Also pipeline.md table: "Build-time execution inputs and accounting → Psi `compilation/build-time-evaluation/`"; "Source acquisition and package storage primitives → Omega `packages/`". |
| **delete** | Unused, duplicated, or re-exported | AGENTS.md rule 6: no second copy of a data structure |

### Proposed representation packages

1. **`psi/compilation/representations/terminal-psi/vocabulary`**
   - **What it is:** the "carried vocabulary" of Terminal Psi, abbreviated
     **TPV** below.
   - **Why it is needed:** many tables are first used by a source-side form
     (`syntax-trees`, `symbol-resolved-trees`, `typed-trees`) but are also read
     by Terminal Psi, Omega, realization or the interpreter. The firewall
     (`boundary.rs:352-358`, `PERMITTED_OWNERS`) forbids those readers from
     reaching source-side forms, so such a table cannot live in the source-side
     form that first needs it.
   - **Judgment:** put shared definitions in the carried representation
     under the Terminal Psi family. The source-side forms depend on it, and the
     Terminal and Omega sides already may reach it, because `terminal-psi/*` is
     in `PERMITTED_OWNERS`.
   - **It follows the existing precedent** of `native-vocabulary`: "owns the
     leaf types every native form carries unchanged".
   - **It should be split by subject** into nested packages (*judgment*):
     `identities`, `scalars` (term language and numeric rules), `declarations`,
     `resources` (access and extents), `layout`. The original rough module estimate was 40–45k lines; an exact post-split
     total is unverified. Keeping subjects separate makes their permitted
     dependencies and constructors reviewable.
2. **`psi/compilation/representations/build-requests`**
   - **What it is:** a proposed carrier for Build evaluation requests and
     results consumed during source preparation: selection requests, executed
     results, product targets and operands, and compiler-evaluation requests.
     Q-B recommends projecting only required facts into later forms, not
     granting Terminal and native consumers access to the whole Build input.
   - **Why it is a Psi package:** `build-time-evaluation` (Psi) produces it, and
     the firewall forbids Psi from depending on Omega.
3. **`psi/compilation/representations/source-files`** (*judgment*)
   - **What it is:** the input of stage 00, which is already named
     `source-files-to-tokens`. It holds `LoadedSource(s)`, `SourceContents`,
     the captured source tree and snapshot (`capture.rs`), `read_accounting`,
     `SourceCatalogue`, and loading-specific identity/custody. Split the small consumed-source
     identity vocabulary needed by Terminal readers into a shared leaf; Q-B
     does not grant them access to loaded files or source acquisition.
   - **Why it is not foundation:** these carry package and source-loading
     meaning, not plain text positions.

### The firewall still decides two things

Psi never depends on Omega (`boundary.rs:228-233`). So:

- A Psi-side table used mainly by Omega still goes to a Psi representation.
- An item with no pipeline user goes to an Omega owner only when no Psi crate
  uses it.

### Text the ruling supersedes

The implementation sequence rewrites applicable instruction text with the corresponding move (AGENTS.md "Scope and escalation"):

- pipeline.md:1057-1063: "Shared foundations … stay independent libraries. Do
  not fold them into a producing stage". Superseded for foundation; the rule
  against merging *stages* stays.
- pipeline.md:1086-1089, 1110, 1440-1447 and 1604-1614: the foundation rows and
  the flow-effects, optimization and function-identity placements.
- `psi/compilation/representations/README.md`: "Shared vocabulary belongs in
  language-semantics", and the flow-effects and optimization paragraph.
- `omega/foundation/README.md` and `psi/foundation/*/README.md` for every crate
  that moves.
- `numerics/README.md`: it becomes the `bignum` README.

## 2. What foundation keeps

Package line counts include all Git-tracked `.rs` files under each crate directory, including in-file
and `tests/` code, as recorded by the tracked-file counter in
[evidence](../evidence/data/counts.json). Rows marked ≈
are estimates from module line ranges.

**Before:** 96,223 lines in 26 crates.

- `psi/foundation`: 84,940 lines, 21 crates including `compiler-storage-derive`.
- `omega/foundation`: 11,283 lines, 5 crates.

**Proposed result (estimate):** about 15k lines in seven libraries plus
`compiler-storage-derive`, all under `psi/foundation/`. Exact totals require the
module moves; no reduction has been implemented. `omega/foundation/` is deleted, because Omega has no
primitive of its own: its only one, the wire helpers, duplicates Psi's.

| Primitive crate | Content | Lines | Note |
| --- | --- | --- | --- |
| `arena` | arenas, handles, paged and rooted arenas; plus `semantic-vocabulary::id_table` (`IdTable`, `IdSet`) and the generic hasher from `symbols::fast_map` | 8,402 + 370 + 89 | Split the generic hasher from `SymbolMap<V>`, whose key is `SymbolHandle`; keep the symbol alias with TPV/symbol identities. [Definition][symbol-map] |
| `compiler-storage` (+`derive`) | record codec | 1,767 | |
| `diagnostics` | diagnostic carriers and display | 598 | The parse helpers (`expected`, `parse_error`, `unexpected_eof`, P01 only) move to P01 |
| `source` | `Span`, `SourceId`, `SourceSpan`, `SourcePosition`, `SourceFile`, `SourceBytes`, `SourceMap`, `display_literal_bytes` | ≈1,100 of 2,556 | *judgment*: generic text positions. `capture.rs` (884) and `source_identity` (≈470) move to `source-files`. |
| `wire` (new) | generic byte cursors, digest encoding, record helpers and `fnv1a` | estimate pending extraction | Merge the byte-level copies. `identity_encoding` and `structural_path_encoding` encode language types and paths; they stay with TPV and its codec. [Type encoder][identity-encoding] [path encoder][path-encoding] |
| `bignum` (today `numerics/src/bignum.rs`) | `BigInt`, `BigRational` | 1,689 | `ExactFloat` (P05 only) and `IntegerBitwiseOperation` (P02 only) have Omega meaning: move them to their stage (*judgment*) |
| `execution-admission` | invocation worker budget | 138 | *judgment*: thread and stack budgeting with no compiler meaning, used by P00, P01 and P06 |

## 3. Verdict for every crate

Abbreviations: P = Psi stage, O = Omega stage, `tp` = terminal-psi family,
`tt` = typed-trees, `srt` = symbol-resolved-trees, `NV` = native-vocabulary,
`TPV` = `terminal-psi/vocabulary`.

| Crate | Lines | User evidence to inspect | Verdict (judgment) | Evidence |
| --- | --- | --- | --- | --- |
| psi `arena` | 8.4k | P00–05, srt, tt, tp, O00, selected-instructions, backend, packages, build | **primitive** | |
| psi `compiler-storage` (+`derive`) | 1.8k | See generated graph and usage data | **primitive** | |
| psi `diagnostics` | 0.6k | See generated graph and usage data | **primitive**; parse helpers → P01 | |
| psi `source` | 2.6k | See generated graph and usage data | **split**: positions stay **primitive** (*judgment*); `capture.rs`, `LoadedSource*`, `SourceContents`, `read_accounting`, `SourceCatalogue`, `DependencyScope`, `SourceOrigin`, `SourceResolutionStratum`, `InvocationSourceConsent` → **`source-files`**, with Terminal-needed source identity separated into TPV | `source_identity` users include `terminal-semantics`, so the retained identity must be separable from source loading (Q-B). Today `source → semantic-vocabulary` (`PackageKeyIdentity`, `lib.rs:44`): this primitive depends on a language crate; the move removes the edge. |
| psi `execution-admission` | 0.1k | P00, P01, P06, source-compiler, compiler, cli, orchestration | **primitive** (*judgment*) | |
| psi `numerics` | 6.8k | See generated graph and usage data | **split**: `bignum` stays **primitive**. `literals`, `arithmetic`, `integer_carrier`, `float_semantics`, `float_projection` → **TPV** (`scalars`): first user is P01/`syntax-trees`, but O00, backend, interpreter and runtime also read them. `scalar_predicates`, `integer_bounds` → **`terminal-psi`** (users: tp + terminal-installation only). Delete the `float_semantics_catalog` alias. | Its README: "[Numeric values] defines the language contract", so the numeric rules are language, not primitive. |
| psi `language-core` | 6.2k | See generated graph and usage data | **split** (§4.3): source-only tables → `syntax-trees` / `srt` / `tt`; tables read on both sides → **TPV**; single-user items → their stage | |
| psi `semantic-vocabulary` | 12.1k | See generated graph and usage data | **split** (§4.1): wire and ID-table helpers → primitives; everything else → **TPV** / `terminal-psi` / owners | |
| psi `language-semantics` | 11.8k | See generated graph and usage data | **split** (§4.2); moves the already-carried `declaration_selection` type | |
| psi `symbols` | 9.6k | See generated graph and usage data | **split** (§4.4): `SymbolTable` → `srt`; handles and builtins → **TPV**; generic `fast_map` hasher → `arena`; symbol-keyed alias → TPV | |
| psi `optimization` | 0.4k | See generated graph and usage data | **split**: wire helpers → `wire` (delete the copy); `optimization_selections` (`PsiOptimizationSelections`, used by `terminal-wire`, P06, O01, `optimization-core`) → **TPV** | Terminal Psi carries the selection identity |
| psi `flow-effects` | 1.1k | See generated graph and usage data | **move into representations**: `InvocationTarget` → `tt`; `ConditionalCarryRequirement`, `ContractDependency`, `template_contract_commitment`, `validate_published_dependency`, `CapabilityFlowKind` → **`terminal-psi`**; `installation_reach` → **`terminal-installation`** (*unverified*: whether provider-selection may take that edge) | Checker output |
| psi `access-plans` | 5.8k | tt, tp ×3, O03, build-time-evaluation, interpreter, terminal-runtime | **move into TPV** (`resources`) | The rules are the access spec and the plan values are program-derived; both cross from `tt` to O03. |
| psi `layout-plans` | 8.2k | See generated graph and usage data | **split** (§4.5) | |
| psi `extents` | 4.9k | See generated graph and usage data | **split** (*judgment*): identities and rights that `access-plans` names (`Extent`, `ExtentRights`, `ExtentLoan`, `LoanPolarity`, `AddressSpaceId`, `Extent*Id`, `MappingEraId`) → **TPV** (`resources`); `compose_claim_bounds`, `ClaimBoundRow`, `ActivationClaimBranch` → **`terminal-semantics`**; ledgers, root grants, provider issuance, `mapping.rs` → **`psi/interpreter`** | `terminal-runtime` already depends on the interpreter |
| psi `evaluation-accounting` | 1.5k | pipeline: `terminal-work` only | **split**: `invocation.rs` → **`terminal-psi/work`**; `lib.rs` sponsor, limits and usage → **`build-time-evaluation`** (nested package `build-time-evaluation/accounting`, so `package-manager` does not pull in the evaluator) | pipeline.md table row on Build accounting |
| psi `build-time-values` | 0.1k | build-time-evaluation, provider-planning, language-semantics::native_bindings | **move to `build-time-evaluation`** after `native_bindings` → `build-settings` | `provider-planning → build-time-evaluation` is already allowed (`FIREWALL_ALLOWANCES`, boundary.rs:105) |
| psi `filesystem-authority` | 2.7k | no pipeline user | **split, move to owners**: `filesystem.rs` and `filesystem_sponsor.rs` → `psi/compilation/build-time-evaluation/filesystem-authority` (nested package; Psi's Build owner, used by `build-time-evaluation` and Omega build and packages); `source_capture.rs` (package-compilation and source-acquisition only) → `omega/packages/source-acquisition`; `lib.rs` `BuildOutputObligation*` (package-compilation only) → `omega/packages/package-compilation` | |
| psi `mutation-matrix` | 0.1k | test-support only | **move to `tools/test-support/mutation-matrix`**; delete the `optimization-core` re-export | |
| psi `source-audit` | 0.1k | no production user; four dev users | **move to `tools/test-support/source-audit`** | |
| omega `function-identity` | 0.15k | See generated graph and usage data | **move into `NV`** | O00 output carried by every native form; removes the only foundation → representation edge |
| omega `optimization-core` | 5.8k | See generated graph and usage data | **split** (§4.6): wire helpers → `wire`; the catalog → **Omega pipeline (`NV`)**; single-owner parts → their representation or products | |
| omega `artifacts` | 0.3k | no pipeline user; compiler, products, build-settings, orchestration, package-compilation, package-manager | **move to `omega/compilation/products/measurements`** (nested package; *judgment*). Products owns reports. A nested manifest avoids the `products ↔ package-compilation` cycle, because `products` depends on `package-compilation`. | pipeline.md:1463 |
| omega `platform-custody` | 1.1k | no pipeline user; cli, orchestration, products, packages ×3, backend `native-artifact` | **move to `omega/packages/platform-custody`** | pipeline.md table: "Source acquisition and package storage primitives → Omega `packages/`". `native-artifact`'s one use (`package_publication.rs:17`, `make_executable`) is product publication; move that call to `products` in the same PR (*unverified*: size of the move). |
| omega `bounded-process` | 3.8k | no pipeline user; cli, packages (resolver-execution, source-acquisition), `native-provider-runtime`, xtask, bench, sample-runner | **split, move to owners**: `private_observation*.rs` (cli only) → **cli**. The rest: the native-provider runtime, package resolvers and dev tools share one process runner. **Remaining question Q-A.** | The runtime boundary (`boundary.rs` `NATIVE_PROVIDER_RUNTIME_ROOT` check) does not let `native-provider-runtime` depend on a `packages/` crate. |

## 4. Module-level splits for the wide crates

Destinations below propose homes for definitions and carried records. A module
that also computes program facts must split its producer into the stage that
has those inputs; moving all its functions into a representation would not
satisfy the ruling. Observed users in these tables identify where to investigate; they are not
a claim of resolver completeness. Use the refreshed usage data and inspect
constructors before each move.

### 4.1 `semantic-vocabulary`

| Module or types | Observed use | Destination |
| --- | --- | --- |
| `fnv1a` | observed cross-pipeline users | **primitive** `wire` |
| `identity_encoding`, `structural_path_encoding` | encode `ScalarType` and `CanonicalStructuralPathSegment` | **TPV and its codec**, not primitive wire; both assign meaning to semantic tags. [Source][identity-encoding] [source][path-encoding] |
| `id_table` | See refreshed usage data | **primitive** `arena` |
| `PackageKeyIdentity` | source loading and retained provenance consumers | **shared provenance leaf**, separate from loaded files; source-files and Terminal vocabulary may depend on it (Q-B). |
| Terminal identities (`BlockId`, `OperationId`, `EdgeId`, `PlaceId`, `ValueId`, `ClaimId`, `ObligationId`, `PropositionId`, `ContractId`, `CallableContractId`, `BoundaryMachineId`, `MachineDescriptorId`, `ScalarApplicationId`, `StructuralCaseId`, `StructuralDomainId`, `ScalarDomainId`, `ScalarQualificationSetId`, `SuspensionCrossingId`, `CycleComponentId`, `DomainSemanticId`, `EvidenceIdentity`, `EvidenceTermId`, `ContentDomainId`, `AdmissionSiteId`, `SelectionDeclarationId`, `SelectionQuestionId`, `ProfileDecisionId`, `CallingAbiIdentity`, `CallableInvocationSite`, `BoundaryRequirementKind`, `WorkScheduleIdentity`) | Terminal, native and runtime readers; keep leaf identity access separate from program access | **`terminal-psi`** (stage 05 output) or **TPV/`identities`** (*judgment*: TPV keeps the ISA crates off the big `terminal-psi` crate) |
| Identities that `tt` also uses (`MachineId`, `ServiceId`, `StructuralTypeId`, `StructuralFieldId`, `InvocationSite`, `BoundaryCompletion`, `ExternalCompletionKind`, `Calling*`) | See refreshed usage data | **TPV/`identities`** |
| Term language: `scalar_terms`, `closed_integers`, `proposition`, `float_meaning_clauses`, `float_integer_conversions`, `scalars::*`, `storage::*`, `behavior_policy`, `byte_predicates`, `scalar_condition_role`, `Termination`, `CallableCrashPolicy`, `ScalarAddressModel` | See [refreshed usage data](../evidence/data/usage.json); `tt` uses `proposition::integer_types` | **TPV/`scalars`**. Not a primitive (owner ruling). |
| `proof_applications`, `PsiSemanticId`, `TargetMachineCallable`, `CallableShapeMismatch`, `boundary::callback_identities` | See refreshed usage data | **TPV** / **`terminal-psi`** |
| `ForeignLocatorCandidate`, `NativeBindingLocator`, `compiler_evaluation`, `source_selections`, `boundary::program_entry` | Build, products, packages, build-time-evaluation, terminal-installation, terminal-production, `tt`, `tp` | **`build-requests`** |
| `declaration_descriptions` | build-settings, source-compiler, `tt` | **`tt`** |
| `source_review` | review-evidence only | **`packages/review-evidence`** |

### 4.2 `language-semantics`

| Module | Observed use | Destination |
| --- | --- | --- |
| `declaration_selection::{AuthoredDeclarationSelection*, ResolvedAuthored*, CompilerDerivedSelectionPartition, CollectionMeasure, EnumExclusionRequest, ExclusionVocabulary, RootBindingRequest, is_unresolved_provider_selection_call}` | srt, tt, P02, P03, P05 | **a leaf carrier in the `srt` family**, shared by `srt` and `tt` (judgment); do not import the entire earlier tree merely to name a ledger. P03 already transfers the ledger and P05 reads `tt`. |
| `declaration_selection::{Build*, Executed*, DescribedProduct*, NativeBinding*, CapturedNativeIntent, RootBinding*, ProviderReadinessRequest, AuthoredRootGrant, AuthoredBehaviorExclusion*, AssumedBindingContract, ProductDeclaration*, ProductOperandRole, ProductTarget, CompositionMode, product_operands}` | Build, products, compiler, build-time-evaluation, packages, provider-*, tt, P03–P05, `terminal-psi` (`CompositionMode`) | **`build-requests`**. The Terminal Psi use of `CompositionMode` raises Q-B. |
| `semantic_identities`, `semantic_domains`, `service_reach`, `machine_termination` | srt, tt, P02–P05, flow-effects, build-time-evaluation, review-evidence | **`srt`** (*judgment*: check whether P02 or P03 writes them). `BoundaryQualificationIssuer` → provider-selection. |
| `external_bindings`, `callback_declarations`, `service_surface`, `opaque_representations`, `build_requests`, `reflection`, `type_identity`, `const_value`, `numeric_intrinsics`, `byte_predicates` | each spans `tt` and `tp`/backend/realization | **TPV/`declarations`**. Merge `opaque_representations` with `typed-trees::opaque_representations` (pipeline.md:1413 names the latter as the owner). |
| `trust_declarations`, `provider_declarations` | `tt` + trust-model / provider-selection / provider-planning | **TPV/`declarations`**. Inspect the actual Terminal publication fields and retention route before moving them. Shared declaration definitions alone do not prove a sidecar. Q-G also covers calling-policy preparation from checked source custody. |
| `quotient_correspondence`, `conformance_identity`, `permissions`, `SuspensionValueOrigin` | tp (+P05, flow-effects) | **`terminal-psi`** / **`terminal-semantics`** |
| `value_domain`; `wire` | tt, P03; build-time-evaluation, source-compiler, P04 | **`tt`** (both; *judgment* for `wire`) |
| `boundary_bodies`, `boundary_provider_review` | provider-selection only | **`realization/provider-selection`** |
| `boundary_values` | backend `layout` only | **native settlement/layout producer and its carrier leaf** (§B6) |
| `native_bindings` | build-settings only | **`build-system/build-settings`** |
| `content` | optimization-unit-semantics only (non-test) | **`optimization-unit-semantics`** (*judgment*) |
| Re-exports of `language_core::{Carry*, …}` (`lib.rs:2437`) | See refreshed usage data | **delete** |

### 4.3 `language-core`

| Item | Users | Destination |
| --- | --- | --- |
| `operator_spelling`, `cast_form`, `source_semantics::receiver_binding`, `constant_value::EvaluatedConstant` | syntax-trees, srt, tt, P01–P05 (+ review-evidence, terminal-production) | **`syntax-trees`**: the first representation that needs them |
| `data_layout_queries`, `source_semantics::{contract_result, place_root}`, `core_vocabulary` | tt/srt, P02–P05, build-time-evaluation, `symbols` | **`tt`** / **`srt`**, following the first user |
| `atomic`, `inline_assembly`, `sealed_intrinsics`, `static_interrupts`, `source_semantics` (`Carry*`, `Multiplicity`, `DomainClassification`, …) | read on both sides (tp, O00–O03, backend, realization) | **TPV/`scalars`** or **TPV/`declarations`** |
| `descriptor_tables` / `layout_storage` | `terminal-psi` / P02 in the observed scan | that user, after feature and constructor review |
| `float_operations` | `terminal-semantics` and backend `target` | **TPV/scalars**; both read the same language catalog. [Target consumer][float-target] |

### 4.4 `symbols`

| File | Users | Destination |
| --- | --- | --- |
| `table.rs`, `storage.rs`, `ParameterOccurrence` | srt, tt, P02, P03, build-time-evaluation, source-compiler, package-compilation | **`srt`** (P02 output) |
| `symbol.rs` (`Symbol`, `SymbolHandle`, `SymbolKind`, `SymbolPath`), `retained_declaration.rs` | Includes tp, check-tables, trust-model and provider-planning; see refreshed usage data | **TPV/`identities`** |
| `builtin.rs`, `primitive.rs`, `sealed_intrinsic_at` | O00, backend effects, review, build-time-evaluation, tt, P02–P05, provider-admission | **TPV/`declarations`** (language tables read by O00) |
| `fast_map.rs` | See refreshed usage data | **split**: generic hashing → `arena`; `SymbolMap<V>` alias → symbol vocabulary because it names `SymbolHandle`. [Source][symbol-map] |

### 4.5 `layout-plans`

| Module | Destination |
| --- | --- |
| `layout_reports` | **TPV/`layout`** (program-derived; written in `tt`, `terminal-psi`, `terminal-wire`, build-time-evaluation and the interpreter) |
| `aggregate_geometry` | **TPV/`layout`**: language layout rules, not a primitive |
| `placement`, `materialization`, `symbolic_values`, `field_values`, `field_codec`, `stored_integer_writes` | **`terminal-psi`** family: portable materialization actions (pipeline.md:1490) |
| `private_callback_placements` | **Omega plan carriers/producer** (§B4): all observed users are Omega |
| `target_facts::LayoutTargetFacts` | **a shared semantic layout leaf**, carried as admitted facts where needed; Build requests may name the same type without exposing their complete state to Terminal readers (Q-B). |

### 4.6 `optimization-core`

| Module | Destination |
| --- | --- |
| `codec_bytes`, `cursor`, `digest_encoder`, `error_display`, `record_views`, `tagged_vocabulary`, `variant_accessors` | **primitive** `wire` (merged with the psi copy) |
| `optimization_core.rs` (the catalog), `contracts/{rule, budget}`, `manifest/work_usage`, the stage-manifest identities, `identities/{rules_and_facts, decisions, bundle}` | **`NV`**. The catalog moves into an Omega representation (judgment applying the ruling). `NV` is the leaf every native form O00–O09 carries (*judgment*: the first Omega form is `abstract-operations`, but O03 and later cannot reach it under `boundary.rs` `EARLIER_PROGRAM_FORMS`). `build-declarations` and `package-compilation` then depend on `NV` (*unverified*: no rule forbids it today). |
| `decisions/*`, `manifest/{decision, pass, fact_reference, error}`, `contracts/{decision, analysis}`, `CanonicalIdentityEncoder`, `OptimizationIdentityBundle` | **`abstract-operations/optimization-unit`** |
| `post_terminal` | **`verified-optimization-input`** |
| `identities/artifacts` | **`machine-code`** |
| `identities/stages` | **final native artifact carrier** (§B3/B6) |
| `refusal_site` | **`target-vocabulary`** (*judgment*) |
| `rollback`, `report_request` | **`compilation/products`** (*unverified*: `package-compilation` uses `report_request` but does not depend on `products`, so it may need `products/measurements`-style nesting) |
| `mutation_matrix` re-export | **delete** |

## 5. Ordered PR sequence (contracts before moves)

These are proposed implementation PRs, not completed work. Each move deletes
its old module and updates the instructions that name it. Run `check-boundary`
and `check-rules`, lint touched Rust crates, and test the moved operation and
its direct consumers. Compiler contract changes also need a focused accepted
and refused program; a path-only move needs byte/codec equivalence where that
is the contract. Do not equate a docs build with these checks.

| Step | Depends on | Change and deletion | Proof specific to the move |
| --- | --- | --- | --- |
| A0 | none | Record reviewed source and complete scan coverage; distinguish definitions, value storage and producers. Add boundary expectations with explicit temporary allowances for current paths. | Scanner covers the real `backend/target`; graph and counts reproduce. |
| A1 | A0 | Remove only individually confirmed unobserved dependencies. There is no promised count of deletions. | Feature/target checks cover each removed edge. |
| A2 | A0 | Move `source-audit` and `mutation-matrix` to test support; retire the old re-export and feature only after migrating their actual tests. | Existing source guards and image mutation tests still run. |
| A3 | A0 | Extract generic wire helpers, `fnv1a`, ID tables and generic hashing. Keep semantic type/path encoders with semantic definitions until A11. | Existing codec bytes and digest identities agree. |
| A4 | A3 | Move `function-identity` into a leaf native vocabulary; delete the old crate. | No reverse edge from vocabulary to a program form. |
| A5 | A0 | Move genuinely private single-user helpers; parsing diagnostics to P01. Exclude `float_operations` and shared handoff types. | Receiving operation tests and all observed consumers. |
| A6 | A0 | Move host/process code after Q-A's owner is settled; split filesystem custody, publication measurements and Build accounting by actual consumer. Keep carrier packages separate from drivers. | Runtime firewall; accounting exact-limit and one-unit-short cases. |
| A7 | A3, A4 and required TPV leaves from A11 | Split `optimization-core`; move product/report code to a products leaf. Delete its crate only after every catalog, identity and codec consumer has moved. | `OMGOPT`, `OMGDEC`, `OMGBDL` bytes and existing optimization identities. |
| A8 | A0, Q-B contract | Extract Build request carriers; project the few admitted values later stages require into their current representation. | No native dependency on a Build evaluator or mutable Build result; identities preserved. |
| A9 | A0, Q-B contract | Split source loading/capture from primitive spans and retained source identity. | P00 source identity and capture tests; Terminal reads provenance without source acquisition. |
| A10 | A3 and relevant A11 leaves | Move source-only language tables and declaration-selection definitions to their owning representation family. Preserve the existing P03-to-P05 ledger transfer. | Handoff retains exact occurrence IDs; no new stage-to-stage dependency. |
| A11 | A3; each leaf before its consumer | Extract TPV by dependency order: identities; scalar and path vocabulary/codecs; declarations; resources; layout. Move checker/work/materialization values to their actual Terminal forms and runtime ledgers to the interpreter. | Firewall and existing wire round trips; moved producers still retain required facts. |
| A12 | A2–A11, all consumers moved | Rename the remaining `numerics` primitive to `bignum`; delete empty semantic crates and then `omega/foundation/`. Q-A must be resolved before deleting that directory. | No old path or re-export remains; closed foundation list passes. |
| A13 | each preceding PR | Remove that PR's temporary allowance and update the usage report as the boundary lands. | A regression in the moved area fails immediately; there is no final-only enforcement switch. |

A11 is a set of leaf-first moves, not one indivisible prerequisite for A7/A10.
A representation's required scalar, identity or plan definition must exist
before its producer moves. The native target and provider contracts in Part B
share those leaves, so their consumer rewrites cannot be treated as unrelated
parallel work.

## 6. Boundary rules (in `tools/xtask/src/boundary.rs`)

These proposed rules enforce structural facts. Review still decides whether a
function computes new program facts; a name pattern cannot prove that.

| Rule | Proposed check | Migration condition |
| --- | --- | --- |
| F1 | A primitive depends only on primitives and external generic libraries. | Split source identity and semantic numeric helpers before tightening the corresponding edges. |
| F2 | Foundation has an explicit reviewed list: `arena`, `compiler-storage`, `compiler-storage-derive`, `diagnostics`, primitive `source`, `wire`, `bignum`, `execution-admission`. No Omega foundation remains. | Remove old entries as A2–A12 land; final list closes only when its consumers moved. |
| F3 | Generate an observed-use report with scanned file coverage and compare it in review. | Unobserved references create investigation items, not automatic move/deletion decisions. Type checks and constructor tracing remain required. |
| R1 | Producing stage crates are called only by declared sequencers. Distinguish dev calls and type imports from production calls. | Build-time evaluation directly runs P04 work; its P02/P03 manifest edges are not evidence that both run in production. Resolve Q-C before removing its allowance. |
| R2 | Representations use explicit leaf dependencies, never a stage or an earlier whole program as hidden input. | Proposed leaf order is primitives → shared Psi identities/vocabulary → native vocabulary → target description → provider-plan carriers → native program forms. Build/source request carriers are separate source-side inputs; they are not automatically visible to Terminal/Omega. |
| R3 | Keep the Psi/Omega firewall and restrict native runtime access to representation carriers and approved host support. | Remove `omega/foundation` and backend paths incrementally. Q-B grants no blanket access to loaded sources; Q-A requires an explicit host-support rule. |

Q-D distinguishes shared definitions from program access. Stable identities may
be named by several forms without re-exporting them through every predecessor.
A stage still receives the current program, including the facts it needs. The
current `verified-optimization-input → target-operations` and
`selected-instruction-analyses → register-homes` edges need carrier splits;
renaming folders does not repair their direction. Likewise,
`verified-optimization-input` currently performs provider closure and task
realization [inside its constructor][native-proposal]. Move that work to a
producer before declaring it a passive representation. The existing
`terminal-semantics` and `terminal-wire` edges to `proof-admission` also require
an explicit carrier-versus-checking interface decision; moving definitions
without resolving those edges does not complete R2.

# Part B: Backend dissolves (owner ruling 2026-11-17)

The ruling: `omega-rust/omega/compilation/backend/` should not exist, because
native compilation is one linear pipeline.

Today the backend is 223,507 lines in 29 crates: all `.rs` lines, counted the
same way as §2. Every Omega stage except O01 depends on backend crates directly,
and so do 6 of the 7 native representation families (all except
`native-vocabulary`). After Omega 09, the native driver keeps working outside
the numbered pipeline: it calls `machine-emission`, `object-file` and
`image-emission`, which turn the program into an object and an image.

The plan:

- Give that work numbered stages (O10–O14) with their own representations.
- Fold encoding into Omega 07.
- Turn the selected target's description into an **input representation** that
  Omega 00 receives and the native forms carry. This is the Omega-side analogue
  of `build-requests`, and it answers Q-E.
- Carry program-derived provider plans in the output of explicit settlement and admission phases; bind later installation facts when their inputs exist.
- Move the rest to products, realization, the interpreter or tests.

## B1. What `boundary.rs` and pipeline.md say today, and what the ruling supersedes

pipeline.md and AGENTS.md describe the backend as a library layer:

| Location | Text |
| --- | --- |
| pipeline.md:1087 | "ISA, ABI, runtime carriers, object/relocation/image mechanics → Omega `compilation/backend/`" |
| pipeline.md:1128-1131 | "In backend, target descriptions, calling conventions and register models belong to backend itself, installation and task evidence to backend runtime, boundary-application companions to backend artifacts, and provider authority evidence to backend plans" |
| pipeline.md:225-227 | "Each transform … uses the semantic, foundation and backend libraries that own reusable operations" |
| pipeline.md:166-167 (connected-route table) | "Resolved program → machine bytes: machine-emission" and "Machine bytes → object/image evidence: image-emission". Two route rows have no numbered stage. |
| pipeline.md:1097 | "Target-aware interpreter host adapters … reusing independent backend ABI/mechanism libraries" |
| AGENTS.md lane table | `backend` lane: "Target, ISA, ABI, object/image encoding, layout, and execution primitives in `omega-rust/omega/compilation/backend/`" |

`boundary.rs` names backend paths in five places:

- `PERMITTED_OWNERS` (line 29) and `OMEGA_COMPILATION` (line 71).
- 9 `STAGE_CALL_ALLOWANCES` rows (lines 125-163). Here `image-emission`,
  `native-artifact` and `machine-emission` call numbered stages; the comment
  says the list "can only shrink. Never add a row."
- The trust-model and package-manager forbidden lists (lines 279 and 303).
- `AFTER_PREPARATION` / `SELECTION_REPRESENTATIONS` (`bound-installations`,
  `register-environment`; lines 477-538).
- The replay-gateway file lists (lines 768-801, 1025-1029).

Each implementation PR rewrites the paths and allowances it actually removes (AGENTS.md "Scope and escalation").

## B2. Where the driver leaves the numbered pipeline (verified in source)

`native-compiler/src/pipeline.rs:150-260` and
`native-artifact/src/fragment_object_publication.rs:170-190` run this sequence:

```
O06 stage_optimized_post_allocation_machine_plan
  → machine_emission::stage_function_frame          (frame geometry + prologue/epilogue bytes → FramedMachineProgram)
O07 stage_optimized_layout_independent_selected_form_encoding(framed)
O08 stage_optimized_resolved_selected_form_layout
O09 execute_resolved_layout_optimization
  → machine_emission::assemble_fixed_frame_realization   (whole-function exit contract, "from stage 09's output alone")
  → StagedOptimizedFunctionFragmentEmissionSource::new(realization, emission_facts, installations)
  → machine_emission::stage_optimized_function_fragment_emission   (unplaced fragments + fixups)
  → machine_emission::stage_function_fragment_frame_application    (insert prologue/epilogue, reflow, re-encode branches)
  → machine_emission::stage_optimized_fixed_frame_text_section     (place into one text section, resolve internal calls)
  → object_file::stage_optimized_relocation_free_object_container
  → image_emission::build_function_fragment_object_artifact        (ObjectArtifact)
  → image_emission::{emit_executable_image, emit_component_shared_image, …} via image-elf / image-macho / image-pe
  → native_artifact::{NativeArtifact, publication receipts}        (product)
```

Two findings follow.

- **Unnumbered transformations sit between O06 and O07 and after O09.** The
  `machine-emission` lib header calls them "four route stages", and its doc
  says "Native realization drives our four route stages in sequence".
- **A side channel bypasses the pipeline.** `native-compiler/src/pipeline.rs:265`
  `emission_facts` "Copy what emission needs from stage 02's output and cannot
  read from the current machine program: the admitted lowering facts and the
  optimized abstract program". It retains an `Arc` to the whole optimized abstract program as well as admitted facts and projection identities. This is a sidecar around O03–O09; nesting that old program inside a later representation would preserve the reach-back. B7 first inventories each downstream field read, then carries only the facts emission requires. [Exact construction][emission-facts]

## B3. New numbered stages after O09

The stage names below are **judgment**. O10–O14 are migration labels after
today's O09, not final numbering: settlement and later installation work also
need positions (§B5). Each representation must carry its target, identities,
installation facts and required emission facts; these cannot remain separate
arguments to a renamed stage. The size column carries the original
`6de27da…` module/function estimates, not regenerated counts of the proposed
stages. Their exact size at this review revision is **unverified**.

| Stage | Input → output | Takes | Historical size estimate |
| --- | --- | --- | --- |
| **O07** (existing, widened) | post-allocation machine → selected-form encoding | `machine-emission::{frame_layout, frame_protocol, function_realization::stage_function_frame}`. The stage's name already says its input is the post-allocation machine; framing becomes O07's first step, and `FramedMachineProgram` stays an internal form (it is already defined in `machine-code/src/layout.rs:261`). Plus the ISA encoders (§B4). | 2,530 + 428 + ≈400 |
| **O10** `resolved-layout-to-function-fragments` | resolved layout → unplaced function fragments | `function_realization::assemble_fixed_frame_realization`, `exit_contract`, `fragment_emission`. `emission_facts` is **deleted only after a field/consumer inventory** replaces its lowering facts, whole abstract program and projection with sufficient current-form records. Exact minimum payload remains **unverified**. | ≈744 + 3,654 + 1,893 |
| **O11** `function-fragments-to-function-fragments` | fragments → framed fragments | `frame_application`: inserts prologues and epilogues and re-encodes stretched branches. Its doc keeps it apart from O08–O09 on purpose: "ensures that branch relaxation operates on exact body displacements first". | 2,571 |
| **O12** `function-fragments-to-text-section` | framed fragments → placed text section | `text_placement`, `code_layout` | 1,800 + 258 |
| **O13** `text-section-to-object` | text section → object | `object-file` (`fragment_container`, `object_plan`, `names`, `relocation_free_object`); `image-emission::{function_fragments.rs` production part, `object_artifact.rs`, `object.rs}`. Their docs: "`function_fragments.rs` is the sole builder of `ObjectArtifact`"; "`object.rs` composes private callback objects and component shared images". | 3,552 + ≈26k + 5,737 + 931 |
| **O14** `object-to-image` | object → executable or component image | `image-emission::{image.rs, entries.rs}`; `image` (builder, placement, relocation application); `image-elf`, `image-macho`, `image-pe` as format writers chosen from the target description; `machine-emission::{native_entry_bridge, native_foreign_bridge, native_bridge_encoding}` (*judgment*: entry and adapter shims are bound into the image in `entries.rs`); the ISA hosted-entry and floating-control sequences (§B4) | 2,032 + 6,254 + 5,281 + 28,805 + 3,069 |

**Contracts required before these moves (judgment):** O07 fixes the instruction
encoding alternatives, frame recipe and fixup geometry; O08 chooses placements
and resolves encodings within those limits; O09 preserves that contract while
optimizing layout. O10 creates fragments without reopening abstract programs;
O11 applies the carried frame recipe and resolves any changed displacements;
O12 assigns text offsets and internal references; O13 seals the object plan;
O14 applies target container/image rules and entry adapters. Each output carries
any facts the next operation needs, and rejects an unresolved reference or a
mismatched program/target identity. These are proposed interfaces, not a claim
that today's structs already meet them.

New representations. pipeline.md:1124 already says "`machine-code` owns
separate selected-form encoding, resolved-layout and final machine-code roots".

| Representation | Holds | From |
| --- | --- | --- |
| `machine-code/function-fragments` | staged and framed fragments, `WholeFunctionExitContract` records, fragment manifests | `machine-emission` public types |
| `machine-code/text-section` | `StagedOptimizedFixedFrameTextSection`, text manifests, code-layout rows | `machine-emission::text_placement` types |
| `object` (new family) | `ObjectArtifact` carriers, `ObjectPlan`, relocation-free object | `object-file::object_plan`, `image-emission::object_artifact::carriers` |
| `image` (new family) | `FinalImage` and its regions and symbols, `ExecutableImage`, `RequestedDynamicElfImage`, `ComponentSharedImage`, `NativeForeignAdapterImage`, and the native-artifact carriers the runtimes read (`NativeArtifact`, `NativeComponentProduct`, `TerminalRuntime*Provision(s)`) | `image::final_image`, `image-emission::image`, `native-artifact` carriers |

**Why the runtimes can follow.** `terminal-runtime` and `native-provider-runtime`
read only these final carriers from `native-artifact` and `image-emission`
(module scan: `NativeArtifact`, `NativeComponentProduct`,
`TerminalRuntimeHostProvision(s)`, `NativeForeignAdapterImage`). The runtime
dependency check allows representations (`PERMITTED_OWNERS[1..]` includes
`omega/compilation/representations`), so the runtimes can depend on the `image`
representation. They can depend neither on `products` nor on the native driver
(index 0, which `[1..]` excludes).

**Validators and replay.**

Default compilation and explicit verification are different routes. The driver
retains original stage inputs only for `retain_native_stage_inputs`; explicit
[audits][audit] require them. Moving those files does not remove their calls to
`native_preparation::verify_execution_plan` or
`abstract_operations_to_target_operations::capture_runtime_borrowed_storage_layouts`.

Put independent verification beside the representation being checked, with
its retained evidence supplied only to the explicit validator. Extract any
shared carrier or independent checking interface before deleting stage-call
allowances. Preserve independent checker logic when sharing it with the
producer would also share the producer's mistake. A relocation alone cannot
make `STAGE_CALL_ALLOWANCES` empty.

The O00 retained-input codec and admitted carrier types read by products and
runtimes belong in representation packages. Their construction authority does
not automatically move with their definitions. In particular, `terminal-runtime`
reads an admitted intrinsic type from `provider-admission`; it must not gain a
dependency on a newly numbered producer. [Runtime use][runtime-admission]

### `machine-emission` (17,781 lines) by module

The crate total is current generated evidence. Individual module figures below
come from the historical inventory described in §0.

| Module | Historical module lines | Users | Destination |
| --- | --- | --- | --- |
| `frame_layout.rs`, `frame_protocol.rs` | 2,530 + 428 | native-compiler (via `stage_function_frame`), O07 reads its output | **O07** |
| `function_realization.rs` | 1,144 | native-compiler | split: `stage_function_frame` → **O07**; `assemble_fixed_frame_realization` → **O10** |
| `exit_contract.rs` | 3,654 | (internal; "from stage 09's output alone") | **O10** |
| `fragment_emission.rs` | 1,893 | native-artifact, image-emission, native-compiler | **O10**; types → `machine-code/function-fragments` |
| `emission_facts.rs` | 42 | object-file, native-compiler | **delete** (sidecar; carry the facts instead) |
| `frame_application.rs` | 2,571 | native-artifact, image-emission | **O11** |
| `text_placement.rs`, `code_layout.rs` | 1,800 + 258 | native-artifact, image-emission, object-file | **O12**; types → `machine-code/text-section` |
| `native_entry_bridge.rs`, `native_foreign_bridge.rs`, `native_bridge_encoding.rs` | 2,053 + 428 + 588 | native-artifact, image-emission | **O14** (*judgment*) |
| `lib.rs`, tests and remaining helpers | remainder | | move tests with their operations; delete the crate only after its O14 bridges move |

## B4. The target description becomes an input representation

New package `omega/compilation/representations/target-description`.

- **Who selects it:** Build chooses the target; an explicit preparation step
  derives immutable target-only tables from that choice (judgment). This matches the board
  item **BUILD-OWNED-TARGET-SELECTION** (TASKS.md:83: "Retire target-prefixed
  machines for target-keyed Build rows").
- **Who receives it:** the early settlement/native input bundle. Each later
  native form carries the selected description or its stable identity (rule
  B-R3); a stage does not consult global target state.
- **Why it fits the existing rules:** Psi never uses it, so the firewall is
  satisfied. The runtimes may depend on representations (`PERMITTED_OWNERS[1..]`),
  so `terminal-runtime` and `native-provider-runtime` keep their ABI geometry.
- **One expected side effect (*unverified*):** `register-environment` today
  caches "one validated instance per target process-wide". As an input
  representation it would be built once by the driver and passed down, which
  removes that process-wide cache (`tools/ratchets/cache_state.txt`
  territory).

| Today | Lines | Goes into `target-description` | Goes elsewhere |
| --- | --- | --- | --- |
| `target` | 3,921 | `target_profile`, `target_semantics`, `x86_features`, `float_realization`, `foreign_locator`, `component_locator`, `elf_loader` | `explicit_target_set` (orchestration, compiler, cli) → **Build** (`build-system/orchestration`); `PackageReviewRepresentation*` → **review-evidence**; app metadata (`ApplicationIdentifier`, `HostedApplicationIntent`, `NativeApplicationMetadata`) → the native product request carried into image construction, separate from target-invariant facts (judgment) |
| `register-model` | 3,514 | all (declarative register model, constraint catalogs, preservation catalogs) | |
| `register-environment` | 5,171 | all (derived purely from the target: "construct and validate the target environment directly from ISA declarations") | |
| `isa-x86_64`, `isa-aarch64` tables | ≈6,212 + ≈4,879 | `register_model.rs`, `machine_effects.rs`, `preservation_storage.rs` | |
| `calling-conventions` | 11,063 | Target-invariant ABI descriptions and geometry only | `evaluate_call_plan(policy, signature)`, entry-plan calculation and signature/program layout derivation → the settlement or native-preparation producer that first has their inputs. Call-plan, callback and boundary records → carried representations. Split `aggregate_layout`, `structural_shapes`, `boundary_values`, `host_operations`, `terminal_layout` by actual input/producer; purity alone is not a classification. [Call-plan calculation][call-plan] |
| `runtime-abi` | 2,356 | target ABI geometry (`scalar_reference`, `borrowed_storage*`, fat descriptors, `NativeForeignFrame`, `static_interrupts`) | host leaves used only by the interpreter (`descriptor_io`, `network_address`, `host_numeric`; `windows_provider` also products) → **`omega/interpreter/terminal-runtime`** |

### Proposed split: target catalogs and program encoding

The module scan of non-test users supports a split:

- **The encoders are stage work and go to the encoding stage, O07.**
  - The files: `selected_form_encoding.rs` (x86 24,017; AArch64 18,243),
    `frame_protocol.rs`, `encoded-scalar-plan`, `normalized-foreign-call-plan`.
    About 46k lines including those plan crates.
  - O07's own doc says it "Routes each instruction kind to its target ISA
    encoder (x86-64 or AArch64)".
  - The main encoder callers are O07 itself: `encode_*_selected_form`, memory,
    scalar-call, foreign-call templates, system and table calls.
  - `encoded-scalar-plan` and `normalized-foreign-call-plan` have no users
    except the two ISA crates.
- **The register and effect catalogs are target description.**
  - The files: `register_model.rs`, `machine_effects.rs`,
    `preservation_storage.rs`.
  - Their users are `register-environment` and `selected-instruction-analyses`.
    Those are read at O03–O06, before anything is encoded, so they cannot
    belong to O07.
- **Image-time sequences go to O14:** `generated_entry.rs`, `hosted_kernel.rs`
  / `hosted_sequences.rs`, `ieee_float.rs`, `floating_control.rs` and the
  `lib.rs` byte constants. Their users are `image-emission`, `native-artifact`
  and `machine-emission` (the bridges).
- **Post-handoff writers go to O14** (*judgment*): `post_handoff_writer.rs`
  together with `post-handoff-writer-plan`, whose README says the plan "derives
  writer programs" and the "ISA crates encode those fragments".
- **The leftover small crates:**
  - `x86_64-encoding` (78 lines): its users are `isa-x86_64` and
    `native-artifact`. It moves into O07's x86 encoder module, and the
    native-artifact use moves with the image-stage code that calls it.
  - `isa-conformance` (579 lines) is a dev-only test suite of both ISA crates.
    It moves beside O07's tests or to `tools/test-support`.

**What does not fit cleanly.** O08 and O09 re-encode branches after O07
(`branch_forms`, `jump`; x86 short-branch relaxation in O09), and so does O11
(frame insertion re-encodes stretched branches). If the encoders live in O07,
these would become stage-to-stage calls, which `boundary.rs` forbids. O08 also
re-encodes private callback materializations to check their canonical bytes
([x86/AArch64 calls][callback-encoding]); branches are not the complete use
inventory. Q-F recommends carrying encoding alternatives and exact fixup
descriptors, subject to proving that they cover all these consumers.

## B5. Provider settlement, admission and later installation binding

The five realization crates contain 36,409 Rust lines at the reviewed source.
They do not form one operation that can all be moved before O00. The current
route has four materially different points:

1. [The compiler closes provider demand][closure]. It discovers reached
   requirements using already chosen exact bodies, prepares calling-policy
   inputs when that set changes, chooses providers, and repeats until the
   bindings stop changing. It refuses a change to an already chosen exact body.
   Retained available code does not itself become an invocation root.
2. [Policy preparation][provider-inputs] receives checked source custody,
   accepted publication, Build bindings and target. The proposed input must
   retain an admitted catalog of policy artifacts and selection authority;
   “Terminal Psi + target” alone does not describe this current API.
3. O00 and native preparation admit the selected providers, entry and
   application evidence. The [native proposal constructor][native-proposal]
   already performs task realization and provider checks despite living in a
   representation crate. Move those producers explicitly.
4. [The root pauses after O02][object-emission]. Installed handlers may run
   through their own native pipelines first. Their emitted objects determine
   numeric stack requirements, which [admission binds][installation-binding]
   before root O03. The callback coordinator rejects cyclic installer/handler
   resource dependencies and reuses each produced object. [Source][handler-stacks]

**Recommendation for Q-G (judgment):** make early provider closure and native
admission explicit numbered transitions, and give later installation binding
its own transition when its measurements exist. Carry selected declarations,
call plans, trust and task facts in the corresponding output. Keep constructors
that issue sealed authority with their checking producer, while runtimes depend
only on the resulting carrier interfaces. `SelectedExecution::settle` is not
the sole constructor: [`from_realization` is public][selected-execution].

A global linear pipeline needs more than an O02/O03 insertion. Today's handler
measurement runs later child stages while the root is paused. **Proposed
contract work:** carry a complete reachable body roster and symbolic
installation demands through selection and frame planning; derive stack facts
from that current form and bind them before their first numeric encoding use.
Inventory O03's numeric consumers first. If a choice cannot be deferred without
repeating selection, split that choice into a later stage. Do not hide recursive
calls to earlier stages inside a newly named binding stage. The precise split
and preservation of current cycle refusals are **unverified** and block the
whole-program linearity claim, not the independent late-emission moves.

| Existing crate | Lines | Proposed ownership (judgment) |
| --- | --- | --- |
| `effects` | 7,782 | Selected evidence carriers → provider-plan representation; derivation and checking → settlement/admission producer. |
| `backend-plan` | 2,726 | Selection/proposal carriers → provider-plan representation; `canonical_schemas` and provider closure work → producer. |
| `program-entry-plan` | 1,300 | Selected entry and physical contract records → admitted input; calculation → admission. |
| `task-plans` | 1,901 | Realized activations → admitted program facts; `realize_task_activations` → producer. |
| `installation-evidence`, `bound-installations` | 357 + 348 | Separate early obligations from later measured/bound facts. The early carrier cannot depend on a later whole program. |
| `boundary-applications` | 1,350 | Artifact-bound selected application records → provider-plan carrier; selection → producer. |
| `calling-conventions` callback/boundary records | included in §B4 | Carriers → provider-plan/current native forms; derivation → the first stage with the required signature/layout facts. |
| `encoded-scalar-plan`, `normalized-foreign-call-plan` | 324 + 1,202 | Encoding producers → O07, carried fixup/encoding records → its output. |
| `post-handoff-writer-plan` | 960 | Image-time writer derivation → O14; record types → image input/output as needed. |

`provider-plans` is therefore a proposed family of carriers, not a crate into
which all current APIs can move unchanged. Early and late records may share
leaf identities; neither may import a stage or an earlier program to recover
missing facts.

## B6. Products, realization and leftovers

| Crate | Lines | Destination | Evidence |
| --- | --- | --- | --- |
| `native-artifact` | 16,394 | **split**. Carriers the runtimes and native-compiler read (`NativeArtifact`, `NativeComponentProduct`, `TerminalRuntime*Provision(s)`, `DynamicElfNativeArtifact`) → **`image` representation**. `fragment_object_publication` (`emit_fragment_object`, which runs the work proposed as O10–O13) → **delete**: the native driver runs those stages. `entry_receiver::admit_executable_entry` → **early native admission**: it runs before object construction, not at image emission. [Current call][native-admission] Publication and receipts (`publication.rs`, `package_publication.rs`, `component_product` receipts) → **`products`**. `physical/` → split production evidence derivation from explicit independent verification; carriers beside `image`, production derivation with its image producer, explicit checks in a separate verifier (judgment; complete call inventory still required). | Its own header: "self-verifying executable/package publication receipts" (product); `products` is the only user of the receipt types. |
| `component-description` | 2,542 | **split**. Producer and publisher (`publish_component_description`) → **`products`**. The carrier and `verify_component`, which O00, provider-admission and verified-optimization-input read when a component is imported → **`provider-plans`** (*judgment*). | Its header: "carrier, its wire codec, its producer, and its source-free independent verifier"; unobserved references in some declared users require confirmation (§0 method). |
| `layout` | 1,934 | **split by inputs**: program/signature layout calculation → settlement/admission producer; shared shapes/geometry records → a leaf carrier. Do not preserve an unnumbered stage-work library merely because both planning and admission call it. Exact function split is unverified. | Observed consumers are provider-admission and provider-planning. |
| `isa-conformance` | 579 | **O07 test support** or `tools/test-support/` | Dev-only |
| `x86_64-encoding` | 78 | **O07** (x86 encoder) | |

**Count check:** the [generated package counts](../evidence/data/counts.json)
sum to 223,507 backend Rust lines, of which 221,538 are under `src/`. The older
221,793-line categorization belongs to `6de27da…`. Package totals are exact
for the stated source; proposed module splits are estimates and are not an
independent line-balance proof.

## B7. PR sequence for Part B

Each row is a contract change followed by its consumer moves and deletion.
Enforcement lands with the row. Existing producer/validator tests and focused
native/Psi agreement check behavior; codecs and images additionally require
byte/identity comparisons when their inputs are unchanged.

| Step | Required predecessor | Change, deletion and acceptance |
| --- | --- | --- |
| B0 | evidence refresh | Inventory actual entry points, carrier fields, constructors and audit routes. Register current edges as explicit temporary allowances; update governing text for each adopted implementation choice. |
| B1 | B0 | Remove only confirmed unobserved backend dependencies. Type-check affected configurations before deleting an edge. |
| B2 | A3 and required TPV/NV leaves | Extract carrier/authority interfaces in dependency order: native vocabulary, target description, early provider records, late installation records. Move host-only ABI helpers to runtime/host support. Keep program-specific calculations with their producer. No new reverse edge is allowed. |
| B3 | B2 and admitted source/policy input contract | Put provider closure and native admission into explicit stages. Remove equivalent work from compiler coordination and representation constructors. Preserve reached-only demand, immutable prior selections and issuer checks. This requires the relevant A8/A11 input/vocabulary moves. |
| B4 | B2; O03 consumer inventory | Prove and implement the installation contract from §B5: complete body roster, symbolic demands, frame-derived measurements and one binding owner before numeric use. Preserve cyclic-dependency refusals and exact program/target identity. Delete recursive reach-backs only after the replacement works. This is required before claiming a whole-program linear pipeline. |
| B5 | B2; frame input/output contract | Move frame geometry/protocol into O07, with frame recipes carried in its output. Delete only the moved `machine-emission` modules. |
| B6 | B5; Q-F fixup contract proved | Move program encoders into O07; carry alternatives, displacement/bitfield rules and callback materialization descriptions for later layout. Delete encoder library routes only after O08/O09/frame consumers stop calling them. Leave image bridge encoders until B11. |
| B7 | B2; field/consumer inventory | Replace `EmissionFacts` with the minimal sufficient facts carried through O03–O09. Delete the original abstract-program access, not just the struct name. Check every existing emission consumer. |
| B8 | B5, B6, B7 | Create O10–O12 and fragment/text representations; driver owns their order. Delete their old route modules and obsolete stage-call allowances. **Keep the `machine-emission` crate while its image bridges remain.** |
| B9 | B8; object carrier contract | Create O13 and `object`; migrate object/container construction and private-function composition. Delete `object-file` after all users move. |
| B10 | B2, B9; verifier interfaces extracted | Move explicit replay and physical-evidence verification beside their forms. Replace calls to producing stages with independent checks or carrier APIs before removing each allowance. Move O00-owned codecs/types needed by products and runtimes to representation leaves. Missing retained evidence must still refuse explicit audits. |
| B11 | B9, B10; image carrier contract | Create O14 and `image`; migrate format writers, entry/foreign bridges, hosted sequences and post-handoff writers. Move runtime consumers to final carriers. Only now delete the remaining `machine-emission` bridge modules and crate. |
| B12 | B3, B4, B11; all remaining users migrated | Move publication/receipts to products; remove duplicate product-owned sequencing and remaining component-description producers. Delete the backend directory only when no package, runtime or validator depends on it. |
| B13 | every preceding row | Remove that row's temporary allowances and add its owner/route assertions. Final check closes the backend list; it does not turn on all enforcement for the first time. |

B5–B11 can progress while B4's whole-program installation contract is being
resolved, provided their existing installation inputs remain explicit.
B12 cannot claim completion without B3/B4. Part A and B share semantic codecs,
identities and plan definitions: their moves follow the leaf dependency order,
not a blanket statement that TPV work is independent.

## B8. Boundary rules for Part B

| Rule | Proposed enforcement | Limit it must state |
| --- | --- | --- |
| B-R1 | No package remains under `omega/compilation/backend` after B12. | Before that, a shrinking list records the modules still waiting for their consumer move. |
| B-R2 | Pin each producing operation to a numbered stage and each sequence to its driver. Check explicit paths/APIs and forbidden dependencies. | A `stage_*`/`execute_*`/`emit_*` name scan misses `evaluate_call_plan`, `derive_*` and constructors. It is a supplemental signal, not proof that representations contain no stage work. |
| B-R3 | Explicit acyclic dependency order: primitive/TPV leaves → NV leaves → target description → early provider carriers → native program forms; late installation carriers use only the necessary leaves. | NV must not import target description or provider plans. Program-specific target calculations are producers. Shared immutable identities do not authorize reading an earlier whole program. |
| B-R4 | Remove a stage-call allowance when its last actual call is gone. | Explicit audits need independent verifier interfaces before their allowances can disappear. A file move does not satisfy this rule. |
| B-R5 | Runtimes may read target, admitted-provider and final image carriers, plus the approved host-support leaf. They may not invoke stages or construct authority unchecked. | Split the current admitted intrinsic/runtime type dependency before relocating admission to a stage. |
| B-R6 | Remove backend from `OMEGA_COMPILATION`/`PERMITTED_OWNERS` only after the relevant users move; retain the Omega-to-source-side Psi firewall. | Representation naming does not make source custody or evaluator access permissible. |


# Owner questions and side findings (both parts)

## 7. Owner questions

The six [recorded rulings](../rulings.md) settle the direction: primitives only,
program facts carried by representations, little shared code, and no backend
layer. They do not ratify specific package names, a host-support exception,
source-input access or a complete installation-stage design. `TPV`,
`build-requests`, `target-description`, `provider-plans` and O10–O14 are this
plan's proposed implementation names.

The [question page](../questions.md) records Q-A, Q-B, Q-C, Q-D, Q-F and Q-G with
current code evidence, options and recommendations. In brief, the recommended
choices are:

| Question | Recommendation — judgment, not adopted |
| --- | --- |
| Q-A: shared process runner | Give non-compiler host support an explicit narrow owner and runtime rule. Do not place it in packages then waive an unexplained runtime edge. |
| Q-B: Build/source access | Carry admitted semantic facts and provenance in the current representation. Split them from source-loading state and whole Build results. |
| Q-C: source/evaluation sequencing | One source sequencer owns continuation and evaluator requests; an artifact-only executor runs checked programs. Remove evaluator stage calls before reversing any dependency: `source-compiler` already depends on `build-time-evaluation`. |
| Q-D: earlier types versus earlier programs | Allow leaf identity/vocabulary definitions across forms; prohibit undeclared access to an earlier whole program. Re-export chains are not a substitute for data flow. |
| Q-F: encoding after O07 | Carry encoding alternatives plus complete fixup rules, including callback materializations and frame-induced displacement changes. Prove the representation before relocating all encoders. |
| Q-G: realization | Number early closure/admission and later binding where their facts exist. Replace the root/child interleaving explicitly before claiming a single whole-program pipeline. |

This review updates only atlas. It does not alter `OWNER_QUESTIONS.md`,
`AGENTS.md` or compiler behavior, and it does not treat these recommendations
as new rulings.

## 8. Side findings (for `OWNER_CODE_CONCERNS.md`; no extra PRs)

- **Duplicated wire helpers.** psi `optimization` and `optimization-core` each
  define `DomainDigestEncoder`, `Cursor`/`TakeError`, `push_encoded` and
  `structural_variant_accessor!`. A3 merges generic mechanisms; semantic codecs remain with vocabulary.
- **Duplicate opaque-representation records.**
  `language-semantics::opaque_representations` duplicates the owner that
  pipeline.md names, `typed-trees::opaque_representations`.
- **Publication inside the backend.** `native-artifact::package_publication`
  does product publication.
- **A very large carried vocabulary.** `terminal-psi/vocabulary` would hold
  an estimated 40–45k lines before extraction. A11 separates subjects and their
  dependency contracts; the post-move count is unverified.
- **The native driver copies stage 02 output past stages 03–09.**
  `native-compiler/src/pipeline.rs:265` (`emission_facts`) hands stage 02's
  lowering facts and abstract program straight to emission, around the
  pipeline. B7 removes it after its complete field/consumer inventory.
- **A process-wide cache in `register-environment`.** It caches "one validated
  instance per target process-wide". Building the target description once per
  compile (B2) may remove it (*unverified*).
- **`emit_fragment_object` lives in `native-artifact`.** It runs four
  emission steps plus object building, so a product crate is driving pipeline
  work. B8/B9 move its production route into the driver; B12 removes the remaining product wrapper.


## Source anchors for this review

All links below pin source revision `3f9ce43f0c9d179048911d18faa91ba93f60176d`.
They establish current behavior; proposed destinations above remain judgment.

[target-profile]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/target/src/target_profile.rs#L11
[float-target]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/target/src/float_realization.rs#L72
[selection-storage]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/representations/typed-trees/src/lib.rs#L1457
[selection-transfer]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/pipeline/03_symbol-resolved-trees-to-typed-trees/src/lowerer.rs#L1622
[selection-read]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/compilation/pipeline/05_typed-trees-to-terminal-psi/src/read/calls.rs#L515
[identity-encoding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/semantic-vocabulary/src/identity_encoding.rs#L5
[path-encoding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/semantic-vocabulary/src/structural_path_encoding.rs#L3
[call-plan]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/calling-conventions/src/plans/call_plan_evaluation.rs#L25
[native-pipeline]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L173
[emission-facts]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/pipeline.rs#L265
[audit]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/backend/images/image-emission/src/function_fragments/replay/validation.rs#L71
[closure]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/execution_settlement.rs#L202
[native-admission]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization.rs#L44
[handler-stacks]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization/callback_thunks.rs#L78
[native-proposal]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/representations/abstract-operations/verified-optimization-input/src/native_proposal.rs#L240
[provider-inputs]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/compiler/src/terminal/provider_execution.rs#L14
[installation-binding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/provider-admission/src/installation_binding.rs#L60
[object-emission]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/native-compiler/src/realization/object_emission.rs#L81
[selected-execution]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/realization/selected-dispatch/src/selected_execution.rs#L14
[runtime-admission]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/interpreter/terminal-runtime/src/execution/host_imports.rs#L365
[callback-encoding]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/omega/compilation/pipeline/08_selected-form-encoding-to-resolved-layout/src/ordinary.rs#L729
[symbol-map]: https://github.com/CathedralOS/Omega/blob/3f9ce43f0c9d179048911d18faa91ba93f60176d/omega-rust/psi/foundation/symbols/src/fast_map.rs#L85

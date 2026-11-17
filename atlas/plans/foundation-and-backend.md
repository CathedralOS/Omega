# Foundation and backend: migration plan (owner rulings of 2026-11-17)

Foundation should hold only generic building blocks, but today `psi/foundation/`
and `omega/foundation/` hold about 96k lines. Most of that code is language
tables (intrinsic catalogs, the scalar and proposition term language, numeric
and float rules, the optimization catalog, access and layout rules) or
program-derived stage output. Several crates are used by no pipeline stage at
all.

Under the refined ruling, foundation keeps only primitives: arenas, the record
codec, wire and digest bytes, plain diagnostics, text positions, big-integer
arithmetic and the worker budget. That is about 14.8k lines in 7 crates under
`psi/foundation/`. `omega/foundation/` disappears.

Everything else moves to one of three places:

- the first pipeline representation that needs it, which then carries it
  forward;
- a carried Terminal Psi vocabulary package, when the firewall forbids its
  readers from reaching source-side forms;
- its real non-pipeline owner.

**Part B** (after §6) dissolves `omega-rust/omega/compilation/backend/`:
221,793 lines in 29 crates.

- After Omega 09, the native driver works outside the numbered pipeline.
  That work becomes numbered stages O10–O14 with their own representations,
  and encoding folds into Omega 07.
- The selected target's description becomes an input representation that
  Omega 00 receives and the native forms carry.
- The provider plans become a second input representation.
- Native-artifact publication moves to `products`.

No step changes what the compiler accepts or emits.

Source revision: `6de27da2a36d86f6c3883cb89527d0d44dea646a` (worker-1, read only).
The first version of this plan, with a `language/` layer, is kept at
`fdn/foundation_plan_v1.md` in the scratchpad.

## 0. Method and how much to trust it

- **Crate edges:** `cargo metadata --no-deps` from a fresh run. It matches the
  cached `meta.json` exactly (1478 path edges). Only production edges count
  (normal and build); dev-dependencies are listed separately.
- **Module and type usage:** scratchpad scripts `fdn/modscan.py`,
  `fdn/report.py` and `fdn/sub.py`, with output in `fdn/usage.json`. They scan
  non-test `.rs` files and skip `tests/` directories, `tests.rs`, `*_tests.rs`,
  `source_guards.rs` and brace-matched `#[cfg(test)]` items. They record
  `crate::Seg`, `crate::{..}` and `crate::mod::{..}` paths. A path counts only
  when the user crate declares that crate as a dependency, which removes local
  modules with the same name (for example `optimization::` inside Omega 06).
- **Limits (unverified):**
  - The scan uses regular expressions, not a Rust resolver.
  - A glob (`use x::*`) or a path inside a macro body can be missed.
  - A type reached through a re-export is credited to the re-exporting crate.
  - The defining file shown for an item is the first `pub` definition the
    script finds, so it is occasionally wrong. Example: `ScalarType` shows as
    `closed_integers.rs::tests`, but it is defined in `scalar_terms.rs:711`.
  - "Declared but unused" means the scan found no non-test reference. Confirm
    each case with a type check before deleting the dependency.
- **Positions:** a position is a Psi stage NN, an Omega stage NN, a
  representation (`psi-repr X`, `omega-repr X`), or a non-pipeline owner:
  backend, realization, products, compiler, packages, build, cli, interpreter,
  tools.

### Corrections to the evidence handed in

| Claim in the brief | What the graph shows |
| --- | --- |
| `mutation-matrix` has zero users | One production user: `optimization-core`, through an *optional* `test-support` feature (`optimization-core/Cargo.toml:8,12`). It is test machinery; its only real call sites are in `image/src/final_image.rs` tests. |
| `source-audit` has zero users | Zero production users, but it is a dev-dependency of 4 crates (`boundary-applications`, `effects`, `task-plans`, `terminal-psi`) through their `source_guards.rs` tests. |
| `extents` is used only by the terminal-psi representation | 4 production users: `terminal-semantics`, `access-plans`, `terminal-interpreter`, `terminal-runtime`. |
| `evaluation-accounting` is used only by the terminal-psi representation | Among pipeline crates, only `terminal-work` uses it. It also has 6 non-pipeline users: `build-time-evaluation`, `terminal-interpreter`, `build-settings`, `package-compilation`, `package-manager`, `provider-planning`. |
| `access-plans` has 2–3 positions | 8 production users: `typed-trees`, `terminal-psi`, `terminal-semantics`, `terminal-wire`, Omega 03, `build-time-evaluation`, interpreter, `terminal-runtime`. |
| `build-time-values` has no pipeline users | `language-semantics` declares it, and it is used by `language-semantics::native_bindings` (`lib.rs:2470`). That module's only user is `build-settings`, so the claim becomes true once `native_bindings` moves. |
| `flow-effects` has 2–3 positions | 9 users declare it, but only 5 use it in non-test code. The other 4 (`build-time-evaluation`, `package-compilation`, `provider-planning`, `source-compiler`) declare it without using it. |

Declared-but-unused production dependencies on foundation crates (step 1 deletes
them; each needs the type check to confirm):

`arena`: compiler, image-elf, image-macho, image-pe, package-manager ·
`artifacts`: omega · `bounded-process`: orchestration · `compiler-storage`:
compiler, products, target · `diagnostics`: Omega 02, layout, target,
ind19-spectrum-validation · `flow-effects`: build-time-evaluation,
package-compilation, provider-planning, source-compiler · `function-identity`:
legalized-operations, machine-code, machine-emission, target-operations, Omega 00
· `language-core`: build-settings, target · `language-semantics`: Psi 06 ·
`layout-plans`: build-settings, target · `numerics`: layout, provider-planning
· `optimization`: compiler, source-compiler · `optimization-core`: image-elf,
target, ind19-spectrum-validation · `semantic-vocabulary`: Omega 07, Omega 09,
target, terminal-codec, ind19-spectrum-validation · `source`:
provider-admission, trust-model · `symbols`: Omega 02, layout, register-homes,
ind19-spectrum-validation.

Spot-checked: grep finds no `flow_effects` reference at all in
`source-compiler`, `build-time-evaluation`, `package-compilation` or
`provider-planning`, and no `function_identity` reference in `machine-code`,
`target-operations`, `machine-emission` or Omega 00.

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
| **move into `<stage or representation>`** | A language table, or a table, handle, plan or summary that a stage fills from the program. Goes to the first representation that needs it; a type used by one stage (plus its neighbor) goes to that stage. | The ruling. Also AGENTS.md "Representation-to-representation pipeline: … Carry needed semantic facts in that representation; do not bypass the pipeline with sidecars". |
| **move to `<non-pipeline owner>`** | No pipeline user | The ruling. Also pipeline.md table: "Build-time execution inputs and accounting → Psi `compilation/build-time-evaluation/`"; "Source acquisition and package storage primitives → Omega `packages/`". |
| **delete** | Unused, duplicated, or re-exported | AGENTS.md rule 6: no second copy of a data structure |

### Two new representation packages the ruling implies

1. **`psi/compilation/representations/terminal-psi/vocabulary`**
   - **What it is:** the "carried vocabulary" of Terminal Psi, abbreviated
     **TPV** below.
   - **Why it is needed:** many tables are first used by a source-side form
     (`syntax-trees`, `symbol-resolved-trees`, `typed-trees`) but are also read
     by Terminal Psi, Omega, realization or the interpreter. The firewall
     (`boundary.rs:352-358`, `PERMITTED_OWNERS`) forbids those readers from
     reaching source-side forms, so such a table cannot live in the source-side
     form that first needs it.
   - **The owner's answer:** such a table lives in the carried representation
     under the Terminal Psi family. The source-side forms depend on it, and the
     Terminal and Omega sides already may reach it, because `terminal-psi/*` is
     in `PERMITTED_OWNERS`.
   - **It follows the existing precedent** of `native-vocabulary`: "owns the
     leaf types every native form carries unchanged".
   - **It should be split by subject** into nested packages (*judgment*):
     `identities`, `scalars` (term language and numeric rules), `declarations`,
     `resources` (access and extents), `layout`. At an estimated 40–45k lines it would
     otherwise be the largest crate in the workspace, which is
     OWNER_CODE_CONCERNS category 5, "Too big to read".
2. **`psi/compilation/representations/build-requests`**
   - **What it is:** the input representation for Build. It holds what Build
     evaluation computes and stages 02–05 read: Build selection requests,
     executed results, product targets and operands, layout target facts, and
     compiler-evaluation requests.
   - **Why it is a Psi package:** `build-time-evaluation` (Psi) produces it, and
     the firewall forbids Psi from depending on Omega.
3. **`psi/compilation/representations/source-files`** (*judgment*)
   - **What it is:** the input of stage 00, which is already named
     `source-files-to-tokens`. It holds `LoadedSource(s)`, `SourceContents`,
     the captured source tree and snapshot (`capture.rs`), `read_accounting`,
     `SourceCatalogue`, and the consumed-source identity (`source_identity`,
     `PackageKeyIdentity`, `DependencyScope`, `SourceOrigin`,
     `SourceResolutionStratum`).
   - **Why it is not foundation:** these carry package and source-loading
     meaning, not plain text positions.

### The firewall still decides two things

Psi never depends on Omega (`boundary.rs:228-233`). So:

- A Psi-side table used mainly by Omega still goes to a Psi representation.
- An item with no pipeline user goes to an Omega owner only when no Psi crate
  uses it.

### Text the ruling supersedes

Step 0 rewrites all of these together (AGENTS.md "Scope and escalation"):

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

Line counts are all `.rs` lines under each crate directory, including in-file
and `tests/` code (`find … -name '*.rs' | xargs cat | wc -l`). Rows marked ≈
are estimates from module line ranges.

**Before:** 96,223 lines in 26 crates.

- `psi/foundation`: 84,940 lines, 21 crates including `compiler-storage-derive`.
- `omega/foundation`: 11,283 lines, 5 crates.

**After:** about 14.8k lines in 7 crates, all under `psi/foundation/`. That is
about 15 % of today. `omega/foundation/` is deleted, because Omega has no
primitive of its own: its only one, the wire helpers, duplicates Psi's.

| Primitive crate | Content | Lines | Note |
| --- | --- | --- | --- |
| `arena` | arenas, handles, paged and rooted arenas; plus `semantic-vocabulary::id_table` (`IdTable`, `IdSet`) and `symbols::fast_map` | 8,402 + 370 + 89 | `fast_map` is a symbol-keyed hasher (*judgment*: generic hashing, moves with the arena tables) |
| `compiler-storage` (+`derive`) | record codec | 1,767 | |
| `diagnostics` | diagnostic carriers and display | 598 | The parse helpers (`expected`, `parse_error`, `unexpected_eof`, P01 only) move to P01 |
| `source` | `Span`, `SourceId`, `SourceSpan`, `SourcePosition`, `SourceFile`, `SourceBytes`, `SourceMap`, `display_literal_bytes` | ≈1,100 of 2,556 | *judgment*: generic text positions. `capture.rs` (884) and `source_identity` (≈470) move to `source-files`. |
| `wire` (new) | `codec_bytes`, `cursor`, `digest_encoder`, `error_display`, `record_views`, `tagged_vocabulary`, `variant_accessors`, `fnv1a`, `identity_encoding`, `structural_path_encoding` | ≈620 after merging 130 (psi `optimization`) + 429 (`optimization-core`) + 82 + 107 | The two copies of the same helpers become one |
| `bignum` (today `numerics/src/bignum.rs`) | `BigInt`, `BigRational` | 1,689 | `ExactFloat` (P05 only) and `IntegerBitwiseOperation` (P02 only) have Omega meaning: move them to their stage (*judgment*) |
| `execution-admission` | invocation worker budget | 138 | *judgment*: thread and stack budgeting with no compiler meaning, used by P00, P01 and P06 |

## 3. Verdict for every crate

Abbreviations: P = Psi stage, O = Omega stage, `tp` = terminal-psi family,
`tt` = typed-trees, `srt` = symbol-resolved-trees, `NV` = native-vocabulary,
`TPV` = `terminal-psi/vocabulary`.

| Crate | Lines | Production users (positions) | Verdict | Evidence |
| --- | --- | --- | --- | --- |
| psi `arena` | 8.4k | 42 (P00–05, srt, tt, tp, O00, selected-instructions, backend, packages, build) | **primitive** | |
| psi `compiler-storage` (+`derive`) | 1.8k | 38 | **primitive** | |
| psi `diagnostics` | 0.6k | 49 | **primitive**; parse helpers → P01 | |
| psi `source` | 2.6k | 32 | **split**: positions stay **primitive** (*judgment*); `capture.rs`, `LoadedSource*`, `SourceContents`, `read_accounting`, `SourceCatalogue`, `source_identity`, `DependencyScope`, `SourceOrigin`, `SourceResolutionStratum`, `InvocationSourceConsent` → **`source-files`** | `source_identity` users include `terminal-semantics`, so `source-files` must be reachable from the Terminal side (Q-B). Today `source → semantic-vocabulary` (`PackageKeyIdentity`, `lib.rs:44`): this primitive depends on a language crate; the move removes the edge. |
| psi `execution-admission` | 0.1k | 7 (P00, P01, P06, source-compiler, compiler, cli, orchestration) | **primitive** (*judgment*) | |
| psi `numerics` | 6.8k | 30 | **split**: `bignum` stays **primitive**. `literals`, `arithmetic`, `integer_carrier`, `float_semantics`, `float_projection` → **TPV** (`scalars`): first user is P01/`syntax-trees`, but O00, backend, interpreter and runtime also read them. `scalar_predicates`, `integer_bounds` → **`terminal-psi`** (users: tp + terminal-installation only). Delete the `float_semantics_catalog` alias. | Its README: "[Numeric values] defines the language contract", so the numeric rules are language, not primitive. |
| psi `language-core` | 6.2k | 40 | **split** (§4.3): source-only tables → `syntax-trees` / `srt` / `tt`; tables read on both sides → **TPV**; single-user items → their stage | |
| psi `semantic-vocabulary` | 12.1k | 81 | **split** (§4.1): wire and ID-table helpers → primitives; everything else → **TPV** / `terminal-psi` / owners | |
| psi `language-semantics` | 11.8k | 34 | **split** (§4.2); includes the `declaration_selection` leak | |
| psi `symbols` | 9.6k | 28 | **split** (§4.4): `SymbolTable` → `srt`; handles and builtins → **TPV**; `fast_map` → `arena` | |
| psi `optimization` | 0.4k | 17 | **split**: wire helpers → `wire` (delete the copy); `optimization_selections` (`PsiOptimizationSelections`, used by `terminal-wire`, P06, O01, `optimization-core`) → **TPV** | Terminal Psi carries the selection identity |
| psi `flow-effects` | 1.1k | 5 real users | **move into representations**: `InvocationTarget` → `tt`; `ConditionalCarryRequirement`, `ContractDependency`, `template_contract_commitment`, `validate_published_dependency`, `CapabilityFlowKind` → **`terminal-psi`**; `installation_reach` → **`terminal-installation`** (*unverified*: whether provider-selection may take that edge) | Checker output |
| psi `access-plans` | 5.8k | 8 (tt, tp ×3, O03, build-time-evaluation, interpreter, terminal-runtime) | **move into TPV** (`resources`) | The rules are the access spec and the plan values are program-derived; both cross from `tt` to O03. |
| psi `layout-plans` | 8.2k | 25 | **split** (§4.5) | |
| psi `extents` | 4.9k | 4 | **split** (*judgment*): identities and rights that `access-plans` names (`Extent`, `ExtentRights`, `ExtentLoan`, `LoanPolarity`, `AddressSpaceId`, `Extent*Id`, `MappingEraId`) → **TPV** (`resources`); `compose_claim_bounds`, `ClaimBoundRow`, `ActivationClaimBranch` → **`terminal-semantics`**; ledgers, root grants, provider issuance, `mapping.rs` → **`psi/interpreter`** | `terminal-runtime` already depends on the interpreter |
| psi `evaluation-accounting` | 1.5k | pipeline: `terminal-work` only | **split**: `invocation.rs` → **`terminal-psi/work`**; `lib.rs` sponsor, limits and usage → **`build-time-evaluation`** (nested package `build-time-evaluation/accounting`, so `package-manager` does not pull in the evaluator) | pipeline.md table row on Build accounting |
| psi `build-time-values` | 0.1k | build-time-evaluation, provider-planning, language-semantics::native_bindings | **move to `build-time-evaluation`** after `native_bindings` → `build-settings` | `provider-planning → build-time-evaluation` is already allowed (`FIREWALL_ALLOWANCES`, boundary.rs:105) |
| psi `filesystem-authority` | 2.7k | 0 pipeline | **split, move to owners**: `filesystem.rs` and `filesystem_sponsor.rs` → `psi/compilation/build-time-evaluation/filesystem-authority` (nested package; Psi's Build owner, used by `build-time-evaluation` and Omega build and packages); `source_capture.rs` (package-compilation and source-acquisition only) → `omega/packages/source-acquisition`; `lib.rs` `BuildOutputObligation*` (package-compilation only) → `omega/packages/package-compilation` | |
| psi `mutation-matrix` | 0.1k | test-support only | **move to `tools/test-support/mutation-matrix`**; delete the `optimization-core` re-export | |
| psi `source-audit` | 0.1k | 0 production, 4 dev | **move to `tools/test-support/source-audit`** | |
| omega `function-identity` | 0.15k | 13 | **move into `NV`** | O00 output carried by every native form; removes the only foundation → representation edge |
| omega `optimization-core` | 5.8k | 56 | **split** (§4.6): wire helpers → `wire`; the catalog → **Omega pipeline (`NV`)**; single-owner parts → their representation or products | |
| omega `artifacts` | 0.3k | 0 pipeline; compiler, products, build-settings, orchestration, package-compilation, package-manager | **move to `omega/compilation/products/measurements`** (nested package; *judgment*). Products owns reports. A nested manifest avoids the `products ↔ package-compilation` cycle, because `products` depends on `package-compilation`. | pipeline.md:1463 |
| omega `platform-custody` | 1.1k | 0 pipeline; cli, orchestration, products, packages ×3, backend `native-artifact` | **move to `omega/packages/platform-custody`** | pipeline.md table: "Source acquisition and package storage primitives → Omega `packages/`". `native-artifact`'s one use (`package_publication.rs:17`, `make_executable`) is product publication; move that call to `products` in the same PR (*unverified*: size of the move). |
| omega `bounded-process` | 3.8k | 0 pipeline; cli, packages (resolver-execution, source-acquisition), `native-provider-runtime`, xtask, bench, sample-runner | **split, move to owners**: `private_observation*.rs` (cli only) → **cli**. The rest: the native-provider runtime, package resolvers and dev tools share one process runner. **Remaining question Q-A.** | The runtime boundary (`boundary.rs` `NATIVE_PROVIDER_RUNTIME_ROOT` check) does not let `native-provider-runtime` depend on a `packages/` crate. |

## 4. Module-level splits for the wide crates

### 4.1 `semantic-vocabulary`

| Module or types | Positions | Destination |
| --- | --- | --- |
| `fnv1a`, `identity_encoding`, `structural_path_encoding` | 2–12 | **primitive** `wire` |
| `id_table` | 12 | **primitive** `arena` |
| `PackageKeyIdentity` | 26 | **`source-files`** (with `source_identity`) |
| 31 Terminal-only identities (`BlockId`, `OperationId`, `EdgeId`, `PlaceId`, `ValueId`, `ClaimId`, `ObligationId`, `PropositionId`, `ContractId`, `CallableContractId`, `BoundaryMachineId`, `MachineDescriptorId`, `ScalarApplicationId`, `StructuralCaseId`, `StructuralDomainId`, `ScalarDomainId`, `ScalarQualificationSetId`, `SuspensionCrossingId`, `CycleComponentId`, `DomainSemanticId`, `EvidenceIdentity`, `EvidenceTermId`, `ContentDomainId`, `AdmissionSiteId`, `SelectionDeclarationId`, `SelectionQuestionId`, `ProfileDecisionId`, `CallingAbiIdentity`, `CallableInvocationSite`, `BoundaryRequirementKind`, `WorkScheduleIdentity`) | 61 crates, none source-side; only `isa-aarch64`, `isa-x86_64`, `runtime-abi`, O07 and `evaluation-accounting` lack a `terminal-psi` edge today | **`terminal-psi`** (stage 05 output) or **TPV/`identities`** (*judgment*: TPV keeps the ISA crates off the big `terminal-psi` crate) |
| 12 identities that `tt` also uses (`MachineId`, `ServiceId`, `StructuralTypeId`, `StructuralFieldId`, `InvocationSite`, `BoundaryCompletion`, `ExternalCompletionKind`, `Calling*`) | 4–52 | **TPV/`identities`** |
| Term language: `scalar_terms`, `closed_integers`, `proposition`, `float_meaning_clauses`, `float_integer_conversions`, `scalars::*`, `storage::*`, `behavior_policy`, `byte_predicates`, `scalar_condition_role`, `Termination`, `CallableCrashPolicy`, `ScalarAddressModel` | 4–41; `tt` uses `proposition::integer_types` | **TPV/`scalars`**. Not a primitive (owner ruling). |
| `proof_applications`, `PsiSemanticId`, `TargetMachineCallable`, `CallableShapeMismatch`, `boundary::callback_identities` | 2–10 | **TPV** / **`terminal-psi`** |
| `ForeignLocatorCandidate`, `NativeBindingLocator`, `compiler_evaluation`, `source_selections`, `boundary::program_entry` | Build, products, packages, build-time-evaluation, terminal-installation, terminal-production, `tt`, `tp` | **`build-requests`** |
| `declaration_descriptions` | build-settings, source-compiler, `tt` | **`tt`** |
| `source_review` | review-evidence only | **`packages/review-evidence`** |

### 4.2 `language-semantics`

| Module | Positions | Destination |
| --- | --- | --- |
| `declaration_selection::{AuthoredDeclarationSelection*, ResolvedAuthored*, CompilerDerivedSelectionPartition, CollectionMeasure, EnumExclusionRequest, ExclusionVocabulary, RootBindingRequest, is_unresolved_provider_selection_call}` | srt, tt, P02, P03, P05 | **`srt`** (P02 writes, P05 reads). New edge `tt → srt`. |
| `declaration_selection::{Build*, Executed*, DescribedProduct*, NativeBinding*, CapturedNativeIntent, RootBinding*, ProviderReadinessRequest, AuthoredRootGrant, AuthoredBehaviorExclusion*, AssumedBindingContract, ProductDeclaration*, ProductOperandRole, ProductTarget, CompositionMode, product_operands}` | Build, products, compiler, build-time-evaluation, packages, provider-*, tt, P03–P05, `terminal-psi` (`CompositionMode`) | **`build-requests`**. The Terminal Psi use of `CompositionMode` raises Q-B. |
| `semantic_identities`, `semantic_domains`, `service_reach`, `machine_termination` | srt, tt, P02–P05, flow-effects, build-time-evaluation, review-evidence | **`srt`** (*judgment*: check whether P02 or P03 writes them). `BoundaryQualificationIssuer` → provider-selection. |
| `external_bindings`, `callback_declarations`, `service_surface`, `opaque_representations`, `build_requests`, `reflection`, `type_identity`, `const_value`, `numeric_intrinsics`, `byte_predicates` | each spans `tt` and `tp`/backend/realization | **TPV/`declarations`**. Merge `opaque_representations` with `typed-trees::opaque_representations` (pipeline.md:1413 names the latter as the owner). |
| `trust_declarations`, `provider_declarations` | `tt` + trust-model / provider-selection / provider-planning | **TPV/`declarations`**. Realization reads them from Terminal Psi (owner answer). This removes a sidecar around the pipeline. |
| `quotient_correspondence`, `conformance_identity`, `permissions`, `SuspensionValueOrigin` | tp (+P05, flow-effects) | **`terminal-psi`** / **`terminal-semantics`** |
| `value_domain`; `wire` | tt, P03; build-time-evaluation, source-compiler, P04 | **`tt`** (both; *judgment* for `wire`) |
| `boundary_bodies`, `boundary_provider_review` | provider-selection only | **`realization/provider-selection`** |
| `boundary_values` | backend `layout` only | **`backend/layout`** |
| `native_bindings` | build-settings only | **`build-system/build-settings`** |
| `content` | optimization-unit-semantics only (non-test) | **`optimization-unit-semantics`** (*judgment*) |
| Re-exports of `language_core::{Carry*, …}` (`lib.rs:2437`) | 14 | **delete** |

### 4.3 `language-core`

| Item | Users | Destination |
| --- | --- | --- |
| `operator_spelling`, `cast_form`, `source_semantics::receiver_binding`, `constant_value::EvaluatedConstant` | syntax-trees, srt, tt, P01–P05 (+ review-evidence, terminal-production) | **`syntax-trees`**: the first representation that needs them |
| `data_layout_queries`, `source_semantics::{contract_result, place_root}`, `core_vocabulary` | tt/srt, P02–P05, build-time-evaluation, `symbols` | **`tt`** / **`srt`**, following the first user |
| `atomic`, `inline_assembly`, `sealed_intrinsics`, `static_interrupts`, `source_semantics` (`Carry*`, `Multiplicity`, `DomainClassification`, …) | read on both sides (tp, O00–O03, backend, realization) | **TPV/`scalars`** or **TPV/`declarations`** |
| `descriptor_tables` / `float_operations` / `layout_storage` | `terminal-psi` / `terminal-semantics` / P02 only | that single user |

### 4.4 `symbols`

| File | Users | Destination |
| --- | --- | --- |
| `table.rs`, `storage.rs`, `ParameterOccurrence` | srt, tt, P02, P03, build-time-evaluation, source-compiler, package-compilation | **`srt`** (P02 output) |
| `symbol.rs` (`Symbol`, `SymbolHandle`, `SymbolKind`, `SymbolPath`), `retained_declaration.rs` | 22, including tp, check-tables, trust-model, provider-planning | **TPV/`identities`** |
| `builtin.rs`, `primitive.rs`, `sealed_intrinsic_at` | O00, backend effects, review, build-time-evaluation, tt, P02–P05, provider-admission | **TPV/`declarations`** (language tables read by O00) |
| `fast_map.rs` | 7 | **primitive** `arena` (*judgment*) |

### 4.5 `layout-plans`

| Module | Destination |
| --- | --- |
| `layout_reports` | **TPV/`layout`** (program-derived; written in `tt`, `terminal-psi`, `terminal-wire`, build-time-evaluation and the interpreter) |
| `aggregate_geometry` | **TPV/`layout`**: language layout rules, not a primitive |
| `placement`, `materialization`, `symbolic_values`, `field_values`, `field_codec`, `stored_integer_writes` | **`terminal-psi`** family: portable materialization actions (pipeline.md:1490) |
| `private_callback_placements` | **`backend/calling-conventions`**: all users are Omega |
| `target_facts::LayoutTargetFacts` | **`build-requests`** (Q-B covers the Terminal-side reads) |

### 4.6 `optimization-core`

| Module | Destination |
| --- | --- |
| `codec_bytes`, `cursor`, `digest_encoder`, `error_display`, `record_views`, `tagged_vocabulary`, `variant_accessors` | **primitive** `wire` (merged with the psi copy) |
| `optimization_core.rs` (the catalog), `contracts/{rule, budget}`, `manifest/work_usage`, the stage-manifest identities, `identities/{rules_and_facts, decisions, bundle}` | **`NV`**. The catalog moves into the Omega pipeline (owner answer). `NV` is the leaf every native form O00–O09 carries (*judgment*: the first Omega form is `abstract-operations`, but O03 and later cannot reach it under `boundary.rs` `EARLIER_PROGRAM_FORMS`). `build-declarations` and `package-compilation` then depend on `NV` (*unverified*: no rule forbids it today). |
| `decisions/*`, `manifest/{decision, pass, fact_reference, error}`, `contracts/{decision, analysis}`, `CanonicalIdentityEncoder`, `OptimizationIdentityBundle` | **`abstract-operations/optimization-unit`** |
| `post_terminal` | **`verified-optimization-input`** |
| `identities/artifacts` | **`machine-code`** |
| `identities/stages` | **`backend/artifacts/native-artifact`** |
| `refusal_site` | **`target-vocabulary`** (*judgment*) |
| `rollback`, `report_request` | **`compilation/products`** (*unverified*: `package-compilation` uses `report_request` but does not depend on `products`, so it may need `products/measurements`-style nesting) |
| `mutation_matrix` re-export | **delete** |

## 5. Ordered PR sequence (lowest risk first)

Every step deletes the old location in the same PR and rewrites the README or
pipeline.md sentences that named it. Each step is proved by:

- `mbx xtask check-boundary`
- `mbx xtask check-rules --base origin/main`
- `mbx clippy -p <each touched crate> --all-targets -- -D warnings`
- `mbx test -p <moved crate> --lib`, plus the receiving crate's tests

"Touched" counts are taken from the tables above.

| # | Lane | Change | Touched | Extra proof |
| --- | --- | --- | --- | --- |
| 0 | docs/repo | Rewrite the superseded text (§1) to the primitives-only rule; record the remaining questions in `OWNER_QUESTIONS.md`. | 0 crates | `check-rules` |
| 1 | tools | Delete the ~46 declared-but-unused foundation dependencies (§0). | ~30 manifests | Admission type check |
| 2 | tools | `source-audit` and `mutation-matrix` → `tools/test-support/`; delete `optimization-core`'s `test-support` feature and re-export. | 6 | Source-guard tests of `effects`, `task-plans`, `boundary-applications`, `terminal-psi`; `mbx test -p image --lib` |
| 3 | psi | New primitive `wire`: merge both helper copies + `fnv1a`, `identity_encoding`, `structural_path_encoding`; `id_table` and `fast_map` into `arena`. Delete the copies. | ~45 | Codec tests of `optimization-core`, `terminal-wire`, `terminal-codec`, `numerics` |
| 4 | omega | `function-identity` → `NV`; delete the crate. | 13 | `check-boundary` (native-form rules) |
| 5 | psi/omega | Single-user moves (§4.2–4.6 "only" rows); delete the language-core re-exports and the `float_semantics_catalog` alias; diagnostics parse helpers → P01. | ~15 | Receiving crates' tests |
| 6 | omega/psi | Non-pipeline owners: `platform-custody` → `packages/` (and `make_executable` caller → `products`); `artifacts` → `products/measurements`; `bounded-process` cli part → cli (rest after Q-A); `filesystem-authority` split; `build-time-values` and the `evaluation-accounting` sponsor → `build-time-evaluation`; `invocation.rs` → `terminal-work`. | ~25 | `check-boundary`; the Build accounting test that `evaluation-accounting/README.md` describes (an exact budget completes and one unit less is refused) |
| 7 | omega | Split `optimization-core` (§4.6); delete it. With step 4 and the 3 crates moved in step 6, **delete `omega/foundation/`** (`bounded-process` waits for Q-A). | ~25 | Byte-identical codec tests (`OMGOPT`, `OMGDEC`, `OMGBDL`) |
| 8 | psi | Create `build-requests`; move §4.1/4.2/4.5 Build rows. | ~15 | `mbx test -p build-time-evaluation`; one `omega run --both` sample |
| 9 | psi | Create `source-files`; move the `source` split rows and `PackageKeyIdentity`. | ~20 | P00 tests; `check-boundary` |
| 10 | psi | Source-side stage output and source-only language tables → `syntax-trees` / `srt` / `tt` (§4.2–4.4). New edge `tt → srt`. | ~15 | P01–P03 tests; `omega run --both` on a positive and a negative sample |
| 11 | psi | Create `TPV` with subject packages. Move in 5 PRs, each deleting its source module: (a) identities + symbol handles; (b) term language + numeric rules; (c) declarations (language-core cross tables, builtins, language-semantics carried modules, optimization selections); (d) resources (access-plans, extent identities); (e) layout. Then flow-effects, extents and materialization actions → `terminal-psi` / `terminal-semantics` / interpreter. | 20–60 per PR | `check-boundary`; Terminal codec round-trip tests; `omega run --both` on one sample |
| 12 | psi | `numerics` becomes `bignum`; delete the now-empty `language-core`, `language-semantics`, `semantic-vocabulary`, `symbols`, `optimization`, `access-plans`, `layout-plans`, `extents`, `flow-effects` crates. | path rewrite | `check-boundary` |
| 13 | tools | Turn on the rules in §6. | xtask | `mbx test -p xtask` |

Steps 1–7 need no owner answer except Q-A, which affects only
`bounded-process`. Steps 8–11 follow from the owner's answers. Q-B affects
whether Terminal-side readers reach `build-requests` and `source-files`
directly, or through copies in TPV.

## 6. Boundary rules (in `tools/xtask/src/boundary.rs`)

| Rule | Form | Current violators |
| --- | --- | --- |
| F1 | A foundation crate depends only on other foundation crates and crates.io | `source → semantic-vocabulary` (fixed in step 9); `numerics → optimization` and `semantic-vocabulary → numerics, optimization` disappear with steps 3/12 |
| F2 | Foundation is a closed list in `boundary.rs`: `arena`, `compiler-storage`, `compiler-storage-derive`, `diagnostics`, `source`, `wire`, `bignum`, `execution-admission`. Any other crate under `*/foundation/` fails, and a crate under `omega-rust/omega/foundation/` fails. Adding a primitive is a reviewed edit of the list. | everything else, until step 12 |
| F3 | Every foundation item is used by at least one pipeline position, and every declared foundation dependency is used. The xtask port of `fdn/modscan.py` writes `tools/ratchets/foundation_usage.txt` (crate · item · positions); `check-boundary` regenerates and compares it, so the evidence cannot drift. | the §0 unused list |
| R1 | Only drivers depend on stage crates: `source-compiler`, `terminal-production`, `native-compiler`, plus the existing `STAGE_CALL_ALLOWANCES`. | `build-time-evaluation → P02, P03, P04` (Q-C); test crate `ind19-spectrum-validation → P00, P01` |
| R2 | A representation package depends only on primitives, on packages in its own family, and on families earlier in the declared order. Order: `source-files`, `build-requests` < `terminal-psi/vocabulary` < `tokens` < `syntax-trees` < `srt` < `tt` < `terminal-psi` < `target-description`, `provider-plans` (Omega inputs, Part B) < `NV` < `abstract-operations` < `target-operations` < `selected-instructions` < `register-homes` < `post-allocation-machine` < `machine-code`. The existing firewall rules stay: Terminal and Omega never reach `tokens`…`tt`. | 2 later-family edges (`verified-optimization-input → target-operations`, `selected-instruction-analyses → register-homes`); ~40 edges skipping straight to `terminal-psi`/`NV` (allowed only if Q-D says "earlier", not "immediate predecessor"); ~25 edges to backend target-description and plan crates (resolved by Part B: `target-description`, `provider-plans`, rule B-R3); `verified-optimization-input → provider-admission, provider-selection, terminal-installation` (realization edges, Q-G); `terminal-semantics, terminal-wire → proof-admission` |
| R3 | `PERMITTED_OWNERS` (boundary.rs:23) drops `"omega-rust/omega/foundation"`. It adds `build-requests` and `source-files` only if Q-B allows the Terminal side to read them. | |

F3 relies on a regular-expression scan until it runs on type-checker output. A
missed reference makes an item look less used, so the check fails toward
moving code out of foundation.

# Part B: Backend dissolves (owner ruling 2026-11-17)

The ruling: `omega-rust/omega/compilation/backend/` should not exist, because
native compilation is one linear pipeline.

Today the backend is 221,793 lines in 29 crates: all `.rs` lines, counted the
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
- Carry the program-derived provider plans in a second input representation.
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

Step B0 rewrites all of these together (AGENTS.md "Scope and escalation").

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

- **Six unnumbered steps sit between O06 and O07 and after O09.** The
  `machine-emission` lib header calls them "four route stages", and its doc
  says "Native realization drives our four route stages in sequence".
- **A side channel bypasses the pipeline.** `native-compiler/src/pipeline.rs:265`
  `emission_facts` "Copy what emission needs from stage 02's output and cannot
  read from the current machine program: the admitted lowering facts and the
  optimized abstract program". This is a sidecar around O03–O09 (AGENTS.md:
  "do not bypass the pipeline with sidecars").

## B3. New numbered stages after O09

The stage numbers and names below are *judgment*. Each name follows the
`X-to-Y` rule (pipeline.md:1618).

| Stage | Input → output | Takes | Lines |
| --- | --- | --- | --- |
| **O07** (existing, widened) | post-allocation machine → selected-form encoding | `machine-emission::{frame_layout, frame_protocol, function_realization::stage_function_frame}`. The stage's name already says its input is the post-allocation machine; framing becomes O07's first step, and `FramedMachineProgram` stays an internal form (it is already defined in `machine-code/src/layout.rs:261`). Plus the ISA encoders (§B4). | 2,530 + 428 + ≈400 |
| **O10** `resolved-layout-to-function-fragments` | resolved layout → unplaced function fragments | `function_realization::assemble_fixed_frame_realization`, `exit_contract`, `fragment_emission`. `emission_facts` is **deleted**: the lowering facts and projection identities it copies must be carried forward in the `machine-code` representations (*unverified*: how much O03–O09 must carry). | ≈744 + 3,654 + 1,893 |
| **O11** `function-fragments-to-function-fragments` | fragments → framed fragments | `frame_application`: inserts prologues and epilogues and re-encodes stretched branches. Its doc keeps it apart from O08–O09 on purpose: "ensures that branch relaxation operates on exact body displacements first". | 2,571 |
| **O12** `function-fragments-to-text-section` | framed fragments → placed text section | `text_placement`, `code_layout` | 1,800 + 258 |
| **O13** `text-section-to-object` | text section → object | `object-file` (`fragment_container`, `object_plan`, `names`, `relocation_free_object`); `image-emission::{function_fragments.rs` production part, `object_artifact.rs`, `object.rs}`. Their docs: "`function_fragments.rs` is the sole builder of `ObjectArtifact`"; "`object.rs` composes private callback objects and component shared images". | 3,552 + ≈26k + 5,737 + 931 |
| **O14** `object-to-image` | object → executable or component image | `image-emission::{image.rs, entries.rs}`; `image` (builder, placement, relocation application); `image-elf`, `image-macho`, `image-pe` as format writers chosen from the target description; `machine-emission::{native_entry_bridge, native_foreign_bridge, native_bridge_encoding}` (*judgment*: entry and adapter shims are bound into the image in `entries.rs`); the ISA hosted-entry and floating-control sequences (§B4) | 2,032 + 6,254 + 5,281 + 28,805 + 3,069 |

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

- Each `validate_*` entry point moves beside the representation it checks, as
  its own package. The precedent is `optimization-unit-semantics` (pipeline.md:
  "beside the representation whose meaning and rewrites it checks").
- `image-emission`'s replay of O02/O03 (`retained_working_program.rs`,
  `function_fragments/replay/*`, `source/retained_source_validation.rs`) moves
  to validator packages beside `target-operations` and `selected-instructions`.
  That deletes the image-emission rows in `STAGE_CALL_ALLOWANCES`.
- The `machine-emission` rows go away because O10–O12 become stages the driver
  runs.
- The `products → O00` row goes away when the retained-input codec moves into
  `verified-optimization-input`; that row's own comment says it "still lives in
  Omega 00 rather than beside `RetainedTerminalArtifact` in
  verified-optimization-input".
- The `native-artifact → O00` row ("Runtime provisions and the entry receiver
  read Omega 00's admitted input") goes away when those admitted-input types
  move to `verified-optimization-input` (*unverified*: which types).

### `machine-emission` (17,780 lines) by module

| Module | Lines | Users | Destination |
| --- | --- | --- | --- |
| `frame_layout.rs`, `frame_protocol.rs` | 2,530 + 428 | native-compiler (via `stage_function_frame`), O07 reads its output | **O07** |
| `function_realization.rs` | 1,144 | native-compiler | split: `stage_function_frame` → **O07**; `assemble_fixed_frame_realization` → **O10** |
| `exit_contract.rs` | 3,654 | (internal; "from stage 09's output alone") | **O10** |
| `fragment_emission.rs` | 1,893 | native-artifact, image-emission, native-compiler | **O10**; types → `machine-code/function-fragments` |
| `emission_facts.rs` | 42 | object-file, native-compiler | **delete** (sidecar; carry the facts instead) |
| `frame_application.rs` | 2,571 | native-artifact, image-emission | **O11** |
| `text_placement.rs`, `code_layout.rs` | 1,800 + 258 | native-artifact, image-emission, object-file | **O12**; types → `machine-code/text-section` |
| `native_entry_bridge.rs`, `native_foreign_bridge.rs`, `native_bridge_encoding.rs` | 2,053 + 428 + 588 | native-artifact, image-emission | **O14** (*judgment*) |
| `lib.rs`, `tests/` | 84 + 307 | | delete / move with modules |

## B4. The target description becomes an input representation

New package `omega/compilation/representations/target-description`.

- **Who builds it:** Build, once per selected target. This matches the board
  item **BUILD-OWNED-TARGET-SELECTION** (TASKS.md:83: "Retire target-prefixed
  machines for target-keyed Build rows").
- **Who receives it:** Omega 00, as an input. Every native form may depend on
  it (rule B-R2).
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
| `target` | 3,921 | `target_profile`, `target_semantics`, `x86_features`, `float_realization`, `foreign_locator`, `component_locator`, `elf_loader` | `explicit_target_set` (orchestration, compiler, cli) → **Build** (`build-system/orchestration`); `PackageReviewRepresentation*` → **review-evidence**; app metadata (`ApplicationIdentifier`, `HostedApplicationIntent`, `NativeApplicationMetadata`, used by O00, image stages, build, products) → `target-description` as the per-product part (*judgment*: it is authored per product, not per target) |
| `register-model` | 3,514 | all (declarative register model, constraint catalogs, preservation catalogs) | |
| `register-environment` | 5,171 | all (derived purely from the target: "construct and validate the target environment directly from ISA declarations") | |
| `isa-x86_64`, `isa-aarch64` tables | ≈6,212 + ≈4,879 | `register_model.rs`, `machine_effects.rs`, `preservation_storage.rs` | |
| `calling-conventions` | 9,425 | ABI rules: `plans/vocabulary`, `plans.rs` entry-plan evaluation, `call_plan_evaluation`, `aggregate_layout`, `structural_shapes`, `boundary_values`, `host_operations`, `terminal_layout` (*judgment*: these are the rule functions over the description; `native-vocabulary`'s README sets the precedent of "pure predicates" inside a representation) | program-derived callback and boundary-application records (`callback_materializations`, `boundary_application/*`, `callback_catalog`) → **`provider-plans`** (§B5) |
| `runtime-abi` | 2,356 | target ABI geometry (`scalar_reference`, `borrowed_storage*`, fat descriptors, `NativeForeignFrame`, `static_interrupts`) | host leaves used only by the interpreter (`descriptor_io`, `network_address`, `host_numeric`; `windows_provider` also products) → **`omega/interpreter/terminal-runtime`** |

### Are ISA encoding tables part of the input, or the encoding stage? Decision: split them

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
these are stage-to-stage calls, which `boundary.rs` forbids. That is open
question Q-F.

## B5. Program-derived plans

The `backend/plans/` and `backend/runtime/` crates are stage output. Realization
computes them, not a numbered stage: provider planning, selection and
admission. Omega 00, 02 and 03 then read them.

Under the rule "move to the stage that computes them", their home is the output
of realization. Realization is not in the numbered pipeline (open question Q-G).
Until the owner rules, this plan puts them in a second Omega input
representation, `omega/compilation/representations/provider-plans`. Omega 00
receives it (*judgment*); `verified-optimization-input` already bundles
"target, provider, callback, interrupt and evidence selections admitted with
it" (native representations README).

| Crate | Lines | Who computes it | Who reads it | Destination |
| --- | --- | --- | --- | --- |
| `effects` | 7,782 | provider-planning / selection ("selected provider realization evidence") | O00, realization, products, packages, build-settings, trust-model | **`provider-plans`** |
| `backend-plan` | 2,726 | provider-planning / selection (`TerminalTargetProviderSelection`) | O00, verified-optimization-input, realization, terminal-runtime, compiler, products | **`provider-plans`**; `canonical_schemas` (provider-planning only) → **provider-planning** |
| `program-entry-plan` | 1,300 | provider-planning (`SelectedProgramStorageEntryPlan`, physical contract plans) | O00, verified-optimization-input, image stage, compiler | **`provider-plans`** |
| `task-plans` | 1,901 | provider-admission (`admit_same_stack_contribution`) / compiler (`realize_task_activations`) | O00, register-homes, machine-code, image stage | **`provider-plans`** |
| `installation-evidence`, `bound-installations` | 357 + 348 | provider-admission ("only admission can construct the sealed records") | O00, O02, O03, target-operations, target-vocabulary, image stage | **`provider-plans`**. `boundary.rs` `AFTER_PREPARATION` / `SELECTION_REPRESENTATIONS` must name the new path, and selection must still not reach `target-operations` through it. |
| `boundary-applications` | 1,350 | provider-selection ("retained beside one canonical Terminal artifact") | O00, verified-optimization-input, image stage, provider-selection | **`provider-plans`** (*judgment*) |
| `encoded-scalar-plan`, `normalized-foreign-call-plan` | 324 + 1,202 | O07 encoders | ISA crates only | **O07** |
| `post-handoff-writer-plan` | 960 | O14 | ISA crates only | **O14** (*judgment*) |

## B6. Products, realization and leftovers

| Crate | Lines | Destination | Evidence |
| --- | --- | --- | --- |
| `native-artifact` | 16,394 | **split**. Carriers the runtimes and native-compiler read (`NativeArtifact`, `NativeComponentProduct`, `TerminalRuntime*Provision(s)`, `DynamicElfNativeArtifact`) → **`image` representation**. `fragment_object_publication` (`emit_fragment_object`, which runs O10–O13 today) → **delete**: the native driver runs those stages. `entry_receiver` (`admit_executable_entry`, native-compiler) → **O14**. Publication and receipts (`publication.rs`, `package_publication.rs`, `component_product` receipts) → **`products`**. `physical/` evidence derivation (≈7k, used inside the crate) → validator package beside `image` (*judgment*). | Its own header: "self-verifying executable/package publication receipts" (product); `products` is the only user of the receipt types. |
| `component-description` | 2,542 | **split**. Producer and publisher (`publish_component_description`) → **`products`**. The carrier and `verify_component`, which O00, provider-admission and verified-optimization-input read when a component is imported → **`provider-plans`** (*judgment*). | Its header: "carrier, its wire codec, its producer, and its source-free independent verifier"; declared-unused in O00 and compiler (§0 method). |
| `layout` | 1,934 | **`omega/compilation/realization/layout`** (nested package). Production users are only provider-admission and provider-planning, and admission does not depend on planning, so a nested shared package is needed. | Module scan |
| `isa-conformance` | 579 | **O07 test support** or `tools/test-support/` | Dev-only |
| `x86_64-encoding` | 78 | **O07** (x86 encoder) | |

**Line balance:** 98,896 (stage work) + 83,120 (target crates) + 16,900 (plans)
+ 20,286 (artifacts) + 2,591 (leftovers) = 221,793. That matches the total
counted from `cargo metadata` paths.

## B7. PR sequence for Part B

Same proof bar as §5: `check-boundary`, `check-rules`, clippy on every touched
crate, and the moved and receiving crates' tests. Every step that moves stage
work also runs `omega run --both` on one positive sample, with no change in
output bytes. The image-stage steps also run the existing image and validator
tests: `mbx test -p image-emission`, `-p native-artifact`, `-p image-elf`.
Each step deletes its old crate or module.

| # | Change | Touched (approx.) | Deletes |
| --- | --- | --- | --- |
| B0 | docs/repo: rewrite pipeline.md:166-167, 225-227, 1087, 1097, 1128-1131, the AGENTS.md `backend` lane row and the representation READMEs to the linear pipeline. Retire or redefine the `backend` lane. | 0 crates | old text |
| B1 | Delete the 22 declared-but-unused backend dependencies found by the scan (*unverified* until the type check), for example `x86_64-encoding` in image-emission, `register-environment` in O06, `target` in O05, `program-entry-plan` in O00/native-artifact/products, `task-plans` in O00/target-operations/products. | ~15 manifests | dead edges |
| B2 | Leftovers: `layout` → `realization/layout`; `isa-conformance` → test support; `x86_64-encoding` into `isa-x86_64` (until B6 moves it). | 6 | 3 crates |
| B3 | Create `target-description`; move `target`, `register-model`, `register-environment`, the ISA tables, the `calling-conventions` ABI rules and the `runtime-abi` geometry. Move the runtime-abi host leaves → terminal-runtime and `explicit_target_set` → Build. Update `boundary.rs` `AFTER_PREPARATION` / `SELECTION_REPRESENTATIONS` paths. | ~60 (target has 59 users) | `target`, `register-model`, `register-environment`, `runtime-abi`; `calling-conventions` ABI half |
| B4 | Create `provider-plans`; move `effects`, `backend-plan`, `program-entry-plan`, `task-plans`, `installation-evidence`, `bound-installations`, `boundary-applications`, the `calling-conventions` callback/boundary records and the component-description reader. | ~35 | 7 crates + `calling-conventions` |
| B5 | Fold `stage_function_frame` (frame layout and protocol) into O07. The driver calls O07 with the post-allocation machine. | 4 | 3 machine-emission modules |
| B6 | Move the ISA encoders, `encoded-scalar-plan` and `normalized-foreign-call-plan` into O07, with the interim branch-encoder arrangement Q-F decides. | ~10 | `isa-x86_64`, `isa-aarch64` encoder parts, 2 plan crates |
| B7 | Carry the `emission_facts` fields forward in the machine-code representations; delete `emission_facts`. | 6–10 (*unverified*) | the sidecar |
| B8 | New stages O10, O11, O12 and representations `function-fragments`, `text-section`; the driver runs them. Delete `machine-emission` and the 5 machine-emission `STAGE_CALL_ALLOWANCES` rows. | ~8 | `machine-emission` |
| B9 | New stage O13 and the `object` representation (object-file + object-artifact building). | ~8 | `object-file`, image-emission object half |
| B10 | Move image-emission's O02/O03 replay and the native-artifact `physical/` evidence into validator packages beside their representations. Move the O00 retained-input codec and the admitted-input types that products and native-artifact read into `verified-optimization-input`. Delete the 3 image-emission rows, the products row and the native-artifact row in `STAGE_CALL_ALLOWANCES`, and the replay-gateway file lists (boundary.rs:768-801). | ~8 | replay modules, 5 allowance rows |
| B11 | New stage O14 and the `image` representation (image, image-elf/macho/pe, image-emission image and entries, bridges, the ISA hosted sequences, post-handoff writers). Runtimes switch to the `image` representation. | ~12 | `image`, `image-elf`, `image-macho`, `image-pe`, `image-emission`, `post-handoff-writer-plan` |
| B12 | Split `native-artifact` (carriers → `image`, publication → `products`, `entry_receiver` → O14) and `component-description` (publisher → products). **Delete `omega-rust/omega/compilation/backend/`.** | ~8 | last backend crates and the directory |
| B13 | Turn on the Part B rules (§B8). | xtask | |

**Ordering against Part A.**

- B1–B4 can run in parallel with Part A steps 1–7. They touch different crates,
  except `target` and its foundation dependencies, which step A3 rewrites; do
  A3 first.
- B5–B12 depend only on B3, B4 and Q-F.
- Part A step 11 (TPV) is independent of Part B.

## B8. Boundary rules for Part B

| Rule | Form | Current violators |
| --- | --- | --- |
| B-R1 | `omega-rust/omega/compilation/backend` must not exist: no workspace package path may start with it. | 29 crates, until B12 |
| B-R2 | **No crate outside a numbered stage owns stage work.** Only crates under `compilation/pipeline/` may define a `pub fn stage_*` / `execute_*` / `emit_*` that turns one representation into another. The check extends today's `single_form_entry_violations` (boundary.rs:875-985), which already pins named stage entry points to their files. A representation crate holds data, codecs, validators-as-separate-packages, and pure rule functions only. | the machine-emission `stage_*` functions, `object_file::stage_optimized_relocation_free_object_container`, the image-emission `build_*` / `emit_*`, `native_artifact::emit_fragment_object` |
| B-R3 | **Representations take the target description as an input.** Every native representation may depend on `target-description` and `provider-plans`. Neither of those depends on any other native representation, on any stage, or on realization; only on primitives, `native-vocabulary` and Terminal Psi. | none expected once B3/B4 land |
| B-R4 | `STAGE_CALL_ALLOWANCES` ends empty; the existing stale-row check keeps it from growing. | 10 rows today (9 backend + products → O00) |
| B-R5 | The runtimes (`terminal-runtime`, `native-provider-runtime`) may reach `target-description`, `provider-plans` and `image`, but no stage. This is the existing `PERMITTED_OWNERS[1..]` rule once `omega/compilation/backend` is removed from it. | none expected |
| B-R6 | `OMEGA_COMPILATION` (boundary.rs:69) drops the backend entry. The Omega → source-side Psi firewall applies to stages, representations and realization unchanged. | |

# Owner questions and side findings (both parts)

## 7. Owner questions

### Answered by the refined ruling

| Question | Answer |
| --- | --- |
| Q1: crates with no pipeline user | They leave foundation for their owners, splitting if needed (§3: packages, products, build-time-evaluation, cli, tools). |
| Q2: source-side tables carried into Terminal Psi | They go to a carried representation, `terminal-psi/vocabulary`, which counts as a representation, not foundation. |
| Q3: Build inputs | They get an input representation, `build-requests`. |
| Q4: optimization catalog | It moves into the Omega pipeline (`NV`). |
| Q5: realization reading source declarations | Realization reads them from Terminal Psi. |
| Q9: the 2026-09-26 "do not fold foundations into a stage" sentence | Superseded for foundation. |
| Q-E: do target descriptions count as a pipeline input? | Yes. The backend ruling makes the target description an input representation, `target-description` (§B4). |

### Still open

Write these into `OWNER_QUESTIONS.md` after `git fetch` and a re-read
(AGENTS.md "Scope and escalation").

- **Q-A. Where does the shared process runner go?**
  - **Who uses it:** the native-provider runtime, package resolvers
    (`resolver-execution`, `source-acquisition`), the CLI and three dev tools
    all start child processes through `bounded-process`. None of them is the
    compiler pipeline.
  - **Why the obvious home fails:** the runtime's dependency check only lets it
    use compiler crates, so it cannot use a crate under `packages/`.
  - **The options:**
    - (a) The interpreter owns the runner, and packages depend on an
      interpreter crate.
    - (b) Packages own it, and the runtime check gains one exception.
    - (c) A small shared host area outside the compiler, for example
      `omega/host/`.
- **Q-B. May Terminal Psi and the native compiler read Build inputs and loaded
  source data directly?**
  - **What reads them today:** Terminal Psi uses `CompositionMode` and the
    layout target facts from Build. `terminal-semantics` uses the
    consumed-source identity.
  - **The options:**
    - (a) The native side may depend on the new `build-requests` and
      `source-files` packages. They contain no source trees.
    - (b) Terminal Psi copies the few fields it needs into its own vocabulary,
      and the native side never reaches the input packages.
- **Q-C. Is Build-time evaluation a second front-end driver?** It calls stages
  02–04 directly instead of going through the Psi facade (`source-compiler`).
  Allow it as a driver, or route it through the facade?
- **Q-D. "Only its predecessor": immediate or any earlier form?** Every native
  representation (all 13 crates, checked in metadata) uses Terminal Psi
  identities directly. A strict immediate-predecessor rule would force each
  form to re-export the previous form's identities.
- **Q-F. Who encodes branches after stage 07?**
  - **The problem:** stage 07 encodes instructions. Stages 08 and 09 lay out
    and relax branches, and the new frame-insertion stage (O11) re-encodes
    branches that an inserted epilogue stretched. All three call the ISA branch
    encoders today (`branch_forms`, `jump`). If the encoders live in stage 07,
    these become stage-to-stage calls, which `boundary.rs` forbids.
  - **The options:**
    - (a) Stage 07 writes every branch's possible encodings (short and long
      forms, byte templates) into its output, so later stages only choose and
      patch. This changes how the output is laid out, but not which programs
      compile.
    - (b) Branch encoding stays a small target library that stages 07–11 may
      call. That is an exception to "no crate outside a stage owns stage work".
    - (c) Fold stages 08, 09 and frame insertion into one layout stage that
      owns branch encoding. That reverses the reasons their docs give for
      keeping them separate.
- **Q-G. Where does realization sit in the linear pipeline?**
  - **What it does:** provider planning, selection and admission (the crates
    under `omega/compilation/realization/`) compute the provider plans,
    installation facts, task plans and boundary applications. Omega 00, 02
    and 03 read those.
  - **What this plan does meanwhile:** treats them as an input representation,
    `provider-plans`, beside the target description.
  - **The options:**
    - (a) Realization stays a separate producer of that input, like Build.
    - (b) Realization becomes a numbered step before Omega 00, and its output
      is Omega 00's input representation.

## 8. Side findings (for `OWNER_CODE_CONCERNS.md`; no extra PRs)

- **Duplicated wire helpers.** psi `optimization` and `optimization-core` each
  define `DomainDigestEncoder`, `Cursor`/`TakeError`, `push_encoded` and
  `structural_variant_accessor!`. Step 3 fixes this.
- **Duplicate opaque-representation records.**
  `language-semantics::opaque_representations` duplicates the owner that
  pipeline.md names, `typed-trees::opaque_representations`.
- **Publication inside the backend.** `native-artifact::package_publication`
  does product publication.
- **A very large carried vocabulary.** `terminal-psi/vocabulary` would hold
  an estimated 40–45k lines. Split it by subject (step 11) so it does not become one
  unreadable crate.
- **The native driver copies stage 02 output past stages 03–09.**
  `native-compiler/src/pipeline.rs:265` (`emission_facts`) hands stage 02's
  lowering facts and abstract program straight to emission, around the
  pipeline. Step B7 removes it.
- **A process-wide cache in `register-environment`.** It caches "one validated
  instance per target process-wide". Building the target description once per
  compile (step B3) may remove it (*unverified*).
- **`emit_fragment_object` lives in `native-artifact`.** It runs four
  emission steps plus object building, so a product crate is driving pipeline
  work. Step B12 deletes it.

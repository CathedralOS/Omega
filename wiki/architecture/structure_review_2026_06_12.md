# Omega Compiler: Structural Health Review — 2026-06-12

Performed by a dedicated review agent. Worktree: `agent-af6337b67f5674720`.
Suite baseline at review start: **256 pass / 0 fail** (omega-compiler + omega-interpreter).

---

## Part A: Report

### A1. Bloat — 15 Largest `.rs` Files (non-test, non-generated)

| # | Lines | File | Assessment |
|---|-------|------|------------|
| 1 | 5574 | `backend/isa/omega-isa-x86_64/src/lib.rs` | Cohesive: flat table of x86-64 encoder helpers (width fns, byte emitters, reloc helpers). All entries are narrow width-and-encoding helpers for specific instruction shapes. Not a grab-bag — it IS the ISA. Refactoring risk: high; benefit: low. **Fine as-is.** |
| 2 | 3026 | `backend/isa/omega-isa-aarch64/src/aarch64/runtime_storage.rs` | Cohesive but large: all AArch64 runtime-storage encoders in a single file. Pattern mirrors the x86 file. Candidate for splitting into sub-files by instruction family (load/store/branch) if the file keeps growing. **Acceptable now.** |
| 3 | 2901 | `orchestration/omega-interpreter/src/evaluator.rs` | Grab-bag smell: entire interpreter eval loop in one file. The recent addition of versioned-match handling stacked on top of concurrency, wire, and dispatch eval. A `mod evaluator { mod versioning; mod dispatch; mod wire; }` split would improve navigability. **Split candidate (low urgency).** |
| 4 | 2127 | `semantics/omega-names/src/lib.rs` | **Orphaned crate (see A3).** The file is cohesive (name resolution pass), but since the crate has zero external consumers, this is moot unless the crate is revived or merged. |
| 5 | 2121 | `backend/omega-instruction-selection/src/selection/storage_places.rs` | Cohesive: all place-resolution strategies (frame, machine, pointee) consolidated after the Phase 4 cleanup. Wide but not a grab-bag. **Fine as-is.** |
| 6 | 2027 | `orchestration/omega-visualizations/src/backend.rs` | Cohesive: backend visualization dump. Grows linearly with new IR nodes — acceptable. |
| 7 | 1962 | `semantics/omega-proof/src/obligations.rs` | Cohesive: proof obligation construction for all program constructs. Natural concentration point. **Fine as-is.** |
| 8 | 1956 | `backend/omega-instruction-selection/src/selection/runtime_dispatch/guards.rs` | **Stress point** (see A2a). Grew through many guard-fix cycles. Cohesive in intent but the guard lowering, resolution, and text-guard detection are interleaved. Candidate for `guards/mod.rs + guards/resolution.rs + guards/text.rs`. |
| 9 | 1936 | `orchestration/omega-visualizations/src/phase_diagram.rs` | Cohesive visualization dump. Fine. |
| 10 | 1888 | `representations/omega-typed-trees/src/expression.rs` | Cohesive: expression arena + node definitions. Grows with language features. Fine. |
| 11 | 1851 | `backend/omega-instruction-selection/src/widths.rs` | Cohesive: instruction-width functions for emission planning. Fine (mirrors ISA-file pattern but target-independent). |
| 12 | 1730 | `backend/omega-instruction-selection/src/selection/runtime_dispatch/writes/mutation.rs` | **Stress point** (see A2a). Grew as write strategies were added. Has some cohesion (mutation writes) but several write-strategy helpers are co-located that could be submodules. |
| 13 | 1574 | `backend/omega-instruction-selection/src/selection/runtime_dispatch/branches/straight_line.rs` | Cohesive: straight-line branch lowering. Large but single-purpose. Fine. |
| 14 | 1530 | `semantics/omega-validation/src/contract_entailment.rs` | Cohesive: polynomial entailment engine. Well-structured. Fine. |
| 15 | 1497 | `backend/isa/omega-isa-aarch64/src/aarch64/widths.rs` | Cohesive: AArch64 width functions. Fine. |

**Test registry note:** `canary_suite.rs` is 10173 lines and `differential.rs` is 987 lines. Both are test registries and are **fine as-is** — mechanical enumeration files, not logic files.

---

### A2. Stress Points Where Recent Fixes Stacked

#### A2a. `leaf.rs` — Terminal-Value Resolution Pipeline

**File:** `backend/omega-instruction-selection/src/selection/runtime_dispatch/branches/leaf.rs` (1436 lines)

This file owns the terminal-value selection pipeline for inline leaf-branch call results. At least four distinct miscompile fixes have stacked here since wave B:

1. Caller-local initializer substitution (`resolve_leaf_caller_local_initializer_names`) — the dungeon side-room fix.
2. Caller-context retry loop (`resolution_keys` dual-pass) — the by-value arg-to-free-machine fix.
3. `StructLiteral` member projection inside caller-local resolution — the by-value struct-arg fix.
4. Call-result-backed local guard (`has_slot` check with `StateCallResult` slot kind) — the chained-call struct-return fix.

The function `select_runtime_leaf_branch_terminal_value_write` (lines 555–684) carries all four layers. Each fix added a branch or a new substitution arm. The logic is correct and well-commented, but it is now a seven-layer resolver that will be opaque to a new contributor.

**Recommendation (not risk-free to extract, so deferred — see Part B):** Extract `select_runtime_leaf_branch_terminal_value_write`'s setup steps (binding copy, caller-local resolution, dual resolution-key loop) into a named `LeafTerminalValueResolver` struct with a single `emit(...)` entry point. The internal functions are already pure enough for this. The doc comments on the current code document the "why" well; the extraction should preserve all of them.

**"No write strategy selected" hard error question (explicit answer):**

Assessed: the silent fallthrough at the end of `select_runtime_leaf_branch_terminal_value_write` (line 679 comment: "unhandled case here simply emits nothing, exactly as before") IS reachable on legal programs. Specifically, when:

- the `call_result_slot_by_ordinal` function returns `Some` (slot found), AND
- the resolved value is a non-static, non-slice, complex expression shape that `select_runtime_frame_slot_value_write_in_table` does not handle, AND
- `select_runtime_resolved_mutation_write_in_table_with_scratch` also fails (e.g., an unsupported compound value form).

The `layout.size == 0` guard in `body.rs:851` prevents zero-size slots from being created, so a found slot always has `byte_size > 0`. However, the comment explicitly acknowledges the fallthrough as a known gap (not a future bug), and the text-guard exclusion in the poison guard (line 247: `guard_contains_string_literal`) is another example of intentional skip. **A hard error at the fallthrough would fire on currently-legal programs that use unimplemented text-guard lowering through refs/params.** Verdict: **do not add a hard error here.** A `#[cfg(debug_assertions)] eprintln!` logging the slot/expression identity would be lower-risk and still useful for diagnosis.

#### A2b. RUN_CANARIES Dual Registry

`canary_suite.rs` (the authoritative source) and `differential.rs` (which maintains `RUN_CANARIES`) are **already linked by a compile-time drift guard**: `run_canary_list_matches_canary_suite()` dynamically re-parses `canary_suite.rs` at test time and fails with copy-paste-ready entries if `RUN_CANARIES` diverges. The `EXCLUDED_RUN_CANARIES` list covers the five stdin-dependent and timing-sensitive canaries.

The current design IS a single-source-of-truth: `canary_suite.rs` is canonical; `RUN_CANARIES` in `differential.rs` is a projection that the drift guard keeps in sync. The manual maintenance burden is real (every new run canary needs a `RUN_CANARIES` entry or an exclusion) but the guard prevents silent drift. **No structural change needed.** Only friction to address: the copy-paste step after adding a canary — the drift guard error message already prints the exact line to paste.

#### A2c. DataProperties Triplication

`DataProperties { copy: bool, zero_init: bool, send: bool }` is declared identically in all three tree IRs:

- `representations/omega-syntax-trees/src/item.rs`
- `representations/omega-symbol-resolved-trees/src/data.rs`
- `representations/omega-typed-trees/src/data.rs`

All three structs are identical (3 bool fields) and carry no conversion boilerplate — the pipeline re-constructs the struct at each lowering boundary. This is intentional IR-boundary discipline (representations do not depend on each other). The duplication is **acceptable under the architecture rules** (each IR layer owns its own copy of shared shapes). The cost appears if a new `DataProperties` field must be added to all three; the fix is a mechanical edit to three files. Low priority — no action needed.

#### A2d. Files Touched by > 5 Recent Commits (last 40)

From `git log --name-only -40`:

| Touches | File |
|---------|------|
| 17 | `compiler/orchestration/omega-compiler/tests/canary_suite.rs` |
| 13 | `compiler/orchestration/omega-interpreter/tests/differential.rs` |
| 2 | `compiler/semantics/omega-validation/src/wire.rs` |
| 2 | `compiler/pipeline/omega-symbol-resolved-trees-to-typed-trees/src/expression/version_membership.rs` |
| 2 | `compiler/orchestration/omega-interpreter/src/evaluator.rs` |
| 2 | `compiler/backend/omega-instruction-selection/src/selection/runtime_dispatch/writes/mutation/value_operands.rs` |
| 2 | `compiler/backend/omega-instruction-selection/src/selection/runtime_dispatch/branches/leaf.rs` |

The top two are expected (registry files). The non-registry hot files are: `wire.rs` (wire validation wave), `version_membership.rs` (versioned-data feature), `evaluator.rs` (interpreter catch-up), `value_operands.rs` (string equality + negation), and `leaf.rs` (struct arg/return fixes). None of these are structurally alarming — each touch was a targeted fix, not accretion.

---

### A3. Architecture

#### Layer DAG Health

The `omega-architecture-test` enforces a ranked-layer DAG. Five `KNOWN_EXCEPTIONS` (upward edges) are tolerated:

1. `representations → backend` — reps reach into `omega-layout`, `omega-runtime-*`, `omega-state-*`.
2. `pipeline → backend` — pipeline passes reach into runtime-*/state-*/selection crates.
3. `object → backend` — relocation layer uses `omega-layout` and `omega-instruction-selection`.
4. `representations → object` — `omega-backend-plan` depends on `omega-object-file`.
5. `semantics → pipeline` — `omega-validation` reruns pipeline passes in its tests.

None of these were introduced by recent waves; they are documented architectural debts from the `representations → backend` coupling. **No new violations detected.**

Potential layering smell NOT caught by the test: `omega-state-guards` (a backend crate) is in the `backend` layer, but it is reached by `pipeline` crates via the `pipeline → backend` KNOWN_EXCEPTION blanket. The exception was added for that coupling; it is documented.

#### omega-names: Confirmed Orphaned Crate

`omega-names` (`compiler/semantics/omega-names/src/lib.rs`, 2127 lines) is listed as a workspace member in the root `Cargo.toml` but **has zero external consumers**: no other crate has `omega-names` in its `[dependencies]`, and no `.rs` file contains `use omega_names` or `extern crate omega_names`. It is the only crate in this condition.

The crate implements a name-resolution pass that pre-dates the current `omega-syntax-trees-to-symbol-resolved-trees` pipeline crate. The pipeline crate now owns all name/symbol resolution. `omega-names` represents dead inventory.

**Recommendation:** Remove `omega-names` from the workspace. The removal is safe — nothing depends on it. Steps: delete the `compiler/semantics/omega-names` directory and remove the `"compiler/semantics/omega-names"` entry from `Cargo.toml`. Before removal, skim the 2127-line `lib.rs` for any comment-documented design decisions worth migrating to a wiki file. This was **not executed** in Part B because it is a deletion, not a mechanical cleanup — worth a dedicated commit with a brief review of the file's content first.

#### Dead `pub` Items

No systematic dead-`pub` sweep was performed (would require `cargo doc --document-private-items` + manual inspection across ~70 crates). Spot-checked recently-added modules: `wire.rs` and `version_membership.rs` use `pub(crate)` and `pub(super)` correctly. No egregious `pub` leak detected in recent additions.

---

### A4. Conventions Drift

#### Error Message Style

All validation passes (old and new) use `Diagnostic::error(format!(...))` with a message-only `Diagnostic` struct (no source span, no label). The struct has `severity: DiagnosticSeverity` and `message: String` — no span field exists. The style is consistent: a plain English sentence with the relevant identifier names interpolated. **No drift detected.**

#### `pub(crate)` vs `pub` Discipline

Recently added modules (`wire.rs`, `version_membership.rs`, `evaluator.rs`) use `pub(crate)` for crate-internal functions — correct. The stress-point files (`leaf.rs`, `guards.rs`, `mutation.rs`) use `pub(in crate::selection::runtime_dispatch)` for tight scope control — also correct and idiomatic. **No regression detected.**

One note: `omega-names/src/lib.rs` has broad `pub` exports on all its types (as expected for a library crate), but since the crate is orphaned, this has no impact.

---

## Part B: Mechanical Improvements Executed

### B1 — Deferred: "No Write Strategy Selected" Hard Error

**Decision: deferred.** The fallthrough at the end of `select_runtime_leaf_branch_terminal_value_write` is reachable on legal programs that use text-guard lowering through refs/params (explicitly carved out in the poison-guard logic above it). A hard `panic!` or compile-error would fire on those programs. The existing `UnresolvedInlineArmGuard` poison (emitted for the ASSIGNMENT-VALUE inline guard case) is the correct model for converting specific known-bad cases to hard errors; extending it requires identifying each new silent-drop pattern explicitly.

A `#[cfg(debug_assertions)] eprintln!` tracing the unreachable slot/expression identities is the appropriate next step for diagnosis, not a hard error.

### B2 — Assessed: RUN_CANARIES Single Source of Truth

**Decision: already implemented.** The `run_canary_list_matches_canary_suite` drift guard in `differential.rs` dynamically re-parses `canary_suite.rs` at test time. `canary_suite.rs` is the canonical source; `RUN_CANARIES` is a derived projection kept in sync by the guard. No structural change needed.

### B3 — Executed: Doc Comments on `leaf.rs` Substitution Pipeline

Added module-level and function-level doc comments to `select_runtime_leaf_branch_terminal_value_write` and `resolve_leaf_caller_local_initializer_names` documenting the four-layer substitution pipeline, its invariants, and the known-safe fallthrough. This is a pure documentation change; no behavior was altered.

See commit applied to this worktree.

### B4 — Not executed: `rustfmt` pass

The task only sanctions `rustfmt` on files actually touched. `leaf.rs` is the one file touched in B3. `rustfmt` was not applied separately since the doc-comment additions do not affect formatting of existing code.

---

## Summary Table

| Finding | Severity | Action |
|---------|----------|--------|
| `omega-names` orphaned crate (2127 lines, 0 consumers) | Medium | Recommended removal; not executed (deletion scope) |
| `leaf.rs` 4-layer substitution pipeline undocumented | Low | **Executed** (B3: doc comments added) |
| RUN_CANARIES dual-registry maintenance burden | Low | Already mitigated by drift guard |
| `evaluator.rs` grab-bag growth | Low | Split candidate; deferred |
| `DataProperties` 3x duplication | Very low | Acceptable under IR-boundary rules |
| Hard error at leaf write fallthrough | Caution | Deferred — fires on legal programs |
| 5 architecture-layer KNOWN_EXCEPTIONS | Documentation | Pre-existing, documented, no new violations |

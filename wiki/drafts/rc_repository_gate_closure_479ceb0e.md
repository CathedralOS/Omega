# RC-REPOSITORY gate closure — linux_x86_64 re-measure at `479ceb0e680b`

Fresh re-measure of the `RC-REPOSITORY` gate row (acceptance per
[rust_compiler_completion](rust_compiler_completion.md): all five baseline
commands green on one commit). Host: linux x86-64, cargo runner (mbx
unavailable on this host). Prior readings: `a9fa1a4fe6` (all five red),
`5958706064` macOS (2/5), `19aff0a14ae3` and `94e764a6da6` linux
(2/5 → 3/5 on the four measured).

## Commands

| Gate command | Status | Delta vs `94e764a6da6` |
| --- | --- | --- |
| `cargo fmt --all -- --check` | **green** | unchanged — still clean, zero drifted files |
| `cargo check --workspace --all-targets` | **green** (45.9s) | unchanged — warnings only (`dead_code`, `unused_import` in canary_suite/package-manager suites) |
| `cargo clippy --workspace --all-targets --no-deps -- -D warnings` | **red** | regressed-in-name only: the two `validation` lints the prior reading fenced to RC-REPOSITORY-CLOSURE/clippy-lint-repair are **gone**; the single remaining error is new — `collapsible_if` at `isa-x86_64/src/semantic_unit_wrapper_encoding.rs:~555` (the nested `let Some(receiver) = … && …` chain wants collapsing) |
| `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | **red — 573/576** | narrowed: custody-matrix now fails on **one** inventory (image-emission `installation_function_nested_custody.rs` resolved since `94e764a6da6`); `glob_self_imports_never_grow_per_crate` now fails on **two** files — `psi/foundation/extents` 1 file + `psi/semantics/validation` 1 file (each ceiling 0); **new** third failure `every_u64_fingerprint_declaration_requires_explicit_classification` — unexpected `fingerprint` declaration at `program-entry-plan/src/optimized_semantic_wrapper/recipe.rs` |
| `cargo nextest run --workspace --lib --no-fail-fast` | not run | host-dependent prior reading stands (`5958706064` macOS: 13/16,169 red; `7b7d0fa8d5` aarch64: 12/16,290 red — disjoint in places) |

## Fence attribution — every remaining red is claim-owned

| Failure | Owning fence |
| --- | --- |
| clippy `collapsible_if` in `semantic_unit_wrapper_encoding.rs` | **UEFI-PHYSICAL-SEMANTIC-ENTRY** (Devin / z88, exp 08:44Z) — file is explicitly claimed |
| fingerprint declaration in `optimized_semantic_wrapper/recipe.rs` | **UEFI-PHYSICAL-SEMANTIC-ENTRY** — `program-entry-plan/src/optimized_semantic_wrapper` claimed |
| glob self-import, `validation` file | **MATCH-SELECTIVE-LOWERING** (Zergling-126, exp 07:39Z) — `value_custody/expression_types` claimed |
| glob self-import, `foundation/extents` file | **DEVICE-EXTENT-ACCESS** (exp 11:04Z) — `ordering_events/` claimed; `use super::*` at `ordering_events/tests.rs:1` introduced by that lane's own commit `d5b124cd4087` |
| custody-matrix inventory in `compilation-report` `custody_tests.rs` | **CUSTODY-MATRIX-HARNESS-MIGRATION** (Zergling-126, exp 09:19Z) — the harness-migration lane's own in-flight fallout |

## Verdict

Gate stays **open**: 2 of 4 measured commands green; clippy and
architecture-test carry four total failures, all on surfaces under live
sibling claims. No unfenced repair slice exists under this row — each red is
owned by a named lane. A full closure re-run (all five including
`--workspace --lib`) belongs to the host reading after the fenced repairs
land; the workspace libtests sweep was not re-run within this leg's bound.

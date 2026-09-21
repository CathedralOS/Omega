# REPOSITORY-BASELINE-GATE — gate measurement (2026-09-20, `9e3edc7be9`)

Bare mined stub (`TASKS.md:9127`) — re-mines the RC-REPOSITORY
five-command gate (`wiki/drafts/rust_compiler_completion.md`). Prior
measurements: `a9fa1a4fe6` (RC-REPOSITORY-CLOSURE row, all five red) and
`a1daf35f2e` (`wiki/drafts/rc_repository_gate_closure_z142.md`, 59 fmt
hunks / clippy 1 / arch 13 / check pipeline_ownership / libtests 88).

Fresh measurement on linux x86-64 (cargo; no mbx) at
`9e3edc7be9a3626d79b68a0b8a7d76f618a0031f` — **gate remains OPEN**:

| Command | Result | Detail |
| --- | --- | --- |
| `cargo fmt --all -- --check` | **GREEN** | 0 diff hunks — flipped from red; the z173 fmt-drift leg and prior repairs landed it. |
| `cargo clippy --workspace --all-targets -- -D warnings` | RED (2) | `permissions_set_readonly_false` at `sources/acquisition/.../traversal.rs:513` (BUILD-PACKAGES-GATE, live 21:49Z); `clone_on_copy` at `syntax-trees-to-symbol-resolved-trees/.../generic_data/substitution.rs:274` (inside CASE-CONSTRAINTS's claimed `preparation/generic_data` dir, 01:13Z). Both fenced. |
| `cargo nextest run -p omega-architecture-test --all-targets --no-fail-fast` | RED 14/575 | glob_self_imports residual 5 files — all fenced (acquisition → BUILD-PACKAGES-GATE; sis2sis ×2; extents; proof-admission → PROOF-RULE-CLASSICALITY-AUDIT; validation → MATCH-SELECTIVE-LOWERING); validation_integration ×11 recast/boundary-ensures lane drift; custody_mutation_matrix (image-emission `installation_function_nested_custody` + compilation-report `pcc/native_evidence/custody_tests` inventory rows lack `*_for_test` matrix drivers); layering `abstract_to_target_translation_validation_cannot_reenter_its_producer` (a2t2 replay node count). |
| `cargo check --workspace --all-targets` | RED | `omega-native-differential-test` `pipeline_ownership` fails compile: 4× E0308 (`optimized_target()` → expected `&Arc<ValidatedOptimizedTargetOperations>`, i.e. `optimized_target_owner()`) + 1× E0004 (`LegalizedScalarTerminator::Crash` arm missing in `ordinary_graph_controls.rs`) — same harness drift recorded at `a1daf35f2e`, owned by the STRUCTURAL-UNIT-CALL-GRAPH-JOINS lane. |
| `cargo nextest run --workspace --lib --no-fail-fast` | RED 75/16,118 | 16,043 pass / 2 skip in 1209.9s (was 88 fail at `a1daf35f2e` — trending down). Clusters: selected-dispatch `boundary_dispatch` ~40 finite_family/generic_requirements + empty_settlement/source_retention (Service<R>-spelling fixture drift — SELECTED-DISPATCH-SERVICE-CARRIER-FIXTURES live 05:06Z); checked-trees-to-lowered-psi ~20 dynamic_composed_unit/structural_control (BASELINE-SERVICE-CARRIER-FAILURES 04:41Z holds `c2l/src/tests`); package-manager 6 review/discovery fixtures (`in Bound` + bare boundary trait rejections — same carrier lane); singles: external-roots safe-point catalog, native-realization exclusion adjudication fixture, package-evidence quotient row, register-environment abi_call_clobbers branch contract, t2c2 authored_selections. |

## Attribution

Every red surface above sits inside a live sibling claim or an explicitly
named owner lane (BUILD-PACKAGES-GATE, CASE-CONSTRAINTS, glob-residual
fences, recast lane, custody-matrix owners, STRUCTURAL-UNIT-CALL-GRAPH-
JOINS for pipeline_ownership). No unattributed mechanical drift found on
this leg — nothing to repair under this name.

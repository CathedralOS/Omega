# RC-NATIVE-MATRIX-HOST-EXECUTION — host records

One section per host that has executed the gate directly. Each section
records a *matching-host* run: the host's own product identity, executing
emitted native programs rather than replaying them by cross-emit.

# linux_x86_64 — matching-host execution

Host-execution record for the RC-NATIVE-MATRIX gate
(`mbx nextest run -p omega-native-differential-test --all-targets
--no-fail-fast`; executed as `cargo nextest run` — `mbx` is not installed on
this worker) at revision `75650d2e94`, host `x86_64-unknown-linux-gnu`,
cargo-nextest 0.9.145, toolchain `nightly-2026-09-04`.

## Result

**1103 pass / 29 fail / 1 skip across 1132 executed legs in 3524 s**, 33
test binaries. The skip is expected: `source_custody_artifact`'s fixture is
owned by its own gate (`#[ignore]`). The long legs are real native
executions — `optimizer_corpus` deterministic legs ran 300–420 s each and
`ieee_comparisons` cross-replayed binary64 against host IEEE for ~240 s.

Every binary compiles and runs: the `pipeline_ownership` custody-handle
drift and `abstract_publication` catalog-count drift that partitioned the
`4dbdaa9bc3` sweep (670 legs over 27 binaries) are repaired upstream
(confirmed clean under `cargo check -p omega-native-differential-test
--all-targets` at this revision), so this row records the unpartitioned
invocation the `e5bbe53956f` sweep in
[rc_native_matrix_hosts.md](rc_native_matrix_hosts.md) first ran.

## Per-target results

| test target | result |
|-------------|--------|
| `abstract_publication` | 57/57 |
| `asm_memory_transfer` | 1/1 |
| `build_time` | 4/4 |
| `coverage` | 64/77 |
| `frontend_drop_expectations` | 26/26 |
| `gui_headless` | 0/1 |
| `hosted_receiver` | 9/9 |
| `ieee_comparisons` | 3/3 |
| `local_record_receivers` | 3/3 |
| `optimizer_corpus` | 7/7 |
| `owned_control_cycles` | 5/5 |
| `physical_child_replay` | 12/12 |
| `pipeline_ownership` | 392/392 |
| `primitive_locals` | 17/17 |
| `primitive_store_return` | 7/7 |
| `real_fs` | 8/10 |
| `recast_views` | 10/10 |
| `scalar_array_results` | 35/35 |
| `scalar_case_results` | 123/125 |
| `scalar_control_cycles` | 24/24 |
| `source_custody_artifact` | 0 run / 1 skipped (expected, host-gated) |
| `terminal_byte_views` | 113/113 |
| `terminal_psi` | 3/3 |
| `terminal_psi_calls` | 3/4 |
| `terminal_psi_conditional` | 2/2 |
| `terminal_psi_debug_spans` | 1/1 |
| `terminal_psi_indexed_receivers` | 79/79 |
| `terminal_psi_record_returns` | 0/6 |
| `terminal_psi_runnable` | 1/5 |
| `terminal_psi_source` | 91/91 |
| `terminal_psi_source_payloadless_optimizer` | 3/3 |

## Failure families (29 legs)

| legs | first refusal | family |
|------|---------------|--------|
| 13 (`coverage` `filesystem_*`) | interpreted execution | Real-FilesystemHost legs reach checked trees and run; the program exits a `fail()` state — observed exit 71 vs pinned 70. The failing transition varies per leg (`open(missing)`/kind mapping, `create_dir(existing)`, `try_exists` on a chmod-0 file, `read_dir_nth` enumeration, `canonicalize`, `hard_link`, `locking`, `ownership`, `remove_dir_all`, `read_dir_stats`, `value_returning_append`, `path_subslice_domain`). Deterministic — `filesystem_std_module_error_kind` fails identically in isolation. These legs were previously refused at checking by the `Service<R>` spelling fence; the migration exposed the execution-stage residual underneath. |
| 6 (`terminal_psi_record_returns`) | proof decode | `ArtifactLowering(ProofDecode(ProofSubjectMismatch))`: claimed `TerminalPsiIdentity.program_fingerprint` ≠ reconstructed fingerprint on record-return shapes. Same legs as prior records. |
| 4 (`terminal_psi_runnable`) | lowering | `InvalidUnitMachinePlan`: `attached Unit closure is missing a checked transitive machine plan` — `Main::main` (×3, local construction stopped at signature) and `Root::enter` (×1, stopped at statement-0 call). Same family as the prior record's single `Root::enter` leg. |
| 2 (`real_fs`) | interpreted execution | Real-fs enumeration residuals, distinct from the retired service-reach family: `real_provider_serves_the_full_virtual_op_set` diverges at parity step 13 of 14; `scoped_wrapper_read_dir_count_enumerates_a_real_directory` exits a mid-drain `bad(74)` state instead of draining 48 seeded entries. |
| 2 (`scalar_case_results`) | proof decode / lowering | `joined_record_cannot_move_and_lend_its_child_to_the_same_call` — `ProofSubjectMismatch` (same leg as before); `owned_record_parameter_return_survives_an_observable_call` — `Lowering(Unsupported("composed Unit scalar call requires structural call custody"))`. Same legs as prior records. |
| 1 (`terminal_psi_calls`) | pinned bytes | `scalar_i32_call_has_exact_exportable_terminal_bytes` — pinned terminal bytes still drift; the reviewed replacement is printed in the failure output. Same leg as prior records. |
| 1 (`gui_headless`) | compile | `window_demo` still references the missing vendored `omega_language_std/console.omg` fixture. The `recast_views` siblings of this family (10 legs at `4dbdaa9bc3`) are repaired — only this leg remains. |

## Families retired since `4dbdaa9bc3`

The 112-failure record reduced to 29:

- **`Service<R>` carrier spelling (~69 legs)** — closed by the carrier
  migration; `coverage` non-fs legs and the `terminal_psi_runnable`
  spellings now pass checking.
- **Vendored `omega_language_std` fixtures (10 of 11 legs)** — the
  `recast_views` fixtures now resolve; only `gui_headless` remains.
- **Checked-body exact-arithmetic / service-reach obligations (8 of 10
  `real_fs` legs)** — the remaining two are enumeration residuals above.
- **Fixed-fuel `natural_writer` drift (8 legs)** — `terminal_byte_views`
  is 113/113.
- **`terminal_psi_source` golden bytes** — 91/91.
- **`pipeline_ownership` in-flight residuals** — the target compiles and
  runs 392/392.

The dominant residual is no longer checking-fence fixture drift; it is the
real-fs execution family (15 legs across `coverage`/`real_fs`) plus the
standing proof-decode/unit-plan set.

## Scope notes

- Every macos_arm64 residual (15 legs at `163618c89557`) is a named member
  of the linux families above — the six `terminal_psi_record_returns`
  proof-decode legs, the two `scalar_case_results` legs, the pinned
  `terminal_psi_calls` bytes leg, four `terminal_psi_runnable`
  `InvalidUnitMachinePlan` legs, `coverage`'s
  `filesystem_path_subslice_domain`, and `gui_headless`. No recorded
  failure is a matching-host execution differential; the redness is
  shared-pipeline, host-independent.
- Cross-emit legs still run: `publish on four targets` families re-encode
  and replay every hosted target from this host. Only direct
  macos_arm64/windows_x86_64 execution rows stay host-gated.
- The canonical release record
  (`tools/release/release_record.py run --target linux_x86_64
  --gate RC-NATIVE-MATRIX`) writes to `tools/release/records/`, currently
  fenced by RC-RELEASE-RECORD-SUBSTRATE; this file is the row's record
  until the record dir opens.
- The neighboring row doc `rc_native_matrix_linux_x86_64.md` is claimed by
  RC-NATIVE-MATRIX-LINUX-X86-64 and keeps its own recording; this row
  records the gate-level host execution only.

# macos_arm64 — matching-host execution

Second host-execution record under the same gate, run at revision
`163618c89557` on a **matching host**: `Darwin` / `arm64` (Apple M4, 16 GB),
so emitted Mach-O AArch64 programs execute directly rather than by
cross-emit replay. This is the first execution of any kind recorded for the
`macos_arm64` runner row.

**Host provenance is asserted, not inferred.** `command_run` does not
validate that the executing host can satisfy the runner it is asked for, so
a passing leg is not by itself evidence the program ran natively. The
`host.os = Darwin` / `host.machine = arm64` fields above are the only proof
this row is direct execution; they were read from the invoking shell
(`uname -s` / `uname -m`), not from the harness.

Invocation `cargo nextest run -p omega-native-differential-test
--all-targets --no-fail-fast` (`mbx` is not installed on this worker),
toolchain `rustc 1.100.0-nightly (a69a63265 2026-09-03)`,
`cargo-nextest 0.9.143`.

## Result

**1113 pass / 15 fail / 1 skip across 1128 legs in 2651 s**, 31 test
binaries. The skip is the same expected `source_custody_artifact` fixture
the linux row records. All 31 binaries compile at this revision — the two
harness legs the linux row could not build (`pipeline_ownership`,
`abstract_publication`) have since been repaired, so this sweep is the
unpartitioned invocation.

Leg count differs from the linux row (1128 vs 670) because that row was
recorded at `4dbdaa9bc3`, before the corpus grew; the two totals are not
directly comparable.

## No failure is a host-execution differential

Every one of the 15 refusals occurs **before native emission** — at
checking, at lowering, or at proof decode. None is a mismatch between an
executed program's observed behaviour and its predicted behaviour. Stated
plainly: on this host, every leg that reached Mach-O emission and ran
executed correctly, and the row's redness is inherited shared-pipeline
redness rather than anything specific to macOS or AArch64.

Nine of the fifteen are the **same named legs** the linux_x86_64 row lists,
which is independent evidence those families are host-independent:
the six `terminal_psi_record_returns` cases and two `scalar_case_results`
cases under `ProofDecode(ProofSubjectMismatch)`, plus
`terminal_psi_calls scalar_i32_call_has_exact_exportable_terminal_bytes`.

| legs | first refusal | family |
|------|---------------|--------|
| 6 (`terminal_psi_record_returns`) | proof decode | `ArtifactLowering(ProofDecode(ProofSubjectMismatch))` on `record return with ordered writes must reach ordinary abstract operations`. Same legs as the linux row. |
| 4 (`terminal_psi_runnable`) | lowering | `InvalidUnitMachinePlan { reason: "attached Unit closure ..." }` lowering O1 source to terminal Psi, for `Main::main` and `Root::enter`. |
| 2 (`scalar_case_results`) | proof decode / lowering | `owned_record_parameter_return_survives_an_observable_call` and `joined_record_cannot_move_and_lend_its_child_to_the_same_call`. Same legs as the linux row. |
| 1 (`terminal_psi_calls`) | pinned bytes | `scalar_i32_call_has_exact_exportable_terminal_bytes` — `ProofSubjectMismatch` on the claimed `TerminalPsiIdentity` fingerprint. Same leg as the linux row. |
| 1 (`coverage`) | checking | `filesystem_path_subslice_domain` — the fs program does not reach checked trees (`cannot prove ...`). |
| 1 (`gui_headless`) | compile | `window_demo_runs_headless_to_native_exit` — `window_demo` should compile for the interpreter. |

## The `Service<R>` family is gone

An earlier reading of this host reported 108 failures. That figure is
**stale and should not be cited**: it predates the `Service<R>` carrier
migration (`e4f4b4bd72e9`, `190df8412de5`), which retired the 69-leg
`coverage`/`terminal_psi_runnable` family the linux row heads its table
with. The drop from 108 to 15 is that migration landing, not a change of
host or method.

## The umbrella row is not flipped here

`rc_native_matrix_hosts.md` still prints `— none —` for the macOS AArch64
runner. That file is owned by RC-NATIVE-MATRIX-HOSTS and is not touched by
this record; its row should now read `red` rather than
`open, unrecorded`, pointing at this section. Handing that edit to its
owner rather than reaching across the fence.

## Scope notes

- This host qualifies for `require_seed_execution_host`
  (`ALPHA_SEED_EXECUTABLE=1` covers `Darwin-arm64`) but cannot run the seed
  gates: every seed program is SIGKILLed (`-9`) under the 16 GB ceiling.
  That limit does not affect this row — the differential legs are not seed
  programs.
- The `windows_x86_64` row remains the only runner with no execution of any
  kind.


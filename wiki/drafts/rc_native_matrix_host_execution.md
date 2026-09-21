# RC-NATIVE-MATRIX-HOST-EXECUTION — host records

One section per host that has executed the gate directly. Each section
records a *matching-host* run: the host's own product identity, executing
emitted native programs rather than replaying them by cross-emit.

# linux_x86_64 — matching-host execution

Host-execution record for the RC-NATIVE-MATRIX gate
(`mbx nextest run -p omega-native-differential-test --all-targets
--no-fail-fast`; executed as `cargo nextest run` — `mbx` is not installed on
this worker) at revision `4dbdaa9bc3`, host `x86_64-unknown-linux-gnu`.

## Result

558 pass / 112 fail / 1 skip across 670 legs in ~1695 s (28 test binaries +
lib). The skip is expected: `source_custody_artifact`'s fixture is owned by
its own gate (`#[ignore]`). The long legs are real native executions —
`optimizer_corpus` deterministic legs ran 300–680 s each and
`ieee_comparisons` cross-replayed binary64 against host IEEE for 361 s.

## Harness legs that cannot compile at this revision

Two of the 29 test binaries do not build; the run above covers the other 27:

- `tests/native-differential/tests/pipeline_ownership.rs` drifted behind
  `83766d57bf`: its call sites pass `&ValidatedOptimizedTargetOperations`
  where `validate_optimized_selection_custody` now takes
  `&Arc<ValidatedOptimizedTargetOperations>`, and
  `scalar_case_results/ordinary_graph_controls.rs` does not cover the
  `LegalizedScalarTerminator::Crash` variant.
- `tests/native-differential/tests/abstract_publication.rs` pins a
  six-member optimization catalog that has grown to seven
  (`[Optimization; 6] == [Optimization; 7]`).

Both surfaces are under live claims (pipeline_ownership →
STRUCTURAL-UNIT-CALL-GRAPH-JOINS; abstract_publication →
NATIVE-DIFFERENTIAL-MATRIX); their exclusion here is a fence, not a verdict.

## Failure families (112 legs)

| legs | first refusal | family |
|------|---------------|--------|
| 69 (coverage ×65, terminal_psi_runnable ×4) | checking | `Service<R>` carrier spelling: `field console on data Main names bare boundary trait Console in value position; the intrinsic Service<R> carrier is the only service value spelling`. Owned by ENTRY-CONTENT-ROOTS / fixture migration. |
| 10 (real_fs) | checking | Checked-body obligations: exact-arithmetic (`cast from i64 to i32 is not provably representable`) and service-reach publication (`publishes service reach <none> but its checked body reaches undeclared services Console + FilesystemHost`). |
| 10 (recast_views ×9, gui_headless ×1) | fixture resolution | Bundled-std shim rename: sources reference `omega_language_std/console.omg`; the corpus now ships `platform/console.omg`. Harness fixture paths need migration. |
| 8 (terminal_psi_record_returns ×6, scalar_case_results ×2) | proof decode | `ProofSubjectMismatch`: claimed TerminalPsiIdentity fingerprint ≠ reconstructed fingerprint — the replayed identity drifts under record-return / joined-record shapes. |
| 8 (terminal_byte_views, natural_writer) | fuel derivation | `derive_fixed_entry_fuel` no longer returns `ControlCycle` for the natural-writer replay body; the pinned expectation is stale. |
| 6 (terminal_psi_source) | artifact admission | `hosted receiver requires exact checked initialization and cleanup custody` — ProgramEntry provisioning residual; also one `attached Unit closure is missing a checked transitive machine plan` shape. |
| 1 (terminal_psi_calls) | pinned bytes | `scalar_i32_call_has_exact_exportable_terminal_bytes` byte drift — reviewed replacement recorded in the failure output. |

## Scope notes

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

## Re-verification — `5bb9a74842` (linux x86-64, Zergling-126, claim `5a2e0ee2`)

Harness compile state improved: `cargo check -p omega-native-differential-test
--all-targets` now finishes clean — the `pipeline_ownership` custody-handle
drift (`optimized_target()` → `optimized_target_owner()`), the uncovered
`LegalizedScalarTerminator::Crash` arm, and the dropped
`produce_checked_canonical_integer_proof` symbol recorded at `f3d0d1748e`
have all been repaired upstream. The gate can now report red/green again
rather than failing to build; the per-host re-run belongs to the canonical
RC-NATIVE-MATRIX-LINUX-X86-64 owner, so no leg sweep was executed under this
claim.

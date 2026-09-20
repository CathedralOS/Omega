# RC-NATIVE-MATRIX-HOST-EXECUTION — linux-x86_64 record

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

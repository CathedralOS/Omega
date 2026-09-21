# RC native matrix — host legs, linux_x86_64

Witnessed measurement of the `RC-NATIVE-MATRIX` row host leg on the
Linux x86-64 host. Recorded at revision `e76d715c8e` (2026-09-20), host
`x86_64-unknown-linux-gnu`, pinned toolchain `nightly-2026-09-04`
(rustc 1.100.0-nightly a69a63265), cargo-nextest 0.9.144 (mbx
unavailable on this host; Cargo used per AGENTS.md).

Row command: `mbx nextest run -p omega-native-differential-test
--all-targets --no-fail-fast`, plus `RC-SOURCE-SEMANTICS`
(`mbx nextest run -p compiler --all-targets --no-fail-fast`), on every
required host.

Verdict: **red** — the native-differential suite cannot even build on
this revision (two test targets fail compilation), the 27 targets that
do build record 112 failures in 662 tests, and the RC-SOURCE-SEMANTICS
companion records 1183 failures in 3070 tests. The row stays open.
This supersedes the curated 8ae40607a3 record
(`rc_native_matrix_linux_x86_64.md`) as the latest linux_x86_64
measurement; that record remains the canonical per-leg table for the
canary-suite native legs.

## Exact skip list

- `pipeline_ownership` test binary — DOES NOT COMPILE at e76d715c8e
  (5 errors): 4x E0308 in
  `stages/realization/structural_units/structural_return.rs:33`,
  `stages/selection/custody.rs:62`, `validation.rs:400`,
  `validation.rs:409` — `validate_optimized_selection_custody` now takes
  `&Arc<ValidatedOptimizedTargetOperations>` but callers pass
  `&ValidatedOptimizedTargetOperations` from `.optimized_target()`;
  1x E0004 in `fixtures/ordinary_graph_controls.rs:32` — match misses
  `LegalizedScalarTerminator::Crash` (variant added since the fixture
  was written). Owner surface: STRUCTURAL-UNIT-CALL-GRAPH-JOINS claim.
- `abstract_publication` test binary — DOES NOT COMPILE at e76d715c8e
  (1 error): E0277 `decision_custody.rs:58` — compares
  `[Optimization; 6]` against `PSI_PASS_CATALOG`-derived
  `[Optimization; 7]`; the catalog gained a 7th pass. Owner surface:
  NATIVE-DIFFERENTIAL-MATRIX claim.
- Host legs linux_arm64, macos_arm64, windows_x86_64 — SKIPPED:
  unavailable on this host (no cross-emitted execution, no QEMU).
- Doc tests — not run by nextest; deferred (nextest does not run
  doctests per AGENTS.md).

## Runs

| leg | command | result | elapsed |
|-----|---------|--------|---------|
| Native-differential suite (partial: 27/29 test targets; the 2 listed above fail to build) | `cargo nextest run -p omega-native-differential-test --test <each target minus pipeline_ownership,abstract_publication> --no-fail-fast` | 662 run: 550 pass (39 slow), **112 fail**, 1 skip | 1648s |
| RC-SOURCE-SEMANTICS companion | `cargo nextest run -p compiler --all-targets --no-fail-fast` | 3070 run: 1887 pass (234 slow), **1183 fail**, 0 skip | 24344s |

## Failure census (112, by test binary)

- coverage: 65
- real_fs: 10
- recast_views: 9
- terminal_byte_views: 8 (all `natural_writer::*`)
- terminal_psi_source: 6
- terminal_psi_record_returns: 6
- terminal_psi_runnable: 4
- scalar_case_results: 2
- gui_headless: 1
- terminal_psi_calls: 1

## Failure families

1. **Service<R> carrier spelling rejection** (~78 legs: 65 coverage +
   6 record_returns + 4 runnable + real_fs Build fixtures + wire_*).
   Fixture programs spelling `field console on data Main names bare
   boundary trait Console in value position; the intrinsic Service<R>
   carrier is the only service value spelling` (and the
   `Filesystem`/`FilesystemHost` analog). Same family as the
   8ae40607a3 record family 1; the closed-`Service<R>` check landed in
   f705cbdb51 keeps rejecting unmigrated fixtures. Owner family:
   ENTRY-CONTENT-ROOTS receiver-carrier migration; the coverage target's
   fixture files are also claimed by
   RC-NATIVE-MATRIX-MACOS-ARM64/coverage-service-fixtures.

2. **Missing `omega_language_std` fixture module** (11 legs: all 9
   recast_views + gui_headless + 1 real_fs recast leg). Fixtures under
   `tests/omega/pass/recast/*` and `samples/gui/window_demo` contain
   only `build.omg`/`main.omg` with `use omega_language_std::console`
   and no `omega_language_std/` directory anywhere under `tests/omega`;
   every lookup fails `failed to resolve .../omega_language_std/
   console.omg: No such file or directory`. The fixtures were authored
   expecting a bundled-std resolution route that is absent on this
   revision.

3. **Exact-arithmetic/division checker rejections** (3 legs in
   real_fs): `i64 -> i32` cast "not provably representable" in
   `Main::main` states `main`/`done`, and "division by zero ... the
   divisor is provably zero" in `IgnoredOperandProbe::run`. Fixture
   programs unkept against the tightened exact-arithmetic admissions.

4. **natural_writer fixed-fuel contract drift** (8 legs,
   terminal_byte_views): `derive_fixed_entry_fuel` no longer returns
   `Err(FixedFuelError::ControlCycle)` where the tests assert it —
   natural-slice-writer control now reaches further than the recorded
   expectation.

5. **terminal_psi_source publication/entry cluster** (6 legs): hosted
   receiver provisioning ("native artifact ProgramEntry receiver
   provisioning failed: hosted receiver requires exact checked
   initialization..."), `progress premise for Scheduler::WeakFair must
   name one identity-preserving parameter or field path`, checked-crash
   native-graph support, control-flow cleanup publication, retired
   lowering rejection, optimizer rejoin, build-bound progress
   retention. Consistent with the hosted-receiver custody cluster in
   `known_baseline_failures.md`.

6. **scalar_case_results** (2 legs): `joined_record_cannot_move_and_
   lend_its_child_to_the_same_call` fails Terminal admission with
   `ProofSubjectMismatch` (claimed fingerprint ae2b018b... vs
   reconstructed 3f53681a...); `owned_record_parameter_return_survives_
   an_observable_call` fails lowering with `Unsupported("composed Unit
   scalar call requires structural call custody")`.

7. **terminal_psi_calls** (1 leg): `scalar_i32_call_has_exact_
   exportable_terminal_bytes` — recorded terminal bytes drifted from
   the reviewed replacement bytes (byte-for-byte expectation mismatch).

8. **Build-shaped failures in real_fs** (remainder of the 10): granted
   build machine asset staging, console boundary serving, duplicate
   descriptor cursor sharing, scoped grant enforcement, staging-sponsor
   errno/ceiling — mixture of Service<R> (family 1) and family 3
   rejections; see full run log for per-test mapping.

Per-test output retained in the session worklog
(shell `natdiff-run2`, run ID 61cd0dca-c296-4c10-82a4-a63b2479b7f9).

## RC-SOURCE-SEMANTICS failure census (1183, by test binary)

- canary_suite: 1056
- samples_compile: 24
- calling_policy_plans: 18
- recast_views: 16
- package_compilation_inputs: 13
- plan_laid_repeated_runtime: 9
- build_target_activation: 6
- source_evaluated_native_realization: 5
- build_config_granted: 3
- composed_internal_unit_arguments: 3
- layout_plans: 3
- service_operational_contracts: 3
- subslice_runtime_end_bounds: 3
- architecture_boundaries: 2
- cyclic_receiver_execution: 2
- hosted_console_input_envelope: 2
- no_selection_golden: 2
- optimizer_opt_in: 2
- private_joint_progress: 2
- 1 each: arithmetic_policy_array_storage, build_behavior_exclusions,
  build_time_admission, call_acknowledgements,
  callback_terminal_custody, joint_call_rankings,
  owned_case_state_transport, provider_plan_facts, terminal_authority

## RC-SOURCE-SEMANTICS failure families

1. **ProgramEntry↔Terminal attachment rejoin** (~1035 diagnostic
   occurrences, dominant in canary_suite and every samples authored-
   entry leg on linux_x86_64/linux_arm64/macos_arm64): `selected
   ProgramEntry establishment rejoins 0 Terminal attachment
   identities; expected one`. Same family as the hosted-receiver
   cluster in `known_baseline_failures.md`; entry/provider surfaces
   are fenced by ENTRY-CONTENT-ROOTS.
2. **Windows authored-entry contract rejection** (~124 legs — every
   windows_x86_64 leg of samples_compile umbrellas): `require either
   the exact bundled Windows x86-64 contract or one accepted
   package-owned Windows x86-64 binding, not .../std/targets/
   windows_x86_64/entry.omg`. The bundled std entry no longer
   satisfies the authored-entry admission rule.
3. **Selected Fused provider requirement** (~90): `Service field
   Main::<field> requires a selected Fused provider for boundary
   <Console|Gui|Clock|Clock2|Input|FilesystemHost>` — fixture machines
   keep unbound service fields under the fused-provider selection
   rule.
4. **Call acknowledgement envelope `block` mismatch** (66):
   `call to `apply` has operational envelope `block` but acknowledges
   neither suspension nor blocking; call acknowledgements must match
   exactly` — e.g. systems/account_ledger.
5. **Borrowed-storage move/restore rejections** (75): `cannot transfer
   a non-copy value out of borrowed storage without replacing its
   owner` — e.g. systems/wire_protocol `self.plans` across `done`
   state exits.
6. **Exact-arithmetic overflow + Wrapping/Exact mixing** (58 `exact
   arithmetic`, 78 `Wrapping`): decision-17 exact-arithmetic overflow
   obligations and domain-mixing rejections on unkept fixtures.
7. **`[u8; N]::Utf8` default-domain field requirement** (29): `cannot
   prove default-domain field requirement ... requires [u8; N]::Utf8`
   on text-carrier fixtures (maze_flood, print_number, dungeon_render).
8. **Roster inventory drift** (24 `roster`): unregistered fail
   fixtures (generics/authored_const_call_operator_unselected_
   provider, operators/mismatched_operand_tuple) and unregistered pass
   fixtures (core/wait_wake_boundary_surface, generics/authored_const_
   application_local_destination).
9. **Unresolved authored selection occurrences** (14): `authored Call/
   Operator declaration selection occurrence N remained unresolved
   after successful checking (CheckedCall/CheckedOperator)` — e.g.
   proofs/math_proofs occurrence 42.
10. **Public-interface privacy** (28): private data/trait selected by
    public interface on unkept fixtures.
11. **Lowering(Unsupported) cluster** — indexed reads, checked
    trapping conversion, composed Unit scalar call, unreachable Unit
    states, callee without checked body.
12. **Misc small clusters**: `no closed native catalog identity` (5),
    `invokes` refinement omissions (8), hosted-receiver closed-`Service`
    carrier qualification rejections (4) + missing `Bound` domain on
    `Service<Console>`, interpreter oracle drift (4), Terminal
    nearest-FMA admitted-plan rejoin (1).

Per-test output retained in the session worklog
(shell `srcsem-run`, run ID 294d1669-6fa3-48e6-ae23-12051a88ac85).
No SIGSEGVs observed in this leg.

## Cross-references

- Prior curated record: `wiki/drafts/rc_native_matrix_linux_x86_64.md`
  (8ae40607a3, red 15/38 on the canary-suite native legs).
- Known-baseline catalog: `wiki/drafts/known_baseline_failures.md`.
- Release matrix row contract: `wiki/drafts/rust_compiler_completion.md`.

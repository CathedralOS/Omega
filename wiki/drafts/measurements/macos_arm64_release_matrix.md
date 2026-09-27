# macOS arm64 release-matrix run at 0293d4cad9

Host-run evidence for `RC-NATIVE-MATRIX-MACOS-ARM64` (TASKS.md): the first
full `tools/release/release_record.py run --target macos_arm64` attempt on a
matching host, plus the host repair needed to get gate commands past the
link step at all. Complements `macos_arm64_rc_gates.md`, which holds the
`mbx nextest` corpus readings at `04d2099ae110`; this file holds the
recorder-procedure readings at `0293d4cad9a39332bd2586a70d917f480b86abf3`
(origin/main at 2026-09-27T03:27Z).

- Host: macOS AArch64 (`darwin`/`arm64`, Darwin 27.0.0, T6020).
- Toolchain: pinned `nightly-2026-09-04` (`rustc 1.100.0-nightly a69a63265`),
  cargo-nextest 0.9.143, `mbx` 1.11.0 present and used as the runner.
- Records written (worktree `repair-local-r1-rc-macos-matrix`,
  `tools/release/records/`):
  - `macos_arm64__0293d4cad9__20260927T033858Z.json` — run 1, default
    environment: every build-bearing command red inside seconds on the
    host's broken default SDK (below). `check`: valid, closure open.
  - `macos_arm64__0293d4cad9__20260927T045612Z.json` — run 2, env-fixed:
    the real per-gate readings tabled below. `check`: valid, closure open.

## First divergence: environment, then checked-in fmt drift

Run 1 found the matrix's first divergence is not a test verdict at all:

- **Host link breakage.** The CommandLineTools `MacOSX.sdk` symlink was
  repointed to `MacOSX27.0.sdk` on 2026-09-26 19:14 local — after the
  09-23 readings on this host. Its `tbd-version: 4` stubs list
  `arm64e.x1-macos` / `arm64e.x1-maccatalyst`, which the pinned toolchain's
  Apple clang 17 (`clang-1700.6.4.2`) tapi parser rejects: every
  build-script and binary link dies with
  `ld: tapi error: malformed file ... unknown architecture`. All 13
  commands exited 101/1 within seconds; nothing compiled.
- **Fix (no sudo, no command-string change):**
  `SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk`
  (the Xcode 26.2 SDK, whose stubs use the classic `arm64-macos`/
  `arm64e-macos` spellings clang 17 parses), plus the established
  `RUST_MIN_STACK=67108864`. Now documented in `tools/release/README.md`.
- **fmt drift on main.** `python tools/fmt.py --check` exits 1: 48 rustfmt
  diff sites across 31 files — import ordering and chain-wrap moves
  introduced by the 09-26 landings `d1286f778a`, `491bd4498c`,
  `40c3cd3715`, `7f5612d541`, `0def9c26d8`. Host-independent under the
  pinned toolchain: RC-REPOSITORY is fmt-red on every lane at this commit.
  Several of the files sit inside other sessions' live claims, so the
  repair is not this lane's to land.

## Gate readings at 0293d4cad9 (run 2, env-fixed)

| Gate | Command | Exit | Wall | Skips |
|---|---|---|---|---|
| RC-REPOSITORY | `python tools/fmt.py --check` | 1 | 72 s | — |
| | `mbx clippy --workspace --all-targets -- -D warnings` | 101 | 67 s | — |
| | `mbx check --workspace --all-targets` | 0 | 86 s | — |
| | `mbx nextest run --workspace --lib --no-fail-fast` | 0 | 655 s | 3 |
| RC-SOURCE-SEMANTICS | `python tools/corpus_gate.py` | 1 | 335 s | — |
| RC-PCC-REPLAY | `mbx nextest run` (5 crates) | 100 | 216 s | 0 |
| | `mbx test --doc` (same 5) | 0 | 1.5 s | — |
| RC-PORTABLE-PSI | `mbx nextest run` (3 crates) | 100 | 39 s | 0 |
| RC-BUILD-AND-PACKAGES | `mbx nextest run` (7 crates) | 100 | 310 s | 3 |
| | `mbx test --doc` (same 7) | 0 | 1.5 s | — |
| RC-NATIVE-MATRIX | `python tools/corpus_gate.py --native` | 2 | 75 s | — |
| RC-DIAGNOSTICS | `python tools/corpus_gate.py --filter fail/` | 0 | 7 s | — |
| RC-REPRESENTATIVE-PROGRAMS | `mbx run -p omega -- refresh-samples` | 1 | 2358 s | — |
| observation | `mbx nextest run -p compiler --test canary_suite -E 'test(/_runs$/)'` | 101 | 0.3 s | — |

`mbx check` and the whole-workspace `--lib` suite (655 s) are green; the
fmt and clippy legs carry RC-REPOSITORY's red. Clippy's two `-D warnings`
escalations are both in `terminal-verifier`
(`collapsible_if` and `then(|| ...)` → `then_some` in
`src/validation/structural/operations/`).

## Per-gate failure detail

- **RC-SOURCE-SEMANTICS** — 17 structural diffs vs
  `tests/omega/corpus_outcomes.txt` (0 new / 0 gone), plus 17 ≥2×-slowdown
  perf moves attributable to the concurrent coordinator build sharing the
  host. Sixteen are `checked -> rejected` on the Saturating-domain
  implicit-weakening family (`implicitly converts 'f32 in Saturating' to
  'f32'`, `implicit domain weakening ... drops Saturating`), dominated by
  `pass/float/` — golden drift relative to the recent domain-checking
  landings, not host-specific. The 17th,
  `pass/capabilities/windows_provides_import_exit`, is a genuinely
  host-dependent diagnostic: the golden (recorded on Linux) pins
  `must use one exact receiverless zero-argument machine call after 'via'`;
  this host reports `foreign locator PeByName is not applicable to selected
  target 'macos_arm64'`. The corpus check leg is therefore not fully
  host-portable — one PE-import fixture diagnoses per host. A closing run
  needs a re-recorded `corpus_outcomes.txt` (or a host-aware expectation)
  before this gate can go green.
- **RC-PCC-REPLAY** — 2546 run / 2539 passed / 7 failed:
  `checked-trees-to-lowered-psi` ×2 (`direct_fallback_routes_keep_entry_parameters_ahead_of_prior_local_values`,
  `dependent_boolean_joins_retain_independent_input_values`),
  `terminal-interpreter` ×2 (`affine_cleanups` pair),
  `terminal-psi-to-abstract-operations` ×2 (`source_continuations_retain_distinct_result_owners_and_ordered_residuals`,
  `omega_preserves_exact_singleton_structural_return_custody`),
  `terminal-verifier` ×1 (`trusted_surface::recorded_digests_match_the_working_tree`
  — panics on `semantics/src/zeroed_structural.rs` being "under a trusted
  root" but "not a registered implementation site or a declared test-only
  module": a trusted-surface ledger miss landed on main).
- **RC-PORTABLE-PSI** — 1365 / 1362 / 3 failed: the two
  `terminal-interpreter::affine_cleanups` members plus the same
  trusted-surface digest test.
- **RC-BUILD-AND-PACKAGES** — 1528 / 1510 / 18 failed / 3 skipped: all 18
  are `package-evidence::suite` — `contract_expressions::evidence_calls` ×8,
  `boundary_application_policy::float_match` ×3, `terminal_permission_policy`
  ×4, `boundary_supply::static_telescopes` ×1, `operators::fixed_token_checked_adapters`
  ×1, `public_api::traits_and_lifetimes` ×1. One family: review-projection
  expectations drifted against recently landed semantics.
- **RC-DIAGNOSTICS** — pass (`--filter fail/` exit 0, 7 s).
- **RC-REPRESENTATIVE-PROGRAMS** — exit 1 after 2358 s; a direct
  `./target/debug/omega refresh-samples` rerun with full output (record
  tail truncates at 64 KiB) reports **0 of 150 samples built across 12
  threads**. Three failure classes, no sample reaches native:
  - 79 × `cannot accept fresh package review evidence ... package
    acceptance is missing or current requirements need review` — every
    sample's trust admission needs the per-sample review
    (`omega update --project <sample>` writes
    `build/package-manager/review-<target>.txt` with `decision row <fp>
    pending` lines; flip each to `accept`, rerun with `--resume`).
    `samples/**/omega.admissions`/`omega.lock` are gitignored host state,
    so a fresh checkout always hits this wall first.
  - 69 × `cannot compile fresh package review: checked compilation
    failed` — real checked-compile reds, dominated by the
    implicit-domain-conversion family (`implicitly converts
    'u64 in Wrapping' to 'u64'` / `... drops Wrapping/Saturating ...`),
    `Fused provider` settlement (`Main::clock` needs `Clock`,
    `Main::fs` needs `FilesystemHost`), and nested value-call
    restrictions; `game_of_life` alone carries 32 diagnostics.
  - 2 × `compilation requires a package project with a sibling
    build.omg` — `samples/cli/proofs/math_proofs` and
    `samples/cli/device/device_extent_access` lack `build.omg`.
  `refresh-samples` intentionally never accepts trust on a sample's
  behalf, so the gate cannot go green on a fresh checkout — completing
  the per-sample reviews once on the lane host is a host-preparation
  prerequisite, like the SDK selection.

## RC-NATIVE-MATRIX and the lane observation

The gate exits 2 because `tests/omega/corpus_native_macos_arm64.txt` does
not exist — the leg itself ran to completion. The full per-fixture record
(captured separately to `build/swarm/corpus-native-0293d4cad9.txt` in the
worktree; `corpus_gate.py` discards it when no golden exists):

- 710 pass/run fixtures → **167 `built`**, 258 `rejected`, 283 `checked`
  (no bound realization target), 0 timeout, 0 crashed, ~75 s at 12
  workers.
- 120 emitted Mach-O AArch64 products executed. **All 13 authored
  `expected_native_exit.txt` oracles match exactly** — exit codes
  70/5/1 and all six `trap` fixtures observed as SIGTRAP (signal 5, the
  correct arm64 trap leaf). Mach-O emission and execution are sound
  wherever the pipeline reaches them.
- Dominant rejects: `selected ProgramEntry establishment rejoins 0
  Terminal attachment identities` (134), `Float::equal` fixed-operator
  body-supply (22), `Fused provider` settlement for `FilesystemHost`/
  `Console` (15) — the same upstream families `macos_arm64_rc_gates.md`
  records; the divergence stays upstream of native emission.

**The lane's documented observation command is stale at this commit**:
`c60792d7bd` ("repo: the compiler is tested through the corpus gate
alone") deleted `compiler`'s `canary_suite` test target — the record's
`--native-execution` exit 101 reads "no test target named `canary_suite`".
The surviving direct-execution command is the gate command itself,
`python3 tools/corpus_gate.py --native`; `known_baseline_failures.md`'s
canary `_runs` row refers to a deleted harness. A future closing run
should pass `--native-execution "python3 tools/corpus_gate.py --native"`
once `corpus_native_macos_arm64.txt` is recorded. Note the golden
record is coordinator-sequenced: `corpus_outcomes.txt` and
`corpus_runner.rs` sit inside a live corpus claim at time of writing, so
this session captured the native-leg record set without writing the
golden.

## Open items this run surfaces

1. fmt drift (31 files) and two `terminal-verifier` clippy lints gate
   RC-REPOSITORY on every lane — both sit inside other sessions' claims.
2. `terminal-verifier`'s `recorded_digests_match_the_working_tree` is red
   on main (`zeroed_structural.rs` unregistered in the trusted-surface
   ledger) — also reachable from RC-PCC-REPLAY and RC-PORTABLE-PSI.
3. `package-evidence` review projections are red ×18 on main
   (RC-BUILD-AND-PACKAGES).
4. `corpus_outcomes.txt` needs a re-record after the Saturating-domain
   landings (16 fixtures) plus a host-difference decision for
   `windows_provides_import_exit` (coordinator-sequenced — see above).
5. `tests/omega/corpus_native_macos_arm64.txt` does not exist — the
   native leg needs a golden before RC-NATIVE-MATRIX can pass; the
   per-fixture record set for this commit exists at
   `build/swarm/corpus-native-0293d4cad9.txt` (worktree-local) and
   reproduces in ~75 s via `OMEGA_CORPUS_NATIVE=1 corpus_runner`.
6. `refresh-samples` needs the per-sample package reviews completed once
   on the lane host before its compile failures are even the binding
   constraint (79 admission-wall + 69 compile-red + 2 missing
   `build.omg` at this commit).
7. The next recorder run should use
   `--native-execution "python3 tools/corpus_gate.py --native"` — the
   canary `_runs` command names a test target deleted at `c60792d7bd`.

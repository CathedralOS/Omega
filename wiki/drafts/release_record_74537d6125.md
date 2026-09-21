# Release record — linux_x86_64 at `74537d6125`

Bounded release-matrix record produced under board item
RC-RELEASE-RECORD-RUN (sibling legs: RC-RELEASE-RECORD,
RUST-COMPILER-RELEASE-RECORD, RC-RELEASE-CLOSURE-RUN,
RC-RELEASE-RECORD-AND-CLOSURE, and the substrate
RC-RELEASE-RECORD-SUBSTRATE). The record substrate is
`tools/release/release_record.py` (schema `omega-release-record/1`); its
`records/` output directory is fenced to the substrate claim, so this leg
dispatches `python3 tools/release/release_record.py run --target
linux_x86_64 --records-dir <scratch>` and records the observed gate state
as a draft document instead of writing a committed JSON row.

- Host: linux x86-64 (`platform.machine()=x86_64`), Python 3.10.12
- Toolchain: `nightly-2026-09-04`, `rustc 1.100.0-nightly (a69a63265 2026-09-03)`
- Runner: `cargo` (mbx unavailable on this host — same resolution the
  substrate's `runner()` performs)
- Commit: `74537d6125c9a5e665c5f985cf5cacc2b12d7a5c`
- Cutoff: ~2026-09-21T06:30Z (lease bound)

## Gate results (linux_x86_64)

| Gate | Status | Evidence |
| --- | --- | --- |
| RC-REPOSITORY | **fail** | All five commands executed. `cargo fmt --all -- --check` exit 0 in 27.1s — clean (the 23-file drift recorded at `e12b9e8e06` no longer reproduces). `mbx clippy --workspace --all-targets -- -D warnings` exit 101 — lint drift below. `mbx nextest run -p omega-architecture-test --all-targets --no-fail-fast` exit 100 — 573/575 pass, 2 fails below. `mbx check --workspace --all-targets` exit 0 in 2.5s. `mbx nextest run --workspace --lib --no-fail-fast` exit 100 in 1245.6s (2 skipped). |
| RC-SOURCE-SEMANTICS | not completed | `mbx nextest run -p compiler --all-targets --no-fail-fast` dispatched ~00:12Z; still executing at the ~06:30Z cutoff — its `canary_suite` binaries consumed ~4h (pass corpus ~240min CPU across workers) and `samples_compile` exceeded 2h CPU. No verdict within this leg's bound — open row, not a pass. |
| RC-PCC-REPLAY | not run | Sequential dispatch never reached this gate within the bound. |
| RC-PORTABLE-PSI | not run | Sequential dispatch never reached this gate within the bound. |
| RC-BUILD-AND-PACKAGES | not run | Sequential dispatch never reached this gate within the bound. |
| RC-NATIVE-MATRIX | not run | Per required host; linux_x86_64 leg not reached within the bound. |
| RC-DIAGNOSTICS | not run | Sequential dispatch never reached this gate within the bound. |
| RC-REPRESENTATIVE-PROGRAMS | not run | Per required host; `samples_compile` standalone not reached — its corpus was still executing inside RC-SOURCE-SEMANTICS' `compiler --all-targets` at cutoff. |

### RC-REPOSITORY clippy drift at `74537d6125`

`mbx clippy --workspace --all-targets -- -D warnings` rejects on
preexisting main drift (crates observed before build stop):
`isa-x86_64` (unused import in `selected_form_encoding.rs`),
`checked-trees-to-lowered-psi` (`needless_return` in
`emission/expression_validation.rs`, `unwrap_or_default` ×2 in
`unit/.../state.rs`), `typed-trees-to-checked-trees`
(`unnecessary_to_owned` in `checks/contracts/writes.rs`,
`needless_borrows_for_generic_args` and `question_mark` in
`checks/termination/progress/origins.rs`, `question_mark` in
`execution/unit/calls/structural_arguments.rs` and
`proof/mathematical_signature.rs`). All are `-D warnings` style lints in
other lanes' in-flight surfaces; none affect this leg's claimed path.

### RC-REPOSITORY architecture-test failures at `74537d6125`

573/575 pass; 2 fail:

- `glob_self_imports::glob_self_imports_never_grow_per_crate`
- `custody_mutation_matrix::every_declared_custody_field_inventory_drives_a_substitution_matrix`
  — substitution-field inventories declared in
  `image-emission` test artifacts
  (`installation_function_nested_custody.rs`, ~14 `*FieldForTest`
  fixtures) and `compilation-report` `custody_tests.rs`
  (`NativePlacedImageEvidenceFieldForTest`) without a driving
  `run_one_field_substitution_matrix` hook — in-flight lane surface.

### RC-REPOSITORY workspace-lib failures at `74537d6125`

`mbx nextest run --workspace --lib --no-fail-fast` exit 100 after
1245.6s of execution (2 skipped). Individual failing test names were not
enumerated within this leg's bound; the run is platform-portable and
reproducible for failure-name extraction on the next leg.

## Platform runners

| Runner | Status |
| --- | --- |
| linux_x86_64 | partially recorded (this document) |
| linux_arm64 | open — no linux/arm64 host on this worker |
| macos_arm64 | open — no macOS host on this worker |
| windows_x86_64 | open — no Windows host on this worker |

## Closure

**open.** Open rows: RC-REPOSITORY (clippy lint drift, 2 architecture-test
failures, workspace-lib test failures), RC-SOURCE-SEMANTICS (dispatched,
no verdict at cutoff), RC-PCC-REPLAY / RC-PORTABLE-PSI /
RC-BUILD-AND-PACKAGES / RC-NATIVE-MATRIX / RC-DIAGNOSTICS /
RC-REPRESENTATIVE-PROGRAMS (not run at this base), and three of the four
required platform runs are structurally host-gated on this worker. The
substrate's `check` would independently classify this record as open.

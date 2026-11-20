# Compiler orchestration test cases

These tests exercised the removed `CompilationSession`, `CheckedCompileRequest`, and `orchestration` APIs. The files preserve the programs, positive and negative cases, and expected diagnostics for migration through the public CLI, project planner, or numbered compiler stage that owns each result.

| Former harness | Current coverage and remaining work |
| --- | --- |
| `process/default_build.rs` and `process/build_session.rs` | The active planner test in `source-rust/subsystems/compiler/build/tests/default_build.rs` checks that one named file supplies two deployments through one default Build execution. The CLI's `named_build_entry.rs` exercises neighboring files, renamed Builds and pre-effect checking. The archived cases retain additional graft, sponsorship and accounting assertions. |
| `process/build_charge.rs`, `process/executed_git_source.rs`, `process/package_batch_sources.rs`, `process/terminal_sharing.rs` | Port the independent accounting, acquisition, target and sharing assertions against the current owners. |
| Source, lowered-form and native integration tests in this directory | Recover each case through its supported compiler or CLI entry. The expected program behavior and diagnostics are in the original files. |
| `support/` | Helpers used by the retired test workflows. The active compiler keeps the library fixture helper its source-stage tests still use. |

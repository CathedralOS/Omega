---
name: layer-measurement
disable-model-invocation: true
description: Use only when the user explicitly requests layer-measurement to compare compiler stage results, check a diagnostic, or measure what a handoff test can distinguish.
---

# Compare compiler and test behavior

Choose one decision and predict what a lawful input and a nearby wrong input will produce. Identify the ordinary stage or CLI entry point that can observe the answer. For source-stage handoffs, several tests live beside the producing stage but run under the [source-compiler test manifest](../../../source-rust/subsystems/compiler/source-to-psi/Cargo.toml). Use the registered target with its required features.

**Compare compiler revisions.** Save the baseline revision, local diff if any, toolchain, host, flags, and exact input including its dependencies and entry point. Keep the outputs and exit codes of the baseline and candidate in separate run directories. For a suite, identify cases by test name and fixture or source occurrence. For diagnostics, retain the stage, reason, file and span. Record the first stop when the intended stage is never reached.

Match the original cases individually. Distinguish a repaired case, the same failure, a changed first stop and a missing case. If a blocker appears for the first time, run that exact input on the baseline before calling it a regression. If a fixture was renamed, added or removed, account for it explicitly. An incomplete run leaves the affected cases unmeasured. Report the original denominator, added cases, and paths to the two sets of results.

**Compare assertion quality.** Hold the producer output fixed and give the old and revised assertions the same accepted control and the same altered copies. Restore the accepted output between changes. Verify that each alteration reached the field under test, then record whether each assertion detects it for the expected reason. Useful substitutions include exchanging two equally typed parameters, keeping a value constant while changing its origin, or attaching a valid diagnostic to the wrong file. The [equal-length view case](../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/lowered_facts.rs) keeps the numeric length unchanged while exchanging the descriptor that supplied it.

Count distinct wrong decisions caught, shared misses and unrepresentable changes separately. If an independent review was requested, reserve a challenge before revising the assertions and report its first result separately from follow-up repairs. A deliberately altered output can demonstrate a weak test. To attribute a defect to the compiler, reproduce the wrong output from ordinary source. Close with what changed in compiler behavior, what changed in the test’s ability to catch a mistake, and what the available inputs could not settle.

# crash_exclusion

`Build::select_provider<assert_kit::main::Assert, NoOpAssert>` + `Optimizations::enable(ControlFlowCleanup)` + `Build::exclude_crash(CrashCause::Trap)` — Trap excluded while the no-op provider composes; vendored assert-kit sibling dep.

Compile-checked only: run a check target, not native.

# Standalone benchmark subject

The smallest deployable Omega program: `data Main {}` with an empty
`Main::main`, bound to every hosted `ProgramEntry` slot, and **no package
dependencies** — not even the standard library.

It exists as a benchmark subject. Every other measured
subject registers a dependency on `source/library/std`, whose compile currently rejects at
the comparison-occurrence realization gate. This subject stands alone: the
compile reaches a published native artifact (~25 s on a w9 Linux x86-64 host),
so `compile_time_ms`, `peak_memory_bytes`, and `code_size_bytes` are honest
measurements of the whole pipeline's fixed cost on the smallest possible
program.

The artifact is not runnable as a clean process. `ProcessExit::exit_process` is
a toolchain-owned boundary whose canonical provider is bound to the std package
identity (`omega-language-std`), and a boundary machine declared in a subject's
own package produces no provider plan — only dependency packages contribute
them. With no exit authority the emitted entry falls off the end and the
process terminates abnormally, so only the compile is measured; the runtime
leg was recorded `skipped`, never with an expected exit code.

## Measuring

Accept the root package once per target, then compile with `--timings`, which
prints one line per compiler phase and the published artifact:

```text
target/debug/omega packages update --project samples/cli/basics/standalone --target linux_x86_64
# Edit the `pending` decisions in the review it names to `accept`, then:
target/debug/omega packages update --resume --project samples/cli/basics/standalone
target/debug/omega --timings --target linux_x86_64 samples/cli/basics/standalone/main.omg < /dev/null
```

`omega-bench compare` cannot measure this subject: its compiler mode runs the
published artifact against an exit-code oracle, and this artifact has no clean
exit. `omega.lock` is host-local generated state and is never committed.

# Standalone benchmark subject

The smallest deployable Omega program: `data Main {}` with an empty
`Main::main`, bound to every hosted `ProgramEntry` slot, and **no package
dependencies** — not even the standard library.

It exists for the benchmark harness (`tools/benchmark/`). Every other measured
subject `depend()`s on `source/library/std`, whose compile currently rejects at
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
process terminates abnormally, so the runtime leg is measured with `--no-run`
(recorded `skipped`), never with an expected exit code.

## Measuring

```text
python3 tools/benchmark/benchmark.py measure \
    --root samples/cli/basics/standalone/main.omg \
    --target linux_x86_64 --no-run \
    --omega target/debug/omega
```

Like any subject, the root package itself must be accepted once per target
(`benchmark.py prepare`, or let `measure` settle it automatically); `omega.lock`
is host-local generated state and is never committed.

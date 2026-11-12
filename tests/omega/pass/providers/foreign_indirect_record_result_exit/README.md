# Indirect foreign aggregate results

On Linux x86-64 with glibc 2.33 or newer, after ordinary package preparation:

```text
omega run tests/omega/pass/providers/foreign_indirect_record_result_exit/main.omg
```

The authored exit is 70 on both engines. `mallinfo2` returns ten `size_t` fields (80 bytes on
this target) through the System V hidden result pointer. The program performs
two foreign calls and observes the first complete record after the second call.
The glibc declaration is in `malloc.h`.

The corpus harness supplies its reviewed test package inputs; that is distinct
from ordinary CLI package review:

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_indirect_record_result_exit
```

This checks a memory-returned record on Linux x86-64, not matching-host ABI
qualification on Windows or either AArch64 target.

The oracle checks relations glibc's `mallinfo2` guarantees for every process
history, not fresh-process values, which differ by engine (the interpreter
calls `mallinfo2` in its provider worker): used plus free space equals the
arena, `usmblks` is zero, the top chunk is one ordinary block, and releasable,
fast-free and mapped figures stay within their totals. The program exits
through the canonical `ProcessExit::exit_process`, so `omega run --both` exits
70 on both engines.

In a fresh native process every field except the block count is zero, so a
swap of two zero fields shows only on the interpreter, whose heap is in use.

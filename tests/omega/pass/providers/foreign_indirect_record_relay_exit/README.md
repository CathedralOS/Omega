# Foreign record results through ordinary calls

On Linux x86-64 with glibc 2.33 or newer, after ordinary package preparation:

```text
omega run tests/omega/pass/providers/foreign_indirect_record_relay_exit/main.omg
```

The authored exit is 70 on both engines. `collect` receives glibc's 80-byte `mallinfo2`
result, passes it by value to `relay` with a scalar argument, and returns the
selected result to its caller. The helper exercises both an owned block arrival
and a fresh foreign result returned from an alternative block. The first record
remains observable after the second complete call chain.

Use the native corpus route for independently staged test package inputs:

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_indirect_record_relay_exit
```

The direct-call control is `providers/foreign_indirect_record_result_exit`.

The oracle checks relations glibc's `mallinfo2` guarantees for every process
history, not fresh-process values, which differ by engine (the interpreter
calls `mallinfo2` in its provider worker): used plus free space equals the
arena, `usmblks` is zero, the top chunk is one ordinary block, and releasable,
fast-free and mapped figures stay within their totals. The program exits
through the canonical `ProcessExit::exit_process`, so `omega run --both` exits
70 on both engines.

In a fresh native process every field except the block count is zero, so a
swap of two zero fields shows only on the interpreter, whose heap is in use.

# Foreign record results through ordinary calls

On Linux x86-64 with glibc 2.33 or newer, after ordinary package preparation:

```text
omega run tests/omega/pass/providers/foreign_indirect_record_relay_exit/main.omg
```

The authored exit is 70. `collect` receives glibc's 80-byte `mallinfo2`
result, passes it by value to `relay` with a scalar argument, and returns the
selected result to its caller. The helper exercises both an owned block arrival
and a fresh foreign result returned from an alternative block. The first record
remains observable after the second complete call chain.

Use the native corpus route for independently staged test package inputs:

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_indirect_record_relay_exit
```

The direct-call control is `providers/foreign_indirect_record_result_exit`.
The host's libc allocation statistics are observed before any program allocation:
one ordinary free block, with zero in the other fields.

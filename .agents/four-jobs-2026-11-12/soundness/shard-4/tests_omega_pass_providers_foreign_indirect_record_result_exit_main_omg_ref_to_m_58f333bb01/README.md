# Indirect foreign aggregate results

On Linux x86-64 with glibc 2.33 or newer, after ordinary package preparation:

```text
omega run tests/omega/pass/providers/foreign_indirect_record_result_exit/main.omg
```

The authored exit is 70. `mallinfo2` returns ten `size_t` fields (80 bytes on
this target) through the System V hidden result pointer. The program performs
two foreign calls and observes the first complete record after the second call.
Before this program allocates from libc, the statistics contain one ordinary
free block and zero in the other fields. The glibc declaration is in `malloc.h`.

The corpus harness supplies its reviewed test package inputs; that is distinct
from ordinary CLI package review:

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_indirect_record_result_exit
```

This checks a memory-returned record on Linux x86-64, not matching-host ABI
qualification on Windows or either AArch64 target.

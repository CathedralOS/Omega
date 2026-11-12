# Indirect foreign aggregate results

On Linux x86-64 with glibc 2.33 or newer, glibc's `mallinfo2` returns ten
`size_t` fields (80 bytes) through the System V hidden result pointer. The
program makes two direct calls with its own 64 MiB `malloc` between them and
checks that the first complete record survives the second call.

The oracle is what the program itself causes, not the host's heap history: the
second record shows exactly one more mapped block, at least 64 MiB larger, than
the first. Each record must also satisfy relations glibc reports for any heap
(`arena == allocated + available`, one or more ordinary blocks, zero unused
bytes, freed and releasable bytes within available). A reused or misplaced
result home fails these checks. The fixture exits through the canonical
`ProcessExit::exit_process`, so both engines observe termination.

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_indirect_record_result_exit
```

Expected exit 70 on both engines (`omega run --both`). This checks a
memory-returned record on Linux x86-64, not matching-host ABI qualification on
Windows or either AArch64 target. The relay control is
`providers/foreign_indirect_record_relay_exit`.

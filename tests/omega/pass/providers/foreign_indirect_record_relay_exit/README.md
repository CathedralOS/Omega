# Foreign record results through ordinary calls

On Linux x86-64 with glibc 2.33 or newer, `collect` receives glibc's 80-byte
`mallinfo2` result, passes it by value to `relay` with a scalar argument, and
returns the selected result to its caller. The first chain returns the owned
argument through `complete`; the second, after the program's own 64 MiB
`malloc`, returns a fresh foreign result from `replace`. The first record must
remain intact after the second chain.

The oracle is the program's own effect: the second record shows exactly one
more mapped block, at least 64 MiB larger, than the first. Each record must
also satisfy relations glibc reports for any heap (`arena == allocated +
available`, one or more ordinary blocks, zero unused bytes, freed and
releasable bytes within available). It exits through the canonical
`ProcessExit::exit_process`.

```text
python3 tools/corpus_gate.py --native --filter providers/foreign_indirect_record_relay_exit
```

Expected exit 70 on both engines (`omega run --both`). The direct-call control
is `providers/foreign_indirect_record_result_exit`.

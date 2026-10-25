# Atomic operations across states

From the repository root, prepare the project with the ordinary package review
commands, then run it on the host:

```text
mbx run -p omega -- run --both samples/cli/systems/atomics_cross/main.omg
```

Use Cargo when `mbx` is unavailable. The expected exit is **70** in both engines,
with `PASS: RMW and every load/store ordering round-tripped` in native output.
Press Enter at the prompt, or supply EOF for a noninteractive run.

The same counter crosses nested machine parameters through store, fetch-add,
compare-exchange and ordered load/store operations. Fetch-add and exchange must
return the instruction-observed prior. This serial check does not establish
concurrent synchronization or the complete atomic memory model.

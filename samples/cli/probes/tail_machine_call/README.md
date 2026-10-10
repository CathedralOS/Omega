# tail machine call

Compile-scope probe. No standard-library dependency.

- tail_machine_call: transition arm destination can be a MACHINE
  name — `-> Main::with_argument(main, 7)` — cross-machine tail
  calls replacing the frame, guarded and unconditional.
- overload_by_use: same-name machines differing only in result
  (`i32 in Logged` vs Unit) selected by statement vs annotated
  value position; a bare `as` does NOT switch selection.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/tail_machine_call/main.omg
mbx run -p omega -- update --project samples/cli/probes/tail_machine_call
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

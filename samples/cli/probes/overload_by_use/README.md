# overload by use

Compile-scope probe. No standard-library dependency.

- tail_machine_call: transition arm destination can be a MACHINE
  name — `-> Main::with_argument(main, 7)` — cross-machine tail
  calls replacing the frame, guarded and unconditional.
- overload_by_use: same-name machines differing only in result
  (`i32 in Logged` vs Unit) selected by statement vs annotated
  value position; a bare `as` does NOT switch selection. The Unit
  sibling recurses through the terminal arm `-> walk(n - 1)`, whose call
  completes the enclosing Unit machine and so selects the Unit sibling.
  Runtime recursion is tail-only
  ([termination](../../../../wiki/spec/language/termination.md#calls-and-productive-loops));
  the earlier statement spelling `walk(n - 1);` keeps the state live across
  the callee, and native production refused it at stack demand
  (`TerminalStackCycle`).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/overload_by_use/main.omg
mbx run -p omega -- update --project samples/cli/probes/overload_by_use
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

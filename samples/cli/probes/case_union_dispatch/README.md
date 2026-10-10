# Case-Union Dispatch

Compile-scope probe: case-subset domains as transition arms. No
standard-library dependency.

- `domain Command::Interactive(command) requires command in
  Command::Move | Command::Say` — a pure case union admits both
  payloads ("Cases Are Domains").
- `transition main.cmd { None | Quit | Interactive }` — the union arm
  covers both tags, so the dispatch is exhaustive with no `_` arm.
- `hits` records which arm ran (Say classifies through Interactive).

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/case_union_dispatch/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_union_dispatch
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

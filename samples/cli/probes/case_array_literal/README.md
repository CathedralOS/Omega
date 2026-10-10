# Case Array Literal

Compile-scope probe: whole case-array literal stores. No
standard-library dependency.

- `main.grid = [Cell::Wall, ..]` — whole array of payload-free
  cases in one assignment; the second literal replaces elements.
- `transition main.grid[i]` — case dispatch on an element subject.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/case_array_literal/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_array_literal
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

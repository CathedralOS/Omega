# Bounded Concat

Compile-scope probe: bounded text carriers concatenate. No
standard-library dependency.

- `"Room " + main.label` copies the literal and the label's live
  bytes into a fresh owned result before replacing text's inline
  storage.
- Capacity is a bound, not readable content: label has capacity 4
  but holds 2 bytes; the receiving proof establishes the result fits
  text's capacity 16.
- `main.text == "Room A1"` — byte-equality observes length 7.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/bounded_concat/main.omg
mbx run -p omega -- update --project samples/cli/probes/bounded_concat
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

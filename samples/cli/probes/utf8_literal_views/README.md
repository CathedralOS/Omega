# UTF-8 Literal Views

Compile-scope probe: the compiler-recognized `valid_utf8` predicate.
No standard-library dependency.

- `domain [u8]::Utf8(bytes) requires valid_utf8(bytes)` — a built-in
  predicate, so `classify("quit")` admits the literal without `as`
  (contrast: user-defined requires clauses over slices have no grant
  path — filed gap).
- `transition text == "quit"` — `==` between a `&[u8] in Utf8` view and
  a literal is content equality, lowered through the same TextEquals
  leaf String uses.
- States return `u64 in Wrapping` — a result-position domain.
- `hits` counts arms; 1 proves the literal matched on content.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/utf8_literal_views/main.omg
mbx run -p omega -- update --project samples/cli/probes/utf8_literal_views
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

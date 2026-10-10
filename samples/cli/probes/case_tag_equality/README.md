# Case Tag Equality

Compile-scope probe: tag equality on enum members via a named empty
`satisfies` conformance. No standard-library dependency.

- `CommandEquality satisfies Equatable<Command> { }` — the empty block
  synthesizes structural `==`/`!=`.
- `main.cmd == Command::None` — a payload-LESS case member is a legal
  equality value even when a sibling case carries a payload: tag
  identity is total equality for the payload-free case.
- `let mut` typed local + reassignment.

Compile-checked only (no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/case_tag_equality/main.omg
mbx run -p omega -- update --project samples/cli/probes/case_tag_equality
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.

# Constrained field call arguments

Both a local owner and a borrowed record lend their constrained byte field
to a scalar-reading helper. The loan must keep the original storage and its
standing `where` fact, not manufacture a borrowable bounded-integer shape.
Nested record fields use the same loan route. A mutating call in an earlier
scalar argument clears the field before the later borrowed argument reads it;
the helper must observe that current original storage, not a hoisted snapshot.
Both native and Terminal execution must return exit 0.

With `omega` on PATH, run from this directory:

```sh
omega update --project . --target macos_arm64 --offline
omega run --both main.omg
```

Select the matching target on another host. If preparation requests review,
accept the named decisions and resume with `omega update --resume --project .
--offline`.

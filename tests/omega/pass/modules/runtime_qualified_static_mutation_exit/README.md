# Module-qualified static mutation

Run from the repository root after the fixture's normal package review:

```sh
omega run --both tests/omega/pass/modules/runtime_qualified_static_mutation_exit/main.omg
```

Both engines must exit 70. `helpers::Counter::write` mutates the caller's local
through an explicit mutable argument, then `helpers::Counter::read` observes it.
The namespace selects a declaration; it supplies neither storage nor an implicit
receiver. The ordinary calls must not be intercepted as Build filesystem facets
merely because their names are `write` and `read`.

The fixture binds all four supported hosted targets. Native/interpreted agreement
has been exercised on macOS ARM64; the other hosts still need matching-host runs.

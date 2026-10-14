# Qualified machine calls

`omega run samples/cli/memory/qualified_machine_calls/main.omg` must exit 0.
Ordinary qualified machines named `store`, `swap`, `fetch_add`,
`compare_exchange` and `load` preserve their normal arguments and results.
Nested calls, mutable arguments and calls in another state use that same route.

An ordinary local named `NoOrdering` is also supplied to qualified calls. Its
spelling must not trigger atomic expansion. Actual dot atomic operations retain
their separate ordering and type checks.

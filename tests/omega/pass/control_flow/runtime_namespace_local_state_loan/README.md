# Local storage across a namespace-qualified state jump

From the repository root, build the compiler and prepare the package-free project:

```sh
mbx build -p omega -j 4
(cd tests/omega/pass/control_flow/runtime_namespace_local_state_loan && ../../../../../target/debug/omega update)
target/debug/omega run --both tests/omega/pass/control_flow/runtime_namespace_local_state_loan/main.omg
```

Both engines must exit `0` with empty stdout. `Main` is a namespace, with no data
owner or hidden receiver. The entry lends a local record and whole byte array to
a successor. The successor writes and reads `70`, observes view length `8`, and
traps if either observation is wrong.

The byte-identical source fails on `e1058e8e89cea520eca0e9baf379d779d9610f6c`
at `state signature: parameter signature: attached data shape, state 0`.
The repair resolves data ownership only for signatures retaining owner storage,
forwards owned-local loans through the common projected-loan path, and replays
the authored whole-collection borrow before ordinary custody checks.

The adjacent `runtime_later_state_shared_array_loan` project varies the namespace,
state position, storage names, array extent, and shared view access. It also
exercises lending a local without an initializer. Prepare that project with
`omega update` in its directory and run its `main.omg` with `omega run --both`;
its oracle is also exit `0` and empty stdout.

Complete separate intended-invalid project. The two-element `values` array
is read at literal index 2. The observed check exits 1 with exactly the pinned
bounds diagnostic in `expected_diagnostic.txt`; it did not report an import,
entry or package error. This pins the ordinary collection bounds rule in
`wiki/spec/language/numeric_values.md#collection-bounds`.

Command from repository root:

```sh
$OMEGA --check --target linux_x86_64 samples/cli/independent/terrain_contour_tracing/controls/intended_invalid/main.omg
```

The project has its own build metadata and std dependency. The invalid read
is rejected during source checking, before any entry admission or execution.

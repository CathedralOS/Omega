Complete separate intended-invalid project. The eight-element `bearing`
table is written at literal index 8. The observed check exits 1 with
exactly the pinned bounds diagnostic in `expected.txt`; it does not report
an import, entry or package error. This pins the ordinary collection
bounds rule in `wiki/spec/language/numeric_values.md#collection-bounds`.

Command from repository root:

```sh
$OMEGA --check --target linux_x86_64 samples/cli/independent/survey_traverse/controls/intended_invalid/invalid.omg
```

The project has its own build metadata and std dependency. The invalid
write is rejected during source checking, before any entry admission or
execution.

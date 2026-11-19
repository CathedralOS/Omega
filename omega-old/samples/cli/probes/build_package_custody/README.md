# build_package_custody

Build-activation custody probe: one dependency package serves the root
through both a Build scope edge (`Build::build_dependency` aliased `"kit"`) and a Product
scope edge (`Build::dependency` aliased `"product_kit"`) in a single request. The dependency's
own build consults a nested `generator` package through a third build edge,
reads `template.txt` through the captured source inventory, and stages a
generated product source through the required-output obligation flow —
`require` → `create` → `write` → `close` → `include_source` → `complete`.

`sample.json` includes both `dependency/` and `generator/` in the caller's
explicit source inventory. Their own project descriptors give them separate
package owners; their files are not implicit reads from a consumer tree.

The root's build calls `kit::value()` in build scope; `main.omg` calls the
same machine through the product edge. `value()` is generated and forwards
to the authored `seed()`: authored source cannot select its own package's
generated declarations, so the generated stratum owns the exported machine.
Both routes must resolve to `7`, and the dependency's build evaluates once for both
purposes and across every requested target:

```text
mbx run -p omega -- --check --target linux_x86_64 --target macos_arm64 \
    --build-input .gitignore --build-input README.md --build-input build.omg \
    --build-input dependency --build-input generator --build-input main.omg \
    samples/cli/probes/build_package_custody/main.omg
```

`controls/` holds same-shaped builds the compiler must reject: an
`artifact_only` application binding an executable root, a forged
`RequiredOutput {}` obligation, a `complete` attempted with a live writer,
and a build whose declaration must be rejected before any declared output
effect can stage a file.

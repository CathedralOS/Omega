# build_package_custody

Build-activation custody probe: one dependency package serves the root
through both a Build scope edge (`build_depend_as "kit"`) and a Product
scope edge (`depend_as "product_kit"`) in a single request. The dependency's
own build consults a nested `generator` package through a third build edge,
reads `template.txt` through the captured source inventory, and stages a
generated product source through the required-output obligation flow —
`require` → `create` → `write` → `close` → `include_source` → `complete`.

The root's build calls `kit::value()` in build scope; `main.omg` calls the
same machine through the product edge. Both routes must resolve to the
generated constant `7`, and the dependency's build evaluates once for both
purposes and across every requested target:

```text
mbx run -p omega -- --check --target linux_x86_64 --target macos_arm64 \
    samples/cli/probes/build_package_custody/main.omg
```

`controls/` holds same-shaped builds the compiler must reject: an
`artifact_only` application binding an executable root, a forged
`RequiredOutput {}` obligation, a `complete` attempted with a live writer,
and a build whose declaration must be rejected before any declared output
effect can stage a file.

A boundary provider receives nested array literals and a copied local array after mutation. Both calls observe the complete values, and the caller retains its copyable local. Expected exit: 70.

Run on macOS ARM64 through the public compiler:

```sh
omega update --project tests/omega/pass/providers/runtime_boundary_array_construction_exit --target macos_arm64 --offline
omega run --both tests/omega/pass/providers/runtime_boundary_array_construction_exit/main.omg
```

If update produces a package review, record its decisions and resume:

```sh
omega update --resume --project tests/omega/pass/providers/runtime_boundary_array_construction_exit --offline
```

Refresh the accepted package lock when the compiler or package changes.

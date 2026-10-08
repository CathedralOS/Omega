Run on macOS AArch64 through the public CLI after package acceptance:

```sh
cd tests/omega/pass/borrows/runtime_affine_local_case_return_exit
omega update --offline
# Review pending decisions, then: omega update --resume --offline
omega run --both main.omg
```

Native and Terminal execution must exit 42. Package preparation reviews the
current std package as well as this fixture; a corpus run does not supply
accepted-package evidence, and the fixture does not publish its generated lock.

The selected case forwards its payload and the independent token to another state. The fallback returns a scalar while disposing both remaining state owners in reverse declaration order. Both branches run in one invocation.

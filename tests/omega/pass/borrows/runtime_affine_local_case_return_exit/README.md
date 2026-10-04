Run `omega run --both tests/omega/pass/borrows/runtime_affine_local_case_return_exit/main.omg` from the repository root after preparing the package with `omega update --project tests/omega/pass/borrows/runtime_affine_local_case_return_exit --offline`. Native and interpreted execution must exit 42.

The selected case forwards its payload and the independent token to another state. The fallback returns a scalar while disposing both remaining state owners in reverse declaration order. Both branches run in one invocation.

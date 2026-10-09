# Nested projected custody and sibling disposition

An affine envelope disposes a linear sibling and returns a nested affine child
with its own linear claim. Construction order differs from the callee's claim
path order; calls bind by exact paths, not machine-local identity order.
Native and Terminal must both exit 0.

Run `omega run --both tests/omega/pass/ownership/runtime_nested_projected_claim_return_exit/main.omg`.

# Restore borrowed storage with a contained claim

```sh
omega run --both tests/omega/pass/ownership/runtime_borrowed_projected_claim_transform_exit/main.omg
```

Expected native and Terminal-interpreter exit: **42**. The extracted affine
inventory contains a linear counter. Fresh and identity-result branches must
transfer that descendant claim and the inventory's affine complement, then
repair the same caller field. The final counter is explicitly consumed.
This checks projected claim paths rather than only a whole-value claim.
The scalar marker preceding the counter forces a nonzero projection offset;
both branches must preserve it as 99.

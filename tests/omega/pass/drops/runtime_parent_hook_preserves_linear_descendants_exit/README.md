The authored result is exit 0 in native and Terminal execution. The affine
owner's hook mutates the values held by its linear children before their hooks.
It must retain their claim identities and receiver storage until those hooks
retire them. A parent hook is not whole-subtree destruction.

This challenges cleanup's assumption that a nominal parent carries no live
linear descendants; it does not introduce a separate lowering route.

The children's hooks have no prerequisites. Applying the parent's guarantees
to linear child cleanup prerequisites remains source-checker work; this test
isolates execution of the already checked claim and storage schedule.

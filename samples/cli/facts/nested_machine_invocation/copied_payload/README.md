# Copied payload before a nested call

NESTED-MACHINE-UNIFICATION: a copied case payload ends its source observation
before the helper mutates that source. `take` forwards the saved scalar to its
nested helper, which clears the original case. The second call observes Empty;
both engines must return exit 0. The invalid control forwards a shared reference
instead of a copied payload, so the mutation still overlaps a retained loan.

This is an additional control for the existing nested-machine fact, not a new
transport matrix. The standard-library customer is `Permissions::set_readonly`.

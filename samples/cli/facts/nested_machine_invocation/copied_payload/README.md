# Copied payload before a nested call

NESTED-MACHINE-UNIFICATION: a copied case payload ends its source observation
before the helper mutates that source. `take` forwards the saved scalar to its
nested helper, which clears the original case. The second call observes Empty;
both engines must return exit 0. The ../invalid_copied_payload control forwards a shared reference
instead of a copied payload, so the mutation still overlaps a retained loan.

This is an additional control for the existing nested-machine fact, not a new
transport matrix. The standard-library customer is `Permissions::set_readonly`.

The [structural composition](structural/main.omg) copies a two-field payload out
of a case below a borrowed record, then clears that case in the nested helper.
The captured fields and surrounding canaries must retain their values. Its
[../invalid_copied_payload twin](invalid_structural/main.omg) removes only the payload's `copy`
property; it must reject moving that value out of borrowed storage. This is the
same operation required by Std's recursive directory traversal, whose selected
`DirEntry` is a structural copy rather than a scalar. Case and member identity
remain in the call path; neither copyability nor path geometry supplies an
active-case premise.

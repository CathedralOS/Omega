# Borrowed field bundle

Expected exit: 0. A helper returns two disjoint mutable borrows as record
fields after nested forwarding calls. Another constructor reverses the fields.
The consuming helper writes 31 and 47 through those fields, and the caller
observes 47 in the first original cell and 31 in the second. The marker also
survives forwarding and permutation. After consuming the bundle, the caller
changes its marker to 42.

After ordinary package review, run:

```sh
omega run --both samples/cli/collections/borrowed_field_bundle/main.omg
```

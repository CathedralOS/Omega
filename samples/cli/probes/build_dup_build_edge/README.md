# build_dup_build_edge

Two `Build::build_depend_as` rows name the same `dependency` directory with
different aliases (`k1`, `k2`). z24 proved two *product* aliases on one
package coexist; this pins whether the *build* purpose row family accepts
the same duplication or rejects it as a collision.

Only `k1::main`'s `seed()` is called — the `k2` edge stays inert to isolate
row admission from import-name collisions.

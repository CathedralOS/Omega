# boundary_ensures_relation

Boundary `ensures` relating two out-params: `size == cap && cap <= 8` bounds size through cap; `&main.buf\[size\] as &u32` byte-region→u32 view recast.

Compile-checked only: run a check target, not native.

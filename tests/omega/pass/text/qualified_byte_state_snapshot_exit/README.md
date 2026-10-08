# Qualified byte snapshots across states

`omega run --both main.omg` returns 70. A qualified bounded byte result is
passed by value through two state bindings while a loan mutates its original
record field. The snapshot must retain `Gate`, the caller must observe `é`, and
the disjoint sibling must remain 29. Both state loans retain the caller's
original record backing.

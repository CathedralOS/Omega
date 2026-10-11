# build_dep_diamond

Two sibling dependencies each declare an edge onto the same nested
package. `dep_a` and `dep_b` both `depend_as` `dep_c`; each sees
`shared()` through its own edge, and the root application sees both
`left()` and `right()` — the transitive closure unifies `dep_c` once
rather than duplicating or conflicting it.

    root (application)
      ├─ kit_a -> dep_a (package left_kit) ─┐
      └─ kit_b -> dep_b (package right_kit) ├─ kit -> dep_c (shared_kit)

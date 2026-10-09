# Selected child custody and retirement

`verify` transfers one shared child of an exclusive stored descriptor into
either runtime-selected record constructor. It reads through the receiving
leaf, then mutates the original backing while both record shells remain live
for their disjoint scalar fields. Both choices must preserve the child until
its last read and restore the parent afterward.

After ordinary package preparation, run:

```text
omega run --both tests/omega/pass/borrows/runtime_selected_reference_child_retirement_exit/main.omg
```

Native and interpreter execution both exit zero. The related
`fail/borrows/selected_reference_child_live_source_write` control selects
between that child and another backing but writes before the receiving leaf's
last read; source checking must retain the possible child and reject the write.

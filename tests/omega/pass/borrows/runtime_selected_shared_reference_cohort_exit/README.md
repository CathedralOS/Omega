# Selected shared descriptor cohort

Both constructor arms copy the same shared child into `Held.target`. Reading
the receiving leaf does not consume the original child: it remains readable
until its own last use. Only after both carriers end may the original backing
be mutated. Scalar sibling fields remain usable in both record shells.

After ordinary package preparation:

```text
omega run --both tests/omega/pass/borrows/runtime_selected_shared_reference_cohort_exit/main.omg
```

Both engines must exit zero for both selections and both descriptor retirement
orders. The original-first route also retains a bare descriptor copy after
the selected record's last reference read. Neither earlier retirement can
restore parent mutation while that copy remains live. Shared descriptor copying
must preserve the complete parent lineage and cohort; it never copies the Cell.

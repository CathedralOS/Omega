# Fixed-layout named constant

The [named-constant sample](main.omg) tests whether reading `SAVED` preserves
the selected `LayoutStorage<Pair, PairLayout>` application. It checks that
`lo - 42` is zero, `hi` is 9, byte 8 is 42, and byte 0 is 9.

The [negative twin](../invalid_const_name/main.omg) writes 41 to `lo`. The cast
must reject that write because `Delivered` requires exactly 42.

# Disjoint occupancy windows

A state edge lends two mutable windows of one occupancy row. `main` lends
the halves of a record field; a later state builds its own row and lends
that row's halves. Each window write is visible through the same window.

Closed stdin prints `3 9 5 7` and exits 0.

The invalid control lends overlapping windows of one row and must be
rejected before execution.

```text
omega run samples/cli/collections/disjoint_occupancy_windows/main.omg < /dev/null
omega --check samples/cli/collections/disjoint_occupancy_windows/invalid/main.omg
```

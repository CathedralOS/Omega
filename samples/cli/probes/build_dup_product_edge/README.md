# build_dup_product_edge

Two `depend_as` product edges on the same dependency directory are
legal — each edge is a distinct dependency occurrence under its own
alias. `main.omg` imports `k1::main` and `k2::main` and calls `value()`
from both occurrences in one expression, mirroring the build-side
`build_dup_build_edge` sample.

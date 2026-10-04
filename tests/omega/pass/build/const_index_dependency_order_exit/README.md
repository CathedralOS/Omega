# Const-index dependency order

The outer constant helper reads the length of a locally instantiated buffer.
Its length depends on another constant helper. The compiler must settle that
inner argument before admitting the outer invocation; a provisional zero is
not a layout the helper may observe.

Build binds the program entry only when the resulting capacity is 4. A wrong
value reaches an invalid product-entry query, so a successful empty Product
cannot hide incorrect Build evaluation. Moving the storage declaration below
the helpers must preserve the result.

Run `omega update --project
tests/omega/pass/build/const_index_dependency_order_exit --offline`, then
`omega run tests/omega/pass/build/const_index_dependency_order_exit/main.omg`.
The native program exits 0.

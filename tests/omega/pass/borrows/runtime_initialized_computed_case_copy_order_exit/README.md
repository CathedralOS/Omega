Run `omega run --both tests/omega/pass/borrows/runtime_initialized_computed_case_copy_order_exit/main.omg`.
The same program as `runtime_computed_case_copy_order_exit` with `cells`
initialized by a whole literal instead of declared and established by element
stores. A copy-typed call argument read through a computed index whose selector
performs a call (`tag(cells[cursor.row][select(&mut cursor)], replace(&mut
cells))`) is copied into its parameter; the selector call and the sibling
argument's call keep their authored order. Both engines exit 0; a wrong
observation crashes with `Trap`.

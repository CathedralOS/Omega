Run `omega run --both tests/omega/pass/ownership/runtime_captured_replacement_selector_exit/main.omg`.

The replacement's RHS changes the index from zero to one. Capture the original
destination before that call; both old-value cleanup and replacement use element
zero, while the caller observes the new selector and the untouched sibling.
Native, Terminal and checked-tree execution must produce zero.

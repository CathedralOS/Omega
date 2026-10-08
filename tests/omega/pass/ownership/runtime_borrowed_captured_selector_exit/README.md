# Borrowed replacement retains its caller destination

Run `omega run --both tests/omega/pass/ownership/runtime_borrowed_captured_selector_exit/main.omg`.

The authored exit is 0. A helper replaces an element through an exclusive array
loan. Its target computation runs once, before the RHS changes the selector to
an out-of-range value. The caller observes the selected update and intact sibling.

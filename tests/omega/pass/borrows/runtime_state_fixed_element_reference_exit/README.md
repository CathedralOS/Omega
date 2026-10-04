# Fixed element reference through a state result

Run `omega run --both tests/omega/pass/borrows/runtime_state_fixed_element_reference_exit/main.omg`.
Both engines must exit 70. The helper forwards its receiver into a renamed
state parameter and returns element 1. Reading the returned reference must see
11 while the neighboring element remains 7. The source origin retains its fixed
selector even though write-effect analysis conservatively covers the array.

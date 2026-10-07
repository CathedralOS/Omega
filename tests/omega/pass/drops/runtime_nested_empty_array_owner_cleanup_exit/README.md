Run `omega run tests/omega/pass/drops/runtime_nested_empty_array_owner_cleanup_exit/main.omg`.
The authored result is exit 0 in native and Terminal execution.

The outer array has two elements; each is an empty array. No Token exists,
so Token's Positive domain cannot prevent establishment of the enclosing
Holder. Its ordinary nominal cleanup prerequisite still applies.

# Local subslice restoration windows

Scalar field stores, whole copy replacements and affine move-out/repair use a
local narrowed view's retained loan. Nested narrowing keeps its offset, and
disjoint elements retain their original values. Correct execution returns 0.

```text
omega update --project tests/omega/pass/ownership/runtime_local_subslice_element_window_exit
omega run --both tests/omega/pass/ownership/runtime_local_subslice_element_window_exit/main.omg
```

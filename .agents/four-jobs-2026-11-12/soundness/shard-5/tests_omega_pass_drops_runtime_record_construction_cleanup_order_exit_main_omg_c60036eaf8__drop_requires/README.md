Partial construction disposes only established fields, in reverse establishment
order. Complete construction invokes the containing hook before its children.
Replacing one field disposes that old field once without retiring its parent.
A moved field and its residual source retain independent declaring-owner order
even when they share backing: later C, moved A, then residual B, exactly once.
Both execution engines must emit `AB|XCBA|BDA|CAB` followed by a newline and exit 0.

From the repository root, prepare the ordinary package review, accept the named
Console output/termination and external-supply decisions, then resume it:

```sh
omega update --project tests/omega/pass/drops/runtime_record_construction_cleanup_order_exit --target macos_arm64
omega update --resume --project tests/omega/pass/drops/runtime_record_construction_cleanup_order_exit
omega run --both tests/omega/pass/drops/runtime_record_construction_cleanup_order_exit/main.omg
```

# Shared whole-view result

A helper returns a whole shared view of its caller's byte array. The caller
passes that returned view to another helper, which reads the original backing.
Native execution and checked interpretation must both exit 70.

After preparing the project's package review for the host, run:

```sh
omega run --both tests/omega/pass/slices/runtime_shared_whole_view_return_exit/main.omg
```

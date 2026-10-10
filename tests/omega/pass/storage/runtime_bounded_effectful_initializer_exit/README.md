The mutable bounded local receives the completed result of an ordinary effectful
call. The call returns 3 within its declared range and increments the counter
once. Native and Terminal execution both check that result and once-only effect.

Run from the repository root:

```sh
omega run tests/omega/pass/storage/runtime_bounded_effectful_initializer_exit/main.omg --both
```

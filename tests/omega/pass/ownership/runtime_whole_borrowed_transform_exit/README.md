# Transform and restore a whole borrowed referent

```sh
omega run --both tests/omega/pass/ownership/runtime_whole_borrowed_transform_exit/main.omg
```

Expected native and Terminal exit: 42. The helper consumes the whole linear
referent, then returns a transformed value to its original storage. Both a fresh
value and an identity result restore the root. The caller observes each update
and explicitly consumes the final value once.

# Owned primitive-array boundary arguments

Run from the repository root:

```sh
omega run --both tests/omega/pass/providers/boundary_primitive_array_argument_exit/main.omg
```

After ordinary local package review, native and checked interpretation must
both exit 70. The selected provider receives scalar 11, array [30, 40, 50, 60],
and scalar 22. Evaluating that last operand writes 99 into the caller's first
array element; it cannot alter the array already captured for the provider.
The caller observes the write and the untouched second element after return.
All four hosted targets bind the same entry and checked provider.

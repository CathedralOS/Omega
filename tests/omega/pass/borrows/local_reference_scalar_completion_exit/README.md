# End local reference custody after scalar completion

After ordinary package review:

```sh
omega run --both tests/omega/pass/borrows/local_reference_scalar_completion_exit/main.omg
```

Both engines exit 70. The helper observes the local record through a shared
reference as its final scalar expression, then releases that carrier before
disposing its original backing. The scalar result survives both dispositions.

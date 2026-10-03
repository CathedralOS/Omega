# Foreign generic case identity

`Optional<First>` and `Optional<Second>` must retain distinct selected case
owners while sharing the original generic declaration. Neither helper borrows
a payload or passes one to a state, isolating this from successor-loan lowering.

```sh
omega --check tests/omega/pending/generics/foreign_case_instance_identity/main.omg
```

At `d11f252e64a`, checking reports that case membership needs the exact
declaring data type, followed by a payload-classifier-as-constructor error.
Expected: acceptance, with `main()` evaluating to true. SQUALR-HEADLESS retains
this dependency for restoring multiple foreign `Optional<T>` APIs.

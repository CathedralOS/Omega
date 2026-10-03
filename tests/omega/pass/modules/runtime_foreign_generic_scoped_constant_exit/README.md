# Foreign generic scoped constant

`values::settings::Holder` owns `VALUE`; its generic declaration identity is
separate from the closed `Holder<u32>` carrier of the constant value. The caller
reads `VALUE.value`, observes 7 and exits 0. Stdout is empty. Both the native
product and checked interpreter must return 0.

From the repository root, using the built compiler:

```sh
omega update --project tests/omega/pass/modules/runtime_foreign_generic_scoped_constant_exit --target linux_x86_64
omega run --both tests/omega/pass/modules/runtime_foreign_generic_scoped_constant_exit/main.omg
```

Use the matching host target for package preparation on another platform.
The sibling `fail/modules/generic_scoped_constant_private_owner` and
`generic_scoped_constant_wrong_constructor` fixtures reject a private foreign
scope owner and a different nominal initializer carrier respectively.

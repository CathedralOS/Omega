# Selected array extents in generic fields

Run from the repository root:

```sh
mbx run -p omega -- run tests/omega/pass/generics/runtime_generic_selected_extent_fields_exit/main.omg
```

The authored exit is 0. An array equation infers `Fixed`'s element type before
its fields substitute that element under qualified and bare selected constant
extents. `Mixed` separately uses a selected fixed extent and an explicit const
binder. Global `Count` is 4, while the template application supplies 2; the
program observes the last written elements and verifies the binder-sized array
has length 2.

The bounded public corpus route checks native and admitted Terminal execution:

```sh
python tools/corpus_gate.py --both --native-target linux_x86_64 --filter runtime_generic_selected_extent_fields_exit
```

Three neighboring refusal fixtures preserve Boolean-carrier rejection and bare
or qualified type-binder shadowing. Specialization must retain authored extent
identifiers for source selection and constant-origin custody.

# Generic constructor binder identity

Run the authored program through the product CLI:

```sh
mbx run -p omega -- run tests/omega/pass/generics/runtime_generic_qualified_constructor_shadow_exit/main.omg
```

The portable corpus route supplies the authored entry binding and checks native
and admitted Terminal Psi execution against exit 0:

```sh
OMEGA_CORPUS_JOBS=2 OMEGA_CORPUS_FIXTURE_SECS=120 python tools/corpus_gate.py --both --native-target linux_x86_64 --filter runtime_generic_qualified_constructor_shadow_exit
```

The `Box` type binder does not shadow the qualified `shapes::Box` declaration.
Nested constructor selection, array-equation inference and both field writes
must retain their independent identities.

The sibling `generic_constructor_shadowed_type_binder` and
`generic_constructor_open_type_binder` refusals pin nested closed specialization
and unspecialized declaration checking. An ordinary type binder cannot become
a generic constructor by sharing a global declaration's spelling. Sources,
entry bindings and exit/refusal oracles were frozen before the compiler repair.

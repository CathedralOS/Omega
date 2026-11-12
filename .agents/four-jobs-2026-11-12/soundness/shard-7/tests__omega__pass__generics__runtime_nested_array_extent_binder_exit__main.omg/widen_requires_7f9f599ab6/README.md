# Declared generic binder kinds

Run the authored program through the product CLI:

```sh
mbx run -p omega -- run tests/omega/pass/generics/runtime_nested_array_extent_binder_exit/main.omg
```

The corpus route supplies the authored entry and requires native and admitted
Terminal Psi agreement at exit 0:

```sh
OMEGA_CORPUS_JOBS=2 OMEGA_CORPUS_FIXTURE_SECS=120 python tools/corpus_gate.py --both --native-target linux_x86_64 --filter runtime_nested_array_extent_binder_exit
```

Ordinary type binders supply the payload types. Const Count supplies array
lengths inferred as 2 and 3 despite a same-named global constant with value 4.
The independent qualified constant remains length 4, and the qualified
`shapes::Count` type remains valid beside the Count value binder.

The sibling `const_binder_used_as_field_type` and
`value_binder_used_as_nested_type` controls reject value binders used as types
even in uninstantiated declarations. Sources, entry bindings and exit/refusal
oracles were frozen before the compiler repair.

# Composite generic field arguments

Run through the product CLI:

```sh
mbx run -p omega -- run tests/omega/pass/generics/runtime_generic_composite_field_arguments_exit/main.omg
```

The portable corpus route compiles the reviewed authored build declaration and
runs the same program natively and through admitted Terminal Psi:

```sh
OMEGA_CORPUS_JOBS=2 OMEGA_CORPUS_FIXTURE_SECS=120 python tools/corpus_gate.py --both --native-target linux_x86_64 --filter runtime_generic_composite_field_arguments_exit
```

`Nested<[u64; 4]>` and `Nested<[bool; 2]>` infer Element and Count from the array
equation. Recursive field argument substitution closes `Holder<Box<[Element;
Count]>>`; scalar writes/readbacks and both array lengths must agree at exit 0.
`Box<Payload>` retains the existing named substitution neighbor.

The sibling `generic_composite_field_explicit_conflict`,
`generic_composite_field_type_extent` and `generic_composite_field_binder_head`
refusals pin explicit Count consistency, extent kind, and a traversed constructor
head's template-binder priority. No runtime inference chooses array layout.
Authored source and exit/refusal oracles were frozen before the compiler repair.

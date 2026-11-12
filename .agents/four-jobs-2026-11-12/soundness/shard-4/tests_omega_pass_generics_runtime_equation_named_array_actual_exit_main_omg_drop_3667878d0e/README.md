# Named actual array extents in structural equations

Run from the repository root:

```sh
mbx run -p omega -- run tests/omega/pass/generics/runtime_equation_named_array_actual_exit/main.omg
```

The authored exit is 0. Data and machine equations infer element types and
array lengths from source-selected constant declarations. A nested array bound
as the whole element type also retains its named extent through instance
identity, layout and execution. The global `Count` has the same spelling as a
template binder; actual-side selection must retain its source scope.

The bounded public corpus route checks native and admitted Terminal execution:

```sh
python tools/corpus_gate.py --both --native-target linux_x86_64 --filter runtime_equation_named_array_actual_exit
```

Neighboring refusal fixtures pin conflicting extents, explicit argument
conflicts, a Boolean extent carrier and an open caller binder. They retain
their authored diagnostic expectations.

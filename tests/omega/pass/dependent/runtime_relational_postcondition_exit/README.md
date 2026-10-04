# Relational out-parameter bounds

The provider writes an offset and ceiling through separate mutable arguments.
Its contract equates the two and bounds the ceiling. The caller transports the
offset across a state edge, forms a four-byte view, and observes byte value 70.
Provider parameter names deliberately differ from the requirement's names.

Product command, after ordinary package preparation:

```sh
omega run tests/omega/pass/dependent/runtime_relational_postcondition_exit/main.omg
```

Hermetic native regression command:

```sh
python3 tools/corpus_gate.py --native --filter dependent/runtime_relational_postcondition_exit
```

The authored native exit is 70. Checking proves both the provider's own and
inherited guarantees and the caller's byte-view bound. Native production remains
blocked at stage 04's `call operation: boundary contracts`: the boundary-call
planner admits parameter qualifications but not this Boolean postcondition.
Do not replace the call or contract with caller-side assertions to bypass that
missing transport.

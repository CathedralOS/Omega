# Constructor observation

The program constructs a bounded record after a guarded transition beside an
unrelated mutable context, then constructs another from a reassigned local.
Both values must be 5; success exits 70, failure exits 1.

From the repository root, after the project's ordinary package review:

```sh
omega run tests/omega/pass/data/runtime_constructor_observation_exit/main.omg
```

The repository-native gate supplies its explicit fixture package acceptance:

```sh
python3 tools/corpus_gate.py --native --filter data/runtime_constructor_observation_exit
```

This fixture does not authorize production packages or carry accepted policy.

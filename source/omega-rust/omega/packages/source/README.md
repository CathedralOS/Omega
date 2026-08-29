# Package Source

This folder owns the hostile-input side of package acquisition.

```text
source/
├── README.md      this entrance
├── acquisition/   immutable identity, local/Git snapshots, and custody
└── execution/     confined native processes used by acquisition
```

`acquisition/` owns successful non-admitting source custody. `execution/` may
realize compiler-selected resolver operations. Neither can
choose package identity, dependency policy, review outcomes, or admission.

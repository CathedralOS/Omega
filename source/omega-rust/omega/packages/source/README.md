# Package Source

This folder owns the hostile-input side of package acquisition.

```text
source/
├── README.md    this entrance
└── execution/   confined native processes used by source acquisition
```

Immutable source identity, local snapshots, and Git acquisition currently live
in `../manager/src/source/`. They are the next extraction: this entrance exists
to make that ownership destination explicit, not to pretend the split is
already complete.

`execution/` may realize compiler-selected resolver operations. It cannot
choose package identity, dependency policy, review outcomes, or admission.

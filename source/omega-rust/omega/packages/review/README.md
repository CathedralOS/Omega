# Package Review

This folder owns review surfaces. It does not prove that a human or model
performed an audit and cannot admit a package by itself.

```text
review/
├── README.md   this entrance
├── evidence/   checked compiler state to deterministic inert evidence
└── advisory/   optional model-facing review protocol and invocation
```

`evidence/` is authoritative only about what the compiler checked. `advisory/`
can recommend investigation but cannot suppress deterministic warnings or
change acceptance. Candidate comparison and root-owned admission policy still
live in `../manager/src/review/` until their transaction boundary is complete.

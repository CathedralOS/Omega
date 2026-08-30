# Omega Package Review

Package review support lives here; the accepting workflow remains in
[`manager`](../manager/README.md).

```text
review/
├── README.md       this entrance
├── evidence/       compiler-owned checked state projected into inert evidence
└── advisory/       optional model-facing recommendation tooling
```

`evidence` is deterministic compiler machinery. It can describe a candidate
but cannot admit one. `advisory` consumes bounded manager-rendered review input
and can recommend an audit, but its availability and response cannot alter
deterministic policy or mutate project state.

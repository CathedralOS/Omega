# Package Review

This branch owns review material after successful checked compilation.
`evidence/` records deterministic compiler-issued facts. `advisory/` may ask a
model for audit guidance, but its availability and output cannot change package
acceptance.

```text
review/
├── evidence/   checked compiler state -> inert, canonically encoded facts
└── advisory/   optional model-facing review protocol and invocation
```

The manager consumes evidence and owns policy. Advisory depends on the manager
only to consume deterministic review input; the manager deliberately does not
depend on advisory.

Return to the [package subsystem map](../README.md).

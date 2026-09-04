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

Under the ratified install/update model, compiler-derived reachability, unsafe
API, and assumption rows inform review and the lock's accepted baseline and
decisions. The project trusts whoever lands the lock. These rows do not certify
lock acceptance; existing promotion/replay layers are implementation to
simplify. Actual compiler proof/reach and native artifact checks remain.

Return to the [package subsystem map](../README.md).

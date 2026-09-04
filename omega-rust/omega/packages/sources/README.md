# Package Sources

This branch owns hostile source handling. Follow `acquisition/` for source
identity, fetch, object verification, snapshots, and retained custody. Follow
`execution/` only for the native child-process boundary used by acquisition.

```text
sources/
├── acquisition/   hostile local and Git input -> immutable source custody
└── execution/     bounded execution of compiler-selected resolver tools
```

Dependency direction is one way: acquisition may request resolver execution;
execution knows nothing about dependency graphs, package review, or admission.
Neither crate accepts a package.

Return to the [package subsystem map](../README.md).

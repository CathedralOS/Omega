# Package Sources

This crate owns hostile source handling: source identity, fetch, object
verification, snapshots, and retained custody. The independent sibling
[`resolver-execution/`](../resolver-execution/README.md) owns the native
child-process boundary used by acquisition.

Dependency direction is one way: acquisition may request resolver execution;
execution knows nothing about dependency graphs, package review, or admission.
Neither crate accepts a package.

Return to the [package subsystem map](../overview.md).

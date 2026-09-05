# Remote fixture build declarations

Local fixtures use sibling Path dependencies so ordinary tests need no network.
Standalone remote repositories cannot use those paths. This directory records
their exact build declarations with pinned Git dependencies:

```text
package-remotes/
├── file-journal/build.omg    filesystem fixture -> pinned host-services
├── process-exit/build.omg    process fixture -> pinned host-services
├── remote-journal/build.omg  combined reach -> pinned host-services
└── graph-workbench/build.omg graph -> pinned arithmetic-kernels and file-journal
```

The corresponding local fixture supplies `README.md` and `main.omg` unchanged.
Remote-content tests construct that exact three-file expectation and compare
the complete fetched content, not just its package name. `host-services` matches
its local five-file fixture without overrides.

[Remote pins](../packages/REMOTE_PINS.md) record immutable revisions and the
filesystem update baseline. These declarations request SSH for the dependency;
an HTTPS root locator alone would not make the whole closure an HTTPS test.

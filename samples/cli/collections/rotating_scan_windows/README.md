# Rotating scan windows

An ordinary helper replaces one descriptor in a borrowed collection of scan
windows. The caller observes the replacement's two-cell extent and full-width
tags, retains its neighboring window, and writes to the displaced backing after
its old descriptor is retired. Expected exit: 0.

```sh
omega update --project samples/cli/collections/rotating_scan_windows --target macos_arm64
omega run --both samples/cli/collections/rotating_scan_windows/main.omg
```

The helper uses a fixed element of a runtime-length view. Its checked bound
comes from the view's own extent, not its caller's fixed-array type. Runtime
selection of the replacement destination remains a separate compiler frontier;
this program makes no concurrency or allocator claim.

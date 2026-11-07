# Selected scan visitor

A runtime-selected visitor writes positive input values into caller-owned output
storage. The exact visitor preserves values; the tagged visitor flips the low
bit. Full output stops the scan, and empty input leaves its count unchanged.
The authored oracle is exit 0 in both native and Terminal execution.

From the repository root, with the development compiler on `PATH`:

```sh
omega update --project samples/cli/collections/selected_scan_visitor --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/selected_scan_visitor/main.omg
```

The original compiler failure treated the stored element descriptor as inline
primitive storage. Element writes now retain the selected descriptor separately
from the leaf within its backing, preserving both its access permission and
the exact window used to establish the bound. The program runs with native and
Terminal exit 0 on macOS ARM64.

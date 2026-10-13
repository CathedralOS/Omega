# Runtime-selected scan with borrowed results

Two named implementations choose the first value at or above, or below, a threshold.
The caller forwards and rebinds a `dyn Scan<i32>` selector independently of
the input slice. Results use the bundled `Optional<&i32>` and still denote
the input's original backing after the selector changes. Empty and no-match
scans return `None`; a subwindow result denotes that window's matching element.

After ordinary package preparation:

```sh
omega run --both samples/cli/collections/runtime_selected_scan/main.omg
```

Expected native and Terminal exit: 0. The program checks each result and traps
on a mismatch. Static selection, copied payloads, or private-implementation
origin inference do not satisfy this public dynamic-call contract.

The scan runs with native and Terminal exit 0. Its public lifetime relation
is instantiated from the requirement signature; private implementation bodies
cannot supply that relation. Each returned descriptor must remain inside the
original input region, including the caller's selected subwindow. The compiler
corpus pins this behavior in `runtime_selected_slice_reference_exit`, alongside
the conflicting-mutation control `runtime_selected_reference_holds_input`.

# Selected record update

Select either of two caller-owned records at runtime, return its mutable
reference, then read and update it through a helper. Both branches must update
the original record and leave the third record untouched. Expected native and
Terminal exit: 0.

```sh
omega update --project samples/cli/collections/selected_record_update --target macos_arm64 --offline --dangerously-accept-all
omega run --both samples/cli/collections/selected_record_update/main.omg
```

The local slice deliberately retains its own descriptor before returning a
record reference. Both engines must follow that descriptor's original backing,
not copy the record or choose a static branch. The third record detects writes
outside the selected storage.

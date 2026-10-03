# Runtime record-array recast stores

```sh
omega run --both tests/omega/pass/recast/runtime_record_array_view_mutable_write_exit/main.omg
```

The program writes nested record fields through a byte-backed `Packet` view.
Both its backing offset and inner array selector are runtime values. It checks
that the writes changed the original bytes and that subsequent view reads
reconstruct those values, returning 70 in native execution and the checked
interpreter on macOS ARM64.

Previously the public command stopped in stage 05 with `record view store field
has no single source leaf`. Typed-record leaf replay now leaves byte-backed
views to their existing captured-loan route. Shared normalized geometry retains
each runtime array coordinate, stride and declared extent. Each coordinate
proves its dimension bound independently from each emitted byte's backing bound.
The loan's outer offset is captured at formation in private primitive storage;
subsequent field writes cannot move that loan.

Run ordinary `omega update --project <fixture-directory> --target macos_arm64`
and accept the local package review before the first run. Package review is
separate from compilation. The authored runtime oracle is exit 70.

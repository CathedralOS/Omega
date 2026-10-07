Run on macOS AArch64, after reviewing this project's two exact foreign supplies:

```sh
cd tests/omega/pass/host/runtime_descriptor_output_streams_exit
omega update
# Review the generated decisions, then: omega update --resume
omega run --both main.omg < /dev/null
```

Both engines return zero. Each independently produces `out\nok\n` on stdout
and `err\n` on stderr; the corpus gate checks both authored stream oracles.
The CLI currently compares exit presentation only.

The byte-view declaration supplies pointer and length as two native words.
The scalar-loan declaration supplies one pointer and an explicit count bounded
by its four-byte referent. Slots share stream ordering, not output buffers per
slot. The companion `runtime_descriptor_direct_array_exit` exercises direct
array-to-view construction and a projected readonly scalar loan without the
forwarding helper.

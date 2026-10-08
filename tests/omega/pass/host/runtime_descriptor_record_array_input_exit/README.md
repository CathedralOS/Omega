Run on macOS AArch64 through the public CLI after package acceptance:

```sh
cd tests/omega/pass/host/runtime_descriptor_record_array_input_exit
omega update
# Review pending decisions, then: omega update --resume
omega run --both main.omg < input.txt
```

From the repository root, the independent corpus route is:

```sh
python3 tools/corpus_gate.py --both --filter host/runtime_descriptor_record_array_input_exit
```

The native and Terminal engines both return zero and print `<abc` followed by
LF, then `!` and LF. The record owns an initialized seven-byte array and a
separate marker. Empty, forwarded short-read and EOF calls lend array windows;
another EOF call lends the whole array mutably, and the final output call lends
that field as a shared slice. The caller
observes the original bytes, untouched prefix/suffix and unchanged marker.
The record's custody is not the selected array's presentation type.

The calling policy uses canonical std vocabulary and explicitly declares the
pointer/count ABI. `Calling<ByteViewCallingPolicy<u8>>` closes a generic named
map whose reference row selects the same non-generic plan body; the map argument
does not become an extra runtime operand. Full std package review remains a
separate acceptance requirement; the corpus does not supply it. See the sibling
[mutable-view fixture](../runtime_descriptor_mutable_view_input_exit/README.md)
for the distinction between reviewed acceptance and disposable test-only bulk
acceptance. Neither fixture publishes its generated lock.

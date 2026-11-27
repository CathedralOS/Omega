# byte_checksum

Read bytes through EOF; exit with the Wrapping byte sum plus count.

This `CANONICALIZE-SCALAR-RANGE-CONTRACTS` customer preserves the original
`7a9a7b8287e6` `add_byte.value: i32 [0..=255]` as an ordinary `requires`
clause. Matching the standard library's `ByteRead::Byte` establishes its
containing-data predicate and supplies both bounds on the exact payload
snapshot before the helper call. `Wrapping` on sums/counts is separate from
this byte-input obligation. EOF and a zero byte remain distinct.

`cases.json` pins 6 finite raw-byte input/output cases, including every
authored `INPUT-GRID` vector and byte-boundary checks. Hex strings represent
bytes, without newline insertion or text encoding. Each case records the exact
exit, stdout and stderr. The default `input.txt` is empty and exits zero.

After ordinary package review, run the default case from the repository root:

```sh
omega run samples/cli/text/byte_checksum/main.omg < samples/cli/text/byte_checksum/input.txt
```

To compare the published native program with the interpreter, run
`mbx xtask sample-gate --filter samples/cli/text/byte_checksum` from the repository root.

The Build file retains all four hosted entry bindings. The source-migration
qualification requires native and Terminal execution to agree on every case;
matching-host execution is needed for other targets.

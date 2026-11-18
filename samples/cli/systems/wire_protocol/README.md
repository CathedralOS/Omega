# Wire Protocol

The sample encodes a sensor packet, decodes it, and checks the bytes, the fields,
and both byte counts. The native program and the Psi interpreter each exit
with status 70 after printing
`PASS: 13 bytes round-tripped with every field intact (exit 70)`.

```sh
mbx run -p omega -- run --both samples/cli/systems/wire_protocol/main.omg < /dev/null
```

The packet carries room 300, samples `[150, -2]` in a fixed vector with capacity
4, and depth −64. Its 13 bytes are `00 00 03 00 AC 02 01 03 AC 02 03 02 7F`.
The format starts with era zero, followed by the nested room header, the packed
samples and the zigzag-encoded depth. [main.omg](main.omg) checks each byte before
reporting success.

`authored_compact_binary_plan` takes its state explicitly. It matches each
field's type to its wire representation. The recursive `fill` and `place`
states use the work bound declared on `fill`.

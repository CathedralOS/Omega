# Whole construction request

The construction adapter collects package rows, dependency rows and all source
units consumed by the Omega construction application. The experiment gives its
exact bytes to the complete Epsilon bootstrap compiler and construction entry.
The evaluator receives both the Epsilon source and the guest stdin in one sealed
EEREQ packet. [Construction input format](../../../source/application/construction/README.md)

## Reproduce the inputs

Build the native Epsilon evaluator using the Linux test launcher and canonical
Delta request. The resulting directory contains the native Gamma compiler,
the Delta compiler and its emitted Epsilon evaluator Gamma source in
`producer.output`, and the native evaluator in `successor`.
[Linux construction](../linux_x86_64/README.md#compile-gamma-source-emitted-by-the-supplied-program)

From the repository root, set `native` to that directory and choose a new
output directory. The constructor writes `product.bin` and its source manifest.
The framing script packs the ordered compiler closure, appends its construction
entry, and seals the source and stdin with the evaluator closure identity.

```sh
native=/tmp/omega-epsilon
work=/tmp/omega-whole-request
python3 tests/bootstrap/independent/whole_request.py "$work" --receipt "$native/producer.output" --entry-control
timeout 480s "$native/successor" < "$work/whole.eereq" > "$work/stock.eeout"
```

The optional `--receipt` checks the canonical Gamma receipt identity, finds
the exact published Delta adapter member and writes a private Gamma source.
Compile it with the same seed-built native compiler and replay the same packet.

```sh
"$native/gamma-compiler" < "$work/epsilon-8m.gamma" > "$work/epsilon-8m"
chmod +x "$work/epsilon-8m"
timeout 480s "$work/epsilon-8m" < "$work/whole.eereq" > "$work/wide.eeout"
```

The optional `--entry-control` separately encodes the same package and dependency
rows with only the original entry source unit. Replay `entry-unit.eereq` in
place of `whole.eereq` for that control.

## Measured sources and limits

At `9d6b86e82d0` the packet contained 852,096 bytes of Epsilon source and
3,955,617 bytes of product input covering seven packages and 304 source units.
The 52-byte EEREQ header brought the total to 4,807,765 bytes, SHA-256
`dbcf638d9ec5e1b6a968129b031aead6aad3cfc9a08291cfb48aaa3c43d48b59`.

The published native evaluator exited 253 with empty stdout and stderr in
5.611 seconds. The Alpha-hosted evaluator returned the same status and empty
output in 10.27 seconds. The generated Gamma entry returns that status before invoking the Epsilon evaluator when
stdin exceeds 4,194,304 bytes. The private input check allows 8,388,608 bytes.
The 4 MiB output check and the enclosing Gamma runtime’s 131 MiB sealed-input
limit both retain their original values.
[Generated entry](../../../bootstrap/delta/to-gamma/support/adapter.gamma)
and [Gamma runtime](../../../bootstrap/gamma/compiler/compiler.gamma)

The measured private build also regenerated the receipt through Delta. It used
the widened adapter and replaced its rolling checksum in the Delta producer's
support-member table. The resulting receipt matched the script's
`epsilon-8m.gamma` byte for byte. The 25,223,480-byte native executable differs from the published evaluator at just offset 25,214,183
(zero-based), where the bound immediate changes from `40` to `80` in hex.
Each boundary test padded a zero-length guest and stdin with trailing bytes. Once the adapter admits
that input, the evaluator reports the trailing data in a 40-byte diagnostic.

| Input provision | Input at the limit | Input one byte above the limit |
| --- | --- | --- |
| Published 4 MiB | 40-byte diagnostic, process 0 | Process 253, empty stdout |
| Private 8 MiB | 40-byte diagnostic, process 0 | Process 253, empty stdout |

The private evaluator also returned the 40-byte diagnostic at 4 MiB + 1 byte.

On the exact full request the 8 MiB evaluator was still running after 480
seconds. The wall-clock watchdog terminated it at 480.739 seconds, with
2,656,232 KiB peak RSS and empty stdout and stderr. The 12 GiB resident
limit and 4 GiB cgroup reserve did not trigger. The run produced no compiler
outcome or Alpha tape.

The entry-unit control retained all seven packages and seven edges, with the
one unchanged 14,509-byte `source/application/construction/main.omg` unit.
The 867,062-byte EEREQ had SHA-256
`b75989e139d6a7cd6b4f43b30a9ef2f0b67c4622fc2a33cca8941ca48f0424e0`.
The same wall-clock watchdog stopped it at 480.299 seconds, with 3,141,308 KiB
peak RSS, empty stdout and empty stderr. A source coordinate remains unknown
for both the full request and this reduced control.

Changing only the package count from seven to 65 preserves the full input
length. The private evaluator ran that control in 59.278 seconds, returning
process status zero and `0003000000`. The guest constructor had read its
header and returned exit 3 for malformed framing.

| Artifact | SHA-256 |
| --- | --- |
| Canonical evaluator Gamma receipt | `1d3a700d970acda04ee4699c19bf03a3654eb481a46755399d0daf759fe030af` |
| Private evaluator Gamma receipt | `e3854c5f93a5368980f5cfdc86aa64516c5de12a7355247a0fa72acf11f0bcc7` |
| Native Gamma compiler | `32c681f83f4ceafb775de7ce5e24a4fa00df8417daaf0f50ca5905ae9f760394` |
| Published native Epsilon evaluator | `80226b25de03c6608638a1c6a35f4d0fcbc23a3dd1445aa474fb9a34194907f8` |
| Private native Epsilon evaluator | `36c91efef706b00107a5f2127d5c6a698ffd850f9826f0b0dd18e7d11e0ed7b8` |

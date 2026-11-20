# Whole construction request

The construction adapter collects package rows, dependency rows and all source
units consumed by the Omega construction application. The experiment gives its
exact bytes to the complete Epsilon bootstrap compiler and construction entry.
The evaluator receives both the Epsilon source and the guest stdin in one sealed
EEREQ packet. [Construction input format](../../../source/application/construction/README.md)

## Build from the seed

The same complete Epsilon source can run in the evaluator or go through the
Epsilon-to-Delta compiler. From the repository root, prepare both requests in
a fresh directory. The script packs the source manifests with the repository's
built `xtask`. The Linux launcher constructs the executables from Alpha and Beta.

```sh
work=/tmp/omega-whole-request
python3 tests/bootstrap/independent/whole_request.py "$work" --entry-control
tests/bootstrap/linux_x86_64/bootstrap --delta . "$work/compiled" "$work/compiler.dcreq"
```

The compiled route first turns the Epsilon-written bootstrap compiler and its
construction entry into a Delta module. Frame a successful result with the
runtime selected by the Epsilon compiler, then pass that program to Delta.
These steps keep the intermediate sources available for inspection. The last
command runs only after both compilers have produced their executables.

```sh
set -e
"$work/compiled/successor" < "$work/whole.epsilon" > "$work/module.out"
python3 tests/bootstrap/independent/whole_request.py "$work" --frame-result "$work/module.out"
"$work/compiled/program" < "$work/omega.dcreq" > "$work/omega.gamma"
"$work/compiled/gamma-compiler" < "$work/omega.gamma" > "$work/omega"
chmod +x "$work/omega"
"$work/omega" < "$work/product.bin" > "$work/compiled.eeout"
```

To run the same checked program directly in the Delta interpreter, give it the
`omega.delta` produced by `--frame-result` and the desired product input.
The runner saves the guest's result in `guest.output`.

```sh
python3 tests/bootstrap/interpreted/delta/interpreter.py "$work/direct" \
    --host linux --profile construction --runner "$work/compiled/gamma-compiler" \
    --source "$work/omega.delta" --stdin "$work/product.bin" --seconds 1200
```

The interpreted route passes the Epsilon source and the product input together
as one sealed request. Reuse the seed-built Delta and Gamma compilers from
above to build the interpreter.

```sh
set -e
"$work/compiled/program" < "$work/evaluator.dcreq" > "$work/evaluator.gamma"
"$work/compiled/gamma-compiler" < "$work/evaluator.gamma" > "$work/evaluator"
chmod +x "$work/evaluator"
"$work/evaluator" < "$work/whole.eereq" > "$work/product.eeout"
```

Both executables emit the Epsilon guest's outcome before its payload. A successful
constructor returns five zero bytes followed by the Alpha tape.
The optional `--entry-control` also frames the original package and dependency
rows with just the original entry unit in `entry-unit.bin` and seals the same
bytes for the evaluator in `entry-unit.eereq`. Both construction
requests select Delta profile 2, which admits the combined source and product
input and can publish a generated module larger than 4 MiB.

## Whole compiler result

The indexed Epsilon compiler compiled all 852,096 bytes of the bootstrap
compiler and its construction entry. With Delta's construction profile, the
Linux executable and the Alpha seed running the 4,324,897-byte compiler tape
each returned status 0, the `compiled-storage` marker and the same
18,269,324-byte Delta module. Adding the declared storage runtime and entry
gave Delta an 18,365,364-byte program. Delta checked that complete source and
published 26,398,215 bytes of Gamma with process status 0 and empty stderr.
The same whole Epsilon source reached the 4 MiB output guard with status 254
on the conformance profile.
[Indexed lowering](../../../wiki/development/bootstrap/native.md)

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Epsilon source with construction entry | 852,096 | `01565beaf9f51478a156fa9b9ca997f5808b06ef39db66a41e4baf644ff59225` |
| Epsilon compiler Alpha tape | 4,324,897 | `7154f042d57d71bdc293bac57871d4c3bebbac7825abdea8d43e42d7a04aded1` |
| Epsilon compiler output, including marker | 18,269,341 | `60d0fce9fa52af3464e95c5d691882369e6d7417cb386258f41cde55b207a0f5` |
| Delta source with runtime and entry | 18,365,364 | `f662e1a8007b819bd02fc1571d6314e49c1fac9785247ed58e278a16ca46dcab` |
| Gamma emitted by Delta | 26,398,215 | `2c9641632eb613696428a96903e02d126a6d7fc5fdbd78c9f08f3d3469f29329` |

The native Gamma compiler rebuilt to the same 226,610-byte executable on two
successive self-compilations from the cold Linux launcher. Using it to
reconstruct Delta and the Epsilon compiler reproduced the two whole-program
outputs in the table. The Beta-built Gamma interpreter received the complete
Gamma source plus the valid 829-byte wire-package input. Its 480-second
wall limit expired with empty output.

The construction-profile evaluator also accepted the 4,807,765-byte packet
with the package count changed to 65. The guest returned exit 3 for malformed
framing (`00 03 00 00 00`) with outer process status 0.

## Earlier conformance and private-adapter measurements

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
[Generated entry](../../../bootstrap/delta/compiler/support/adapter.gamma)
and [Gamma runtime](../../../bootstrap/gamma/compiler/compiler.gamma)

To reproduce the private adapter used for these measurements, add
`--receipt /path/to/profile-1/evaluator.gamma` when preparing a new directory.
The script verifies that receipt and writes `epsilon-8m.gamma`.

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

## Product wire package through the construction entry

The ordinary constructor compiled the 323-byte product wire decoder as a separate
package. Its caller imports `wire::u32_from_le_bytes` and checks zero, mixed
bytes and all bits set. The two source units and the dependency edge go through
the same row encoder as the full compiler product.
[Wire decoder](../../../source/foundation/wire/codec.omg) and [caller checks](../conformance/omega/wire_word.omg)

```sh
python3 tests/bootstrap/independent/product_wire.py /tmp/omega-product-wire --evaluator "$native/successor"
```

Omitting `--evaluator` writes `linked.stdin`, `missing-edge.stdin` and
`trailing-byte.stdin` for direct replay by a compiled constructor.

At `e45acdff0a9` the stock Linux-native Epsilon evaluator produced 1,607 bytes
of Alpha tape. The Alpha seed ran it to exit 42 with empty output. The tape's
SHA-256 was `f5ac1bf6ddc9158a4164a644c9342f73cc6c6698c93773e32dfe035176e3a371`.
Removing the dependency from the same two source units returned Exit 1 and
coordinate 11, the start of the caller's import. An extra byte after the
linked input returned Exit 3 with empty guest output.

## Source staging for the entry-unit control

Delete the final byte of `entry-unit.eereq` and subtract one from the
four-byte little-endian stdin length at packet offset 16. This preserves
all seven packages, seven dependencies, the unit row and the declared
14,678-byte arena. The stock evaluator reached the missing byte after
staging 14,677 bytes and returned the constructor's framing failure
(`0003000000`) in 95.4 seconds, with 552,376 KiB peak RSS. The exact packet
SHA-256 was `581349db3ec656406f4e85fab26a2fc21e38091aa0a36c9df5ea33e6c7b70436`.

The independent Epsilon model from `8092cfd9c0a5` ran the complete, unchanged
entry-unit packet and returned `0001000000a9000000`. This is Exit 1 at packed
byte 169, the first `use` in the unit. Only the construction package supplies
a source unit in this control. The earlier 8 MiB evaluator run on that packet
stopped at its 480-second deadline.

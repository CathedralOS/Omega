# Conservative CKIR12 backend implementation note

[`CKIR12`](OMEGA_BOOTSTRAP_CHECKED_IR_V12.md) inherits CKIR11 and adds one
private shared `&[u8]` type, program-static literal roots, opcodes 22 through
25, and the exact synthetic nonempty-edge block. The shared
`omega-bootstrap-checked-ir-v5-to-elf.alp` backend accepts major 12 without
changing the accepted identities or output paths for majors 4 through 11.

The type-kind-7 representation is backend-private: a 16-byte, 8-aligned frame
object containing an address and length. A zero address with zero length is
the canonical empty descriptor. Opcode 22 initializes that object from the
validated direct-child byte DAG and publishes only its frame address. The
literal bytes occupy the read-only load segment; an empty literal retains one
unobservable zero anchor so its address relocation remains defined.

The conservative x86-64 selections are:

- `SliceNonEmpty`: load the descriptor length, compare it with zero, normalize
  `setne` through `movzx`, and store the Boolean result;
- `SliceHead`: recheck length, branch to the shared `ud2` trap when zero, load
  and zero-extend the first byte, then publish the scalar result; and
- `SliceTailOne`: recheck length, trap when zero, add exactly one to the
  address, subtract exactly one from the length, initialize a fresh private
  descriptor, and publish only that descriptor's frame address.

The backend independently validates the CKIR12 control custody before sizing
or emission: exactly one flagged block, one slice parameter, a unique opcode-23
guarded true-edge predecessor passing that same slice, no false-edge or case
predecessor, only opcode 24/25 operations consuming the parameter, both
operations present, and a final jump to a non-synthetic authored block. The
runtime head/tail checks remain emitted even after that proof; the conservative
artifact does not erase safety checks from static control facts.

`../gates/delta-checked-ir-v12-backend-fixture.py` recognizes the complete
descriptor initialization, nonempty, head, tail, branch, and read-only literal
templates. The former producer-backed wrapper joined deterministic fixtures to
independent one-byte/empty execution meaning, native/self artifact identity,
instruction and schema mutations, resource statuses, empty rejected
publication, and the CKIR11 regression. Replay is suspended until canonical
Delta publication.

The focused positive carrier returns 70 on both paths. The one-byte carrier
takes the synthetic true edge, observes head 70, and produces an empty tail.
The empty carrier takes the authored false bypass, so neither partial operation
executes. This is backend evidence only; it does not define public slice ABI,
mutable views, general pointer arithmetic, or source-language surface rules.

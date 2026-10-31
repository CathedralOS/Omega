# Generic terminated array

Parent: `CANONICALIZE-SCALAR-RANGE-CONTRACTS`; necessary native-binding repair:
`BUILD-OWNED-NATIVE-BINDINGS` in [TASKS.md](../../../../TASKS.md).

A nonempty fixed array retains its last-byte terminator through transport and a generic native string read.

Each place calls the same boundary once with explicit Count and once with Count
inferred from the borrowed field's declared extent. Both calls must observe the
same bytes and result; the existing explicit call and oracle stay intact.

The generic boundary requires `Count > 0` before forming `text[Count - 1]`,
then requires that byte to be zero. Its Build binds the requirement directly
without a forwarding declaration. A four-byte array carries the same fact in
every place. `strlen` must return 3 for `[0x61, 0x62, 0x63, 0]` and 1 for
`[0x61, 0, 0x63, 0]`; both neighbor canaries must remain `0xa5` / `0x5a`.
Earlier NUL bytes are legal. Count is erased, not an extra native argument.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

Each reject changes only the first array's final byte to 1, violating the
carrier's default predicate at construction. The positive match join exercises
both matching and wildcard paths with different expected lengths, so swapping
or collapsing those paths traps. Caller checks observe runtime results; they
do not establish the terminator needed by the boundary invocation.

## Requirements and current evidence

- Establishment: construction checks `Text.bytes[3] == 0`; the generic signature
  forms its index only from its preceding nonempty requirement. An unformed goal
  cannot establish that requirement.
- Identity and lifetime: the borrowed full array retains its extent and original
  bytes. The adjacent prefix/suffix are outside the native string's loan.
- Composition: the nine places transport `Text` without a caller guard repairing
  a missing predicate. The callee consumes the field predicate directly.
- Soundness: each reject breaks construction. Separate focused controls check a
  recursive generic call changing Count to zero and a reversed requirement order.
- Psi lowering: the generic count is static; runtime storage and the full-array
  borrow must survive Terminal lowering. Actual engine acceptance is pending.
- Provider/ABI: Build binds `CText::length` on all four hosted targets, retaining
  exact closed applications and package/native admission. The C ABI has one
  pointer argument, with no runtime Count argument. Runtime qualification is pending.
- Instructions and image emission: all places must execute the stated oracle
  with `omega run --both` on the available host. Other hosted targets are bound
  but have no execution claim here.

The frozen pre-repair producer at `fac4a53` rejects the local generic signature
with `cannot form specification index`. The prototype at `12cfb317` passes that
formation and refuses provider projection with `closed Import application
requires its complete retained source application`. These are current baseline
observations, not passing acceptance. Raw source/producer hashes and commands
are retained in the session's ignored qualification evidence. Full row execution,
reason-matched reject captures, corpus pins, and the calculator's native/Terminal
acceptance remain open.

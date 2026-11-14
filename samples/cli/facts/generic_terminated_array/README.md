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
| local | [local/main.omg](local/main.omg) | [invalid_local/main.omg](invalid_local/main.omg) |
| field | [field/main.omg](field/main.omg) | [invalid_field/main.omg](invalid_field/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [invalid_parameter/main.omg](invalid_parameter/main.omg) |
| call_result | [call_result/main.omg](call_result/main.omg) | [invalid_call_result/main.omg](invalid_call_result/main.omg) |
| match_join | [match_join/main.omg](match_join/main.omg) | [invalid_match_join/main.omg](invalid_match_join/main.omg) |
| const_name | [const_name/main.omg](const_name/main.omg) | [invalid_const_name/main.omg](invalid_const_name/main.omg) |
| other_state | [other_state/main.omg](other_state/main.omg) | [invalid_other_state/main.omg](invalid_other_state/main.omg) |
| other_machine | [other_machine/main.omg](other_machine/main.omg) | [invalid_other_machine/main.omg](invalid_other_machine/main.omg) |
| target_body | [target_body/main.omg](target_body/main.omg) | [invalid_target_body/main.omg](invalid_target_body/main.omg) |

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
  borrow survive Terminal lowering. All nine places execute with native exit 0
  and Terminal exit 0 on macOS arm64. Each checks explicit and inferred `Count`.
- Provider/ABI: Build binds `CText::length` on all four hosted targets, retaining
  exact closed applications and package/native admission. The C ABI has one
  pointer argument, with no runtime Count argument. The available macOS arm64
  host executes both lengths through the real native binding and Terminal provider.
- Instructions and image emission: all places must execute the stated oracle
  with `omega run --both` on the available host. Other hosted targets are bound
  but have no execution claim here.

The frozen pre-repair producer at `fac4a53` rejects the local generic signature
with `cannot form specification index`. The prototype at `12cfb317` passes that
formation and refuses provider projection with `closed Import application
requires its complete retained source application`. These are retained
pre-repair observations. The optimized producer from
`815f96ad1d3899ddb260c996cb3f9c7d31de0a94` (SHA-256
`be05129cf3848392d37738411b704448d09626cea5ee9a96224fec307411d4d6`)
executes all nine positive places with native exit 0 and Terminal exit 0 on
macOS arm64. All nine twins reject with ``data `Text` literal violates the
default domain``. Source-identical positive and negative fixtures are pinned
under `tests/omega/{pass,fail}/facts/generic_terminated_array_through_*`.
The retained `r5-row-acceptance-r2.json` ledger binds source, Build and producer
hashes, actual engine reports, empty output streams and intended refusals.
Other hosts, the calculator's interactive acceptance, and the parent migration
remain open; this row does not establish their completion.

## Read-only calls inside generic functions

A generic function must keep a string's terminator guarantee when it reads a
separate string. This is needed by the standard library on the continuing
compiler migration and Cathedral path (`PROVIDER-SELECTION-AFTER-TERMINAL`).

The [direct forwarding example](probes/generic_forward/main.omg) calls the same
string-length operation through a generic function. The
[second example](probes/generic_forward_after_call/main.omg) first reads an
unrelated string, then forwards the original one. Both retain this row's
four-byte arrays, results 3 and 1, and neighboring canaries. Their invalid twins
remove only the forwarding function's terminator requirement:
[direct](probes/generic_forward/invalid/main.omg) and
[after another call](probes/generic_forward_after_call/invalid/main.omg).

Two additional rejection controls permit an external call to write to the
array, [directly](probes/generic_forward_after_call/invalid_mutation/main.omg)
or [through a returned reference](probes/generic_forward_after_call/invalid_returned_mutation/main.omg).
The old terminator guarantee cannot justify a later string read after either
write. These controls check invalidation; they do not claim that a particular
external implementation actually performs that write.

Before this repair, optimized compiler `2484aa7758a` runs direct forwarding
with native and Terminal exit 0, but rejects the second example at the final
string-length call. Both missing-contract twins and both mutation controls
reject for the missing terminator guarantee. The compiler was treating a
read-only call with a constant generic argument as having unknown writes.
The repaired optimized producer `6271352bf00defae77ffcf708f5f11e55f7daf7a`
(SHA-256 `a6771cee35b9a45ad3b666a8366ed29136dfaf193b996fa5a1f8d1497626bc3c`)
runs the matching corpus fixtures for both probes with native and Terminal
exit 0 on macOS arm64. All four
controls reject for the missing terminator guarantee. An existing generic
statement-call program also passes both engines, and three existing invalid
programs retain their intended argument or write-invalidation refusals.
The six new fixtures have checked outcomes recorded from that producer.
These probes do not extend the historical nine-place execution claim above.

An additional attempted control with an unresolved element type in a returned
mutable reference stops at an earlier typing check. It is excluded from these
passing controls. The storage analysis conservatively retains possible aliasing
for unresolved element types and lengths; no execution claim is made for that
separate mutable-return case. The broader migration and Cathedral boot remain
unfinished.

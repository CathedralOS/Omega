# Reference recast backing footprint

A shared two-byte Pair view starting at byte 1 of live [u8; 4] preserves the original first-byte address and full two-byte loan while its second field reads 37.

The source is always the live mutable array `[11, 23, 37, 41]`. The shared
`&Pair` is always formed by `&bytes[1] as &Pair`; `Pair` has two `u8` fields,
so its referent occupies bytes 1 and 2 at alignment 1. The positive observes
`address_of(&view.first) == address_of(&bytes[1])` and `view.second == 37`.
The original address is captured before transport and the backing stays in
place. The final branch observes the operation's result; it grants no access,
formation bound or membership.

Every place repeats the same fact sentence, fixed geometry and operation.
Each negative is byte-for-byte its positive with only `bytes[2] = 99;` added
while the transported view is still needed. The array is mutable in both
programs: the intended failure is the live shared loan of the second referent
byte, not immutable storage or incompatible representation. The authored
refusal fragments are:

```text
mutates `bytes[2]` while local borrow
is still active
```

A formation error or unrelated refusal does not count.

## Places

| Place | Positive | Reject | Transport |
| --- | --- | --- | --- |
| local | [main](local/main.omg) | [invalid](local/invalid/main.omg) | Formed reference retained as a local. |
| field | [main](field/main.omg) | [invalid](field/invalid/main.omg) | Lifetime-bound `Holder.view` stores the formed reference. |
| parameter | [main](parameter/main.omg) | [invalid](parameter/invalid/main.omg) | Observer receives the reference and original address as explicit formals. |
| call result | [main](call_result/main.omg) | [invalid](call_result/invalid/main.omg) | Identity helper returns an already-formed reference to caller storage. |
| match join | [main](match_join/main.omg) | [invalid](match_join/invalid/main.omg) | True forwards directly; false forwards through the identity helper; both selections execute. |
| const name | [main](const_name/main.omg) | [invalid](const_name/invalid/main.omg) | `const START: u64 = 1` selects the same runtime byte, following the indexed-address row's named-geometry pattern. |
| another state | [main](other_state/main.omg) | [invalid](other_state/invalid/main.omg) | Named state receives the formed reference and original address. |
| another machine | [main](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) | Relay forwards both formals to a separate observing machine. |
| target body | [main](target_body/main.omg) | [invalid](target_body/invalid/main.omg) | Each hosted target body performs the same address comparison and field read. |

Every positive declares `Expected exit: 0`, has native and interpreter exit-0
oracles, and binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`,
`macos_arm64`, and `linux_arm64`. Bindings do not establish execution on those
hosts. All nine pairs are pinned as
`tests/omega/pass/facts/reference_recast_backing_footprint_through_<place>_exit`
and
`tests/omega/fail/facts/reference_recast_backing_footprint_through_<place>_live_backing_write`.
Sample and pinned fixture source, Build, and applicable oracles are identical.

The const place does not store a reference or numeric interpreter identity in
a portable constant. The call-result place does not return helper-owned
storage. The row does not demand mutation outside the referent while a real
retained parent or call-held operand remains live: it cannot skip that lineage.

## Requirement and evidence

Owning task: **REFERENCE-LOCAL-BINDING-ROUTES** in [TASKS.md](../../../../TASKS.md).
This row targets the admitted recast producer's original backing, exact
formation and full referent footprint through reads, calls and state forwarding.
It also exercises the original-storage obligation of
**STRUCTURAL-BORROW-IDENTITY**; it does not close every producer or host there.

1. **Establishment:** the fixed four-byte backing, literal start 1 and two-byte,
   alignment-1 record justify the complete region. Shared representation
   compatibility remains the [recast judgment](../../../../wiki/spec/layouts/recasts.md).
2. **Identity and lifetime:** the first field exposes the original start-byte
   address; the second field reads the same live backing. The twin writes that
   second byte before the last observation, testing the full loan rather than
   only its start address. [Loans](../../../../wiki/spec/terminal-psi/loans.md)
   retain exact formation, captured places, access and parent lineage.
3. **Composition:** the nine carriers transport the already-formed view. Neither
   scalar addresses nor equal geometry manufacture a new loan or caller fact.
4. **Soundness:** every twin retains all values, bounds, reference access and
   transport, adding only the conflicting write. Address exposure grants no
   authority under [counts and addresses](../../../../wiki/spec/language/counts_and_addresses.md#address-exposure).
5. **Psi 05–07:** acceptance requires retained original backing and selected
   coordinates in canonical Terminal Psi, with native/interpreter agreement.
   No success is inferred from Source checking alone.
6. **Omega 00–02:** acceptance uses ordinary ProgramEntry settlement, two-byte
   record geometry and reference call/state plans, without foreign authority.
7. **Omega 03–06:** address computation and the second-byte read must use the
   same backing home through call registers, state arguments and activation
   storage. No assembled copy may substitute for that identity.
8. **Omega 07–09/emission:** actual emitted images must agree with Terminal
   execution at exit 0; exact producer/target and canonical artifact records
   belong in qualification evidence, not this static source inventory.

## Current status and remaining acceptance

The nine-place sample row and its nine pinned corpus counterparts passed on
`macos_arm64` with actual producer
`43278ece1f0f79ae108f0040a086fb967668b9c3`. Fresh authored copies of all nine
sample places produced native and Terminal exit 0, retained nonempty products,
and passed canonical Terminal admission. Each twin rejected with both authored
live-loan fragments. The two state-arrival and nested-scalar prerequisite
pairs also passed their unchanged positive and negative oracles in that same
22-program capture.

The separate normal corpus capture passed all nine permanent positives with
native/Terminal exit 0 and agreement, retained 18 nonempty products, and admitted
all nine canonical Terminal artifacts. Its nine permanent twins rejected with
the full authored fragments. This is bounded evidence for the unchanged fact,
geometry and operation across all nine places on this host. Earlier producer
refusals retain their original identities in the execution evidence.

`windows_x86_64`, `linux_x86_64` and `linux_arm64` are declared but unexecuted
for these captures. Other admitted reference producers, the owning task's
remaining routes and host qualifications remain open. Normal qualification,
independent exact-candidate final review and main publication are still owed;
the successful row does not close either parent capability.

```sh
omega run --both samples/cli/facts/reference_recast_backing_footprint/<place>/main.omg
omega --check samples/cli/facts/reference_recast_backing_footprint/<place>/invalid/main.omg
python3 tools/corpus_gate.py --both --filter facts/reference_recast_backing_footprint --runner <actual-runner>
```

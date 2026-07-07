# PARKED REPRO (2026-07-07, diagnosis CORRECTED same day)

Found authoring std::time's Duration core (the first multi-state pure-value
machines in std). Interpreter correct throughout; native diverged. `main.omg`
is an early differential canary iteration; `time_at_repro.omg.txt` the
matching std/time.omg body. The LIVE canary
(canaries/pass/time/runtime_duration_core_exit) now passes BOTH engines via
the workarounds below.

## Face A — same-type receiver aliasing, VALUE-CALL flavor (KNOWN bug)

`self.sum.checked_subtract(self.b)` with several Duration-typed fields on Main
resolves the RECEIVER to the FIRST field of the type (`a`): the emitted ops
read `Main_storage@0` where `sum`'s offset was meant. Same root as the
contained-machine same-type aliasing entry in TASKS.md
(`machine_storage_offset` resolves by TYPE). `checked_add` "worked" only
because its receiver WAS the first field. WORKAROUND: route every receiver
through the first field of its type (copy into it first).

## Face B — payload write cascade drops `(cast) % literal` field values (KNOWN landmine class)

A case-payload field value of shape `(x as u32) % 1000000000` (Binary with a
Cast operand) is SILENTLY DROPPED by the parallel write cascade: the tag and
sibling fields land, that field never writes (ZII garbage read back). Bare
`x % literal` works. WORKAROUND: do the Exact re-domaining cast in the entry
LETS so emit states construct from bare params.

## Misdiagnoses corrected (kept so nobody re-chases them)

- "`as T in Domain` emits i8-width converts" — FALSE: backend_report renders
  convert widths in BYTES; `as i8->i8` is an 8-byte u64 identity convert.
- "receiver substitution stops at the entry" — the report's guard TEXT is
  rendered unsubstituted, but the emitted ops show the real story: the
  receiver resolves by TYPE to the first field (face A), entry included.

## Next

The deep fix is the KNOWN one: thread the receiver FIELD OFFSET through
value-call dispatch (TASKS.md same-type aliasing entry — now high-leverage,
std value types make same-type fields ubiquitous). Face B's cascade arm
(Binary-with-Cast-operand payload values) is a separate missing-arm fix.
Delete this directory when face A's fix lands and the first-field workaround
is removed from time.omg's canary.

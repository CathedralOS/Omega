# Epsilon evaluator entry boundary

`sh tests/epsilon/evaluator-entry/run.sh` reconstructs the canonical
section-11 evaluator edge and executes its request boundary: the packed
`epsilon_compiler.delta.sources` closure (617,354 bytes, SHA-256
`4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e`) plus the
canonical entry
[`evaluator_entry.delta`](evaluator_entry.delta), compiled with the bound
Delta support section through the bound Delta compiler (`DCREQ`, profile 1,
`ConformanceBytesV1`), on the bound Gamma evaluator tape.

The entry source is bound: 10,950 bytes, SHA-256
`52032438c1236f51095b761afcb3111df2ae2d73ac9be7e91883bbfbd273e5e3`. The
reconstructed canonical evaluator receipt is bound: 729,060 bytes,
SHA-256 `bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368`. Both identities are pinned in
[`tools/bootstrap/epsilon/evaluator_env.sh`](../../../tools/bootstrap/epsilon/evaluator_env.sh)
and recorded in
[`bootstrap/4_epsilon/EVALUATOR_ENTRY.md`](../../../bootstrap/4_epsilon/EVALUATOR_ENTRY.md),
which owns the envelope and profile prose.

## EREQ v1 request

The sealed input is the complete request envelope:

```text
0..8    identity 45 45 52 45 51 01 00 00 ("EEREQ" version 1, reserved zero)
8..12   evaluator profile u32; version 1 assigns exactly 1 (ExactConsoleV1)
12..16  source closure length u32, high bit clear
16..20  sealed stdin length u32, high bit clear
20..52  claimed evaluator-closure identity: SHA-256 of the packed
        epsilon_compiler.delta.sources closure, checked against the bound
        identity embedded in the entry
52..    exact source closure bytes, then exact stdin bytes, then exact end
```

## Publications

A status-zero invocation publishes either a canonical observation or an EEOUT
refusal frame; the first byte keeps the classes disjoint:

- `00` Exit: four-byte little-endian `i32` exit code, then exact stdout.
- `01` Trap: one closed trap-kind byte, then exact stdout prefix.
- `02` Reject: one closed rejection-reason byte, then four-byte
  little-endian source coordinate.
- `FF 45 45 4F 55 54 01 00` ("EEOUT" v1): a 40-byte refusal frame with
  outcome, coordinate space, code, coordinate, limit, and requested fields.
  Request refusals carry outcome 1; evaluator-internal contradictions carry
  outcome 3.

Resource refusals publish neither: the evaluator program cannot intercept the
lower chain's refusals, so sealed-input extent, publication extent, and
evaluator context/storage exhaustion remain nonzero statuses with empty
stdout. At the edge boundary each is the section-10 outer `Incomplete`:
status 253 carries `Incomplete(sealed input, 4194304, submitted extent)`,
status 254 `Incomplete(published observation, 4194304, 4194305)`, status 250
`Incomplete(live call contexts, 256, 257)`, and status 252 `Incomplete(pair
nodes, 40265318, 40265319)`. Every other process observation carries the
outer `InternalFailure`. The status table and derivations live in
[`EVALUATOR_ENTRY.md`](../../../bootstrap/4_epsilon/EVALUATOR_ENTRY.md). The
status-252 refusal is too slow for this routine gate; it is executed directly
against this canonical receipt by the explicit slow
[`pair-boundary`](../pair-boundary/README.md) gate.

## Controls

The gate executes exact/adjacent pairs against the reconstructed canonical
receipt:

- header truncation at every extent 0..51 against the exact 52-byte admit;
- each adjacent identity byte (0..8) and each adjacent closure-digest byte
  (20..52) against the exact bound values;
- unknown profiles 0, 2, 3, and 2^32-1 against profile 1;
- each length field's high bit against the u31 rule;
- declared source/stdin sections absent and one beyond remaining, with
  `limit`/`requested` reported, against exact one-byte sections;
- a trailing byte after the complete envelope;
- the 4,194,304-byte sealed-input boundary, admitted exactly and refused
  adjacently at 4,194,305 as `Incomplete` via status 253 with empty stdout;
- the live-call-context boundary: 34 nested non-tail machine calls admitted
  with the canonical `Exit` observation, the 35th refused as `Incomplete` via
  status 250 with empty stdout.

Canonical observations witnessed: `Exit` with stdout (exit 42, `A`), the
sealed stdin section reaching `Console.read_byte` (echo of `Z`, exit 90),
`Trap` with a written prefix (division by zero, kind 2, `B`), and `Reject`
observations for an empty source (`UnexpectedEnd` at 0) and an invalid source
byte (`InvalidSourceByte` at 0). No control publishes an observation on a
refusal path.

On macOS arm64 the receipt reconstruction and controls took 266.7 seconds at
the measured revision; that is a host measurement, not a profile bound.

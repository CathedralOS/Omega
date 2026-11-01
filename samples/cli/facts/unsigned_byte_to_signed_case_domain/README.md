# Unsigned byte conversion into a case payload

Fact: widening an observed u8 to an i32 payload preserves 0..255 in ByteParcel::Byte, so 255 - value remains exact through transport.

Owning task: `PIPELINE-ARCHITECTURE-ENFORCEMENT`.
Claim ticket: `e7ea8d46d21149ebabf29142d37cba24` (`Cloud six / pipeline`).

The source observation supplies the byte bounds. The conversion retains the
receiving integer carrier and its complete arithmetic qualification, with an
integer embedding of the already observed operand. A destination annotation
cannot supply bounds or excuse failed conversion formation. This is the same
case-domain construction required by the ordinary standard console provider.

Each place consumes ByteParcel values made from 128 and 255 and checks exact
results 127 and 0. The Empty case is also checked; the match join exercises both
arms. The field place checks its adjacent marker. The selected-target place
uses an ordinary named HostedByteSource implementation selected by Build for
all four hosted targets.

Each reject twin adds one to the converted payload, which can produce 256,
while retaining the receiving 0..255 requirement and subtraction. In the const
place only the two named constants change, so the rejection belongs to constant
construction rather than an unused invalid generic producer.

| Place | Witness | Reject twin |
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

Every positive project binds ProgramEntry for linux_x86_64, linux_arm64,
macos_arm64 and windows_x86_64 and declares exit 0. Identical sources, Build
files and oracles are pinned under tests/omega/{pass,fail}/facts.
Run a witness with `omega run --both <place>/main.omg`; run its twin with
`omega --check <place>/invalid/main.omg`.

Local execution evidence and the exact optimized producer are retained in the
worker qualification directory. Foreign bindings are authored coverage;
execution on another operating system has not been performed by this Linux
cloud session.

## Measured local acceptance

On linux_x86_64 all nine positive projects execute with native exit 0 and
Terminal exit 0. All nine twins reject the case-domain constraint; the constant
twin identifies the false constant construction. The 54 sample/pin file pairs
are byte-identical. The optimized producer has SHA-256
`d8530587e625f1f95576eb3a458bfa60730984d61f809f8ac924720d34d57c04`.
The worker retains raw commands, source hashes, Terminal/native artifacts and
fuel receipts. This establishes this fact row, not the entire pipeline task.

The [composition](compositions/policy_and_return/main.omg) exercises the same
conversion through an explicit Wrapping qualification and a later Exact
qualification, including a byte returned by a named narrowing operation. This
additional production keeps the original source observation; it does not get
0..255 from its receiving i32 annotation.

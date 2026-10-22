# Checked assembly rank decrease

sub records the decreased register, and the backedge reads that record.

The seed-2 mov/sub/jnz countdown is unchanged. This machine adds the ranked
decrease `terminates by eax -> Nat::Descending`. The countdown fact's label
contracts `loop requires eax > 0` and `done requires eax == 0` are ordinary
edge obligations. They are not this termination promise.

`checked_asm_countdown` does not carry this witness.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |

The local positive declares `Expected exit: 0` and exits 0 on both Windows
engines. The twin changes only `done requires eax == 0` to
`done requires eax == 1`. The other eight places are not written.

# Gamma host and target comparison

A compiler running on Alpha can emit a Linux executable. Conversely, the
Linux-hosted compiler can emit Alpha tape. On Linux x86-64 at `e488ba404bf`,
both hosts independently built the real Delta compiler for each target. Every
row rejected an invalid Gamma program, compiled the same Delta byte-string
customer and ran it on empty input, embedded NUL and all 256 byte values.

| Gamma compiler runs on | Emits | Delta compiler bytes | SHA-256 of the Delta compiler | Time to compile Delta |
| --- | --- | ---: | --- | ---: |
| Alpha | Alpha tape | 1,219,139 | `6fde052af5b348f3b83f29635aaaedbd6959343352da3a3af1442000a102b1d0` | 13.39 s |
| Linux | Alpha tape | 1,219,139 | `6fde052af5b348f3b83f29635aaaedbd6959343352da3a3af1442000a102b1d0` | 2.94 s |
| Alpha | Linux ELF | 3,856,065 | `e7d271e82ee7e9c652dc892e6f9fee90e47fe0d53ba28bd1b2810e9aba4bf7d1` | 19.28 s |
| Linux | Linux ELF | 3,856,065 | `e7d271e82ee7e9c652dc892e6f9fee90e47fe0d53ba28bd1b2810e9aba4bf7d1` | 8.42 s |

All four Delta compilers emitted identical Gamma source for the customer
(3,082 bytes). Its executables also matched across hosts for each target.
With `--epsilon`, all four Delta compilers emitted the same 726,601-byte
Gamma source for the full Epsilon evaluator. Both Alpha-target runs built
the same 7,844,855-byte tape. Both Linux-target runs built the same
25,223,480-byte ELF. All four evaluators passed the five Epsilon language
checks and two invalid-syntax controls. Replaying the five previously
published `@` inputs on those same executables gives 2/5. Epsilon specifies
`&[u8]` and `&mut self`. The original requests and expected results remain
available under `--profile historical-at`. [Portable checks](../../../tests/bootstrap/conformance/README.md)

Times are single wall-clock observations on a shared machine. The two cold
Gamma constructions took another 70 s for Alpha and 165 s for Linux.

For review, the compiled routes share 32,980 source bytes in the Gamma compiler
and frontend. The Alpha backend adds 2,355 bytes in one Gamma source file. The
Linux backend adds 7,341 bytes and includes ELF layout and Linux syscalls.
The interpreted route omits both emitters. The Alpha-targeted tape is a useful
comparison artifact because a reviewer can reproduce it with either host and execute
it in the audited VM. Both hosts still share the frontend and backend source,
so cross-host equality cannot detect a mistake common to those sources.
[Alpha backend](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma) · [Linux backend](../../../bootstrap/gamma/compiler/linux_x86_64/native.gamma)

From the repository root, use fresh output directories. The Python comparison
harness runs outside the construction chain. Its `xtask source-closure`
helper materializes the ordered support bytes used in the test request.

```sh
tests/bootstrap/linux_x86_64/bootstrap --alpha . /tmp/omega-gamma-alpha
tests/bootstrap/linux_x86_64/bootstrap . /tmp/omega-gamma-linux
python3 tests/bootstrap/gamma_host_matrix.py --epsilon \
  /tmp/omega-gamma-alpha /tmp/omega-gamma-linux /tmp/omega-host-comparison
```

Use `--wire` in place of `--epsilon` to take the same four constructions one
step further. The test gives both target evaluators the complete Omega
[wire decoder](../../../source/foundation/wire/codec.omg) and a caller that checks
zero, `0x12345678` and `0xffffffff`. It compares the emitted Alpha programs
byte for byte and executes the common tape on the seed VM. Both evaluators
returned the same 1,612-byte observation for the 848,481-byte request. Its
1,607-byte tape (SHA-256 `6b6a22fc219aab246a14ad9c7f34ea538fedbe518ecac3850d51d63bdc6e7d8f`)
exited 42 with empty output. An earlier Alpha-hosted attempt reached its
1,500 s watchdog on the shared machine. The successful replay took 495 s.

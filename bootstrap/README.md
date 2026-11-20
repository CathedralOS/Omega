# Bootstrap construction paths

[The bootstrap mandate](MINIMIZATION.md) governs this work. Each viable path must
construct the Omega compiler from its declared auditable seed and then use that
compiler to rebuild itself reproducibly. No path is selected during exploration.

All routes start with the audited Alpha VM and a separately admitted Beta
compiler tape. Running it on Beta's own source reproduces the tape byte for
byte. Rebuilt Beta then compiles the Beta-written Gamma evaluator to Alpha. On compiled routes this evaluator constructs the
first Gamma compiler. [Alpha seed](interpreted/0_alpha/README.md) · [Beta tape audit](interpreted/1_beta/AUDIT.md)

**Alpha** means tape executed by the seed VM. **Native** means a Linux
x86-64 executable. The Delta compiler is written in Gamma and emits Gamma
on every route.

| Route | Delta compiler runs on | Epsilon implementation | Demonstrated workload |
| --- | --- | --- | --- |
| [Original interpreter](../tests/bootstrap/interpreted/epsilon/README.md) | Gamma evaluator on Alpha | Epsilon evaluator, interpreted by Gamma | Reconstructed the full evaluator in 567 s and passed its five portable language cases and two syntax controls. |
| [Gamma compiled to Alpha](compiled_vm/README.md#compare-a-production-request-operation-across-evaluator-hosts) | Alpha | Epsilon evaluator on Alpha | Compiled the product's `Request.clear` and ran the resulting tape on Alpha. |
| [Gamma compiled to native](independent/README.md) | Native | Native Epsilon evaluator | Compiled `Request.clear` and the product's complete wire decoder to Alpha. |
| [Epsilon compiled to Delta, Alpha target](independent/README.md#public-omega-lexer-observations) | Alpha | Epsilon-to-Delta compiler on Alpha | Compiled the string scanner. Its 8,391,472-byte tape passed 17 cases. The full public scanner exceeded the Alpha tape limit. |
| [Epsilon compiled to Delta, native target](independent/README.md#public-omega-lexer-observations) | Native | Native Epsilon-to-Delta compiler | Compiled the same string scanner and the full public lexer. The full lexer ran 23 cases. |

The first three use one Delta-written evaluator to interpret the Epsilon-written
[bootstrap compiler](interpreted/5_omega/README.md)
(D in older records). The last two use a Delta-written compiler to translate Epsilon
into Delta modules. Adding a runtime and entry point lets the Delta and Gamma
compilers build an executable.

The original Gamma interpreter also ran four predicates from the bootstrap
compiler's lexer on six input bytes covering whitespace, digits, identifiers,
a hexadecimal letter and byte 255. All 29 output bytes matched the native
evaluator and the independently specified result, including Epsilon exit 42.
On the same reconstructed evaluator, the 180-byte portable Omega program
reached a 300 s wall limit and the full 23-case lexer reached 480 s. Neither run published a result during its allowance.
[Interpreted measurements](../tests/bootstrap/interpreted/epsilon/README.md)

Native can lead back to Alpha. Compile the Gamma-to-Alpha compiler for Linux
and it emits tape for the original VM. Native-hosted and Alpha-hosted compilers
produced identical numeric lexer tapes. The tape and Linux executable agreed
on all thirteen inputs. [Cross-compilation](independent/README.md#construction-and-acceptance) · [Lexer comparison](independent/README.md#calling-the-numeric-lexer-with-a-stored-enum)

## Gamma host and target comparison

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
available under `--profile historical-at`. [Portable checks](../tests/bootstrap/conformance/README.md)

Times are single wall-clock observations on a shared machine. The two cold
Gamma constructions took another 70 s for Alpha and 165 s for Linux.

For review, the compiled routes share 32,980 source bytes in the Gamma compiler
and frontend. The Alpha backend adds 2,355 bytes in one Gamma source file. The
Linux backend adds 7,341 bytes and includes ELF layout and Linux syscalls.
The interpreted route omits both emitters. The Alpha-targeted tape is a useful
comparison artifact because a reviewer can reproduce it with either host and execute
it in the audited VM. Both hosts still share the frontend and backend source,
so cross-host equality cannot detect a mistake common to those sources.
[Alpha backend](compiled_vm/alpha_backend.gamma) · [Linux backend](independent/native.gamma)

From the repository root, use fresh output directories. The Python comparison
harness runs outside the construction chain. Its `xtask source-closure`
helper materializes the ordered support bytes used in the test request.

```sh
bootstrap/linux_x86_64/bootstrap --alpha . /tmp/omega-gamma-alpha
bootstrap/linux_x86_64/bootstrap . /tmp/omega-gamma-linux
python3 tests/bootstrap/gamma_host_matrix.py --epsilon \
  /tmp/omega-gamma-alpha /tmp/omega-gamma-linux /tmp/omega-host-comparison
```

Use `--wire` in place of `--epsilon` to take the same four constructions one
step further. The test gives both target evaluators the complete Omega
[wire decoder](../source/foundation/wire/codec.omg) and a caller that checks
zero, `0x12345678` and `0xffffffff`. It compares the emitted Alpha programs
byte for byte and executes the common tape on the seed VM.

## Building the compiler

From the repository root on Linux x86-64, run
`bootstrap/linux_x86_64/bootstrap . /tmp/omega-gamma` with a new output
directory to build the native Gamma compiler offline. Put `--alpha` before
`.` to build the Alpha-tape compiler. Both commands rebuild the Gamma compiler
twice with an empty `PATH`, using the embedded seeds and the supplied source files.
The same entry accepts the Delta compiler source and an Epsilon evaluator
compilation request. [Offline commands and inputs](linux_x86_64/README.md)

The bootstrap compiler's job is to build the **Omega compiler** (C in older
records), the product written in Omega.
Construction of the full product and its reproducible self-build remain open.
The product uses a deliberate subset of Omega while preserving the language's
semantics and explicit refusals. [Omega compiler and libraries](../source)

The complete construction input used here contains 304 Omega source units
in seven packages. Its 3,955,617 bytes fit the constructor's source-input
provision. Packaging it with the Epsilon-written bootstrap compiler for the
evaluator takes 4,807,765 bytes. Both stock targets returned outer status
253 with empty output at the Delta adapter's 4 MiB input check. A native
evaluator rebuilt with an 8 MiB input limit reached the constructor on a
same-size control. The real request ran for 480 s on that evaluator and
published neither an artifact nor a compiler diagnostic before the watchdog.
[Product input](../source/application/construction/construct.py) · [Evaluator experiment](../tests/bootstrap/independent/WHOLE_REQUEST.md)

A host Rust toolchain builds the Rust comparator, which can compile Omega
sources during development. Record that dependency when reporting a
Rust-assisted build. [Rust comparator](../source-rust)

Host-side tests, reference interpreters and development accelerators live under [tests/bootstrap](../tests/bootstrap/). When promoting a chain, take its candidate tests and the [portable conformance checks](../tests/bootstrap/conformance/README.md). The compiled routes share their comparison helpers in [compiled.py](../tests/bootstrap/compiled.py).

Record seed and trust assumptions, actual cold runs, development acceleration
and failed experiments.
The [edge contracts](CONTRACT.md) describe the retained language edges.
Scripts locate their sources through [`tools/bootstrap/paths.sh`](../tools/bootstrap/paths.sh).
[Proof research](proofs/README.md) is optional for construction.

# Bootstrap chain manifest

[Lattice overview](bootstrap_lattice.md) | [Active work](../../../TASKS_BOOTSTRAP.md)

This is the short audit ledger for the current direct chain. Content hashes name
the exact subjects; construction commands and large test inventories remain
beside their owners. “Gate accepted” is deliberately narrower than “compiler
correct”: an open row is not laundered by a later fixed point or differential
agreement.

## Alpha execution floor

Committed native seeds:

| target | artifact | bytes | SHA-256 |
| --- | --- | ---: | --- |
| Darwin arm64 | `source/alpha/alpha_arm64_macos` | 314800 | `110a59187b4ae4564297867670ac94499670babe976a7556f4efb8adb99f65bf` |
| Windows x86-64 | `source/alpha/alpha_x64_windows.exe` | 267264 | `0b8c3bb6d374d5a7a03de1e16be1f7206248acae990c2594a040291c7c866cb2` |

The accepted input is one length-prefixed Alpha tape; the output is the exact
byte stream, process status, trap, or divergence defined by
`source/alpha/SEMANTICS.md`. `source/alpha/verify.sh` binds provenance,
conformance, and assembler reproduction. Remaining admissions are the human
binary/listing audit and the physical hardware, firmware, kernel, loader, and
ISA realization of the written machine model.

## Alpha assembler

Exact source:
`source/alpha/assembler/assembler.alpha`, 24110 bytes,
SHA-256 `dba58ab9131adfdf430627d0df58bc1858d273be7dd441949168813325b2cbe8`.

Exact output tape: 5208 bytes,
SHA-256 `c019a8cce9f99bf6f505961383fba9f2013ff318381ffd44ace0d20444d48d0d`.
It is embedded in the two committed host containers below and reproduced
byte-for-byte by `source/alpha/assembler/selfhost.sh`:

- Darwin arm64: `df561b86b3e60aacf030d3d76b1c8b74c666003e51b8757e7168b95a650a7623`;
- Windows x86-64: `791f041884201ebcc6a27b0f92446eae60baf0c808bd0e3ede80c865931b4387`.

## Beta compiler

Exact input source: `source/beta/compiler/bc.beta`, 32565 bytes,
SHA-256 `fe4b5af69f87163ce919d22e2aa662ad0b5f2a044a6904581c91e8638749aa25`.

Exact output artifact: `source/beta/compiler/artifacts/bc.tape`, 52141
bytes, SHA-256
`1b32401c4c8fb60598e97178d415136227c9aa2231e28d9eb44b30e7a2818a2f`.
The Alpha-written cold start reconstructs it and the Beta compiler reproduces
its fixed point. The independently Alpha-written structural checker has source
SHA-256 `df9f5696d7d5f6ac457d74d035ffc4db64f4f68a2f8201f7bb5ea0a28b471a2e`
and emits a 3035-byte tape with SHA-256
`6d892139f7e4153dbe30cb353e5eb498c7b1d5260a486470616dc684b55cd67f`.
That checker accepts framing, reachable control-flow, procedure-region, and
tape-capacity obligations only. The adjacent canonical maximal-observation
reconstruction emits one 79,124-byte ROOT Alpha tape; its exact digest is
`feb10320cf6e9caa10aadb8a89af3e6dd8da0529aeadf5b85d6c5a8c3b93a5b1`.
This is strong executable evidence, but it is not yet a derivation in the
rooted checker calculus. Complete source/artifact admission remains open.

## Gamma meaning artifacts

The Beta artifact deterministically compiles the following canonical sources:

| role | source bytes / SHA-256 | output tape bytes / SHA-256 |
| --- | --- | --- |
| interpreter | 47948 / `95f9ef743a587e9bd098a162e97f41b40caad2785db195ed76c4a7399d8cdc74` | 94903 / `47942ec8d6bc161abe313535fbf28df5e832f58da6013fbcdc54e6a386b1c6c5` |
| type checker | 22059 / `aed69a455d95bae79dfa889aebe842b93765acd1cf59cf6a4d11c0808dc0e1d3` | 45230 / `d580f6b88f077043f12a76b91e7643d0bf62e9046ce667459ce20867358399c5` |

Their exact inputs are `source/gamma/interp.beta` and
`source/gamma/typeck.beta`. Their outputs are reconstructed rather than
committed. Gamma execution is fuel- and resource-bounded as documented beside
the interpreter.

## Derivation checker

The current logical checker input is
`source/alpha/checker/implementations/beta/check.beta`, 68349 bytes,
SHA-256 `965cb8417582fee099450a905a6c41535cf53b8bcef2c5a22bd0fe1def27fb10`.
The accepted output is `source/alpha/checker/artifacts/check.tape`, 150937
bytes, SHA-256
`34330f0bf94385fdc403fa989ff113d5c579c8cf8318e1c6c3a86bc080ebd37c`.
The Alpha-written cold Beta compiler constructs it without `bc.tape`, and
`source/alpha/checker/reconstruct-artifact.sh` reproduces it twice and compares
the committed bytes before discriminating accept/reject controls. A checker
compiled by `bc.tape`, plus the Gamma and Python checkers, is differential
evidence only and cannot admit `bc.tape`.

## Delta publication

The canonical source snapshot is
`source/delta/compiler/validation/source-closures/canonical-compiler-v1.json`
with closure SHA-256
`31324d8342f1e3c2da95553418245e4bd981985558256fcc012eb8e6a804ff32`
and content-set SHA-256
`9559bdee641d8dfcb4452d1eb9d9510c0534a031df86737258d370d3e08d3c42`.
Its canonical LF image is 168560 bytes with SHA-256
`a0ecad14670247857e300b5539e0058d8f72054f92fabd1645fc4457b0ac53c9`.
The lower-rung elaboration and publication verifier are present; the two exact
full Gamma executions and their accepted publication receipt are still open.
No Delta compiler output is admitted by this manifest.

## Omega product edges

The exact transitive compiler closure `C`, `omega₀`, and `omega` do not yet
exist as closed subjects, so they have no hashes here. The package/security
accepted-lock projection remains an external authority dependency. The product
build entry is design-blocked on `OWNER_QUESTIONS.md` Q8; implementation of
ordinary source modules not exercising that target-selection question remains
unblocked.

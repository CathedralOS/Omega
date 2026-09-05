# Alpha tape executor

Alpha is the unchanged execution floor: a small virtual machine with byte I/O,
fixed-width integer operations, bounded flat memory, branches, calls, halt, and
trap. [`SEMANTICS.md`](SEMANTICS.md) defines the exact tape machine.

Alpha has no textual source language, assembler grammar, type system, or proof
kernel. The selected bootstrap floor consists of an audited native Alpha seed
plus the admitted Beta compiler tape.

## Owned files

```text
alpha_x64_windows.exe    audited Windows x86-64 VM container
alpha_x64_windows.hex    annotated x86-64 audit listing
alpha_arm64_macos        audited macOS arm64 VM container
alpha_arm64_macos.s      hand-authored arm64 implementation
alpha_arm64_macos.lst    committed arm64 disassembly
SEMANTICS.md             AlphaBootstrapV2 execution and tape semantics
```

Host seed selection and tape stamping live under `tools/bootstrap/alpha/`.
Conformance and the independent reference VM live under `tests/alpha/`.
`tests/bootstrap/alpha-beta-edge.sh` checks behavior and optional native-source
provenance.

Trusted Beta lives under `bootstrap/beta/`. Its readable compiler source
reconstructs the admitted tape byte-identically and supplies the next language
edge to the Gamma evaluator.

## Retention inventory

| Retained files | Direct role | Deletion condition |
| --- | --- | --- |
| `SEMANTICS.md` | Authoritative Alpha execution and raw-tape relation. | Replace only atomically with a ruled Alpha revision and every consumer. |
| `alpha_arm64_macos`, `alpha_arm64_macos.s`, `alpha_arm64_macos.lst` | Audited macOS arm64 realization, source, and listing. | Delete only with platform retirement or an equally audited replacement. |
| `alpha_x64_windows.exe`, `alpha_x64_windows.hex` | Audited Windows x86-64 realization and listing. | Delete only with platform retirement or an equally audited replacement. |

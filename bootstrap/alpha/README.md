# Alpha tape executor

Alpha is the execution floor: a small virtual machine with byte I/O,
fixed-width integer operations, bounded flat memory, branches, calls, halt, and
trap. [`SEMANTICS.md`](SEMANTICS.md) defines the exact tape machine.

AlphaBootstrapV3 selects 1 GiB of semantic memory while preserving the initial
stack offset `0x10000000`, all opcode transitions, and the 16 MiB stamped hole
(16,777,212 raw tape bytes). Upper memory does not extend the downward-growing
stack. The conformance gate checks zeroed upper bytes, byte and final-word
stores, and the unchanged first-call return-address location without invoking
undefined out-of-range accesses.

The macOS source rebuild uses the selected Xcode or `xcrun` CommandLineTools
clang and SDK with `-arch arm64 -isysroot SDK -Wl,-no_uuid`; the Alpha/Beta
edge gate compares the rebuilt and committed containers after removing their
OS signatures. The V3 container was rebuilt with the available CommandLineTools
toolchain (Apple clang 17.0.0, build `clang-1700.0.13.5`, macOS SDK 15.5), not
by patching the older native binary. Its audited disassembly
reflects that linker layout; the tape hole remains at file offset 32,768.

The Windows PE listing keeps the same instruction handlers and file offsets.
Its zero-filled data virtual size and `SizeOfUninitializedData` grow to
`0x40001000`; the tape section moves to RVA `0x40004000`, the loader's absolute
tape address becomes `0x180004000`, and `SizeOfImage` becomes `0x41004000`.
The tape hole still begins at file offset `0x1400`. Reconstructing the complete
listing changes exactly those five capacity/address bytes from the V2 PE.
Windows runtime validation is not established by that byte audit.

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
SEMANTICS.md             AlphaBootstrapV3 execution and tape semantics
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

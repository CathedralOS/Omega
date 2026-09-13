# Alpha tape executor

Alpha is the execution floor: a small virtual machine with byte I/O,
fixed-width integer operations, bounded flat memory, branches, calls, halt, and
trap. [`SEMANTICS.md`](SEMANTICS.md) defines the exact tape machine.

AlphaBootstrapV4 selects 1.75 GiB of semantic memory while preserving the initial
stack offset `0x10000000`, all opcode transitions, and the 16 MiB stamped hole
(16,777,212 raw tape bytes). Upper memory does not extend the downward-growing
stack. The conformance gate checks zeroed upper bytes, byte and final-word
stores, and the unchanged first-call return-address location without invoking
bounds traps.

The macOS source rebuild uses the selected Xcode or `xcrun` CommandLineTools
clang and SDK with `-arch arm64 -isysroot SDK -Wl,-no_uuid`; the Alpha/Beta
edge gate compares the rebuilt and committed containers after removing their
OS signatures. The V4 container was rebuilt with the available CommandLineTools
toolchain (Apple clang 17.0.0, build `clang-1700.0.13.5`, macOS SDK 15.5), not
by patching the older native binary. Its audited disassembly
reflects that linker layout; the tape hole remains at file offset 32,768.

The Windows PE listing keeps the same file offsets.
Its zero-filled data virtual size and `SizeOfUninitializedData` grow to
`0x70001000`; the tape section moves to RVA `0x70004000`, the loader's absolute
tape address becomes `0x1b0004000`, and `SizeOfImage` becomes `0x71004000`.
The tape hole still begins at file offset `0x1400`. The V4 capacity change
affected five capacity/address bytes from the V3 PE; the current seed also
contains the bounds checks below. Windows runtime validation is not established
by the listing's exact reconstruction.

This extent fits the existing static native containers, including the
[PE32+ image-size limit of 2 GiB](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format).
Growing past that image bound would require a different native allocation
strategy; the current capacity is not a language-level restriction on Gamma
programs.

Alpha has no textual source language, assembler grammar, type system, or proof
kernel. The selected bootstrap floor consists of an audited native Alpha seed
plus the admitted Beta compiler tape.
It contains no compiler framework or higher-language primitive. Host stamping
packages a raw tape; it does not compile a language.

## Ratified bounds-hardening contract

[SEMANTICS section 8](SEMANTICS.md#8-bounds-and-fixed-capacity) assigns failed
runtime range checks and oversized stamped input to the existing abnormal,
non-resumable Trap. Runtime failure preserves prior stdout; loader failure occurs
before execution and has empty stdout. Neither adds diagnostic bytes, a new
opcode, or a higher-rung resource result.

Both native implementations and the independent reference check complete fetch,
operand, data, and call/return stack ranges, and reject a stamped length above
16,777,212 before copying. Dispatch checks the unsigned Alpha offset before
fetch, then the full instruction width using a 21-byte table. Data and stack
guards use unsigned end-minus-width comparisons; no address can wrap into an
admitted range. The arm64 implementation keeps the fixed extent in a preserved
native register. Neither adds semantic memory, a stack partition, or a new
outcome. The reference also captures a call's target before an overlapping
return-address write, as both native implementations do.

The Windows occupied code/table extent grows from 1,079 to 1,248 bytes; its
headers, imports, tape hole, and total container size remain unchanged. The
macOS source and refreshed disassembly retain the existing container layout.
See [bounds conformance](../../tests/alpha/README.md#bounds-conformance) for
tested behavior and observation limits. macOS execution and source rebuild
are checked; Windows listing reconstruction is not Windows runtime validation.

Windows host I/O scratch occupies RVAs `0x3800..0x381f`, immediately after
the 256 eight-byte Alpha registers at `0x3000..0x37ff` and before semantic
memory at `0x4000`. The two handles use `0x3800` and `0x3808`, the byte buffer
uses `0x3810`, and the count slot uses `0x3818`. The eight-byte count slot is
initially zero; host I/O writes its low four bytes, leaving the high half zero
for the native word read. All storage fits the existing writable zeroed page.
This corrects the former alias with registers 16–19 by relocating eleven
address immediates (22 binary bytes), without adding instructions or capacity.
The [register fixture](../../tests/alpha/io-registers.hex) checks initial zero
values, full-word preservation, and direct I/O operands. Its macOS/reference
results do not establish Windows runtime validation.

## Owned files

```text
alpha_x64_windows.exe    audited Windows x86-64 VM container
alpha_x64_windows.hex    annotated x86-64 audit listing
alpha_arm64_macos        audited macOS arm64 VM container
alpha_arm64_macos.s      hand-authored arm64 implementation
alpha_arm64_macos.lst    committed arm64 disassembly
SEMANTICS.md             AlphaBootstrapV4 execution and tape semantics
```

Host seed selection and tape stamping live under `tools/bootstrap/alpha/`.
Conformance and the independent reference VM live under `tests/alpha/`.
`tests/bootstrap/alpha-beta-edge.sh` checks behavior and optional native-source
provenance.

Trusted Beta lives under `bootstrap/1_beta/`. Its readable compiler source
reconstructs the admitted tape byte-identically and supplies the next language
edge to the Gamma evaluator.

## Retention inventory

The selected native container identities are SHA-256 commitments to the exact
repository bytes, separate from their realization/conformance obligations:

| Container | Bytes | SHA-256 |
| --- | ---: | --- |
| `alpha_arm64_macos` | 16,942,384 | `348bc9601a9f44d4afa98febd7292f77d016b3c1060e20b15768dc23e4061082` |
| `alpha_x64_windows.exe` | 16,782,336 | `bc71f8bee48cbd4c70c533e57b5dfcd04e04199ac3cf055cbed8e76ad6fb1c40` |

| Retained files | Direct role | Deletion condition |
| --- | --- | --- |
| `SEMANTICS.md` | Authoritative Alpha execution and raw-tape relation. | Replace only atomically with a ruled Alpha revision and every consumer. |
| `alpha_arm64_macos`, `alpha_arm64_macos.s`, `alpha_arm64_macos.lst` | Audited macOS arm64 realization, source, and listing. | Delete only with platform retirement or an equally audited replacement. |
| `alpha_x64_windows.exe`, `alpha_x64_windows.hex` | Audited Windows x86-64 realization and listing. | Delete only with platform retirement or an equally audited replacement. |

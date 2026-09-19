# Alpha tape executor

Alpha is the execution floor: a small virtual machine with byte I/O,
fixed-width integer operations, bounded flat memory, branches, calls, halt, and
trap. [`SEMANTICS.md`](SEMANTICS.md) defines the exact tape machine.

AlphaBootstrapV5 selects 128 GiB of semantic memory while preserving the
initial stack offset `0x10000000`, all opcode transitions, and the 16 MiB
stamped hole (16,777,212 raw tape bytes). No container section can express the
extent — the V4 image already sat at the PE32+ image-size bound — so both
seeds obtain `M` at startup instead of in a loadable section: `VirtualAlloc`
on Windows (an added kernel32 import) and the `mmap` syscall on macOS arm64
(no new dylib; the syscall drops the 1.75 GiB `mem` zerofill entirely). Host
allocators deliver zeroed pages, so `M` remains a flat zeroed array.
Allocation refusal is a startup trap before any tape byte is copied. Upper
memory does not extend the downward-growing stack. The conformance gate checks
zeroed upper bytes, byte and final-word stores, and the unchanged first-call
return-address location without invoking bounds traps.

The macOS source rebuild uses the selected Xcode or `xcrun` CommandLineTools
clang and SDK with `-arch arm64 -isysroot SDK -Wl,-no_uuid`; the Alpha/Beta
edge gate compares the rebuilt and committed containers after removing their
OS signatures. The V4 container was rebuilt with the available CommandLineTools
toolchain (Apple clang 17.0.0, build `clang-1700.0.13.5`, macOS SDK 15.5), not
by patching the older native binary. Its audited disassembly
reflects that linker layout; the tape hole remains at file offset 32,768.

The Windows PE listing keeps the same file offsets.
The semantic memory section is gone entirely: `.data` shrinks to the registers
and I/O scratch page (`VirtualSize` and `SizeOfUninitializedData` are
`0x1000`), the tape section returns to RVA `0x4000`, the loader's absolute tape
address is `0x140004000`, and `SizeOfImage` is `0x1004000`. The loader calls
`kernel32!VirtualAlloc(NULL, 0x2000000000, MEM_COMMIT|MEM_RESERVE,
PAGE_READWRITE)` through a fourth import entry, traps on a null return, and
keeps `MEMSIZE`/`MEMSIZE-8` in callee-saved `r13`/`r14` because `imm32` cannot
encode the extent. The tape hole still begins at file offset `0x1400`.
Windows runtime validation is not established by the listing's exact
reconstruction.

The extent is a startup allocation, so the
[PE32+ image-size limit of 2 GiB](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format)
no longer bounds it; both containers stay small. The current capacity is not a
language-level restriction on Gamma programs.

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

The macOS source and refreshed disassembly retain the existing container
layout apart from the startup `mmap` sequence in `__text`, the dropped `mem`
zerofill (`__bss` shrinks to the registers page plus the I/O byte), and the
mechanically repacked linkedit tables. On Windows the occupied code extent
grows for the allocation call and register compares; the tape hole, file
offsets, and total container size are unchanged.
See [bounds conformance](../../tests/alpha/README.md#bounds-conformance) for
tested behavior and observation limits. macOS execution and source rebuild
are checked on a macOS host; Windows listing reconstruction is not Windows
runtime validation.

Windows host I/O scratch occupies RVAs `0x3800..0x381f`, immediately after
the 256 eight-byte Alpha registers at `0x3000..0x37ff`; semantic memory is
now the `VirtualAlloc` extent rather than the `.data` that followed it. The two handles use `0x3800` and `0x3808`, the byte buffer
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
SEMANTICS.md             AlphaBootstrapV5 execution and tape semantics
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
| `alpha_arm64_macos` | 16,942,368 | `3a9cc3112f9f7645fca00716c347865d1b160f56d458fb6340c66f237d1ae616` |
| `alpha_x64_windows.exe` | 16,782,336 | `4ee9ee0f97c1b11c5a7ef32ffd05f1eeb193d1e9b327cb89df9ac54431aad701` |

| Retained files | Direct role | Deletion condition |
| --- | --- | --- |
| `SEMANTICS.md` | Authoritative Alpha execution and raw-tape relation. | Replace only atomically with a ruled Alpha revision and every consumer. |
| `alpha_arm64_macos`, `alpha_arm64_macos.s`, `alpha_arm64_macos.lst` | Audited macOS arm64 realization, source, and listing. | Delete only with platform retirement or an equally audited replacement. |
| `alpha_x64_windows.exe`, `alpha_x64_windows.hex` | Audited Windows x86-64 realization and listing. | Delete only with platform retirement or an equally audited replacement. |

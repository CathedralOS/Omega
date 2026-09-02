# Design Brief: Extern Boundaries And Foreign Formats

Current as of 2026-08-31. This brief defines the durable extern model. Concrete
binding/layout grammar remains subject to the referenced subsystem briefs.

## Abstract API, target binding

Application code calls an abstract `boundary trait`. Target/toolchain packages
provide implementations; application code never imports a DLL, syscall number,
or firmware table as if it were an ordinary module.

```omega
boundary trait WindowSystem {
    machine create(spec: WindowSpec) -> CreateWindowResult;
    machine present(window: &mut Window, pixels: &[Pixel]) -> PresentResult;
}
```

A `ProviderPlan` maps requirements to their checked realizations, but it is a
**derived normalized artifact**, not a value assembled row by row by user
code. Authors provide three inputs:

1. boundary-trait/operator requirements or explicit top-level `boundary
   requirement` declarations;
2. ordinary checked machines that explicitly `satisfy` those requirements;
   and
3. irreducible bodyless boundary leaves declared with `satisfies ...`, using
   `via <Binding>` only for an undiscoverable payload.

The binding vocabulary is an ordinary closed sum, not a growing family of
keywords:

```omega
data DllImport<
    const ObjectLength: u64,
    const SymbolLength: u64,
    const VersionLength: u64
> {
    case PeByName(
        library: [u8; ObjectLength],
        export: [u8; SymbolLength],
    );
    case PeByOrdinal(library: [u8; ObjectLength], ordinal: u16);
    case ElfVersioned(
        object: [u8; ObjectLength],
        symbol: [u8; SymbolLength],
        version: [u8; VersionLength],
    );
    case MachODylibSymbol(
        install_name: [u8; ObjectLength],
        symbol: [u8; SymbolLength],
    );
}

data Binding<
    const ObjectLength: u64,
    const SymbolLength: u64,
    const VersionLength: u64
> {
    case DllImport(
        import: DllImport<ObjectLength, SymbolLength, VersionLength>
    );
    case Syscall(number: u64);
    case VtableField(field: NativeFieldIdentity);
}
```

That is the eventual closed surface. The current compiler-owned
`core/external_binding.omg` carries all four `DllImport` cases plus
`Binding::DllImport` and `Binding::Syscall`. Ordinary syscall producers return
`Binding<0, 0, 0>`, and the evaluator accepts the number only for selected
Linux targets and the existing downstream `u32` syscall range. `VtableField`
remains on its visibly segregated legacy carrier until ordinary typed
`NativeFieldIdentity` values can replace it; `CompilerIntrinsic` never gains a
`Binding` case.

Exact cases may grow only when a genuinely different irreducible binding
mechanism exists. Host-specific flags and `host:` mini-languages are not part
of Omega. Foreign struct offsets and bit positions belong to programmable
layout/format declarations, not a generic `Binding::Value` escape hatch.
Foreign table calls name a field in that validated layout; authored numeric
slot ordinals are not binding identity. Compiler intrinsics carry no binding
value at all: the exact realization declaration, signature, and selected target
select the sealed catalog entry.
The first two production catalog rows are Linux
`Console::exit_process(i32) -> Unit` and `Console::write_byte(i32) -> Unit`.
Their target libraries author exact target-scoped bodyless `boundary machine`
declarations and satisfaction edges without payload-free `via`; provider
planning derives each compiler-intrinsic candidate from its retained
selected-source origin. Each physical catalog entry requires exact
accepted-package/toolchain custody of the requirement and realization symbols,
their normalized signatures and conformance, and a selected canonical Linux
profile. Canonical Terminal replay contributes only the exact demanded boundary
identity; later compiler-owned evidence rejoins that demand to the selected
plan before choosing `exit_group` or the single-byte `write` realization.
Lookalike or unscoped symbols, targetless plans, legacy-authored `via`,
uncatalogued sibling Console operations, and non-Linux targets confer no
physical catalog identity. The write-byte row additionally retains the exact
i32 source, its scratch-register materialization and stack interval, and the
balanced one-byte syscall bytes through object, image, installation, and D41
replay. Direct incoming-parameter custody remains closed until installation has
an exact scalar ABI ledger.
The exit row closes physical provider selection and emission only. D39 requires
a distinct checked terminal-effect completion identity before
`TerminalTraceV1` may observe successful external termination; neither the Unit
signature nor the backend's knowledge that `exit_group` does not return may
manufacture that semantic fact.
Privileged target instructions belong to parsed, contract-emitting `asm {}`;
`Binding::Instruction` is retired rather than preserving two ways to state the
same operation with different visibility to effect and authority analysis.

Target packages use ordinary target-scoped machines to compute these values.
The locator is one typed variant, so its object-format coordinates cannot drift
apart, while raw foreign bytes remain honest data rather than Omega names:

```omega
windows_x86_64 machine WindowsBindings::write_file() -> Binding<12, 9, 0> {
    Binding::DllImport {
        import: DllImport::PeByName {
            library: "kernel32.dll",
            export: "WriteFile",
        },
    }
}

boundary machine Kernel32::write_file(handle: WinHandle, bytes: &[u8]) -> WriteResult
    satisfies Kernel32Requirements::write_file
    via WindowsBindings::write_file();
```

The `via` expression must be compile-time evaluable to a normalized closed
`Binding<ObjectLength, SymbolLength, VersionLength>` application. The const
arguments are ordinary type identity and preserve the exact coordinate widths;
unused coordinates normalize to zero. A quoted literal in one of these
fixed-array positions copies exactly its source bytes, and a width mismatch
rejects. No evaluator reference or dynamically sized byte primitive crosses
the boundary. The binding value is the external-realization variant of the
machine supply slot, not an executable body and not a self-authored trust
assertion. Its identity feeds the derived plan. Structural validation checks
the declaration; admission assigns the trust class and receipt.

The satisfied boundary requirement independently owns its `Calling<C, Policy>`
relationship. Evaluating that policy against the normalized signature produces
the `CallPlan`; the binding mechanism must refine it but never carries or
reselects a duplicate plan.

Binding identity is never reconstructed by looking up text. Every explicit
evaluated `Binding` is normalized and fingerprinted together with its producer
closure and selected target. Compiler-intrinsic identity instead comes directly
from the exact boundary-machine symbol, signature, and selected target; there is
no empty binding value to retain. For a DLL import, the typed locator variant
owns all physical coordinates as one value; a raw library, export, version, or
ordinal is neither an Omega symbol nor a requirement/provider-selection key.
`build.omg` may select the target package or provider but cannot replace fields
inside its evaluated binding.

Validation is variant- and target-specific: it checks required nonempty fields,
forbidden terminators/bytes, ordinal ranges, object-format encoding, target
applicability, unused-coordinate zeros, and any versioning rules before the
binding enters a provider plan. Fixed byte arrays supply ordinary owned values,
not an ambient text interpretation; the locator case supplies their physical
meaning.

The Rust comparator now has the first dependency-light representation rung: a
sealed target-bound normalized locator for atomic PE-by-name, PE-by-ordinal,
versioned-ELF, and Mach-O dylib-symbol coordinates. It validates target
applicability and basic coordinate shape and derives a domain-separated,
length-prefixed compatibility identity. Provider import rows and opaque
executable-TCB projections now retain that whole normalized locator, and
package-review/manifest output preserves its target, case, identity, and raw
coordinates without rebuilding strings. The current source evaluator is
visibly segregated as a temporary string-backed bootstrap. Trust artifacts now
carry the atomic locator and render exact raw coordinates without text
reconstruction, rejecting target drift before report installation. The calling
bridge, ordinary authored machine validation, object locator side table,
relocation replay, and PE name/ordinal emission retain that same atomic value.
Mach-O AArch64 final emission now consumes `MachODylibSymbol` without decoding
either coordinate as text. A whole-import preflight validates exact target and
case, unresolved symbol shape, normalized-identity uniqueness, the exact raw
install-name roster, and the current fifteen-ordinal encoding limit before any
thunk, symbol, data, or executable-region mutation. `LC_LOAD_DYLIB` payloads and
dyld bind symbols use the retained raw bytes; load commands deduplicate by exact
install name in first-reference order, and their image-local ordinals remain
derived layout data rather than locator identity. The object-local symbol name
survives only as a diagnostic and executable-region label.
Versioned ELF rows now reach a canonical final-image request
with exact raw object/symbol/version coordinates and relocation sites. The
first dependency-light loader-plan input rung only seals one exact
target/deployment-supplied interpreter pathname for a Linux x86-64 or AArch64
profile. It preserves raw non-UTF-8 bytes, rejects paths that are empty,
relative, or contain NUL bytes, and fingerprints the exact profile and
length-framed bytes.
The first ELF-owner join consumes one exact final image beside that input and
accepts only its nonempty canonical referenced `ElfVersioned` row set under the
same Linux profile. The non-clone carrier privately retains every symbol
handle, raw locator, normalized identity, and relocation site. Target drift,
string-backed or unused interpreter input, and canonical-request failure return
the original image and interpreter unchanged. These carriers grant no loader,
section, publication, or admission authority. Runnable dynamic emission stays
fail closed. The first complete address-free table plan now consumes that
preflight and independently replays an exact NUL-terminated `PT_INTERP`
payload, canonical raw-byte `.dynstr`, the reserved undefined `.dynsym` row
plus one sorted undefined global function row per import, one concrete System V
`.hash`, one address-free `.gnu.hash`, parallel `.gnu.version`, grouped
`.gnu.version_r`, private import-to-symbol/version indexes, and the exact
`DT_NEEDED` string-index roster. Shared
strings, objects, and object/version requirements deduplicate by exact bytes;
permuted import insertion cannot change the table contents or their
deterministic identity. The GNU-hash carrier preserves the canonical dynamic-
symbol order with `symoffset == 1`, one bucket, one 64-bit bloom word, shift
five, and one exact bounded chain. Independent replay rejects header, bloom,
bucket, chain, or terminator drift. Its layout, two-bit bloom lookup, low-bit
chain terminator, and DJB-derived symbol hash follow the original GNU
[`DT_GNU_HASH` implementation](https://sourceware.org/pipermail/binutils/2006-July/048074.html);
the adjacent GABI hash rules below govern only the distinct System V `.hash`
table.

The table invariants follow the primary [System V ABI program-header
rules](https://gabi.xinuos.com/elf/07-pheader.html), [string-table
rules](https://gabi.xinuos.com/elf/04-strtab.html), [symbol-table
rules](https://gabi.xinuos.com/elf/05-symtab.html), and [dynamic hash
rules](https://gabi.xinuos.com/elf/08-dynamic.html#hash-table), together with
the [LSB symbol-version requirement
format](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html).
The plan still grants no loader, layout, publication, or runnable-image
authority. The next sealed rung serializes its contents as seven exact ELF64
`ELFDATA2LSB` payloads: `.interp`, `.dynstr`, 24-byte `Elf64_Sym` rows in
`.dynsym`, the word-oriented `.hash`, the GNU-hash header/bloom/bucket/chains,
half-word `.gnu.version` rows, and
16-byte `Elf64_Verneed`/`Elf64_Vernaux` chains in `.gnu.version_r`. A distinct
bounds-checked decoder replays exact lengths, rows, hash indexes, linked
version offsets, dynamic-string references, and section-kind boundaries before
sealing payload identity. It rejects truncation, trailing bytes, endian drift,
invalid counts/indexes, offset cycles, and mutation while retaining the exact
validated structural plan. The wire rules come from the primary System V ABI
[ELF64 data sizes and
alignment](https://gabi.xinuos.com/elf/01-intro.html#sixty-four-bit-data-types)
and [least-significant-byte-first
encoding](https://gabi.xinuos.com/elf/02-eheader.html#data-encoding).

A following sealed descriptor rung now binds all seven payloads to seven
address-free semantic section kinds with their exact ABI type, flags, payload
size, alignment, entry size, semantic link, and `sh_info` meaning. Independent
validation replays every name, payload relationship, symbol/version count,
System V hash chain count, and version-need object count before sealing a
deterministic descriptor identity while retaining payload custody on failure.
An append-only section-name seed fixes the current `.interp`, `.dynstr`,
`.dynsym`, `.hash`, `.gnu.version`, `.gnu.version_r`, and `.gnu.hash` offsets and reserves
`.shstrtab`; it is not a complete name table and supplies neither a `.shstrtab`
descriptor nor final numeric section indexes. The metadata follows the primary
System V ABI [section-header, type, flag, link, and info
rules](https://gabi.xinuos.com/elf/03-sheader.html) and the LSB [GNU section
type assignments](https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/sections.html).

The first address-free procedure-linkage relocation rung consumes those
descriptors and joins every private import binding to its exact canonical
request and source call sites. It admits only intact unresolved x86-64 `CALL
rel32` or AArch64 `BL` placeholders with the matching four-byte, zero-addend
text relocation. One canonical logical PLT/GOT slot belongs to each imported
dynamic symbol and carries one semantic RELA `JUMP_SLOT` requirement
(`R_X86_64_JUMP_SLOT` or `R_AARCH64_JUMP_SLOT`); multiple calls to one import
share that slot. Target drift, non-procedure uses, malformed or overlapping
sites, and binding/slot/relocation drift reject with exact descriptor custody.
Because every unresolved use is accounted for as a procedure call, the plan
proves that the current admitted image needs no general `.rela.dyn` row. The
relocation contracts follow the primary [x86-64
psABI](https://gitlab.com/x86-psABIs/x86-64-ABI) and [AArch64 ELF
ABI](https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst).
Logical slots and relocation requirements grant no address, physical
GOT/PLT/section index, serialized relocation, placement, or mutation authority.

The next sealed target-template rung consumes that semantic linkage and emits
only fixed ELF64-LSB `.plt`, `.got.plt`, and `.rela.plt` bytes plus exact zero
placeholders. Its x86-64 small/medium lazy-binding policy gives GOT[0] the sole
semantic `_DYNAMIC` fixup, leaves GOT[1]/GOT[2] reserved, and directs each
import slot to its PLT lazy tail. Its AArch64 standard lazy policy leaves all
three GOT header words zero/reserved, directs each import slot to PLT0, and
never assigns `_DYNAMIC` to AArch64 GOT[0]. Typed fixups cover every
placement-dependent PLT/GOT/RELA field and unresolved source call, with
explicit signed-displacement, page-delta, low-12 alignment, and branch-range
constraints. A separate replay validates exact opcodes, relocation symbol/type
rows, zero mutable fields, nonoverlap, semantic targets, constraints, and
deterministic identity while preserving the original linkage plan on failure.
The sequences follow the primary [x86-64 dynamic-linking
rules](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/dl.tex)
and [AArch64 procedure-linkage-table
rules](https://github.com/ARM-software/abi-aa/blob/main/sysvabi64/sysvabi64.rst#procedure-linkage-table).
They still grant no address, physical section index, placement, resolved fixup,
image mutation, or runnable-image authority.

The following address-free descriptor rung retains those templates and extends
the owning section-name seed append-only from 79 to 103 bytes with `.plt`,
`.got.plt`, and `.rela.plt` at offsets 79, 84, and 93. The three semantic rows
bind the exact template payload sizes to their ABI section types, flags,
alignments, and entry sizes. AArch64 `.plt` retains
`SHF_AARCH64_PURECODE`; `.rela.plt` retains typed `.dynsym` `sh_link` and
`.got.plt` `sh_info` meaning together with `SHF_INFO_LINK`, without assigning
either numeric index. Independent replay checks the unchanged seven-row seed
prefix, every appended name and metadata field, row order, target distinction,
and deterministic identity while preserving template custody on rejection.
The result is a ten-semantic-section view, not a completed section roster or
`.shstrtab`.

A following semantic `.dynamic` plan now retains the significant exact
`DT_NEEDED` prefix followed by the complete owned fixed tag roster and one
trailing `DT_NULL`. Literal rows bind `.rela.plt` and `.dynstr` byte counts,
the `Elf64_Sym` entry size, RELA kind, and exact version-requirement record
count. Eight typed zero-address obligations target `.got.plt`, `.hash`,
`.gnu.hash`, `.dynstr`, `.dynsym`, `.rela.plt`, `.gnu.version`, and `.gnu.version_r`
without assigning a pointer or numeric index. Independent replay checks raw
library-name offsets and significant order, exact tag multiplicity/order,
relocation closure, the target-specific future-`.dynamic` GOT policy, every
literal/obligation, identity, and descriptor custody. General RELA,
bind-now, text-relocation, init/fini, runpath, soname, and target-optional tags
remain absent because the sealed inputs own none of those meanings. A further
serialization carrier consumes the plan into exact 16-byte ELF64-LSB
`Elf64_Dyn` rows: signed `d_tag` then unsigned `d_un`, both little-endian.
Literal values are copied exactly; address values and the final null value stay
zero, with the eight address obligations translated to typed eight-byte fixups
at their exact value-field offsets. An independent bounded decoder requires
the exact row count with no trailing bytes and replays endianness, row order
and values, fixup bounds/non-overlap/targets, deterministic identity, and plan
custody. The serialized carrier supplies no `.dynamic` name, descriptor,
section index, address, or placement authority. The following address-free
descriptor carrier extends the exact 103-byte append-only name seed with raw
`.dynamic\0` at offset 103 and retains one semantic `SHT_DYNAMIC` row with
writable/allocated flags, exact payload size, alignment eight, entry size
sixteen, a typed `.dynstr` link, and no info relationship. Independent replay
checks the complete 112-byte seed, raw name, unique semantic link, every field,
identity, and payload custody. No final numeric `sh_name`/`sh_link`, section
index, address, or placement is assigned. The section-name-table carrier then
adopts the unchanged exact 112-byte seed as the complete `.shstrtab` payload.
The name reserved once at offset 59 remains unique, `.dynamic\0` ends the table
at byte 112, and the semantic descriptor records `SHT_STRTAB`, no flags, size
112, alignment one, zero entry size, and no link/info relationship. Independent
bounded replay walks every contiguous NUL-framed name and checks every byte,
field, identity, and exact `.dynamic`-descriptor custody. It assigns no numeric
section index, `e_shstrndx`, address, or placement. A numeric-roster carrier
then consumes that owner into thirteen closed rows: null index zero; the retained
seven base rows at 1–7; `.plt`/`.got.plt`/`.rela.plt` at 8–10; `.dynamic` at 11;
and `.shstrtab` at 12, which is also the exact `e_shstrndx`. Existing `sh_name`
offsets and literal `sh_info` values survive unchanged; semantic links and the
relocated-section info relationship resolve to exact in-roster indexes.
Independent replay checks order, unique coverage, every metadata
field/reference, identity, and name-table custody. The roster assigns no
address, file offset, or payload placement. A section-header serialization
carrier then consumes the exact roster into thirteen 64-byte ELF64-LSB
`Elf64_Shdr` templates, 832 bytes total. It copies every numeric roster field
and leaves each `sh_addr` and `sh_offset` placement field zero. Twenty-three
typed fixups identify the twelve non-null file offsets and the eleven allocated
virtual addresses. An independent bounded decoder rejects truncated or
trailing bytes and replays every field and row, exact fixup order and
coordinate, zero placeholder, bounds and non-overlap, identity, and roster
custody. The carrier grants no placement or `e_shoff` authority.
A following indexed-payload carrier consumes those templates and binds every
numeric row to its exact already-owned bytes: null, the seven base payloads,
PLT/GOT.PLT/RELA.PLT, `.dynamic`, and `.shstrtab`. Row bytes replay against
their upstream serializers and lengths replay against numeric `sh_size`.
Procedure-linkage and source-text obligations remain a distinct typed fixup
family, while the eight `.dynamic` obligations remain another; both map their
storage and semantic targets to exact numeric section rows without resolving an
address. Source text stays an explicit non-section storage domain, and the
twenty-three section-header placement fixups are not duplicated. Independent
replay checks all rows, bytes, sizes, fixups, mutable masks, zero placeholders,
constraints, storage bounds and targets, identity, and exact header-template
custody. The carrier grants no address or placement authority.

A relative payload-layout carrier now consumes that indexed roster and derives
one exact read-only, read-execute, read-write, or file-only domain for every
non-null row from its retained `sh_flags`. Within each domain, numeric roster
order and `sh_addralign` determine checked relative offsets and a complete span;
independent replay rejects row, domain, order, geometry, span, identity, or
overflow drift while preserving indexed-payload custody. Every domain begins at
relative offset zero. These are neither `sh_offset` nor `sh_addr`, and the
carrier grants no absolute base, segment, header-fixup, byte-mutation, loader,
publication, or runnable-image authority.

The following absolute-load carrier consumes that exact relative owner. Both
current Linux profiles select the fixed `0x400000` image base and a sealed
64-KiB maximum-page alignment, while AArch64 instruction relocations continue
to use their distinct 4-KiB `Page(expr)` rule. The carrier closes one canonical
`PT_INTERP`, R/RX/RW `PT_LOAD`, `PT_DYNAMIC` order with
`p_paddr == p_vaddr`, congruent load offsets/addresses, and strict W^X. The R
load owns the ELF/program-header prefix and read-only dynamic sections; RX owns
the retained source text and `.plt`; RW owns writable dynamic sections,
initialized source data, and the aligned BSS zero-fill tail. `.shstrtab` and
the 832-byte section-header table receive file-only coordinates outside the
load file extents. All twenty-three typed section-header placement obligations
receive exact retained values without mutating their zero templates.
Independent replay checks source, section, segment, alignment, alias, and
fixup coverage plus the deferred procedure-placement envelopes. This remains
geometry only, not loader, publication, or runnable-image authority.

The section-header application carrier now consumes that exact absolute owner,
copies the retained 832-byte template, and applies only the twelve resolved file
offsets and eleven resolved virtual addresses to their exact typed zero
placeholders as little-endian `u64` values. Its independent bounded decoder
replays all thirteen roster rows, every unchanged field, exact application
order/coordinate/kind/value, null and file-only `.shstrtab` semantics,
deterministic content-bound identity, and load-layout custody. It assigns no
`e_shoff`, serializes no ELF or program header, mutates no `FinalImage`, and
grants no runnable authority.

The internal `.dynamic` application carrier now consumes that placed-header
owner, copies only indexed roster row eleven, and resolves the exact eight
`DT_PLTGOT`, `DT_HASH`, `DT_GNU_HASH`, `DT_STRTAB`, `DT_SYMTAB`, `DT_JMPREL`,
`DT_VERSYM`, and `DT_VERNEED` address obligations from their allocated section virtual
addresses. It patches only the typed zero `d_un` fields as little-endian `u64`
values. Independent replay rejoins the semantic tag plan, serialized fixups,
indexed storage, target section identities, every literal/null and unchanged
byte, deterministic identity, and the complete placed-header/load-layout
custody chain. It does not resolve procedure/source relocations, serialize
headers, mutate the image, or grant loader authority.

The dynamic file-envelope carrier now consumes that resolved owner and emits
the exact 64-byte ELF64-LSB header followed by the five already-planned 56-byte
program-header rows. It binds the retained entry symbol, target machine,
`e_shoff`, fixed table geometry, every exact load-layout row, and the already-
applied 832-byte section-header table as a separate file fragment at that exact
offset. An independent bounded decoder rejoins both fragments to the entry,
absolute layout, and placed-header owner, and rejection returns the complete
resolved owner. This carrier is deliberately non-runnable: it copies no payload
into an image, applies no procedure or source relocation, and mutates no
`FinalImage`.

A following non-runnable procedure-linkage rung copies the retained source
`.text`, `.plt`, `.got.plt`, and `.rela.plt` fragments and applies every exact
indexed procedure/source fixup from the absolute load layout. Its typed
application ledger retains storage, coordinate, encoding kind, semantic target,
source and target addresses, and encoded field. Independent replay rederives
each target and encoding, checks range, alignment, mutable masks, fixed opcode
bits, complete fixup coverage, and every unchanged byte. Rejection returns the
complete file-envelope owner, and the retained `FinalImage` remains immutable.

The next non-runnable assembly carrier consumes that resolved-linkage owner and
places the exact header prefix, retained source text/data, all twelve non-null
section payloads, resolved `.plt`/`.got.plt`/`.rela.plt` and `.dynamic` bytes,
file-only `.shstrtab`, and the applied section-header table into one owned file
buffer at their absolute offsets. Its typed fragment ledger and independent
replay check exact source custody, file extents, non-overlap, complete fragment
coverage, and zero-filled alignment/page gaps; BSS remains memory-only and the
retained `FinalImage` remains immutable. Rejection returns the complete
resolved-linkage owner.

The final-byte admission rung now consumes that assembly, recovers the exact
retained `FinalImage` through the complete ownership chain, applies only the
already-resolved source-text bytes, and independently rejoins the complete
assembled file, Linux target-specific format, image statistics, and placed
executable-region inventory. Rejection retains the intact assembled owner;
success retains the mutated image beside exact `ExecutableImageOutput` bytes
but grants no publication receipt or execution event. The first production-
emitter bridge now consumes only that admitted carrier and independently
rejoins its exact final image, import/relocation counts, compiler-text
relocation envelope, executable-region inventory, target, and bytes to the
borrowed source-free `ObjectArtifact`. Any target or artifact drift rejects
with the admitted carrier intact. Success is deliberately a distinct
non-installable custody carrier rather than `ExecutableImage`, so the bridge
adds no installation, publication, or execution authority. A complete source-
free chain driver now consumes one exact import-bearing `ObjectArtifact` plus
one `NormalizedElfInterpreterPlan`, builds the final image, and advances
transactionally through every existing section, linkage, placement, fixup,
assembly, admission, and production-bridge owner.
Each rejecting rung is retained in a stage-tagged error rather than flattened
to a diagnostic; a malformed procedure-call placeholder therefore returns the
descriptor owner still carrying the exact image, interpreter, and normalized
imports. Both Linux profiles replay to deterministic production output, while
the result remains the non-installable custody carrier above. The object-bound
image-emission request router now derives its writer path from that exact
object custody. Its authority-distinct direct request carries only the PE
subsystem, and its dynamic-ELF request carries only the consumed normalized
interpreter. An import-bearing ELF object cannot fall back to direct emission;
an unused or mismatched interpreter rejects with typed custody intact.
Independent replay preserves the distinction between ordinary
`ExecutableImage` authority and the non-installable dynamic result. The dynamic
result retains the complete source `ObjectArtifact`; replay therefore rejects
PSI-only or other semantic/evidence substitution even when object bytes and
layout are unchanged. This does not supply loader policy or the compiler's
general native-artifact admission owner. The first compiler/native-artifact
continuation now threads that request through the ordinary Terminal-to-object
owner. The direct compatibility entrance still returns only the existing
`NativeArtifact`. Import-bearing Linux custody instead returns a distinct
`DynamicElfNativeArtifact`, retaining the complete canonical Terminal, object,
selected-plan closure, provider execution, Terminal authority review, D29
coverage, D32 evidence, and requested image inputs. Validation independently
reruns object-bound writer selection and rejects PSI-only or outer-object
substitution; rejection returns the exact image request. The dynamic carrier
is authority-free and cannot enter installation or publication APIs. The
object-boundary production-retention prerequisite is now source-free:
`MachineCodeFunction` can own ordered `ForeignCallRelocation` rows joining one
semantic call owner and exact native relocation field to the complete
`NormalizedForeignLocator`. Ordinary object construction independently replays
the x86-64 `CALL rel32` or AArch64 `BL` placeholder, semantic provenance,
target, order, uniqueness, and diagnostic-fingerprint collision boundary. It
then deduplicates exact locators into unresolved import symbols, the atomic
locator side table, and distinct call-site relocations. The dynamic-ELF driver
tests now begin at this ordinary object builder rather than constructing its
private result. The first preceding production path is now closed for one
`Unit`-returning normalized import leaf with no scalar arguments or one through
the target's complete fixed-width 8/16/32/64-bit integer argument plan: six
SysV x86-64 registers through `R9` followed by canonical 8-byte outgoing stack
slots, or eight AAPCS64 registers through `X7` followed by canonical 8-byte
outgoing stack slots. Every evaluated placement must be one exact complete
register or stack fragment from the selected plan. An argument may be an exact
literal or the runtime result in an exact preceding attached-`Unit` scalar
call's durable home. Checked compilation
retains extracted external-binding rows before consuming typed trees. Native
settlement rejoins one unique retained row only through the complete selected
`ProviderPlan`, its exact selected-plan evidence, and the admitted same-stack
contribution; compact report fingerprints provide no plan-selection authority,
and an equal-report substitute rejects. A distinct target operation survives
assignment. Ordinary x86-64 and AArch64 machine emission then produces the
unresolved `CALL rel32` or `BL` field and retains the complete normalized
locator, provider execution, evaluated call plan, admitted contribution, and
physical `Unit` stack evidence in the foreign-call row. Every literal-bearing
row additionally binds its occurrence-specific source value, integer type and
immediate, parameter index, evaluated register-or-stack placement, and exact
materialization/store byte interval. Runtime-home rows instead bind the exact
source value and assigned durable home plus their load/store interval. Machine
emission derives the exact outbound extent from the canonical plan, allocates
it before every argument, rebases durable-home loads across the adjustment,
and releases it immediately after the call. With multiple arguments, the rows
and intervals stay in parameter order between allocation and call and every
interval ends exactly where the next begins. The
complete register-resident fixed-integer result family is also closed: exact
signed and unsigned 8/16/32/64-bit results from normalized foreign calls in
attached `Unit` bodies are normalized from their evaluated result registers
into durable 64-bit scalar homes and may feed later normalized foreign calls.
The declared sign and width, canonical shape, home roster, producer ordinal,
exact result-store interval, and later argument-load interval survive ordinary
object construction and both Linux dynamic-ELF drivers. The Windows x86-64
lane now carries the same exact `u32` producer result from its evaluated RAX
placement into a durable home, then reloads that home into the evaluated RCX
placement of a later normalized PE import. Object and final-PE replay retain
both atomic locators, Win64 shadow-space custody, ordered result-store/argument-
load byte intervals, and both relocations; stripped or drifted home/type/shape/
ordinal/placement identity rejects. Machine emission
rejoins each row to its preceding constant, exact preceding scalar-call
producer, or exact preceding normalized-foreign result producer and emits the
compact target register materialization.
Object construction repeats that semantic rejoin—including the literal source-
value check—and independently replays the complete ordered call plan, result
normalization/store bytes, placements, semantic call ownership, and physical
stack custody before consuming the rows. Both Linux profiles advance from the
exact native rejoin through target, assignment, machine, ordinary object
construction, and the complete dynamic-ELF driver. Stripped, reordered, or
drifted source/type/value/home/index/register/byte/plan/stack custody rejects.
General runtime expressions, non-fixed-integer, float, aggregate, and indirect
result shapes, the external-root
`StackPlan`/lease/entry-epoch join, stronger foreign-call alignment, optional
hash-policy extensions beyond the now integrated `.hash` plus `.gnu.hash`
carriers, general external-admission ownership, and later admission/publication
integration beyond the authority-free requested native candidate remain open
engineering work. An owned direct `[u8; N]` destination now contextually copies a quoted
literal into an ordinary raw-byte array only when `N` is a
resolved integer literal and the source byte count matches exactly; non-byte
or unresolved/mismatched widths reject, and hermetic evaluation observes the
array value. Producer closure, evaluator receipt, and ordinary source `via`
evaluation are live through the target-constrained Terminal proposal. That
proposal exactly rejoins every selected evaluated import to one retained
physical row and rejects missing, duplicate, locator-substituted,
legacy-shaped, or unmatched normalized rows. Native settlement still requires
admitted task-stack custody. A first consuming coordinator now accepts that
custody beside external provider-execution evidence without accepting a caller-
chosen plan, index, target, or locator. It derives exact demand from canonical
Terminal operations and realizes a zero-argument structured
`MachODylibSymbol` source through target lowering, assignment, machine code,
ordinary object construction, and a validated AArch64 Mach-O image. Object and
image custody retain the exact semantic owner, normalized locator, execution
record, and relocation offset. Missing, duplicate, extra, wrong-plan, builtin-
substituted, and callback-bearing inputs reject. The coordinator grants no
deployment admission and does not synthesize provider execution or stack
authority. Specialized string-only adapters and any remaining versioned-ELF
extensions remain to migrate; production library source no longer uses the
legacy string-pair `Binding::DllImport` form.

Changing raw foreign bytes changes the normalized binding, forces every final
artifact whose reachable closure contains it to relink, and requires fresh
admission. No parallel endpoint registry or sealed metadata language exists.
Audit reports enumerate the actual evaluated locator rather than a nominal name
that could map elsewhere.

Composite behavior is checked Omega code rather than plan-shaped call
sequences. A Console adapter that gets a standard handle, appends a newline,
and performs one or more writes is an ordinary machine satisfying the Console
requirement. This permits caching, batching, policy, and stronger contracts
without extending a call-shape DSL. Constants and foreign formats similarly
stay in their existing semantic homes.

The toolchain computes the selected provider type's conformance closure and
derives plan coverage, signatures, effect summaries, dependencies, normalized
identity, and admission inputs. Only explicit `satisfies` edges participate;
structural coincidence never makes a provider. Build-time machines may select
among declared candidates or compute a leaf `Binding` value, but they never
imperatively append plan rows.

`Binding` values themselves may be constructed freely; construction grants no
authority and makes no provider selectable. Provider handling then has four
distinct artifact stages: derive a candidate deterministically from
declarations; validate structural coverage, signatures, calling/layout
plans, and normalized identity; admit its semantic claims under boundary grant
authority and issue receipts; then select it for a slot under that slot owner's
capability. A target package supplies ordinary provider-type defaults,
`build.omg` selects a default target profile, and explicit
build/test/component configuration may override individual slots. Defaults
are package declarations/data, not compiler magic. Slot selection changes the
selected provider; it does not reconstruct its rows. Every authored slot path
must resolve to exactly one canonical boundary-trait identity in the loaded
closure. An exact canonical name wins; a short leaf fallback is accepted only
when unique, and qualified/unqualified aliases cannot be used to select the
same slot twice.

The selected plans survive typed-to-checked lowering as one canonical checked
fact set. Every retained plan is revalidated as fully covering, selected names
must resolve exactly once, and duplicate or identity-colliding selections
reject. Later backend, generated-machine, and provider-execution work consumes
that immutable normalized carrier rather than scanning `satisfies`
declarations again. The carrier publishes both each plan's normalized identity
and a deterministic identity for the complete selected set. External-root
construction resolves its boundary slot against this carrier and copies the
resulting plan identity into the root candidate before validation; an absent or
ambiguous retained selection rejects.

The compiler's exact external-binding projection is a separate non-authorizing
sidecar of the checked phase result. It derives from checked-retained typed
declarations, the original ordered selected-plan input, and the already-
evaluated boundary calling-plan realizations. Projection stages every row
before publication; equal or empty settlement preserves the existing Arc, and
rejection preserves its prior identity and contents. Backend planning consumes
only that retained sidecar rather than a pre-lowering vector couriered by the
driver. The sidecar preserves binding/calling-plan identity and order but grants
no provider admission, selection, ABI, or execution authority.

There is no parallel source-level primitive-provider registry. The retired
top-level `provider Name : Category;` declaration and operator-local
`provider Name` clause are bootstrap artifacts; requirement declarations do not
select their implementations. Checked satisfiers and bodyless boundary
satisfiers declare candidates; only leaves with an undiscoverable payload carry
`via`. Target defaults, `build.omg`, or installation choose admitted provider
plans through owned slots.

The normalized service schema also retains each linear routed parameter
qualification as a structured entry claim. Its carrier-aware semantic-domain
identity, `accepts` authority-flow verb, and born-strict compiler carry policy
participate in provider-plan identity. The external-root selection bridge
copies those rows beside that identity, and the qualification artifact reports
them with the selected-plan receipt. The row records what an admitted external
entry may supply; only the matching concrete entry receipt establishes a source
fact for one invocation. The durable trust report copies the same normalized
provider-schema claims rather than parsing type displays: exact plan
fingerprint, requirement, parameter/result subject, authority flow, semantic
domain, carry policy, predicate-discharge requirement, and grant provenance.

Checked-adapter dispatch consumes that retained carrier as well. Only an exact
`CheckedAdapter` row in the selected plan may rewrite the corresponding
boundary call; an unrelated or unselected adapter cannot overlay the selection.
Every selected schema method and row carries a nonempty canonical overload
identity. Name-only singleton matching is not a compatibility form: the
readable method name is only a drift check beside exact identity.
Every checked adapter belongs to a nominal provider type. The rewrite retains
the selected entry-state symbol and complete nominal machine name for both
statement and value calls. Standard Console publishes one complete nominal
provider closure per hosted target, with checked
`write`/`write_line` adapters and compiler-intrinsic rows selecting the
target's existing `read_line`, `read_byte`, `write_byte`, and process-exit
lowerings.

The static build root names a boundary service and one exact nominal provider
type, for example
`builder.select_provider<target::Console, SerialConsole>();`. The selection is
harvested only from the authoritative `build.omg` machine, and succeeds only
when that provider's derived candidate exists in the loaded dependency closure,
applies to the selected target, and covers the complete slot schema. Thus the
selection grants neither rows nor trust; it spends the build root's slot-
selection authority over an already-derived and independently admitted
candidate.

The satisfied requirement supplies the public contract, including service
reach, suspension, blocking, and guarded-crash ceilings. The external
realization's behavior is derived from the binding/provider contract and must
refine every ceiling at validation/admission. A `via` machine does not repeat
those clauses.

This is one boundary-contract shape, not FFI-only ceremony. A checked Omega
provider derives facts from its body. An opaque provider supplies admitted
facts through its binding. Trust is classified per fact, and a composite
guarantee reports the weakest input together with the exact provider premise
that made it so.

```text
StackPlan
  class: admitted
  input: Firmware.foreign_stack_ceiling

callback_acyclic
  class: derived
  input: checked invokes graph
```

## Reach, authority, and trust

Decision 22 applies without an extern exception:

- the boundary-trait identity contributes service reach;
- capability/evidence values carry authority;
- the selected external provider produces a trust receipt; and
- `suspends`/`blocks` and guarded crash routes are independent
  operation/provider ceilings when applicable.

A checked wrapper may refine operational behavior or reduce trust expenditure;
it does not erase the abstract service reach from callers compiled against that
trait.

Reach and executable trust remain separate. Checked bodies infer complete
service reach, bodyless surfaces publish it, and callers receive the transitive
closure. Static import is not a runtime operation: a call through a statically
selected Windows provider reaches `WindowSystem`, while an explicit runtime
loader call additionally reaches `DynamicLibraryLoading`. Deployment policy
decides which reach entries warrant refusal or a loud report; capability
authors do not opt into propagation.

TCB expansion is a selected-provider property. The same source requirement may
select checked Omega, an opaque in-process binary, or an isolated endpoint.
Selection therefore contributes a normalized executable entry to the artifact
without changing the source reach contract. Each known entry retains:

- exact provider, provider-plan, and executable/artifact identity;
- implementation evidence class and admission provenance;
- origin as static selection or Omega-mediated runtime admission;
- execution scope; and
- scoped containment guarantees with their own trust evidence.

Containment guarantees are named by what they establish: memory isolation
outside explicitly shared authority, forcible termination, fault containment,
and bounded resource use. Mechanism names do not imply the complete set. A
process needs explicit quotas before it supplies resource containment; a
same-address-space mechanism supplies only the guarantees its admitted
enforcement actually establishes. Implementation evidence and containment
remain independent axes: an admitted hardware or instruction fact is not an
opaque executable in the caller's address space.

The manifest separately reports whether its known entry list is complete for
one execution scope:

```text
Complete(scope, evidence)
Incomplete(scope, attributed uncontained providers)
```

An uncontained opaque in-process binary makes the caller-address-space
manifest incomplete. It may load or generate executable code without using
Omega's loader, so the runtime ledger can report only entries Omega admitted,
not every executable actually present. A constrained dynamic-loading envelope
is enforceable only inside a containment regime that controls executable
admission. A checked adapter cannot remove this provenance.

The implemented runtime ledger is an append-only snapshot scoped to one exact
execution domain. Only its Omega-mediation boundary can add an entry, and that
boundary requires pinned executable, provider-plan, implementation-evidence,
and admission-receipt identities; it rejects receipt replay and has no path or
loader-name identity input. Union marks each such entry as a runtime admission.
Without separate executable-closure evidence the entry remains known but adds
an attributed incompleteness cause. With that evidence, the union retains a
complete static scope as complete; evidence remains visible beside unrelated
causes, and repeated union is idempotent.

Build and deployment profiles evaluate the selected entry set, manifest
completeness and evidence, required containment guarantees, and approved
platform or third-party identities. Platform baselines are policy allowlists,
not different language semantics. Development profiles may admit and mark an
incomplete artifact; safety profiles fail before artifact installation when
their requirements are not met. An isolated provider is an endpoint in the
caller's manifest and receives its own executable manifest for its execution
scope.

The implemented isolated-scope carrier makes that separation structural. A
selected closure is assigned a nonzero isolated scope before opaque admissions;
the manifest-set admission then binds the child's exact manifest and admission
receipt to one exact endpoint entry in the parent. Endpoint containment stays
on the parent entry, while every child entry and its completeness result remain
under the child scope. Scope drift, duplicate child scope identity, and mixed-
scope child entries reject. Parent and child profiles are evaluated separately.

Binding authors publish the widest contract they can honestly support.
Over-approximation may cost usability: an unconstrained synchronous invocation
ceiling rejects from an acyclic context, and a blocking edge without finite
wait evidence yields `NoFiniteGuarantee(Edge(edge), UnboundedWait)`.
Under-approximating an opaque
provider is an unsound admitted claim. The compiler checks the consequences and
internal coherence of a declaration; it cannot establish its truth from a DLL.

## Boundary declaration coherence

The checker reads existing contract axes rather than a separate foreign-use
plan:

- bodyful checked providers infer operational facts; bodyless surfaces author
  their ceilings;
- `blocks` must fit the caller's blocking ceiling and carry the source
  acknowledgement at the call site; without selected finite wait evidence the
  response report is `NoFiniteGuarantee(Edge(edge), UnboundedWait)`;
- `invokes` contributes direct synchronous edges and rejects a realized
  component-boundary cycle;
- a reference grants use only before return; a result claim retaining storage
  after return must receive that authority from a consumed input through the
  ordinary conservation mapping;
- the selected executor must satisfy the operation's thread or apartment
  affinity; and
- `addr` and `Ptr<T>` remain inert ABI data and cannot substitute for an
  established storage claim.

## Calling plans

Boundary-entry behavior is one normalized artifact with independent `CallPlan`
and `StatePlan` facets. The first owns parameter/result placement and ordinary
ABI clobbers; the second owns initial machine regime, interrupted state,
save/restore commitments, and permitted transitive machine-state use. It is not
inferred from library or symbol strings.

Bindings cite a plan identity. Provider admission verifies that the binding and
entry stub implement the pinned boundary-machine contract. See
[`calling_plans.md`](calling_plans.md).

The evaluated plan belongs to the satisfied requirement through ordinary
`Calling<C>` policy composition. The old `boundary(<Plan>)` marker is retired;
`boundary` identifies the trust/supply edge and does not carry deployment data.

### Floating control state

`f32` and `f64` requirements assume Omega's canonical semantic floating-control
configuration. A native boundary must therefore state how the relevant control
bits cross it:

- a preserving binding proves that the foreign call leaves the masked
  MXCSR/FPCR semantic controls unchanged;
- a general binding saves and restores those controls in its trampoline; and
- an inbound callback establishes the canonical Omega controls before checked
  code runs, then restores the foreign controls on exit.

Sticky floating status flags are not part of this semantic invariant.
Directed-rounding operations do not alter ambient control state, and
`Trapping` does not unmask hardware exceptions. A library or callback that
silently enables FTZ/DAZ cannot leave behind a valid Omega hardware-float
realization.

The settled first outbound realization is conservative by mechanism: every
returning import or indirect vtable/table call receives an aligned trampoline
that saves and restores the caller's complete MXCSR/FPCR around the existing
call sequence. Direct syscalls do not execute a returning user-space
counterparty and receive no envelope. The maintained x86 backend implements
this for ordinary returning foreign calls: it saves complete MXCSR before
outbound allocation and argument staging, preserves the existing call layout,
releases outbound stack, restores MXCSR, and only then normalizes a scalar
result. The maintained AArch64 backend applies the same ordering with an
eight-byte reusable frame slot and exact `MRS`/`STR` plus `LDR`/`MSR` FPCR
sequences. Machine, object, and native-artifact replay retain each target's
exact slot, per-call intervals, and bytes. Inbound callback envelopes remain
unimplemented; their policy predicates are requirements, not execution
evidence. An admitted per-binding preservation proof may later select a
zero-envelope optimization.

## Foreign execution and stack accounting

Execution placement is selected through ordinary providers and runtime
executors. It is not a language-level foreign-call disposition. A bodyless
binding declares blocking and affinity; a checked provider derives them. The
selected execution context must permit blocking and satisfy the required
thread or apartment affinity. Thus a Windows message loop may call a blocking
`GetMessage` directly on its dedicated pinned UI executor, while a codec-style
opaque call may be wrapped by an ordinary blocking-executor package.

Hosted direct calls use the host-managed stack and its guard according to the
selected calling plan. Callback entry preflights the exact Omega WCSU when the
host profile requires it. A fixed-stack or freestanding provider instead needs
an admitted foreign contribution or a separately provisioned provider stack.
An isolated provider crosses the existing process/component boundary and
exposes an endpoint rather than a special FFI call kind.

A boundary requirement's resource ceiling is not evidence that an opaque
implementation fits it. Checked Omega realizations derive WCSU. A native
binding needs admitted foreign demand, or an enforced guarded capacity whose
overflow remains an abnormal-exit route rather than proof of successful
completion. Trust composes by the weakest input and reports the exact foreign
premise.

For the implemented direct same-stack lane, object construction retains the
exact caller-live bytes and admitted foreign contribution at every physical
call site. Stack closure adds those two quantities and takes the maximum across
sequential calls. Native-artifact validation rejoins the contribution to the
semantic requirement, provider execution, and strong selected-plan digest;
canonical installation stores only a replayable projection and rejoins it to
the exact image before reuse. The lane currently accepts at most the proved
16-byte physical alignment. Greater admitted alignment fails closed until the
emitter has real padding evidence.

A hosted blocking executor is an ordinary package assembled from activations,
bounded queues, moved custody, linear completion claims, suspension, and
provider selection. It keeps a blocking call off a no-block scheduler worker
but does not change the foreign contract. An in-process worker cannot be killed
safely; a detached call pins its worker, storage, and provider era until native
return. Bounded recovery from a genuine hang requires process isolation.

## Registered callbacks

A callback protocol is declared by an ordinary boundary requirement carrying
its `Calling<C>` policy. A named static `boundary machine` explicitly satisfies
that requirement. The registration operation declares
`where machine Selected satisfies Trait::requirement`; the nominal requirement
supplies the complete signature and contract without structural repetition.
Passing the selected machine chooses its explicit satisfaction row, validates
the published and actual refining envelopes plus their `CallPlan + StatePlan`,
and lets the compiler materialize the foreign ABI thunk and relocation inside
that exact binding. The registrar's evaluated outbound `CallPlan` carries one
normalized callback-materialization row per nominal callback binder. Each row
maps the registrar's binder-slot identity, not its later selected-machine
argument, to one declared `NativePlace`: either a direct native parameter or a
field projected through a validated native layout. The plan fingerprint is
therefore fixed across callback selections; the per-use row separately retains
the selected machine, satisfaction, entry plan, and private thunk identity, and
lowering joins those identities only when emitting the private relocation.
Signature coincidence and unique visibility are not
selection rules. A signature-free requirement path must resolve uniquely or
reject, consistently with domain `established by` clauses. The source surface
does not need a general function-pointer value.

A direct callback is an interleaved native-only parameter owned by the
registrar requirement. For example, a foreign ABI shaped as
`install(kind, procedure, module, thread)` may be declared conceptually as:

```omega
machine install<machine Handler>(
    kind: HookKind,
    native callback procedure from Handler,
    module: ModuleHandle,
    thread: ThreadId,
) -> Registration
where machine Handler satisfies HookProcedure::call;
```

Source calls omit `procedure`; it has no Omega runtime type or value. The
declaration contributes one nominal private-callback entry at that exact
position in the ordered native telescope, and the compiler publishes its
target-closed function-pointer shape and `NativePlace::Parameter` demand. The
policy places the entry but cannot create, reorder, or retarget it. A trailing
hidden argument, an `addr`-typed callback formal, or a position inferred from
binder order is not an equivalent declaration.

Ordinary semantic-formal projections and private callbacks share one nominal
native-parameter identity space. `NativePlace::Field.parameter` names an
ordinary entry whose validated layout owns the field; `NativePlace::Parameter`
names a whole entry. Native order is fingerprinted separately from those
stable identities. Exact replay consumes a boundary-plan application identity
covering the requirement, ordered native telescope, each identity-to-placement
row, callback materializations, and reusable physical plan. This catches an
equally shaped parameter reorder that a placement-only fingerprint would miss.
The migration from ordinal-derived IDs uses a new fingerprint version and
reissues affected artifacts rather than reinterpreting them.

The first downstream direct-parameter carrier begins address-free. For
exactly one whole native parameter on the ordinary unoptimized normalized-
import path for supported ELF/Mach-O targets, compiler-owned lowering joins the
retained callback occurrence to its registrar call by Terminal `OperationId`, preserves
the selected callback-thunk identity and exact authored native position through
target lowering, and assigns the policy-selected physical destination. The
cohort is limited to the established fixed-width integer semantic arguments
and result and requires exactly one binder, demand, and materialization, a
target-pointer-shaped application, and one complete register or stack placement.
Carrying that identity alone does not create a code pointer, source argument,
relocation, or executable registration. The separately retained checked body
now lowers to an isolated canonical Terminal artifact, compiles into a
disjoint compiler-private machine-code function, and is replayed into one exact
private object symbol and final executable-function span. Its artifact-local
`MachineId` never joins the semantic program namespace. The direct-parameter
continuation now selects the address load, emits the registrar call, targets
the private symbol with exact x86-64 or AArch64 relocations, and independently
decodes the final patched address. A source-evaluated normalized-import canary
crosses that complete native-artifact route. Field-projected and multiple
callbacks remain fenced, as do runtime registration, lifetime, installation,
and publication.

A projected native callback field is a typed private-materialization demand in
the normalized layout plan, not a field of the source-visible specification.
The target package declares its stable identity as an explicitly named
`Layout satisfies PrivateCallbackSlot<Trait::requirement>` conformance, and the
layout policy must cite that exact conformance in its private placement entry.
The declaration alone is inert: layout evaluation never enumerates visible
conformances, and ordinary third-party evidence cannot inject a demand into an
existing plan. The subject supplies exact layout identity while the static
argument supplies one signature-free callback-requirement path; overload
ambiguity rejects.

Layout validation records the conformance-owned slot identity, exact callback
requirement, and target-closed placement independently. Complete outbound-plan
validation requires every such demand to be supplied exactly once by a
compatible callback-materialization row. Missing, duplicate, wrong-layout,
wrong-requirement, overlapping, shape-incompatible, or unresolved demands
reject. Source cannot read, write, serialize, or address the field. The
authoritative layout may author or compute its physical offset, but neither
binder order nor a repeated byte offset is a calling-plan placement rule.
Changing only the selected callback changes per-use/thunk identity; changing
the evaluated offset changes target-realization and artifact identity while
the target-neutral requirement declaration remains stable.

Callback placement does not own native-argument storage lifetime. Direct
arguments, call-scoped staging, retained pointees, snapshots, and stable roots
remain ordinary outbound calling-plan and foreign-storage dispositions. A
common copying registrar needs only call-scoped staging; an API that retains a
caller-supplied native object must satisfy the general retained-storage rules
below. The callback row records only binder slot and destination.

Durable registration returns an ordinary linear package value. It owns the
foreign registration and any code/component lease needed to keep the entry
valid; its explicit terminal operation unregisters before releasing those
obligations. Call-scoped callback parameters remain borrowed for the call.
Foreign context storage carries an inert protocol token or generational handle,
while the owning state remains in an Omega registry or another ordinary
package-owned value. The registration occurrence retains the selected machine
in provenance, but possession alone imports no narrower implementation facts;
an API forwards any caller-visible guarantee explicitly.

Synchronous entry and deferred registration are separate contracts. A bodyful
machine infers its `invokes` set from the body, including forwarding through
local helpers. A bodyless requirement declares every binding it may invoke
before returning:

```omega
boundary trait EventSource {
    machine register_and_fire(handler: Handler) -> Registration
    invokes handler;
}
```

`invokes handler` contributes the handler trait and the selected conformance's
operational envelope to the current invocation's normalized reach. The returned
linear registration separately establishes a future external root carrying
that same concrete conformance and envelope. A registration operation without
`invokes handler` cannot enter the handler synchronously on its current call
chain. A separately activated root may run according to the registration
contract, including concurrently with registration.
Root establishment requires the selected root policy to admit the concrete
handler envelope; the sealed registration establishment route records that
fact. It is not a freely assertable postcondition.

Cycle checking uses the direct synchronous `invokes` graph, never the
transitive service-reach closure. The realized synchronous graph across Omega
component boundaries must be acyclic. A protocol that needs a cycle moves one
edge within an artifact or breaks it structurally through a mailbox, queue,
scheduler handoff, or other new-activation boundary. Deferred roots may form
reach cycles in the final program graph without creating nested component
stacks.

Hosted callback entry may continue on the provider stack, preflight its
remaining capacity against the exact Omega WCSU and target reserve, or enter a
target-supported owned stack. Preflight proves the predicted segment fits; an
owned hard-limited stack additionally detects underestimation at its own
boundary. Foreign calls made by a separated-stack callback return to the
provider stack domain before entering opaque code.

That selected execution stack is only one part of root provisioning. Each
installed callback root also carries every admissible arrival context and its
finite entry/body/exit epoch sequence. Epochs retain the active domain,
per-domain occupancy, and phase-specific nesting allowance, so a software
stack switch divides the sequence while an atomic hardware switch does not.
Terminal-Psi WCSU joins only the body execution domain. Architectural arrival
comes from a sealed target rule applied to installed facts; emitted adapters
are derived from their bytes; opaque adapters require admitted evidence.

Native protocols may synchronously re-enter application callbacks. A platform
adapter exposes exact `invokes` ceilings, checks each ordinary Omega handler's
realized envelope, answers synchronous platform queries through restricted
handlers, and queues ordinary application events until the outermost native
dispatch returns. This package-local construction does not require inferring
the opaque provider's internal call graph.

A raw opaque callback remains trust-relative. Its binding may enforce a
chain-scoped active/depth limit only when the protocol supplies a valid
unavailable result. Otherwise finite mixed-chain admission requires a checked
provider contract or structural isolation; a handwritten native header does
not become proof of non-re-entry.

## Foreign data and formats

Foreign layout is expressed by authored programmable layout policies built from
compiler-known placement primitives. Plain `data` supplies the semantic shape;
layout policy supplies the foreign byte representation. Format packages publish
their selected plan, codec requirements, realizations, and trust evidence.

Inbound paths are explicit:

1. receive raw bytes/pointers under a boundary contract;
2. validate or materialize according to the layout/format policy;
3. establish predicate facts and any authorized semantic qualification; and
4. expose ordinary Omega values or checked borrowed views.

Decision 19 governs the transitions. `as` may prove a refinement or declare an
authorized representation-identical semantic commitment; executable conversion
is an ordinary contracted call. A recast may expose the same storage under a
weaker/alternate stated layout only when the representation and lifetime laws
permit it. No cast fabricates stronger foreign validity.

Outbound paths forget semantic facts or execute an explicit encoding/conversion
before crossing the boundary. The foreign vocabulary does not leak into normal
program types merely because one provider uses it.

The filesystem open-flag migration is the first concrete instance. Application
and portable standard-library code author `OpenOptions`; selected target-package
machines encode those semantics into Darwin, Linux, or MSVCRT flag words. The
bit positions are checked target-format implementation facts, not provider-plan
`Value` rows and not portable constants. Foreign record offsets remain on the
retirement path until placed/recast views can consume the validated layout plan
directly; exposing a public raw-offset accessor is not an acceptable bridge.

## Foreign addresses and storage lifetime

`addr` is numerical address data. `Ptr<T>` is a sealed, inert foreign-ABI
carrier whose parameter supplies representation and pointee-shape information
to boundary lowering. Neither is authority. Ordinary Omega code cannot
dereference, index, or manufacture a reference from either carrier. A binding
materializes a `Ptr<T>` only from an established storage claim after validating
the selected marshaling and calling policies; inbound carriers become checked
views only through an authorized establishment route.

Foreign storage use has three outbound ownership shapes:

1. **Call-scoped:** an ordinary `&T`, `&[T]`, `&write T`, `&write [T]`,
   `&mut T`, or `&mut [T]` permits only its exact access set before the call
   returns.
2. **Retained after return:** the public requirement states the caller-visible
   lifetime/custody contract. An ordinary linear protocol value such as
   `PendingRead` or `Registration<Storage>` may own stable storage; a
   lifetime-parameterized `Registration<'a>` may retain a checked borrow; or a
   realization may hide stricter native retention behind a private stable
   snapshot when a semantic snapshot contract permits copying. A terminal
   completion redeems public custody and releases private backing.
3. **Process-lifetime:** the authority moves into an already-established static
   or process-lifetime root. Omega has no general permanent-custodian spelling;
   other permanent retention remains unsupported until a concrete customer
   justifies one.

Post-return retention is never an untracked extension of a call-scoped borrow.
A lifetime-parameterized protocol value may carry an explicit checked loan;
otherwise its linear claim owns the keepalive and reclamation authority for its
backing place, not necessarily the bytes inline. It may lend ordinary lexical
views over rights the foreign side does not hold. A read-only foreign operation
can therefore preserve semantic facts and lend Omega read views; a writing
operation invalidates facts over exactly the writable extent and re-establishes
them from terminal completion evidence.
Separated partial release is an ordinary split in the claim-content algebra:
the returned subextent leaves flight while the disjoint remainder stays under
the same protocol claim.

The compiler learns that use survives return from the published result contract
and ownership conservation. A consumed `Buffer` may map into the content
retained by `PendingWrite`; `submit(&buffer) -> PendingWrite` rejects when the
unparameterized result claims owned retention, while a result explicitly
parameterized by the borrow lifetime may carry that checked loan.
Unambiguous consumed-input-to-produced-claim mappings are inferred; ambiguous
or unsupported mappings reject unless an ordinary postcondition pins the
correspondence. A content-bearing exact qualification supplies its projection
through its owner-unique core `Content<A>` conformance; the binding does not
invent a separate foreign-extent algebra or projection annotation.

Every pointer-valued native slot retained after return carries checked
provenance to an exact stable root, range, access mode, lifetime, and any
revision or lease. Unknown provenance rejects until an admitted provider route
establishes it. Embedded nested layouts have a finite structural closure;
recursive or dynamically sized pointer graphs instead retain one arena/extent
root covering the graph rather than asking the compiler to traverse runtime
pointers. A private snapshot is legal only under an explicit semantic contract
that permits an independent value copy and requires neither identity
preservation nor unchecked write-back. Concurrent foreign and Omega access is
External placed backing; exclusive foreign mutation may instead move storage
into the protocol value and return it under the requirement's declared
preserved, invalidated, or outcome-dependent content qualification.

Provider-specific backing never changes a separately compiled public result
type. Requirements publish unavoidable caller-visible lifetimes and custody;
realizations record and validate their concrete backing recipes. Private
snapshot bytes count as persistent demand per live protocol occurrence. A
static aggregate bound therefore also requires a finite live-occurrence
capacity authority: success moves the exact authority into the registration,
rejection returns it unchanged, and successful unregister returns the same
occurrence. A consumable lifetime budget is a different authority. Static thunk
code is bounded separately by distinct admitted callback identities, not by the
number of simultaneously live registrations.

The executable checker admits the unique compatible consumed input. When
several compatible owned inputs exist, one exact authored equality may select
the source by relating the whole entry projection of that parameter directly to
the whole current result projection in the same content algebra. Partition
equations and structural subplaces do not select custody. Borrow-only retention
is admitted only by the first exact checked and Terminal lifetime rung: one
content-bearing linear result with one erased lifetime slot, one compatible
whole direct shared parameter, and the same explicit callable lifetime on the
parameter and result. Its retained fact carries the exact callable, source and
result places, lifetime/slot coordinates, result nominal and semantic domain,
and both full projection plans. An unparameterized or elided result, a lifetime
mismatch, multiple compatible borrowed sources, mutable/write-only access,
`self`, nested/indirect carriers, structural subplaces or partitions, runtime
generic arguments, and authored-equality laundering all reject. Terminal
lowering independently replays the checked row and retains one canonical
declaration-only boundary content contract carrying the exact callable,
source/result places, lifetime ordinal, result nominal and semantic domain, and
both complete projection definitions. That contract is non-executable: it
cannot be targeted by a boundary call or provider row and does not widen the
established Unit/scalar boundary-result ABI. Native-slot lowering, private
snapshots, completion/reclamation, and provider-specific backing remain later
rungs.

The reverse direction uses the same types. A provider-owned view whose
invalidators require exclusive access to one receiver is an ordinary borrow
from that receiver. More precise or nonlexical protocols return a linear view
claim and require every invalidating operation to consume the claims it kills.
Global, thread-local, or asynchronously invalidated foreign storage must be
copied, mediated by such a protocol claim, or accepted under an admitted
stability promise. A claim cannot prevent an opaque provider from invalidating
storage through an unmodeled route.

Completion is an establishment point. Its contract correlates one event with
one live claim through a unique/nonreused identity, a generation-checked
identity, or an exclusively ordered channel. A progress event releases nothing
unless its contract returns an exact separated subclaim; cancellation requests
release nothing until a terminal acknowledgement. Reused tokens require
generations wherever stale foreign copies can survive.

Terminal Psi retains that correlation on successful bodyless boundary calls as
an exact completion-receipt row `(operation, boundary, argument position,
claim)`. Verification reconstructs the complete live-claim set for every
consumed argument and rejects missing, extra, duplicate, reordered, or
cross-argument receipts. Interpretation and native realization bind the same
rows to the admitted provider execution. A rejected provider effect records no
receipt and consumes no custody.

The compiler canaries pin both halves of this ownership split. A synchronous
fixed-array pointer import releases its ordinary source loan before the next
owner mutation. Retained custody cannot originate from a borrow; the accepted
round trip consumes an owned buffer into one linear pending claim and permits
only a terminal completion consuming that claim to re-establish buffer custody
under the same `Content<A>` algebra. Provider-owned storage uses the ordinary
receiver-borrow path when all invalidators require exclusive receiver access:
invalidation after the view's last use passes, while invalidation before a
later use rejects. Providers with independent invalidation instead return the
view from an explicit linear validity claim; the invalidator consumes that
claim, with the same last-use acceptance and live-view rejection.

The selected provider era enters the compiler-tracked set of live claims for a
value (its claim frontier) only when that value's meaning depends on state owned
by the exact era. A
provider-created handle or pending operation pins that era; a rebindable service
binding names a slot and does not. Pins block reclamation rather than teardown
execution: an old era remains callable while it discharges roots it owns, then
waits for application-held claims, establishes quiescence, and unloads. Static
custodians discharge their outlives relationships at build time and create no
runtime ledger noise.

The safe parameter and result types carry access and lifetime behavior.
Calling/marshaling plans describe representation only: for example, that
`BoundarySignature` parameters 0 and 1 encode one contiguous slice, or that one
validated descriptor denotes several separated extents. A selected policy that
defines a native slice ABI derives the ordinary reference case. Raw
pointer/count pairs and descriptor graphs require an authored binding policy;
the compiler never guesses their association.

`&write T` is the call-scoped provider-writes-only form. It borrows one existing
valid `T` exclusively and permits mutation without observation. It is not an
output/construction slot, never covers `Vacant` storage, and creates no durable
custody transfer. A mutable borrow may attenuate explicitly to it; the callee
cannot derive `&T` or `&mut T`, take or swap old content, perform
read-modify-write, or call a helper with broader access.

For a byte-producing operation, the outcome contract names the exact modified
prefix or other write footprint. The untouched suffix is unchanged, so caller
facts over that suffix survive; the returned count does not establish a value
that was absent at entry. Each replacement separately requires freely
discardable displaced content and preservation of the referent's validity.
Partial writes through structured `T` are therefore accepted only when validity
follows from static structure, written inputs, and deliberately supplied facts
without loading the referent.

Checked Omega providers enforce non-observation transitively through their
entire call closure. An opaque provider physically receiving an address may
still read it; the selected provider evidence admits compliance unless target
isolation enforces it. Artifacts retain the write-only mode and exact outcome
write frame rather than widening the call to read/write. Identity-only
retention remains an ordinary stable keepalive claim that lends no memory view.
Storage with no live `T` and typed foreign construction are separate future
features rather than alternate meanings of `&write`.

The native leaf declares the foreign signature's actual parameter structure.
Separate pointer and length parameters are not interchangeable with a record
containing the same fields: the selected calling policy may place them
differently. Safe slice/text carriers remain private Omega representations and
are rejected as bare native leaves unless an explicit custom `Calling<C>`
policy publishes their ABI. Fixed arrays and records, by contrast, may be
structurally classified because their public normalized shape determines the
aggregate facts the policy consumes. Omega never performs C array decay.

Every reclaimable installed callback/interrupt entry is also an external
artifact root. Because no Omega call edge reaches it, the dynamic root ledger
retains its reach, authority/trust receipts, state footprint, stack domain,
context-indexed stack epochs, nesting relation, and version pins until its
linear registration proves
unregistration and required quiescence. A process-lifetime statically linked
callback needs the same build report but no live replacement ledger. This
reuses provider admission rather than creating an entry-specific trust system.

## Process entry

Process entry is one required environment-to-program root slot in a target
profile. The program binds an exact semantic source machine while the target
fixes the separate physical arrival contract, calling policy, bootstrap
adapter, provider setup, physical-result map, and source-visible entry shape.

```omega
machine start() {
    Console::write_line("Hello, Omega.");
}

machine build(builder: &mut Build) {
    builder.roots.bind(
        windows_x86_64::ProgramEntry,
        start
    );
}
```

On Windows the generated stub may read native command-line/environment
surfaces; on ELF it may read the initial stack; on firmware its exact physical
requirement may receive a firmware handoff. Those details stay in scoped
providers and the target-authored bootstrap behind a generated ABI shell.
Native handles remain typed physical inputs rather than semantic storage roots.
Target selection and semantic slot binding belong in `build.omg`, not a
target-specific source dialect or a `main` naming convention.

A hosted schema normally hides raw image and storage roots. If the bound entry
has one `&mut self` receiver, the bridge provisions exactly one ZII-valid
instance beneath an admitted storage root and lends it only to that activation.
A freestanding schema may deliberately expose `image: Extent in Granted` and
`initial_storage: Extent in Granted` because provisioning is then the
application's responsibility. A separate semantic installation edge introduces
those exact occurrences after the bootstrap establishes their evidence. The
combined shell and adapter remain the installed external root in both cases and
contribute their complete derived contract.

## Engineering order

1. Normalize boundary-machine contracts and calling-plan identities.
2. Represent `Binding` as resolved target/provider data.
3. Validate provider admission and emit the transitive executable-entry,
   containment-guarantee, and scope-completeness manifest; distinguish static
   selection from Omega-mediated runtime admission and retain exact
   incompleteness attribution.
4. Lower imported calls and inbound stubs from checked plans only.
5. Integrate programmable layout validation/materialization.
6. Add final-artifact state-footprint validation and external-root reporting.
7. Add boundary-coherence rejection canaries: retained-after-return custody
   sourced only from a borrow, blocking under a no-block root, incompatible
   affinity, and undeclared or cyclic synchronous invocation.
8. Implement the narrow Windows `user32` acceptance slice in `TASKS.md`.
9. Add foreign-retention and provider-view canaries.
10. Delete host-string special cases and legacy target blocks.

## Still open

Target-specific launch/exit details not covered by existing calling plans.

Exact `Build` library method names for choosing a target profile remain
ordinary library/API engineering. Provider override binds one target-owned
typed slot to the exact satisfier or complete named conformance demanded by
that slot; equivalent scoped APIs for tests and replaceable-realization owners
remain engineering work, not an open grammar question. "Binding" here is
build/artifact state, not a source `slot` construct.

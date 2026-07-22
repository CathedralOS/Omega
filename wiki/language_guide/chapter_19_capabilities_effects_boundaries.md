# Chapter 19: Capabilities, Effects, And Boundaries

Omega should model host and compiler boundaries explicitly.

> **Effect rows settled 2026-07-18 (frozen decision 22; authoritative
> record: [effects_authority_and_observation.md](../design_briefs/effects_authority_and_observation.md)).**
> Omega has one kinded `effects` row. Boundary-trait identities contribute
> service reach; a tiny core set contributes operational possibilities such as
> `Suspend` and `Block`. Authority remains capability values, trust remains
> provider receipts, failure remains sums, mutation remains ownership, and v1
> resource bounds remain dependent contracts. The lowercase fixed vocabulary
> documented later in this chapter is the current compiler compatibility layer,
> not the end-state language model.

The outside world is not one thing. Linux may expose raw syscall numbers,
Darwin normally routes process IO through `libSystem`, Windows imports APIs
from DLLs such as `Kernel32.dll`, Wasm imports host functions, and embedded
targets may jump through firmware tables. The shared concept is not "Unix
syscall." The shared concept is an imported boundary whose implementation is
not Omega code.

## Boundary Surfaces

`boundary` marks a surface where ordinary Omega proof stops and an audited
provider begins. There is no separate top-level root declaration: the boundary
is carried by the construct that crosses the edge, such as an operator,
library entry, authority contract, trait, or target policy.

Core operators keep their public contracts visible while primitive lowering
stays behind the compiler/runtime boundary. A core operator carries a fixed
`spelling` so the surface symbol resolves to it without hiding the signature or
proof obligation:

```omega
boundary operator Slice::index<T>(items: &[T], index: u64) -> T
    spelling []
requires
    index < items.len;

boundary operator Slice::range<T>(items: &[T], start: u64, end: u64) -> &[T]
    spelling [..]
requires
    start <= end && end <= items.len;
```

The `spelling` clause sits above the `boundary` modifier and the `requires`
clause, so binding a fixed spelling never hides the signature or proof
obligation.

Working interpretation:

- `requires` remains a proof obligation for callers.
- `spelling` binds the surface operator symbol (`[]`, `[..]`) to the named
  operator without hiding its contract.
- `boundary operator` says implementation lowering is accepted from the
  compiler/runtime provider for that operator.
- The boundary report records boundary operators, library/authority boundary
  clauses, target policies, and unchecked policies.
- Every boundary implementation binding references a registered
  `BoundaryProvider` (see "Boundary Primitive Registry"). Ordinary application
  code cannot mint new host/compiler providers as a proof escape hatch.

This is the current boundary between browsable core semantics and private
compiler machinery. Users should be able to inspect `Slice::index` and see the
proof contract. They should not need to inspect the private descriptor, pointer,
or codegen mechanism used after the contract is proved.

## Boundary Traits

A boundary trait names callable behavior whose implementation crosses out of
proved Omega code. It is still a trait: callers see machine signatures,
requirements, guarantees, and effects. What makes it a boundary is that the
implementation is accepted through a host package, target binding, firmware
surface, dynamic loader, or other boundary edge.

`boundary` is not a synonym for "has effects." These are separate axes:

- `effects` names what externally visible behavior class can happen.
- `export` names what symbols belong to an artifact/API surface.
- `boundary` names where ordinary Omega proof stops and boundary provider
  guarantees begin.

Ordinary Omega code can have effects if it calls lower boundary surfaces. It is
still proved Omega code. Boundary code is different: the compiler accepts its
declared guarantees from a configured boundary provider because the implementation is
not available as normal Omega source.

```omega
boundary trait Readable {
    machine read(
        path: [u8] in Utf8,
        out: &mut Vec<u8>
    ) -> ReadResult
        effects Suspend;
}
```

Working interpretation:

- `boundary trait` means the machines describe behavior outside proved Omega
  code.
- Each `machine` is a callable boundary surface.
- `requires` clauses are obligations the caller must prove before crossing the
  boundary.
- `ensures` clauses are guarantees accepted from the boundary implementation.
- Boundary-trait identity automatically contributes service reach; the
  written `effects` clause adds other service or operational ceilings such as
  `Suspend` and `Block`.
- Build policy decides which boundary providers are allowed for a target.
- Safe application packages cannot silently create new host boundaries. A
  provider must come from the toolchain, target configuration, or an explicitly
  whitelisted boundary package.

## Settled Effect Rows

The source row is one `+`-separated ceiling of name-resolved members:

```omega
machine backup(
    src: [u8] in Utf8,
    dst: [u8] in Utf8
) -> BackupResult
    effects Readable + Queryable + Suspend
{
}
```

V1 members have two mechanically distinct kinds:

- service reach from boundary traits (`Readable`, `Queryable`, `Clock`);
- operational possibility from the core set (`Suspend`, `Block`).

The names are identifiers, not reserved words. Rows are ceilings: effects
accumulate transitively and absence is the guarantee. If `Block` is absent, no
checked callee or admitted provider may block a worker. If `Writable` is absent,
the machine cannot reach that service even when it possesses a Writable
capability.

Internal rows may be inferred. Exports, trait requirements, and boundary
operations write their row; omission there means empty. Implementations and
providers refine by subset. Imports use pinned requirement rows, so later
provider selection cannot widen a compiled consumer.

No masking, subtraction, scoped allowance, or algebraic handlers exist. A
checked in-memory Readable provider can remove a trust receipt and refine
operational behavior, but the abstract Readable reach remains visible. V1 also
has no quantitative row members: heap/region bounds use capability contracts;
task and version capacity use their own declared budgets.

The complete laws, algebra, identity rule, tests, and deferred spaces are in
the decision-22 brief.

## Legacy Standard Effects (compiler compatibility layer)

The remainder of this section documents the currently implemented lowercase
`u64 EffectSet`. It is retained to explain existing compiler behavior and old
samples while the decision-22 migration lands. These names are not new
language canon and must not be extended as the effects architecture.

The legacy design treated effects as stable language-level names for externally
visible behavior rather than host-specific syscall names. Decision 22 replaces
those fixed names with symbol-resolved service traits and kinded operational
members while preserving the provider-independent intent.

Implemented compatibility names:

- `alloc`: may allocate memory through the language/runtime allocator.
- `dealloc`: may release memory through the language/runtime allocator.
- `stdin_io`: may read from standard input or an equivalent console input
  stream.
- `stdout_io`: may write to standard output or an equivalent console output
  stream.
- `stderr_io`: may write to standard error or an equivalent diagnostics stream.
- `filesystem_io`: may read, write, open, close, query, rename, or delete
  filesystem objects.
- `network_io`: may use network sockets, packet interfaces, or equivalent
  network services.
- `process_spawn`: may create or launch a process/task outside the current
  program image.
- `process_exit`: may terminate the current process/task.
- `process_signal`: may signal, cancel, suspend, resume, or otherwise affect
  another process/task.
- `env_read`: may read process, user, host, or build environment values.
- `env_write`: may mutate process, user, host, or build environment values.
- `clock_read`: may observe wall-clock time, monotonic time, timers, or
  scheduler time.
- `random_read`: may read entropy or random values from a host/runtime source.
- `thread_spawn`: may create a concurrent thread/task/fiber of execution.
- `thread_block`: may block the current thread/task/fiber waiting for another
  event.
- `sync_wait`: may wait on a synchronization object such as a lock, condition,
  join handle, semaphore, futex, or channel receive.
- `sync_wake`: may wake or signal a synchronization object such as a lock,
  condition, semaphore, futex, or channel send.
- `device_io`: may interact with hardware, drivers, firmware, or memory-mapped
  device registers.
- `memory_map`: may map, unmap, remap, pin, share, or change permissions on
  virtual/physical memory regions.
- `dynamic_link`: may load, unload, resolve, or call through dynamically linked
  code.

Effect-row purity is the empty row, not a named member. It proves no service
reach and none of the represented operational possibilities; it does not by
itself prove termination, absence of failure, or absence of owned-state
mutation. Adding a boundary call, allocator reach, wait, or other row member
later changes the inferred row and must satisfy the caller's ceiling.

The compiler-side source of truth for this transitional vocabulary is the
standard effect name list in `omega-effects`. The representation migration
replaces it with normalized symbol/kind identities; the old bit positions may
survive only as a cache/projection.

Declared effects are ceilings. A trait can say "any implementation of this
machine may require at most these effects." A concrete machine may declare the
same set or a smaller set, because some providers are less effectful on a given
target. It may not declare a new effect outside the trait requirement.

```omega
boundary trait Console {
    machine write_line(text: &[u8])
    effects
        stdout_io;
}

machine Console::write_line(text: &[u8])
effects
    stdout_io
{
    HostConsole::write_line(text);
}

// A host provider, not normal application code, binds HostConsole to Darwin
// libSystem, Linux syscalls, Windows APIs, firmware, or a test harness.
```

In that shape, `Console::write_line` is ordinary Omega standard-library code.
It is statically linkable and proof-checked like any other machine. The
boundary is the lower `HostConsole` provider edge where the implementation is a
syscall, imported symbol, firmware call, loader hook, or boundary test surface.

Effects propagate through the call graph. The compiler should compute direct
effects for each callable and then compute transitive effects for every machine
from the machines it can call.

Effect declarations are policy surfaces, not required noise on every machine:

- Boundary traits must declare effects. They are the boundary edge where
  externally visible behavior enters the program.
- Exported library APIs should declare effects. This makes the public contract
  stable and lets callers reject libraries that unexpectedly grow filesystem,
  network, process, dynamic-link, or other host behavior.
- Private/internal machines may omit effects. The compiler infers and reports
  their reached effects from their bodies and callees.
- Executable entry points may omit effects in normal development builds. The
  final executable manifest still records the union of effects reachable from
  the entry point so an OS, loader, store, or build policy can prompt, deny, or
  audit the requested behavior classes and authority flows.

When a concrete machine declares an `effects` block, that block is a ceiling
for the machine's reached effects. Omitting the block means "infer and report
this machine's effects." Declaring the block means "this machine must not
reach anything outside this set."

```text
Main::main
  declared: <none>
  direct:   <none>
  reached:  stdin_io, stdout_io, process_exit

executable manifest:
  stdin_io, stdout_io, process_exit

Grep::search
  declared: filesystem_io
  direct:   <none>
  reached:  filesystem_io
```

A stricter release, OS, or audited build can require an explicit checked-in
effect and authority manifest for executable entry points. That requirement
belongs to build policy. It does not mean ordinary application authors must
manually thread every reached effect through `main` while iterating locally.

The compiler can represent standard effects as a compact bitset because the
standard vocabulary is finite and stable for a given toolchain. Names remain
the source syntax, diagnostic format, package manifest format, and OS prompt
format. Bit positions are an implementation detail owned by the compiler and
loader, not a user-facing ABI.

This gives each layer the shape it needs:

- Source and docs use readable names such as `filesystem_io`.
- Compiler checks use fast set operations such as subset, union, and
  intersection.
- Optimizers can use the propagated bitsets as reordering, inlining, and
  scheduling facts.
- Executables can carry named effect and authority manifests plus any
  loader-native bitset encoding needed by the target OS.

The list should stay intentionally small. More specific host details belong in
boundary provider metadata and authority-flow facts, not in effect names. For
example, `stdout_io` is enough for the language-level effect report; whether
the implementation uses Darwin `libSystem`, Linux `write`, Windows console
APIs, or a firmware UART is a provider detail.

The legacy implementation rejects names outside its closed table. End-state
rows resolve ordinary boundary-trait/core-member identifiers instead; user-
defined operational-member declarations remain deferred by decision 22.

Console boundaries should use the same shape:

```omega
// The result of a byte-level read. `Eof` is ordinal 0: a zero-initialized
// ByteRead IS end-of-input -- ZII, no sentinel value anywhere (ruled
// 2026-07-16; the -1 spelling was rejected as legacy non-ZII).
data ByteRead {
    case Eof;
    case Byte(value: i32 [0..=255]);
}

boundary trait Console {
    machine write(text: &[u8])
    effects
        stdout_io;

    machine write_line(text: &[u8])
    effects
        stdout_io;

    machine read_line(out: &mut [u8])
    effects
        stdin_io;

    machine read_byte() -> ByteRead
    effects
        stdin_io;

    machine write_byte(byte: i32)
    effects
        stdout_io;

    machine exit_process(code: i32)
    effects
        process_exit;
}
```

The byte ops are the universal filter surface (`stdin_checksum` and its
siblings): `read_byte` yields each raw byte as `ByteRead::Byte { value }`
and `ByteRead::Eof` at end-of-input -- the payload's declared `[0..=255]`
range is construction-enforced, so downstream arithmetic gets honest facts
for free, and native lowerings exploit the ZII rule directly (the result
slot is pre-zeroed; only an arrived byte writes the non-zero tag, so the
EOF path executes no write at all).

Implementation state: the standard Console is now a boundary trait with effect
rows. Its friendly `write` and `write_line` members are ordinary checked Omega
adapters over `write_byte`: they accept a borrowed byte view directly and walk
it with a measured state machine. The raw byte operation remains the provider
leaf. `read_line` accepts a mutable byte view. Its current owned-destination
route requires a concrete `[u8; N] in D` carrier at the call site: boundary
planning derives `N` from that place, writes directly into its inline bytes,
and establishes the carrier's runtime length. The legacy 256-byte String
scratch limit is never reused for a shorter carrier. Legacy composite provider
rows remain temporarily for deliberately nonstandard semantic-test
declarations; the ordinary corpus imports the standard package. `TASKS.md`
tracks compatibility-row deletion.

Domain requirements stay normal proof language. A filesystem boundary should
not invent special "initialized" words when a domain is what it means:

```omega
domain [u8]::NonEmpty {
    self.len > 0;
}

boundary trait Filesystem {
    machine open(path: &[u8] in NonEmpty)
    effects
        filesystem_io;
}
```

The same idea likely extends to text encodings and ABI string constraints.
Instead of growing separate surface types such as `CString`, `OsString`, or
`Utf16String`, a boundary should usually ask for the string domains it
actually needs:

```omega
boundary trait CConsole {
    machine write(text: &[u8] in Utf8 & NoNul)
    effects
        stdout_io;
}
```

That keeps encoding and interop requirements inside Omega's ordinary domain
system. The byte slice is the borrowed window passed across the boundary; no
separate `string` view type is required.

Text measures and text domains split by cost:

- `length` and `non_empty` are exposed first. They are cheap, O(1) facts read
  from the `{ptr,len}` descriptor.
- `no_nul` and `utf8` are domains established at a validating boundary
  constructor. The sequence-wide fact is asserted once at construction, then
  carried as a fact and never re-proved per use.

Establishing `no_nul` or `utf8` once at the validating constructor is the
decided answer to the cost of sequence-wide proofs: common text handling
downstream reads the carried fact instead of re-scanning the byte sequence.

## Capabilities And Authority Flow

Effects are not authority by themselves. `Readable` or `Writable` says the
corresponding service surface may be reached, but it does not say whether the
code was handed a folder by the caller, prompted the user, stored a handle for
later, or merely derived a narrower file handle from a folder it already had.
The row is a ceiling; capability values are possession; provider receipts are
trust. None can substitute for another.

Omega should model authority as ordinary values plus facts. A filesystem handle
should usually be one stable type with permission domains, not a family of
separate permission-flavored types:

```omega
data Folder {
}

domain Folder::Readable {
}

domain Folder::Writable {
}

domain Folder::ReadWrite {
    self in Folder::Readable;
    self in Folder::Writable;
}
```

Boundary and standard-library APIs then state normal requirements and
guarantees:

```omega
boundary trait Desktop {
    machine choose_folder(prompt: String) -> Folder
    ensures
        result in Folder::Writable;
}

boundary trait Writable {
    machine write_bytes(folder: Folder, path: String, bytes: &[u8])
    requires
        folder in Folder::Writable;
}

boundary trait Readable {
    machine read_bytes(folder: Folder, path: String, out: &mut Vec<u8>)
    requires
        folder in Folder::Readable;
}

boundary trait Filesystem: Readable + Writable {
}
```

This should not require new source keywords such as `uses capability` or
`acquires capability`. The compiler can infer authority flow from types,
domains, call contracts, returns, stores, drops, and boundary provenance.

Important report verbs:

- Accepts: authority enters through parameters or machine-owned fields.
- Uses: an operation requires authority facts such as `folder in
  Folder::Writable`.
- Returns: authority leaves through a return value or output parameter.
- Stores: authority is retained beyond the current call.
- Acquires: fresh authority is minted by a boundary, host prompt, ambient host
  surface, package permission grant, loader, or OS/runtime broker.
- Derives: a narrower or related authority is produced from an existing
  authority, such as `Folder::Writable -> File::Writable`.
- Releases: an authority is closed, dropped, revoked, or otherwise ended by the
  code.

`derives` is intentionally separate from `acquires`. Opening a file inside a
caller-provided folder is a sub-capability operation. It expands the set of
values flowing through the program, but it does not independently obtain new
host authority.

Example ordinary use:

```omega
machine Thumbnailer::write_cache(
    cache: Folder,
    image: Image
)
requires
    cache in Folder::Writable
effects
    Writable
{
    Filesystem::write_bytes(cache, "thumb.bin", image.thumbnail_bytes());
}
```

Expected package report shape:

```text
authority flow:
  accepts: Folder where Folder::Writable
  uses: Folder::Writable
  derives: none
  stores: none
  acquires: none
  returns: none
  releases: none

effects:
  Writable
```

Example acquisition:

```omega
machine Thumbnailer::choose_and_write_cache(image: Image)
effects
    Desktop + Writable
{
    let cache: Folder = Desktop::choose_folder("Choose cache folder");
    Filesystem::write_bytes(cache, "thumb.bin", image.thumbnail_bytes());
}
```

Expected report shape:

```text
authority flow:
  accepts: none
  uses: Folder::Writable
  derives: none
  stores: none
  acquires: Folder::Writable via Desktop::choose_folder
  returns: none
  releases: none

effects:
  Desktop, Writable
```

Package and build policy should be able to set ceilings over this inferred
flow. A package may be allowed to reach `Writable` only through
caller-provided folders, while being forbidden from acquiring a folder through
`Desktop::choose_folder` or opening an ambient absolute path.

Authority flow and boundary calls are related but separate reports:

- Authority flow answers what power-bearing values a package can accept, use,
  derive, store, return, release, or acquire.
- Service reach answers which abstract boundary traits the package directly or
  transitively reaches. Provider receipts separately answer which host,
  runtime, compiler, syscall, imported library, broker, or prompt realizations
  were selected.

A library can therefore be audited along three axes:

- Effect ceiling: which service surfaces and operational possibilities may be
  reached.
- Authority-flow ceiling: what authority values may move through or be minted
  by the package.
- Provider/trust ceiling: which direct and transitive realizations are allowed.

This distinction matters because two packages can both reach `Writable`
while having very different blast radii. One only writes into a folder supplied
by the caller. The other prompts the user, consults the environment, or calls a
raw host provider to acquire filesystem authority itself.

Target metadata such as library artifact, symbol, syscall number, calling
convention, and boundary provider belongs in toolchain host packages or explicitly
whitelisted boundary providers. Pulling in `Filesystem`, `Console`, or
`ProcessExit` service reach is visible to the build; provider receipts reveal
which realization supplies it, and a restricted build can reject either axis.

The compiler should understand boundary traits, provider packages, libraries,
symbols, calling conventions, boundary providers, and target image imports
generically. It should not special-case every Windows, Darwin, Linux, or SDK
API.

## Host Providers

Some targets do not need a named user-mode library for the lowest boundary.
Linux can expose a target syscall surface directly. That mapping is a
derived `ProviderPlan` for a boundary trait, not a different user-facing
callable concept. There is no `provides` declaration keyword and no authored
row-builder API.

The target's core/std package declares leaf machines satisfying the raw syscall
requirements `via Binding::Syscall { ... }` and ordinary checked adapter
machines satisfying Console. The compiler derives their normalized plan from
the explicit conformance closure, validates it, admits it with trust receipts,
and selects its provider type for the Console slot. `build.omg` normally
selects the target package's default provider set; a test harness or component
manager holding selection authority may substitute a different admitted
provider for an individual slot. Defaults are target-package declarations,
not compiler tables.

At the static root, an override is explicit and type-per-slot:

```omega
machine build(b: &mut Build) {
    b.select_provider<Console, TestConsole>();
}
```

The build declaration can select only a complete candidate already present in
the loaded dependency closure and applicable to the selected target. It does
not append rows, admit a candidate, or widen the requirement's effects.

This is the same proof shape as a library import:

- Omega proves caller-side type and state invariants.
- The imported boundary is accepted to satisfy its declared guarantees.
- The irreducible mapping is authored as a compile-time `Binding` value on a
  `via` declaration and recorded as a `HostAbiCall` provider in the boundary
  registry.
- The build artifact records which registered boundary providers were used.

`via` bindings are the external-provider supply form of otherwise ordinary
machines. Raw syscall numbers, imported DLL functions, firmware jumps,
compiler intrinsics, and instruction leaves are binding details; sequences,
argument reshaping, newline policy, caching, and other composition are normal
checked Omega machines. The satisfied requirement contributes the public
effect ceiling, while the binding/provider contract supplies behavior that
must refine it. Trust is assigned at admission rather than selected by source
spelling.

## Freestanding Targets And Hardware Facts

A hosted target's lowest boundary is an operating system. A FREESTANDING
target (ring 0, kernel, firmware payload) has no host below it: there is no
syscall surface, no stdin/stdout capability, no process exit. The lowest
boundary is the hardware itself.

The direction: freestanding is a target whose host-provider set is EMPTY and
whose boundary providers instead declare facts about hardware. The same
trust model applies unchanged -- a boundary is where proved Omega code accepts
declared, audited guarantees it cannot itself verify -- but the guarantees are
now hardware claims rather than OS claims:

- "after writing this value to the translation-base register, the mapping
  described by this page-table value is active" (an MMU provider),
- "this MSR read returns the current value of register X" (a register
  provider),
- "stores to this physical range reach device Y in program order" (an MMIO
  access provider, see
  [Memory Layout And ABI](chapter_20_memory_layout_abi.md) on volatile),
- "this instruction sequence masks interrupts until the matching unmask" (an
  interrupt-control provider).

These are the most serious trust statements in any system built on Omega: a
kernel's trusted computing base is, in large part, exactly this provider set,
and it is enumerable in the build artifact like every other boundary. The
audited inline-assembly subset
([Inline Assembly](chapter_23_inline_assembly.md)) is the implementation
vehicle for many of these providers -- the asm instruction contracts ARE
hardware-fact declarations in small form.

A freestanding target also needs an entry contract: who calls the entry, in
what machine state (which firmware handoff, what is mapped, what is zeroed).
The entry is an ordinary **exported callable** --
`boundary machine Main::run(handoff: EfiHandoff) -> EfiStatus`. A
`boundary machine` declares "we export this as a callable surface": its
parameter list is the shape imposed over the platform's arrival bytes (the
boundary performs the recast; a raw `&[u8]` parameter stays first-class for
programs that want unclaimed bytes). The satisfied target requirement pins a
normalized calling policy through ordinary `Calling<C>` trait composition.
`C` satisfies the ordinary core `CallingPolicy` relationship; its compile-time
machine evaluates the normalized signature to an accepted or structured-
rejected boundary plan. Accepted plans are compiler-validated and canonicalized.
The evaluated `CallPlan + StatePlan`, not the policy symbol or source body,
belongs to requirement identity;
`boundary(<Plan>)` is retired because it fused trust treatment with deployment
policy. "No host" is `b.freestanding = true` in
`build.omg` (an orthogonal `Build` field; see
`design_briefs/build_and_package_model.md`). The machine-state
guarantees are normalized provider/entry-plan facts surfaced by the build
artifact and checked or accepted through the ordinary admission spine.

The selected provider binding does not choose a calling convention from its
mechanism name. `DllImport`, `Syscall`, `VtableSlot`, and similar realizations
must validate against the policy already pinned by the satisfied requirement.
Provider-specific register allocation and footprint certificates remain
implementation evidence behind that published plan identity.

Hardware entry points with no Omega caller are external artifact roots. Their
effects, trust receipts, state footprints, stack domains, nesting relations,
and version pins must enter whole-artifact analysis at installation; otherwise
an interrupt or callback could launder behavior by sitting outside the ordinary
call graph.

The reusable extent, placed-view, checked-assembly, materialization, and root
ledger model is specified in
[`os_memory_and_hardware_foundation.md`](../design_briefs/os_memory_and_hardware_foundation.md).
Exact carrier APIs and validators remain open there; no separate interrupt or
MMIO grammar is implied.

### Admitted executable installation

Omega has no operation that converts arbitrary bytes into host code and no
general `ExecutableMemory` capability. Executable eligibility is a sealed
admission fact over a reusable immutable artifact. A package cannot establish
that fact for itself, and mutation invalidates it.

Installation borrows the admitted artifact and consumes authority over one
destination. Its normalized states are:

```text
CodePlacement (writable, non-executable)
    -> materialize declared sections and relocations
FrozenPlacement (readable, non-executable; no remaining writer)
    -> validate the exact final bytes and footprint
ValidatedPlacement
    -> contracted installation and instruction-fetch visibility
InstalledCode (readable, executable)
```

Each state is sealed: the only operation that can produce the next state
requires the previous one. The artifact remains reusable; the linear placement
authority prevents one destination from being spent twice. Validation evidence
is bound to artifact identity, placement, and final content, so it cannot be
transplanted to different bytes.

The installation provider alone performs the target-specific permission
transition, cache maintenance, ordering, and visibility work. Checked assembly
and page-table APIs emit the same admitted-artifact and installation-authority
obligations; neither is a raw bypass. A future fetcher requires visibility
before entry, while replacement of possibly running code separately requires
quiescence before retirement.

Installation prevents code injection. It does not prove that transfers within
installed code are legal. Sealed entry references cover much of the forward
edge; protected returns and the final control-flow-integrity certificate remain
the independent owner question in
[`OWNER_QUESTIONS.md`](../../OWNER_QUESTIONS.md).

## Invariant Parameters

Imported signatures should lean on invariant-parameterized types rather than
duplicating normal type facts in ad-hoc `requires` clauses.

```omega
&[u8, [non_empty, initialized]]
&mut [u8, [writable, initialized]]
HANDLE<[process, vm_read]>
RemoteBuffer<const u8, [readable_by<process>]>
```

The invariant names are resolved in the namespace of the type being
instantiated. `&[u8, [initialized]]` and `HANDLE<[initialized]>` do not
have to mean the same thing.

A type can define which invariant parameters it accepts:

```omega
builtin slice &[T, I]
    where I subset {non_empty, initialized}
    exposes len: u64
    invariant non_empty = len > 0
    invariant initialized = elements.initialized

builtin slice &mut [T, I]
    where I subset {non_empty, initialized, writable}
    exposes len: u64
    invariant non_empty = len > 0
    invariant initialized = elements.initialized
    invariant writable = elements.writable
```

The exact declaration syntax is provisional. The important point is that
`&[T]` and `&mut [T]` are built-in borrowed slice-view types with a core
semantic surface, not ordinary user-defined machines. They expose proof-visible
facts such as `len`, and they define the invariant names that callers may
attach to that slice view.

The public semantic name should be short and browsable, such as `Slice`, even
if the compiler lowers it through a private descriptor such as pointer plus
length. Users should be able to navigate to names like `Slice::Length` and
read the ordering or measure the proof checker uses. They should not need to
inspect the raw pointer carrier used by code generation.

The same split likely applies to other core collection and text concepts:

- `Array` owns fixed-size inline storage and can borrow as `Slice`.
- `Vec` owns dynamic contiguous storage and can borrow as `Slice`.
- `String` owns text storage; it has capacity and `push_str`.
- `string` (lowercase) is the borrowed text window, spelled `&string` /
  `&mut string`. Capitalization distinguishes the owner (`String`) from the
  window (`string`); the older `StrView`/`&str` naming is retired. The text
  window shares the same `{ptr,len}` slice descriptor carrier underneath.
- Low-level carriers such as `Ptr` or buffer descriptors may exist in core or
  a primitive layer, but they are the boundary where boundary/compiler-managed
  representation begins.

Working private carrier model:

- `Slice<T>` / `&[T]` lowers to a descriptor containing a primitive element
  pointer plus a length.
- `&mut [T]` uses the same descriptor shape, plus the type/borrow checker owns
  the uniqueness and writable-region facts.
- `string` / `&string` lowers to a byte pointer plus byte length (the same
  `{ptr,len}` slice descriptor carrier), with a text-domain fact that the bytes
  are valid text for the selected encoding.
- `Array<T, N>` owns inline storage and can produce a slice descriptor whose
  base points at the first element and whose length is `N`.
- `Vec<T>` owns a growable buffer carrier with base pointer, length, capacity,
  and allocator/runtime provenance.
- `String` uses the same owned-buffer shape as `Vec<u8>` plus text-domain
  facts; a borrowed `string` window is a descriptor over its initialized bytes.
- `Ptr<T>` and pointer-range construction are primitive-boundary concepts, not
  ordinary fields users manipulate through safe collection APIs.

This keeps the magic boundary narrow. Core declarations expose contracts such
as `Slice::range`, `Vec::with_capacity`, or `String::push_str`; private
carriers and boundary primitive providers implement the descriptor rewrite, pointer
offset, allocation, and initialization details after the proof obligations are
satisfied.

This sketch needs more design work, but the direction is important:

- Callers name exported invariants.
- Invariant names are scoped to the type that defines them.
- The type implementation maps public invariant names to private facts.
- Callers should not need to know private field layout to state proof
  requirements.
- Safe Omega source does not expose raw pointer fields for ordinary slices or
  vectors. Address-level representation belongs to compiler/runtime lowering
  and explicit boundary modeling, not the normal surface language.
- Core operators such as slice indexing and subslicing should have visible
  signatures and contracts; their implementations may be bound to explicitly
  boundary compiler/runtime primitives below the public core surface.

Short forms such as `&[T]` and `&mut [T]` mean the same slice views with no
extra invariant parameters.

## Boundary

Boundary is the authority for accepting a guarantee that Omega cannot prove from
Omega code.

For an imported function:

- Omega proves the caller satisfies the parameter and state invariants.
- The imported implementation is accepted to satisfy the declared guarantees.
- Omega may use those boundary guarantees as facts after the call.

In proof vocabulary:

- Input refinements and invariant parameters are caller requirements.
- Return refinements and boundary clauses are callee guarantees.
- The call creates obligations for the caller.
- `boundary` explains why unproved imported guarantees are accepted.

For ordinary Omega code, the compiler should know the contracts for
assignments, arithmetic, borrows, transitions, calls, and field access. For
imported libraries and syscall surfaces, the contract must be declared or
imported from an audited package.

Core primitives use the same discipline. A public declaration such as a slice
indexing operator states the visible contract. The implementation then binds to
a registered boundary provider such as slice indexing, descriptor construction,
pointer offset, allocation, or target ABI lowering. The provider name is not a
general-purpose user escape hatch: it must come from the toolchain, core
package, target configuration, or an explicitly whitelisted audited provider,
and it appears in the build boundary report.

`omega::language::core::ptr` is the natural home for pointer-level primitive
boundary providers. Safe source should generally work through owners and views, but the
language still needs a browsable place to audit names such as pointer offset,
read/write, and pointer-range construction.

## Boundary Primitive Registry

Compiler/runtime boundary providers are tracked, not free-floating names. The
slice indexing, pointer offset, descriptor construction, allocation, and host
ABI call surfaces are recorded in a registry of `BoundaryProvider` records.

Each `BoundaryProvider` record carries:

- `name`: the provider name a boundary implementation binds to.
- `category`: one of `SliceIndexing`, `PointerOffset`, `PointerAccess`,
  `DescriptorConstruction`, `Allocation`, or `HostAbiCall`.
- the public contract it implements (a reference, so the proof obligation and
  signature stay visible).
- its normalized kinded effect row.
- its target applicability.
- the origin package that declared it.

Core primitives are authored as restricted core declarations whose `boundary
operator` binds a named provider. Host providers are authored as
target-package metadata.

Decided rules:

- A package may declare providers only if it is whitelisted: core, host, or
  toolchain packages.
- Every boundary implementation binding must reference a registered provider.
- Unregistered provider names outside whitelisted packages are rejected.
- The emitted boundary build report lists the registered providers actually
  used, as the audit artifact.

This replaces the earlier state where `boundary operator` names and `boundary
<name>` host clauses floated free without validation. A bound provider name now
resolves to a registered record or the build is rejected.

## Blocking Boundaries

Imported entries that can block must say what can unblock them, or they must be
reported as boundary opaque waits.

Blocking and parking are distinct. An imported/provider contract carries
`Block` when it may occupy the calling worker and `Suspend` when it may park a
task. That row is checked against the pinned requirement at admission; the
eventual provider cannot widen a consumer compiled against a no-block/no-park
slot. Decision 23 represents v1 positive wake/fairness premises as sealed,
grant-backed opaque progress profiles on the pinned operation/provider
contract. They participate in admission and trust reports but do not become
ordinary proof facts or follow merely from row membership.

Examples:

- A pipe read may block until a matching write, close, timeout, or external
  event.
- A process wait may block until the target process exits.
- A socket receive may block on external network input.
- A driver call may block on hardware interrupt, timeout, cancellation, or a
  boundary opaque device contract.

The proof/invariant checker can reason about modeled waits. It can audit
boundary opaque waits. A proved-concurrency build may reject opaque blocking
boundaries.

## Host vs Standard Library

The standard library is the portable API most application code should use. It
can provide `Console.read_line`, formatting, strings, slices, data structures,
and higher-level process or filesystem helpers. These machines are ordinary
Omega code unless they are explicitly modeling the bottom host edge.

Host packages are the audited bottom edge. They contain imported libraries,
syscall surfaces, startup bindings, and boundary providers.

Typical layering:

```text
application code
  -> standard-library Omega machines
    -> boundary host trait/provider
      -> syscall / imported symbol / firmware jump / loader hook
```

Static vs dynamic linkage is not the same question as boundary vs normal code.
A statically linked standard-library wrapper is still normal Omega code if the
compiler can check its body. A dynamically imported, syscall-backed, firmware,
or externally supplied implementation is a boundary because its guarantees are
boundary rather than proved from Omega source.

Most users should not author raw Windows, Darwin, Linux, firmware, or console
SDK contracts for ordinary applications. They should import toolchain-provided
surfaces:

```omega
use omega::std::console;
use omega::host::windows::kernel32;
```

Advanced users can author libraries for custom OSes, firmware, game consoles,
or unusual hardware. Doing so explicitly expands the boundary base.

## Build Artifacts

Compiler artifacts should list imported libraries, syscall surfaces, the
registered boundary providers used, inferred authority flow, direct/transitive
host calls, and unchecked policies.

Example shape:

```text
authority flow:
  accepts:
    Folder where Folder::Writable
  uses:
    Folder::Writable
  derives:
    File::Writable from Folder::Writable
  stores:
    none
  acquires:
    none
  returns:
    none
  releases:
    none

effects:
  declared: Writable
  reached: Writable

trust receipts:
  omega_windows_kernel32_read_file -> accepted HostAbiCall contract
  omega_darwin_libsystem_write -> accepted HostAbiCall contract

imported libraries:
  Kernel32 -> Kernel32.dll calling_convention winapi
  DarwinLibSystem -> libSystem.B.dylib calling_convention c

direct boundary calls:
  none

transitive boundary calls:
  omega::std::fs::Folder::open
  omega::host::filesystem::openat
  omega::host::filesystem::write

registered boundary providers used:
  omega_windows_kernel32_read_file  category HostAbiCall  -> Kernel32.ReadFile
  omega_darwin_libsystem_write      category HostAbiCall  -> DarwinLibSystem.write
  omega_core_slice_index            category SliceIndexing -> Slice::index

target image imports:
  Kernel32.dll!ReadFile
  libSystem.B.dylib!_write
```

The "registered boundary providers used" list is the audit artifact for the
boundary registry: every entry resolves to a `BoundaryProvider` record, and a
binding that names no registered provider is rejected before this report is
emitted.

A build with proofs or contracts disabled should be stamped loudly rather than
silently behaving like a normal safe build.

The goal is not to pretend these edges disappear. The goal is to make every
boundary explicit, scoped, and auditable.

# Design Brief — The Extern Surface & Format Domains (settled 2026-07-01)

> **AMENDED 2026-07-02 by [`programmable_layouts.md`](programmable_layouts.md):**
> the compiler-known format *catalog* (§5/§6) decomposes one level down —
> blessed **placement primitives**, with formats as authored *layout policies*
> composing them; `wire data` (§9, §10.5) is **retired** in favor of plain
> `data` with optional identity numbers + `retired N;`; fully-static policies
> are additionally legal in *type position* (generalizing §8's structural
> zero-copy). §7's validate/materialize split, forget-on-boundary-write, and
> the never-import-foreign-concepts rule all stand unchanged.
>
> **AMENDED 2026-07-02 (second pass, same session): §3's provisional RHS
> spelling is replaced by the `Binding` sum (§12)** — no `syscall` contextual
> keyword, no DLL-as-module-path; mechanisms are case constructions. §12 also
> adds the **foreign-pointer taxonomy** and closes §11's WndProc dodge via
> entry stubs + registration guards.

> **For:** Omega maintainer · **Status:** SETTLED (chat session 2026-07-01, Zach) —
> spellings marked *open* below are the only undecided parts. · **Driver:** Tier-2
> windowed software rendering (TASKS.md) needs Win32 API calls; the host layer was a
> May-era sketch (`host:` blocks, `x = disabled` flags, `capability X { entry ... }`
> packages) that was never designed.
>
> Companion to [`chapter_18`](../language_guide/chapter_18_capabilities_effects_boundaries.md)
> (capability/boundary concepts) and the string/encoding-domain work (#66), whose
> carrier + domain machinery this design generalizes.

---

## 1. Bottom line up front

**There is no `extern` keyword. The foreign-function surface is the existing
`boundary trait`; the DLL is named only in a target's `provides` mapping; OS/ABI
structs are wire data serialized through *format domains* — encoding facts bound to
byte storage, chosen at the use site. No foreign language's concepts (no `c_layout`,
no repr attributes, no serde-style traits) enter the language. Omega's in-memory
layout stays sovereign; the boundary is an explicit, proof-carrying encode/decode
edge.**

Hard rule that shaped everything: **never import another language's concepts.**
Exhaust existing constructs (boundary traits, capabilities, domains, wire data,
provides) before inventing; never name a concept after another language's feature.

## 2. The extern surface = `boundary trait` (unchanged app code)

App code declares and injects capabilities exactly as today. It **never names a
DLL** — foreign-ness is a property of the provider, not the contract:

```
boundary trait Surface {
    machine present(frame: &[u32 in Bgra8], width: i32, height: i32);
    machine tick_count() -> u64;
}

data Main {
    console: Console;
    surface: Surface;   // capability injected like any other
}
```

## 3. The DLL binding lives in `provides` mappings

REVISED 2026-07-02 (the original `machine -> syscall N` contextual-keyword form
and the `gdi32::Sym` pseudo-path both die — see §12 for why): the mapping table
keeps its shape (`name -> value` arms, transition-arm grammar), and the RHS
becomes an **ordinary case construction of the compiler-known `Binding` sum**:

```
// omega/host/targets/windows/surface.omg
windows_x64 provides omega::host::contracts::Surface {
    present    -> DllImport("gdi32", "StretchDIBits")
    tick_count -> DllImport("kernel32", "GetTickCount64")
}

// omega/host/targets/linux/stdout.omg
linux_x64 provides omega::host::contracts::Stdout {
    write_line -> Syscall(1)        // Linux's stable ABI IS the number table
}
```

`provides` is NOT a Rust `impl`: it is a **binding/witness table** ("on this target,
the authority behind this contract is *this* mechanism"), **target-indexed**
(windows/linux blocks coexist, selected by build target), and it is the **audit
point** — authority (`dynamic_link`, `device_io`, `clock_read`) attaches to the
mapping. Never-name-a-DLL means the entire FFI audit surface is a grep over the
provides files. When a provider needs real glue code, a provider machine with a body
plays the impl role (an `[import(kernel32::WriteFile)]`-style attribute on such a
machine is a possible later extension; the mapping form is the settled core).

## 4. The target block: `host:` and the flag dialect are DELETED

The old `host: omega::host::targets::linux { abi = syscall  stdout = fd(1)
filesystem = disabled ... }` blob (May-06 "opaque build sketch", cargo-culted into
~180 build.omg files, flags never enforced) dies entirely:

```
target windows_x64 {
    boundary omega::host::contracts
    boundary omega::host::targets::windows::stdout
    boundary omega::host::targets::windows::surface
}
```

- A target = the boundary packages it **trusts** (positive grants only).
- **Absence = denial.** Code that injects a capability with no provider on the
  target is a clean compile error naming the missing contract. `filesystem =
  disabled` is redundant with the capability model and dies.
- `abi = ...` derives from the mapping kinds; `stdout = fd(1)` is provider
  implementation detail and moves into the Linux provider source.
- Denial granularity = package granularity (the repo layout already has
  per-capability files under `omega/host/targets/<os>/`).

## 5. OS/ABI structs = wire data + format domains (NOT layout control)

**Serialization is chosen; layout is never tweaked.** Reasons, in strength order:

1. Layout control re-imports the ABI into the type system (c_layout generalized)
   and threads a second offset axis through every compiler stage.
2. OS-filled data needs a validation step regardless (see §7 forget-on-boundary-
   write) — zero-copy layout buys no correctness, only skips a memcpy.
3. The one hot path (framebuffer) needs no layout control: where the derived codec
   is the identity for (type, format), encode is a no-op borrowed view — **the copy
   vanishes by theorem, not by declaration** (§8).

A Win32 struct is bytes with an externally specified layout — exactly what wire
data models (and better than C does: `cbSize` fields ARE version discriminators,
Win32 reserved fields ARE `reserved N;`, OS-filled buffers are validated decode):

```
wire data BitmapInfoHeader {
    1: size: u32;        // Win32 cbSize — a version discriminator, natively modeled
    2: width: i32;
    3: height: i32;      // negative = top-down
    4: planes: u16;
    5: bit_count: u16;
    reserved 7;
}
```

## 6. Format domains: encoded bytes = domain-refined carriers

**`in D` never changes what bytes ARE (still indexable, hashable, sendable); it
records what you KNOW about them.** `Utf8` is the shipping precedent — an encoding
fact bound to byte storage, alive at codegen. A serialized message is the same kind
of fact. The format is a property of the **use site** (the destination's domain),
never of the type — so one value serializes to many formats with no annotation on
the declaration:

```
save:  [u8; 256] in Protobuf<Level>;    // spelling of generic domains OPEN (§10)
cache: [u8; 256] in OmegaWire<Level>;   // Omega framing = just the default format
self.save  = encode(self.level);        // codec derived from schema; target-directed
self.cache = encode(self.level);        // same call, different derived codec
```

- **Formats are a compiler-known catalog** (`omega` default, `win32_x64`,
  `protobuf` later) with **derived codecs + conformance proofs**. Not
  user-programmable codegen, not serde traits: hand-implemented serializers forfeit
  derivation and the conformance theorem. Genuinely custom serialization = an
  ordinary machine writing into a carrier (exists today; ordinary proofs; no
  schema-conformance theorem, which is honest).
- **Compatibility is an obligation**: deriving `win32_x64` for a schema with a
  variable-length field is a clean compile error; encoded-size ≤ carrier capacity
  is the existing length-fits theorem — schema growth that breaks a buffer budget
  is a **compile error at the schema bump**, not runtime truncation.
- A `-> kernel32::Sym` provides-mapping **implies** win32 format on its wire-data
  arguments (edge kind determines format; no annotation in the common OS case).
- Versions: outbound may pin (`in Protobuf<Level::v1>` for an old peer); inbound
  accepts the family, version dispatch = the existing version-match arms
  (decision 14 era matcher). Migration = decode → materialize → encode.

## 7. Domain entry = MINTS ONLY. `is`/`as` do not exist; `when` dies.

Establishment is **functions**, uniformly — no membership operator:

```
machine validate(bytes: &[u8]) -> ValidUtf8 {
    case Valid(text: &[u8] in Utf8);   // case payloads carry domains (forwarding spine)
    case Invalid;
}
// caller: ordinary enum dispatch on the result — no is, no as, no Result type
transition Protobuf<Envelope>::validate(&self.rx) {
    Valid(env) -> route(env)
    Invalid    -> drop_packet()
}
```

- A function returning an `in X` payload must **prove** the invariant of its result
  (prover checks the body), or be a **trusted/audited mint** (boundary provider,
  domain-owning module). Format validators are compiler-**derived** mints. Nothing
  can trivially claim membership — the gated-domain concept survives intact.
- **`as`-into-domain does not exist, period** (the trapdoor that would collapse
  everything). **`is` was never surface syntax and is never added.**
- **`when` dies**: it parses in exactly one place (`parser/domain.rs:83`, the
  domain header) and every ~200 usages are the same vestigial
  `domain [u8]::Utf8 when valid_utf8(self)` line. Invariants become domain
  MEMBERS; migration is mechanical.
- **Decode splits in two**: *validate* (fallible mint, `&[u8]` → refined borrow,
  same lifetime) then *materialize* (refined bytes → value, **TOTAL** — all
  fallibility concentrates in the mint). Zero-copy = the shipped borrowed-view
  decode (#43/#46): scalar fields copy out, blob fields (`body: &[u8]`) borrow into
  the buffer. **There is no field projection on raw bytes** — fields come from
  types, never from domains.
- **Forget-on-boundary-write**: a buffer passed `&mut` across a boundary loses its
  domains (the OS scribbled); re-enter via the mint. Same shape as
  forget-on-reassign, extended to boundary calls.

## 8. Three domain kinds (taxonomy that fell out)

| Kind | Invariant | Entry | Examples |
|---|---|---|---|
| **Structural** | true of every value of the carrier type (a layout fact) | free theorem, discharged statically, writes can't break it | `Win32Dib` over `[u32; W*H]` |
| **Checked** | value-constraining, validator exists (written or derived) | validating mint | `Utf8`, `Protobuf<Level>` |
| **Gated** | no validator; meaning is extrinsic | trusted mints only | `Hwnd`, `Sorted`, authority tokens |

The framebuffer hot path is structural: `frame: [u32; 320*200] in Win32Dib` costs
nothing — membership is a free theorem, `present(&self.frame)` discharges
statically, **zero copy per frame**. Cold structs (WNDCLASSEXW once, MSG per event)
pay a real encode/decode, which is noise.

## 9. wire data survives, demoted to schema identity

Format domains absorb wire data's serialization role. What survives is what `data`
+ domains structurally cannot do: **durable cross-version field identity**. The
save-file argument: v1.0 writes `{1: seed, 2: hp, 3: gold}`; v1.1 deletes `hp`,
adds `mana`. With order-derived tags the old save decodes *validly* into the new
schema — 9999 gold becomes 9999 mana, **no error anywhere** (both versions are
internally consistent; the broken thing is the correspondence between two schemas
across time, which no single compile can see). With wire data: `reserved 2;` burns
the number forever, gold keeps identity, `mana` is absent→ZII, and reusing a burned
number is a **compile error** — the corruption is unrepresentable. Numbers handle
*compatible* evolution; `version` blocks handle *breaking* rewrites. Renames are
free (identity is the number).

## 10. Open spellings (the only unsettled parts)

1. **Generic domains — ANSWERED 2026-07-01: no generic semantics needed.**
   Nothing is ever polymorphic over the schema parameter (a relay forwards raw
   `&[u8]`; consumers always know the concrete schema), so `Protobuf<Level>` is a
   PARAMETERIZED NAME resolving at name-resolution time to a flat derived domain
   instance — monomorphization-by-instantiation with no unification, bounds, or
   inference. Mirrors the landed stage-1 data/machine monomorphization, and is
   easier (domains have no layout). Remaining spelling detail: the surface
   grammar for the angle-bracketed domain name.
2. **encode/decode surface** — builtins (like min/max) vs boundary-operator
   surfaces (like the stdlib slice contracts).
3. **Field-peek accessors** — per-field derived accessor *functions* for validated
   buffers (a call, never projection); exact spelling open.
4. **Streaming/append** — a log carrier appending encoded messages wants the
   concat-domain law + the in-place-append prover frontier (known, unbuilt).
5. Whether `wire data` keeps its name now that it means "schema with stable field
   identity," not bytes.

## 11. Migrations + engineering ladder unlocked

Cleanups (mechanical): delete `when` (1 parser site + ~200 .omg headers); delete
`host:` blocks + flag dialect (~180 build.omg + `parser/target.rs`); retire the
stale May-era `omega/host/**` sketch packages (`capability X { entry ... }`,
`String`, `Slice<u8>` spellings). Engineering (in order): general Win64 call
encoder (marshal N args, shadow space, rax return) **proven by re-expressing the
existing 5 kernel32 ops through it with no behavior change**; multi-DLL import
descriptors in `omega-image-pe/imports.rs`; `provides` path-mapping parse; PE
subsystem GUI toggle; opaque handle domains; wire-data format codecs (win32 first).
Callbacks (WndProc) stay out of the *rendering ladder's* scope — DefWindowProcW +
PeekMessageW poll + StretchDIBits blit avoids machine-as-C-function-pointer for
Tier-2. The general answer now exists as design: §12's entry stubs +
registration guards.

## 12. Foreign pointers & the `Binding` sum (added 2026-07-02)

### 12.1 The `Binding` sum — mechanisms are data

The §3 RHS forms were pseudocode (`syscall 1` needed a contextual keyword;
`gdi32::StretchDIBits` pretends a DLL is an Omega module path — it isn't; a DLL
name is a string in the world, not a namespace in the language). Fix: binding
mechanisms are a **compiler-known, closed sum** — same discipline as
`FieldPlan` (the compiler must know how to *lower* each mechanism; new
mechanism = new case + new lowering, never user-invented):

```
data Binding {
    case Syscall(number: count);                    // Linux stable ABI = the number table
    case DllImport(module: Text, symbol: Text);     // Windows stable ABI = named exports
    case VtableSlot(index: count);                  // COM/UEFI = per-object dispatch
}
```

The mapping block needs zero new grammar: `name -> value` arms (transition-arm
shape) whose RHS is ordinary expression syntax. One honest distinction:
`Syscall`/`DllImport` are **static** bindings (resolved at build/link time);
`VtableSlot` is a **dispatch recipe parameterized by the call's first
argument** — deref `this`, read the vtable pointer, read slot N, call at the
declared convention. A third *kind* of mechanism, not a third instance of the
same kind. Each Binding kind also **implies the edge's calling convention**
(`Syscall` → the target's syscall plan; `DllImport`/`VtableSlot` → its C
plan) — conventions are stated layouts over registers, one plan feeding both
the call encoder and the entry stub (see
[`calling_plans.md`](calling_plans.md)); nobody names one in the common case.

```
boundary trait ISum {                      // the contracts ARE machine signatures
    machine query_interface(this: ComPtr, iid: &Guid, out: &mut ComPtr) -> HResult;
    machine add_ref(this: ComPtr) -> u32;
    machine release(this: ComPtr) -> u32;
    machine add(this: ComPtr, a: i32, b: i32) -> i32;
}

windows_x64 provides ISum {
    query_interface -> VtableSlot(0)
    add_ref         -> VtableSlot(1)
    release         -> VtableSlot(2)
    add             -> VtableSlot(3)
}
```

Native `dyn` never touches foreign layouts in either direction: Omega's trait
objects keep their private representation; foreign vtables are provides-bound
dispatch, all the way down. (Precedent: windows-rs models COM exactly this way
— generated vtable structs + raw calls, never Rust `dyn`.)

### 12.2 The foreign-pointer taxonomy — four cases, no fifth primitive

The unifying rule: **the boundary converts the foreign representation into a
native discipline-carrying value once, at the mint — downstream code never
sees an address.** Same pattern as bytes (validate → refined borrow →
materialize), applied to pointers.

1. **Call-scoped pointer arguments** (`ReadFile(handle, buffer, …)`) —
   SHIPPED. A borrow already *is* a pointer: `&mut [u8]` lowers to
   address+length; the call encoder passes the address of a buffer the caller
   legally holds. `&mut`-across-a-boundary forgets domains (the OS scribbled);
   re-enter via mint.
2. **Retained pointer arguments** (OVERLAPPED async IO, anything stashing your
   buffer past the return) — the KNOWN GAP, already met in another costume:
   ch20's zero-copy-decode rejection ("borrow facts cannot see a call output
   retaining a borrow of another argument"). Tracked in TASKS. Interim pattern
   that works today: **ownership transfer** — move the buffer in, get a
   completion token, the completion machine returns the buffer. No loan, no
   gap.
3. **Returned data pointers** — two flavors:
   - *Opaque* (`HWND`, module handles): **gated-domain tokens** — no memory
     operations exist on them; pass-back only.
   - *Dereferenceable* (`HeapAlloc`, mapped views): the binding mints an
     **owned value with foreign backing** (extent = the provider's audited
     axiom; drop = the release call), and ordinary borrows flow from the owner.
   - **Borrowability rule**: a borrow requires the *no-invisible-writer*
     premise. RAM you own → borrow. Device registers → never (access-as-event
     semantics; volatile operators only — a register can be mutated under you
     by the device, so `&mut`'s exclusivity claim would be a lie the optimizer
     exploits: hoisted polling loads, elided FIFO reads, merged register
     writes). In-flight DMA buffers → RAM but device-mutable: **move, don't
     borrow** (ownership transfer to the transfer, returned at completion).
4. **Function pointers, both directions:**
   - *Inbound* (`GetProcAddress`, COM vtables, UEFI tables): boundary-trait
     machines are the contracts; `VtableSlot`/`DllImport` mappings bind them;
     the win64 call encoder at a runtime pointer is the (tracked) lowering
     work. The minted callable **borrows from its owner** (module token, COM
     object) so it cannot outlive `FreeLibrary`/`Release`.
   - *Outbound* (WndProc, COM implementations, the UEFI export table): the
     code address is a **link-time constant to a compiler-emitted entry stub**
     (code is immutable and `'static` — nothing to borrow); the lifetime
     discipline attaches to the *state* via a **registration guard** —
     `register(stub_for(M), &mut state) -> Guard`, drop = unregister *before*
     the loan ends. Stale-callback-into-freed-state becomes unwritable.
     Entry stubs are one design shared with interrupt entry (foreign-initiated
     activation; see the boot brief) — bounded static tables, never
     first-class runtime function-pointer values in Omega semantics.

Summary line: every foreign data pointer is a borrow you gave out, a token you
can't deref, or a mint with a named axiom; every foreign call target is a
declared contract on a provides-bound slot; every callback we hand out is a
static stub plus a guard on its state.

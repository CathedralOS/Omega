# Design Brief — Build & Package Model (`build.omg`, reach, and pinned closures)

> **For:** Omega maintainer · **Status:** SETTLED — model (chat 2026-07-02);
> `Build` v1 schema + the target-block retirement SETTLED (chat 2026-07-04,
> Zach); granted-filesystem revision SETTLED (chat 2026-07-07, Zach) — see the
> two addenda at the end. · **Driver:** Cathedral is starting on boot and needs the
> per-package boundary manifest that `separate_compilation.md` calls for — this
> settles what that manifest is. · **Depends on:**
> [`build_time_evaluation.md`](build_time_evaluation.md) (pure build-time eval),
> [`separate_compilation.md`](separate_compilation.md) (component boundary =
> package; machines swap within; sealed IR + manifest artifact),
> `chapter_15_modules_imports_visibility.md`. · **Companion:** Cathedral
> `wiki/architecture/repository_layout.md` ("Reach: imports are declared, never
> ambient") and the `developer_experience` chapter (pinned closures +
> content-dedup).

---

## 1. Bottom line up front

**The per-package build manifest is `build.omg` — an *effect-free Omega machine
that augments a mutable `Build`*, not a config file and not a config grammar.**
It is code (one language, real types, full power — no TOML to fight, no
`build.rs` cliff, and crucially no invented `depends { } target { }` block
dialect), but because it declares no effects and is *interpreted* at build time,
its result is plain inspectable data (the dependency set, targets, options). One
mechanism spans "set a few fields" to "compute the target matrix"; there is no
second, worse escape hatch.

Four consequences settle long-standing questions:

1. **Pure plan, effectful executor.** *(REVISED 2026-07-07 — see the second
   addendum: build.omg now runs with a granted, scoped Filesystem capability
   and stages assets itself; the grant is the audit surface.)* `build.omg`
   computes *what* to build; the package manager / toolchain performs the
   remaining effects (fetch, compile, link). The driver still runs that plan —
   the same split as everywhere else in the system.
2. **No toolchain seed, no lockfile.** `build.omg` is *interpreted* by the tool
   in hand, so it can declare its own toolchain without circularity; and because
   dependencies are pinned in the manifest itself, there is nothing to lock.
3. **Dependencies are local aliases bound to pinned sources** (hash / path /
   git-rev). No semver solving — pinning is exact.
4. **The package is the reach boundary.** A package imports only what its
   `build.omg` declares; an undeclared package is not nameable. The build-time
   capability model.

## 2. `build.omg` is a machine, not a config grammar

The config-as-data (Cargo) vs config-as-code (Jai/Zig) tension dissolves because
Omega already has the two pieces that make code safe as config:

- **The effect system** guarantees a `build.omg` with no declared effects does
  no IO — it cannot fetch, read the environment, or be nondeterministic.
- **Build-time evaluation** ([`build_time_evaluation.md`](build_time_evaluation.md))
  means the toolchain simply *runs* it and reads back a value.

The critical discipline — **no invented config grammar.** `build.omg` is not a
`build { depends { … } target { … } }` block dialect (that would be a secret
export language, exactly the Cargo-`.toml` disease). It is an **ordinary machine
that augments a mutable `Build`** — machine calls and field assignment, the Zig
`build.zig` shape in Omega. `build` is a conventionally-named entry machine the
toolchain invokes with a fresh (ZII-default) `Build`, like `main`; `Build` is a
toolchain-provided type (ordinary `data` + machines):

```omega
// source/boot/uefi/build.omg — provisional syntax; every token is existing Omega
machine build(b: &mut Build) {
    b.depend("uefi", path("../../contracts/uefi"));   // machine call, appends to reach

    let target = b.target(Uefi64);                    // ordinary value
    target.entry     = main;                          // field assignment
    target.subsystem = EfiApplication;
    target.stack     = 128 * KiB;
}
```

Nothing there is grammar — `path`, `hash`, `Uefi64`, `EfiApplication`, `KiB` are
library *values*. The simple case reads almost like config (set a few fields);
the powerful case (branch on the target, loop over a set) is the *same*
mechanism scaled up, not a `build.rs`-style bolt-on.

**Why augment-a-`Build` beats return-a-value.** Because it mutates a
*passed-in local* (not an effect), it stays pure and analyzable — the toolchain
runs `build`, reads the resulting `Build`, and the build report shows it as data
(authored as code, consumed as data). But it also makes **composition and
layering fall out**: a workspace `build` seeds shared defaults into a `Build` and
passes it to each member's `build`, which augments it (org → package → local as
ordinary sequential mutation — no `[workspace]` grammar, no invented precedence).
This is the general Cathedral config stance — *all* config is an augmenting
machine over a typed settings value, never a parsed file format (see
`configuration_and_policy`: ZII defaults, `inherit` = fill-absent augmentation,
`constrain` = a ceiling machine).

## 3. Pure plan, effectful executor

> **REVISED 2026-07-07 (chat, Zach) — see the second addendum.** The
> "describe, never do" framing below is RETIRED: build.omg runs interpreted
> with a granted, scoped `Filesystem` capability and copies assets itself.
> The section is kept for the parts that remain true (the augmented `Build`
> is still the plan the driver executes for fetch/compile/link).

Building has effects — fetching a git dependency, invoking the backend, writing
artifacts. `build.omg` has none of them, because it only *describes*:

```text
tool-in-hand ── interprets ──▶ build.omg (pure) ──▶ the augmented Build (data)
                                                        │
package manager / toolchain ── executes the plan ──────┘   (fetch, compile, link — the effects)
```

`build.omg` returns "I need `render` at hash X, targeting Uefi64"; the driver
performs the fetch and the build. This is the layout-policy pattern
(`programmable_layouts.md`) and the whole-system pattern (compute the plan
purely; a trusted executor performs the effects) applied to the build.

## 4. No seed, no lockfile

The apparent chicken-and-egg — *"how can `build.omg` name the toolchain if the
toolchain compiles `build.omg`?"* — does not arise, because **evaluating
`build.omg` and building the package are two different executions**:

- The tool in hand *interprets* `build.omg` (a pure function) via the reference
  interpreter every Omega binary carries — it is never *compiled by the
  toolchain it names*.
- Interpreting a pure function is **version-invariant**: any correct interpreter
  yields the same value (exactly the property the differential oracle
  guarantees). So the interpreter needs no pin either.

Therefore `build.omg` may declare its own toolchain with zero circularity, and
there is **no data-config artifact anywhere** — the only irreducible thing is
"an Omega binary is installed," which is ambient tooling, not a file in the
repository. Everything is code. ("Compile to IR" is the same point from the
other side: IR is the toolchain-robust checkpoint, downstream of `build.omg`'s
interpreted evaluation.)

**No lockfile** in the pinned case: the exact pins live *in* `build.omg`, so
there is no separate resolution to freeze. A lock reappears only if an alias is
bound to a *mutable* ref (a git branch) and you want to record what it resolved
to — opt-in, mutable-ref-only.

## 5. Dependencies = local aliases → pinned sources

Keep the one good idea in Cargo — the indirection — and drop the two bad ones
(TOML, semver solving):

- Code names a dependency by a **local alias** it chooses (`use render.Surface`);
  `build.omg` **binds** the alias to a pinned source (`render = hash(…)` /
  `path(…)` / `git(rev)`). The alias is stable; the binding is what moves, so
  code survives a dependency relocating disk → git → content-store untouched.
- Aliases are **package-local and renameable** (copy Rust). Two packages
  depending on the same artifact alias it independently — no global namespace to
  collide, and honest to content-addressing: the alias is cosmetic, the hash is
  the truth.
- **No version solving.** Bindings are exact. Different pinned versions are
  different files (dedup by hash), so there is no DLL hell and no resolver.

## 6. The package is the reach boundary

`build.omg` is where a package declares what it may reach, so it is the
build-time analog of the capability model:

- **Imports resolve only against declared aliases.** A package the manifest did
  not bind is not nameable — the layer law becomes *self-enforcing*, not merely
  linted (`boot`'s `build.omg` omits `core`, so `boot` cannot express an import
  of `core`). This is the import-side gate tracked as the Omega ask in
  `cathedral_alignment.md` item 4; `chapter_14` name resolution must consult the
  declared set so a fully-qualified path cannot bypass it.
- **`pub` and the manifest are the two-sided contract:** `pub` (chapter 15
  visibility) says what a package *offers*; `build.omg` says what it may *reach*.
- **Different reach-set → different package.** Visibility (`pub` tree) and
  hot-swap points (machines — `separate_compilation.md`: deployment unit ≠ swap
  unit) nest *within* a package with no sub-manifest. A part that needs external
  dependencies the rest of the package must not touch is, by that fact, a
  separate package — split it. So `build.omg` stays one-per-package; the
  "self-updating private sub-module" is a private machine that is a swap point
  inside the package's reach, not a nested manifest.

## 7. Workspace composition

A workspace `build.omg` composes member packages' `build.omg`s by **value
aggregation** — it references them and combines their descriptions. Because it
is code, composition is a language feature, not a `[workspace] members = […]`
grammar. Shared settings (the toolchain, shared pins) live at the workspace
root, and a member **reaches up to the root** to inherit them.

This is the one legitimate "reach up": the *build* walks up to discover the
enclosing boundary (the nearest `build.omg`), the way Cargo walks up to
`Cargo.toml`. **Code** never reaches up — it reaches only through declared
aliases. Boundary-discovery reach-up is fine; code reach-up is the ambient
authority this model forbids.

## 8. What is still open

- **The `Build` type's schema** — the exact typed fields and augmentation
  machines it exposes (deps,
  targets, entry points/interfaces, stack sizes and other options, the toolchain
  pin, and whatever build-time capabilities the executor needs). Design when the
  first real `build.omg` lands, not before.
- **Surface syntax** — the `build … { }` spelling above is provisional.
- **The resolver change** — making `chapter_14` name resolution gate on the
  declared dependency set (the import-side gate); interim enforcement is the
  graph check (`imports ⊆ declared deps`, build-failing).


## Addendum — SETTLED 2026-07-04 (chat, Zach): `Build` v1 and the target-block retirement

**The in-source `target <name> { subsystem ... }` block dies.** It was a block
dialect — exactly the invented-config-grammar disease §2 forbids — that landed
as a stopgap before this brief's model was executable. Image facts live in
`build.omg`; the block's canaries/samples migrate; the parse is removed.

**`Build` v1 — only what the pipeline consumes today, true ZII:**

```omega
data Subsystem {
    case Console;                    // ZII zero case = the default, for free
    case Gui;
    case EfiApplication;
    case Unspecified(value: u16);    // the voila hatch: any loader value a
}                                    //   platform invents, with NO compiler release

data Build {
    subsystem: Subsystem;            // PE loader METADATA (a header u16 the
                                     //   loader branches on; the compiler only
                                     //   copies it -- it does NOT select the
                                     //   emitter; PE-vs-ELF is the target
                                     //   OS/arch; ELF has no such field)
    freestanding: bool;              // "trust no host packages" -- previously
                                     //   FUSED into the EfiApplication name;
                                     //   now stated as itself, orthogonal
}
```

Design rulings inside that shape:
- **Named cases + `Unspecified(u16)` beat a raw `u16`** because the ZII zero
  case is `Console` — the correct default falls out of the type, where a raw
  number's zero is `IMAGE_SUBSYSTEM_UNKNOWN` (a wrong default needing a fixup).
  The escape case preserves full programmability: a new loader value is data.
- **Decompose fused facts.** `efi_application ⇒ freestanding` was two facts in
  one name; `Build` states each. The compiler branches on `freestanding`
  (empty host-ABI plan), and passes `subsystem` through.
- **Absent `build.omg` ≡ empty `build` machine ≡ zero `Build`** — three
  spellings of the console default. Nothing is required until overridden.
- Fields arrive WITH their features (`entry`, `stack`, `depend` when their
  machinery lands), never speculatively.

**Entry-shape checking moves here too:** the platform's arrival contract
(what registers/bytes the subsystem's convention delivers to the entry) is
compiler-known; the exported `boundary machine`'s declared parameter shape is
checked to FIT it per built target — a loud per-target error, no target-side
entry declaration (that was a third statement of information already present;
see `calling_plans.md` §7).

## Addendum — SETTLED 2026-07-07 (chat, Zach): granted filesystem; "describe, never do" retired

**build.omg DOES the asset staging itself.** No declarative asset list, no
copy-plan the driver replays: build.omg runs INTERPRETED with a granted,
scoped `Filesystem` capability (read: the source tree; write: the build dir)
and copies what it wants where it wants — "this is the whole point of making
the build system code." §3's purity framing is retired for the filesystem;
**capability grants are the audit surface** that purity used to be:

```text
tool-in-hand ── interprets ──▶ build.omg (fs-granted) ──▶ the augmented Build (data)
                                     │                          │
                     stages assets (its own effect,             │
                     scoped to read:src write:out)              │
                                                                ▼
package manager / toolchain ── executes the remaining plan (fetch, compile, link)
```

What this does NOT change:

- **Everything else stays fenced.** Only `Filesystem` is granted; a build.omg
  touching any other host boundary (console, clock, gui, network-when-it-
  exists) is rejected — statically by the effect gate, dynamically by the
  interpreter's non-fs backstop. Additional grants would be their own,
  equally explicit decisions.
- **The augmented `Build` is still the plan.** Dependencies, targets, options
  ride back as data; the driver still performs fetch/compile/link. The
  retirement is narrow: asset staging moved from "described to the driver"
  to "done by the manifest under grant."
- **No toolchain circularity (§4).** build.omg is still interpreted by the
  tool in hand, never compiled by the toolchain it names. One nuance
  inherits a condition: §4's "interpreting a pure function is
  version-invariant" now reads "version-invariant GIVEN the filesystem
  state" — the pins/toolchain argument is unaffected (those fields are
  plain data), and the differential oracle still pins the interpreter's fs
  semantics (the hermetic virtual fs is unchanged and remains the default
  everywhere except an actual granted build).
- **Scoping is enforced, not advisory.** Grant checks canonicalize both the
  roots and every op path (a not-yet-existing leaf rides its canonicalized
  parent), so `..` traversal and symlinks that escape a granted tree resolve
  to their real target and refuse with EACCES; both ends of a rename need
  write authority; fd-based ops need no re-check because an fd only enters
  the table through an authorized open.

**Engineering state (2026-07-10):** the interpreter side is LANDED —
`interpret_with_options` + `FilesystemAccess::{Virtual, RealUnscoped,
RealScoped(FsGrants)}` (omega-interpreter), the real-fs provider
(create/open/read/write/seek/positioned I/O/dirs/read_dir/metadata/rename;
the rest -1/ENOTSUP loudly), and `evaluate_build_machine_with_filesystem`
(the augmenting-machine runner with the fs-allowing backstop). The
compiler-side entry (`pipeline/build_config.rs`) still enforces the retired
empty-effect gate; relaxing it waits on the OPEN QUESTIONS below.

**Open (design, not yet settled):**
1. **Capability injection spelling** — how `machine build(b: &mut Build)`
   receives its `Filesystem`: a field on `Build` (`b.fs`), a second
   parameter, or machine-owned data. Every build.omg will spell this.
2. **Grant derivation defaults** — read = the package dir (main.omg's
   directory)? write = which output dir? May build.omg request extra roots,
   and does that require a CLI acknowledgment (`--allow-read=...`)?
3. **Gate shape** — relax the effect gate to "⊆ {filesystem}"
   unconditionally, or behind an explicit opt-in?
4. **Console for build logging** — currently rejected (the strict,
   reversible choice); print-logging is the obvious want.

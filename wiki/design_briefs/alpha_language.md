# Alpha — the hand-audited rung-0 seed language

> Alpha is the smallest subset of Omega that can host its own compiler. This is
> the start of Alpha's standalone spec (it will eventually split into its own
> wiki, separate from Omega). This document covers the **auditability
> constraints** — the limits and exclusions that keep Alpha bootstrappable and
> trustworthy.

## What Alpha is

- **A strict syntactic + semantic subset of Omega.** Every Alpha program is a
  valid Omega program with *identical* runtime meaning. Omega is Alpha's exact
  syntax + semantics plus a monotonically growing set of features and proof
  obligations (the conservative-extension invariant). Climbing the rungs you gain
  power *and* gain demands; you never reinterpret or weaken an existing construct.
- **Prover-free by construction.** Every safety property in Alpha is either
  *statically computable* (no theorem-prover) or *runtime-checked* — never
  *proven*. This is the whole reason the seed can be tiny: it contains no prover.
- **Brought up by "the Alpha seed"** — a tiny, hand-written trust root (raw asm,
  or a hex0-style chain) that produces the first Alpha-compiler binary, then
  retires to being the audit/provenance anchor. The seed is the *ground*; Alpha
  is *rung 0* (the lowest place you can stand and write Omega). Alpha may be
  brought up in internal stages, but every stage is still Alpha (same language).

## Why the constraints exist

Two jobs, and every limit below serves one of them:

1. **Keep the seed compiler tiny and fixed-buffer.** No dynamic allocation in the
   seed — every table is a fixed-capacity arena sized at seed-build time. → the
   *resource budgets*.
2. **Keep Alpha prover-free.** No feature whose soundness needs a theorem-prover,
   refinement engine, borrow checker, or effect system. → the *banned features*.

A third, cross-cutting requirement — **determinism** — is what makes the
self-reproduction fixed point and Diverse Double-Compilation checks meaningful.

## Resource budgets (fixed buffers — no dynamic allocation)

All are constants fixed at seed-build time. Each must exceed what compiling the
**Alpha compiler itself** requires, plus headroom; numbers below are illustrative
placeholders, finalized once the compiler exists.

### Input
- **Multiple source files, ONE translation unit — multi-FILE, not multi-MODULE.**
  The compiler takes an *ordered* list of source files and concatenates them
  (with a separator) into a single token stream. There is NO module system: no
  imports, no per-file scoping, no visibility rules, no separate compilation —
  one flat global namespace. A name-collection pre-pass (needed anyway for mutual
  references) makes declaration order irrelevant. The ordered file list keeps
  output deterministic (required for the fixed point / DDC). Seed cost is
  near-zero (read a list instead of one file); the win is large — per-platform
  lowerings live in separate files, so the hand-audited *compiler* is readable
  instead of one giant file. (The earlier "single file only" rule was
  over-conservative: it shrank the seed trivially while monolithizing the
  compiler — the opposite of the auditability goal.)
- `MAX_SOURCE_BYTES` — total source size (e.g. ~1 MiB).
- `MAX_TOKENS` — token-buffer capacity.
- `MAX_IDENT_LEN` — longest identifier (fixed name storage).
- `MAX_STRING_LITERAL_LEN` and `MAX_LITERAL_BYTES_TOTAL` — the read-only data
  section is a fixed buffer.
- `MAX_LINE_LEN` — optional, if the lexer wants a fixed line buffer.
- **ASCII-only source**, except *inside* string-literal bytes (which pass
  through). Full Unicode lexing is a large amount of seed code; identifiers,
  keywords, and punctuation are ASCII.

### Structure (one fixed arena per kind)
- `MAX_AST_NODES` — the AST node arena (children are `usize` indices, not
  pointers).
- `MAX_NESTING_DEPTH` — the explicit parse/walk **worklist** arena. This is the
  replacement for a call-depth limit (see *No recursion*): it bounds how deeply
  nested a program structure Alpha can process. Overflow is a loud, declared
  failure.
- `MAX_MACHINES`, `MAX_STATES_PER_MACHINE`.
- `MAX_DATA_TYPES`, `MAX_FIELDS_PER_DATA`, `MAX_CASES_PER_SUM`.
- `MAX_LOCALS_PLUS_PARAMS_PER_STATE` — fixed frame layout.
- `MAX_SYMBOLS` — the symbol table (a fixed-capacity table: open-addressed, or a
  sorted arena + binary search over `&[u8]` name keys; no heap hashmap).
- `MAX_OUTPUT_BYTES` — the code+data output buffer.
- `MAX_RELOCATIONS` — the patch/fixup list arena.

## Banned features (no hard machinery in the seed)

- **No heap / no allocator.** Static storage + the call stack + fixed-capacity
  arenas only. (Bulk-free = reset the arena.)
- **No recursion in the call graph.** Machine-calls must form a **DAG**
  (a syntactic, decidable check — no prover). Stack depth is therefore a static
  budget = longest DAG call-path × per-machine frame (`max(state sizes)`); stack
  overflow is impossible by construction. *Within* a machine, transitions
  (including tail self-transitions = loops) are allowed and may iterate
  unboundedly — non-termination is a programmer bug, not an unsoundness, and
  Alpha does not try to prove termination (that is a higher rung). Recursive
  algorithms are written as an explicit worklist + a loop, single-machine, no
  cross-machine cycle.
- **No prover-dependent features:** refinement types, range constraints
  (`a..b`), encoding/arithmetic domains (`Utf8`, `Wrapping`/`Saturating`/
  `Trapping`), `requires`/`ensures` contracts, `terminates { decreases }`.
- **No generics.** Monomorphic only; the compiler's own containers are
  hand-specialized (as `FixedVec` ships concrete today).
- **No effect/capability checking.** Exactly one hard-wired boundary —
  `read` / `write` / `exit` — emitted directly by the seed. No effect rows, no
  provider resolution.
- **No lifetimes / borrow checking.** Alpha programs are structured so slices
  borrow from long-lived buffers (the source buffer, the arenas) that outlive
  every use by construction; no borrow checker is needed to keep the seed sound.
- **No concurrency.** Single-threaded batch process: read input → compute →
  write output → exit. No `spawn`/`await`/`suspend`/atomics.
- **No drops / destructors.** Heap-free, so nothing to free; ZII reset suffices.
- **No traits / dyn dispatch.** Replaced by concrete `match` on a `case` tag.
- **No closures / first-class functions.** Named machines only; dispatch is a tag
  + multi-way branch.
- **No floating point.** The compiler doesn't need it; banning it shrinks the
  seed's encoder.
- **A small fixed integer-width set** (e.g. `u8`, `i32`, `usize`) — enough for
  bytes, general arithmetic, and indices; keeps the seed's encoder minimal.
- **No macros / preprocessor.**

## Pinned semantics (auditable, no prover)

- **Indexing: DECIDED — runtime bounds-check + trap on out-of-bounds.** Keeps
  Alpha memory-safe without a prover (~1 cmp+branch per index); a higher rung
  proves the check away and deletes it.
- **Integer overflow: DECIDED — trap.** Deterministic, catches bugs; a higher
  rung discharges it (or opts into a `Wrapping`/`Saturating` domain).
- **General rule: trap on any unprovable runtime violation.** With no prover,
  Alpha's safety net is "check at runtime, trap on violation" everywhere a higher
  rung would carry a static proof. Traps are deterministic and a trap is never
  unsound (it halts; it doesn't corrupt).
- **Errors:** first-error-and-halt — print a diagnostic, exit nonzero. No
  exceptions, no recovery/resync in the seed.

## Determinism (required for the fixed point + DDC)

- **Byte-identical output for identical input.** No clock, no environment, no
  randomness, no nondeterministic iteration order (fixed arenas iterate in
  insertion order, so this largely falls out).
- This is what makes `rung0.exe(rung0_source) == rung0.exe` (the
  self-reproduction fixed point) a meaningful check, and what lets two
  independent build paths (the seed lineage vs a hex-chain lineage) be compared
  byte-for-byte to defeat trusting-trust.

## Self-hosting / target constraints

- **The Alpha compiler must be writable in Alpha** (the fixed-point test) and
  must fit its own budgets (every `MAX_*` ≥ what the self-compile needs +
  headroom).
- **One target per seed** — a single (ISA, OS/exec-format) at a time;
  cross-compilation/retargeting is a later concern (re-roll the tiny seed per
  target; the rest of the tower is portable).

## Open decisions
- The concrete `MAX_*` values — set after the Alpha compiler exists; generous
  round-number placeholders until then.
- Exact integer-width set for Alpha (minimum that still lets the compiler express
  itself comfortably).

## Build approach (decided)
"Action produces information": write a **throwaway Alpha compiler in Rust** (the
on-ramp) to discover what Alpha actually needs, then port that compiler's
structure to Alpha so Alpha compiles itself. The on-ramp's TRUST lineage does not
matter (it's discarded) — purity comes later from the Alpha-in-Alpha compiler +
the hand-written Alpha seed / DDC. What matters is that the on-ramp is written in
simple, arena-based, monomorphic Rust that **ports 1:1 to Alpha**, and that its
front-end **enforces the Alpha subset** (the front-end is the executable
definition of Alpha). Bootstrap sequence: on-ramp compiles `alpha_compiler.alpha`
→ `alphac`; `alphac` compiles `alpha_compiler.alpha` → `alphac'`; require
`alphac == alphac'` (byte-identical fixed point = self-hosting). The clean trust
root (tiny hand-written seed, cross-checked by DDC against the on-ramp lineage)
is a later step.

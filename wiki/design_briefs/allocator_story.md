# Design Brief: Allocator Story

Scouted 2026-06-12. Status: AWAITING SIGN-OFF (decisions A1-A5 in TASKS.md).

## Current State

- `Vec<T>` is browsable surface (core/vec.omg) with ZERO runtime (no
  lowering/codegen). `String` is statically fixed-capacity (push_str is a
  proof obligation against capacity; no heap anywhere). `alloc`/`dealloc`
  are effect NAMES only (chapter 18).
- The boundary-provider registry (frozen decision 4) already reserves an
  `Allocation` category — the designed hook for allocator providers.
- Decision 15 (lifetimes) makes allocator-borrowing containers spellable
  (`Vec<'r, T>` borrowing a region).

## Recommendations

1. **No ambient heap, ever.** Allocation is an explicit capability —
   matches Cathedral's kernel model (no invisible global state) and the
   capability discipline everywhere else.
2. **`Region<'r>`** is the allocator surface (name chosen over `Arena` to
   match Cathedral's resource model): stage 1 contract-only boundary trait;
   stage 2 a capability value with `allocate` machines bound through the
   `Allocation` provider category (host malloc provider vs Cathedral arena
   provider per target).
3. **Vec ladder**: stage 1 = fixed-capacity only (ArrayVec-like; all ops
   carry `requires len < capacity` proof obligations; no allocator at
   all). Stage 2 = `Vec<'r, T>` borrowing a Region, capacity fixed at
   `with_capacity`, NO growth. Stage 3 (only if needed) = pluggable
   `Allocator` trait + growth.
4. **Failure semantics: proof-obligated capacity** — `push` requires
   provable room; `try_push -> Result` is the optional stage-2 fallback
   for dynamic cases. No silent traps; OOM at a Region boundary is a
   boundary contract question.
5. **Drops**: elements drop immediately when the Vec dies (chapter 16
   obligations); the REGION frees memory in bulk on its own drop (memory
   release and object cleanup are separate concerns).
6. **`alloc` effect** is declared at the allocator boundary machine only
   and propagates transitively (decision 12 machinery) — individual Vec
   reads/writes carry no effect.

## Touches

omega-runtime-abi/omega-layout (a {ptr,len,capacity} 3-word descriptor),
drops analysis, effects, instruction selection (index/push/pop/as_slice),
proof engine (Length/Capacity measures, capacity invariants).

## Staging

1. Heap-free: fixed-capacity Vec with proof-obligated ops; Region trait
   surface (contract only). Cathedral kernels get typed bounded buffers.
2. Region-backed: `Region<'r>` providers (host malloc / arena),
   `Vec<'r, T>` with fixed capacity, element drops + bulk region free.
3. Pluggable allocators + growth + try_push, if demand materializes.

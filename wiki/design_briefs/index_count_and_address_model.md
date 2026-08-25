# Design Brief — The Index, Count & Address Model

> **For:** Omega maintainer · **Status:** SETTLED and implemented: `usize` and
> `isize` are rejected, counts use explicit integer carriers, and addresses use
> `addr`. Signed indices require a proved nonnegative lower bound; unsigned
> indices establish it by type.

---

## 1. Bottom line up front

1. **`usize` is retired.** It conflated four roles (pointer width, address bits,
   count, index). Split into an explicit **count** carrier, a distinct
   **address** carrier, and carrier-agnostic proof-bounded indexing. No global
   width equality or ordering connects those roles.
2. **Indexing is untyped.** `arr[i]` carries one obligation — `0 <= i < len` —
   and accepts *any* integer type that can discharge it. The proof is erased at
   codegen; there is no `Fin`-style index value and no index width in the surface.
3. **Width is a lowering detail, not a surface correctness concern.** A proven
   index/count lowers to the narrowest machine width that provably holds its
   bound (default `u64`).
4. **Named index types fall out of the domain spine for free**, and domain
   membership is closed-world (established, never inferred from absence).

The unifying rule under all of it (§7): **a sound conclusion is *derived* from a
positive fact; the bug is *assuming* one from the absence of a disproof.**

---

## 2. `usize` is gone; count and address are separate axes

The evidence that conflation is a *portability bug*, not a style preference:
Rust's [usize-on-CHERI](https://github.com/rust-lang/rust/issues/65473) — on Arm
Morello a pointer is a 128-bit capability while addressable size stays 64-bit, so
one type cannot be both; AVR's segmented memory breaks it the other way. Every
language that thought hardest about it splits the concepts, and Hare *orders*
them (`width(size) <= width(uintptr)`).

- **`addr` (pointer/address width).** First-class — **not quarantined.** Omega is
  an OS/driver language (Cathedral OS); addresses are the domain (MMIO, page
  tables, DMA, allocator). The lesson from the survey is *separation, not hiding*:
  `addr` must never be the silent default for a non-address (a count, an index),
  but a driver gets ergonomic, first-class address arithmetic. Ideally `addr` is
  itself a *proven capability* (in-region, aligned), the same bound-carrying
  discipline as an index, tying into the allocator-as-capability and lifetimes
  work — an address in Omega is a checked capability, not an unsafe escape.
- **count (magnitude / `len` / sizes).** Non-negative; the carrier of lengths and
  sizes. Its width is a lowering detail (§4). Until the dedicated count
  lowering exists, the source carrier is `u64`, never `i64` or `addr`.
- **Compiler-facing policy data follows the same rule.** Schema byte sizes,
  lengths, counts, and indices use `u64`. Signed carriers remain only for
  genuinely signed offsets/addends or an explicitly documented sentinel.
  Existing `i64` count fields in layout/calling policy records are migration
  debt, not a competing design.
- **No global carrier-width relation exists.** A `u64` count may exceed a
  32-bit address space, while a CHERI-class address representation may exceed
  the count carrier. An operation interpreting a particular count as address
  geometry proves that occurrence fits the selected address bound. Unrelated
  counts remain ordinary magnitudes and need no address interpretation.

A package may explicitly form a native-width integer if a future admitted
`UInt<const Bits>` carrier family and a canonical target-width observation make
that expression meaningful. It still cannot become Omega's `usize`: `.len`,
`Extent.length`, and general count APIs retain the target-independent count
carrier, while indexing has no privileged carrier at all. A package alias does
not rewrite those contracts or create implicit conversions among address,
count, and index roles.

## 3. Indexing is untyped — the obligation is everything (model "C")

`arr[i]` requires exactly `0 <= i < len`. Consequences:

- **Any integer type may index**, signed or unsigned, *iff* it discharges the
  obligation. An **unsigned** index discharges `0 <= i` for free (by type); a
  **signed** index must *prove* `0 <= i` like any obligation.
- This is why we **do not ban signed indices** — and it is the correct fix for the
  confirmed segfault. The crashing program did not have its `0 <= i` proof; the
  signedness was never the bug, the *missing proof* was. (Practically: the
  soundness fix requires the lower-bound proof only for *signed* indices; unsigned
  are exempt by type — which collapses the earlier 19-canary blast radius to a
  handful.)
- **The proof is erased before codegen** (ATS, since PLDI'98): the subscript is a
  bare offset. There is **no `Fin n` index value**; arithmetic (`i+1`, `i-1`)
  happens on a plain integer in the proof domain, where going out of range is a
  *failed obligation*, never a silent wrap. (This deliberately avoids Lean's
  `Fin` modular-wrap trap, which would corrupt decreasing-counter loops.)
- **Two proof sources, by location:**
  - **Stated `requires` at machine boundaries** — the load-bearing case. A
    growable collection's reader declares `requires self.length > index` against
    the *live* length (see `FixedVecI32x4::get`). Locally reviewable; the caller
    proves it.
  - **Flow inference inside a machine** (`incoming_guards`) for a loop counter —
    the smaller surface, and the one whose missing lower-bound check segfaulted.
    Acceptable because it is *derived* from a real guard (§7), not assumed.

## 4. `len` is a count; width is chosen by the proof

- `arr.len` is a **count**: non-negative, and the checker already carries its
  *bound* — exactly `N` for `[T; N]`, `<= capacity` for a growable vec. The bound
  is the content; the width is incidental.
- A proven index/count **lowers to the narrowest machine width that provably
  holds its bound** (Dafny's `newtype`-picks-narrowest pattern), `u64` as the
  fallback when the bound is unknown. A `[T; 4]` index can lower to a byte; a
  dynamic vec index stays `u64` unless a capacity bound is proven.
- The inclusive bound must fit. On a target whose exclusive address bound is
  `2^32`, `no_wrap(base, length)` proves only `length <= 2^32`; it does not prove
  that `length` fits `u32`. A separate fact such as `base > 0` yields
  `length <= 2^32 - 1` and licenses narrowing. The whole-space value at
  `base == 0, length == 2^32` remains representable by the source count but not
  by `u32`. Lowering must never turn `<= addr::Bound` into `< addr::Bound`.

## 5. Narrowing conversions are proof-gated (closed-world)

- A narrowing `wide -> narrow` (e.g. `len -> u32`) is allowed **iff it is PROVEN
  that the value fits** the narrow range. This is exactly Decision-17's **Exact**
  case (proven-fits ⇒ no wrap/saturate/trap).
- The **unproven default must reject**, or require a named conversion with an
  explicit `Wrapping | Saturating | Trapping` result policy. "Allow unless we can prove it is
  *too big*" is unsound — it is the open-world bug (§7): absence of a disproof is
  not a proof.
- A narrowing may *drop* the tighter refinement (a proven-`<= 4` value becomes a
  plain `u8`), so a downstream use may need to re-prove. Completeness, not
  soundness.

## 6. Named index types (the "E" idea) — free from domains

A nominal index type is a **domain over the index integer**, plus every accessor
that takes/grants it carrying that domain. `RoomIx` and `ItemIx` then refuse to
cross-index even though both are just proven-`< len` integers.

- The domain tag is **purely nominal** (who may index what); the **bound is still
  the `requires`/obligation** (§3). The two guarantees are orthogonal.
- **Growable storage makes it interesting.** Over a growable vec a named index
  becomes a *handle tied to the live `len`*: a grow (`push`) is monotone and keeps
  it valid; a shrink (`pop`/`clear`) can invalidate it. Two sound options:
  - **re-prove at use** (pure C): the tag is nominal, the `requires i < len` is
    checked against the current length, so a stale handle simply fails to prove —
    cheap, but you cannot hold an index across a shrink;
  - **generational index** (`{ value, era }`, an ordinary arena-handle shape): a
    shrink/realloc bumps the era; a stale handle fails the era check — holdable
    across mutation.

## 7. The governing soundness principle

The "open-world vs closed-world" framing is a proxy; the real line is:

> **Is the conclusion DERIVED from a positive fact, or ASSUMED from the absence of
> a disproof?** Sound is always the former; the bug is always the latter.

This sorts every case in this brief:

- **Domain membership / "is a valid `RoomIx`"** — there is nothing to derive it
  *from*; it is a granted axiom, so it must be **minted/established**, never
  inferred from "no predicate was violated." (Closed.)
- **Index bound `i < len`** — a *theorem* derived from the guard / the `requires`.
  Feels "open" (you did not state it at the use site) but it grounds out in a real
  fact. (Open, but provably.)
- **Narrowing `fits`** — a *theorem* derived from the value's bound. Same shape.

The confirmed segfault and the "empty domain ⟹ everyone is a member" trap are the
**same** open-world bug: the index check read "not proven `< 0`" as "fine"; a
naive domain check would read "not proven outside `D`" as "inside `D`." Absence of
a counterexample is not presence of the property.

## 8. Future rung — multidimensional shape-typing

`[Row, Col]` axis-typing (Dex / Chapel domains): each axis is its own bounded
index set, so the product is in-bounds **by construction** — sidestepping the
*nonlinear* `row*cols + col < R*C` proof that the interval engine is weak at. This
is the one "E" idea with real teeth (matrices/grids/tensors), because it *dodges*
a hard proof rather than solving it. Not now; a later rung built *on top of* the
model above (a multidim index still lowers to a proven integer).

## 9. Current implementation and later work

- The source corpus uses `u64` for general counts and `addr` for addresses;
  `usize` and `isize` reject. Signed-index lower bounds are checked.
- Compiler-facing `i64` fields that actually represent counts remain migration
  debt. A dedicated count carrier may replace source `u64` later without
  coupling count width to address width.
- **Not on the critical path:** relational domains (an `Index(c)` whose predicate
  references another field, `self < c.len`) are *not* needed — dynamic collections
  prove the bound with a `requires` against the live length, not a domain. Index
  domains over *fixed* arrays (constant bound) are the only place a bound-bearing
  index domain is even expressible, and even there the `requires` form suffices.

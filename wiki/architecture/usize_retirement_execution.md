# usize retirement — execution recipe

**Owner directive (2026-07-13):** "usize is not a fucking thing in Omega. We
do not have usize. We have addr, we have primitives. Conflating addresses &
size is a semantic disaster."

## Semantics of the rewrite

- `usize` (sizes, lengths, counts, indexes) → **u64**. Today `usize` IS
  u64 in everything but name: identical 8-byte layout (`sizing.rs` lowers
  `Usize | Isize | Addr` through one arm), unsigned literal bounds, u64
  domain classification. The rewrite is behavior-preserving.
- Values that genuinely hold ADDRESSES → **addr** (already a builtin,
  `PrimitiveType::Addr`). Rare in the corpus; judge per site during sweep.
- `isize` retires symmetrically → **i64** (1 corpus file). Flag to the
  owner in passing — the ruling names usize, but the conflation argument
  covers both.

## Inventory (measured 2026-07-15)

- **380 .omg files** (canaries + samples) use `usize`; 53 of them combine
  it in `terminates by` ranking witnesses. 1 file uses `isize`.
- **~15 compiler files / ~32 sites** mention `PrimitiveType::Usize`: the
  two parse maps (typed-trees + symbol-resolved-trees `types.rs`),
  layout/sizing, literals + arithmetic_domains classification, selection
  storage_places + writes/mutation, wire validation, ranges/ownership
  checks, interp evaluator, layout_plans, termination order.rs.
- **~8 language-guide chapters + wiki pages** mention it in prose/examples.

## Stages (each independently green-gated)

1. **DONE (2026-07-15): termination accepts u64 naturals.**
   `natural_measure_names_match` treats usize/u64 as one measure class
   (order.rs synthesizes "usize" for `.len` projections and subtraction
   measures — those keep matching u64-declared measures). Pinned by
   pass/proofs/runtime_decreases_u64_measure_exit (an existing usize
   decreases canary rewritten wholesale to u64).
2. **Corpus sweep:** `sed s/\busize\b/u64/g` (+ the one isize → i64) over
   canaries/ + samples/, full gate after. Sweep in FAMILY-SIZED batches
   (arithmetic, calls, proofs, ...) so a surprise localizes. Watch for:
   sites that should become `addr` instead (pointer-ish fields), and any
   canary whose HEADER narrates usize semantics (update prose too).
3. **Chapters/wiki sweep:** examples compile-checked where the guide
   harness covers them; prose updated to the addr/primitives story.
4. **Compiler rejection:** remove "usize"/"isize" from BOTH parse maps →
   unknown-type diagnostic (loud, names `u64`/`addr` as the fix); delete
   `PrimitiveType::Usize`/`Isize` variants and let the compiler surface
   every remaining match arm; purge diagnostic strings
   (arithmetic_domains.rs prints "usize"); retire the order.rs
   equivalence class (synthesize "u64" for len/subtract, compare
   directly).
5. **Fail canary:** types/usize_rejected pinning the rejection wording.

## Non-goals

- No behavior change anywhere: u64 ≡ usize today. Any battery delta during
  stage 2 is a real pre-existing bug surfacing — pin it, don't paper it.
- `nat` (the proof-surface name, order.rs:359) is untouched; whether
  measures someday spell `nat` instead of u64 is a separate design.

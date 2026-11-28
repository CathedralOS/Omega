# Handoff: PNG decoder native speed, SIMD unfilter

Goal: make omega-prism's PNG `decode_into` competitive with zlib + FFmpeg
(Windows benchmark last showed about 8.1×; the gate is `upper_95_ratio <= 1.05`).

## Done (merged into cathedralos/omega main)

- #6395: native fix, an inlined helper's `PackedAddU8x16` was dropped by the
  Stage 05 selection-vector flush elision (`slot_read_later`).
- #6400: native lowering of `view as @[T; N]` / `@row[a..b] as @[u8; N]`
  (shared fixed-array reference into a view).

## In flight

- **PR #6644**, branch `claude/project-thread-xijdvs`: lets
  `@mutable row[a..b] as @mutable [u8; N]` lend a mutable fixed-array
  reference on Psi and native. Rebased on main d5252ba6f, fmt and
  check-rules clean, assigned to Smotto. Waiting on the `admission` check;
  Jarod authorized squash-merging once `admission` passes.
  - Still refused with errors: whole-view `row as @mutable [u8; N]`,
    `@mutable row[..] as ...`.
  - Known failures that also fail on main (not ours):
    `stored_sum_rows_select_the_same_instructions_as_main`,
    `a_new_borrow_does_not_choose_an_arbitrary_incoming_parent`,
    `a_foreign_call_receives_the_established_bytes_and_their_extent`,
    `a_call_replaces_only_the_selected_byte_view`.

- **omega-prism SIMD Up filter** (omega-prism is not a git repo; files are
  here in `handoff/png-simd-unfilter/`):
  - `unfilter-simd-up.diff`: `up_row` in `packages/png/unfilter.omg` now does
    16 bytes per step with `u8x16` load / wrapping_add / store, then a scalar
    tail. `unfilter.omg` is the full patched file.
  - `png-build.diff`: adds the system library (`{system}/simd`) as a
    dependency of `packages/png`. **The path is absolute to the cloud
    worktree; point it at your Omega checkout's `source/library/system`.**
  - Result on the cloud Linux box (9 seeds, `runpng.sh`): 9,821 µs → 9,609 µs
    per image, ratio 18.85× → 18.52× (about 2%). Output matched the reference.
    The cloud box is much slower than the Ryzen, so compare ratios only on one
    machine.

## Next steps

1. Merge #6644 when `admission` is green, then restart the branch from main.
2. Look at the native code for `put16` (disassemble with
   `objdump -D -b binary -m i386:x86-64 -M intel --adjust-vma=0x400000`):
   check the `u8x16` load/add/store become `movdqu`/`paddb`, and that the
   per-step bounds checks (9 compares) and calls don't dominate. The 2% win
   suggests overhead, not the add, is the cost.
3. Up is only one filter. Sub, Avg and Paeth (FFmpeg uses SSSE3 Paeth) and
   inflate (about 1.6 ms of 3.3 ms on Windows) are the bigger levers.
4. Mirror the change into `unfilter_in_place.omg` (Adam7) once it pays off.
5. Rerun the full Windows benchmark (`benchmarks/png_decode/benchmark.py
   --omega <omega.exe>`), and update its README table.

## Commands

- Build compiler: `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega < /dev/null`
- Tests: `mbx test -p omega --no-fail-fast --test fixed_array_slice_casts --test packed_integer`
- Format: `mbx xtask fmt --check --base origin/main` (never `mbx xtask fmt <files>`)
- PR rules: `mbx xtask check-rules --base origin/main --title T --body-file F`
  (Gate and Proof lines need `<x> | <y>` form)
- Linux decode bench: `omega --target linux_x86_64 --build-dir D
  benchmarks/png_decode/decode/build.omg`, copy `libpngdecodebench.so` into
  D (built by `build-reference.sh`), then `sh runpng.sh D 9`.

## Gotchas

- `omega run --engine` no longer exists on main; use `omega run`.
- Contract arithmetic like `si + 16u64 <= raw.length` fails ("not proved
  total" / proof budget). Compute end offsets with Wrapping and guard with
  `si < se && se <= len && se - si == 16u64`.
- A cast written inline as a call argument is refused ("call borrow of a
  value that is not a place"); bind it with `let` first.
- Only Jarod and Zac edit `wiki/spec/`; put proposed spec text in
  OWNER_QUESTIONS.md.

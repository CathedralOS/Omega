# NEW-BUILD-DIR-SNAPSHOT-ROOT-FENCE-DEDUP — re-verification ledger

Board row: `TASKS.md` `**NEW-BUILD-DIR-SNAPSHOT-ROOT-FENCE-DEDUP.**` (:10370) —
mined candidate, slice landed (worked unclaimed pre-marker). Re-verified on
this host (linux x86-64) at `7d03d489e3` by Zergling-126 (claim `c869fa72`,
draft path only).

## Landed dedup — confirmed

`build-evaluation/src/evidence/filesystem_scope.rs` carries the single
spelling-independent helper: `overlap_key` (`:88`) normalizes paths before
comparison and `reject_overlapping_roots` (`:145`) is the one diagnostic
constructor. All three former hand-rolled sites route through it:

- `bind_captured_source_input` — snapshot↔build-dir and snapshot↔source-root
  rejections at `:324` and `:330`
- `ensure_write_roots` — build-dir↔named-input-snapshot rejection at `:554`
  (with the `overlap_key`-based `starts_with`/`!=` checks at `:542`, `:565`,
  `:644`)

## Scoped check — green

`cargo nextest run -p build-evaluation --lib` — **95/95 PASS** on linux
x86-64 (the crate's suite has grown three tests since the row's recorded
92/92). No residual slice exists under this name; the correct in-fence
artifact is this ledger.

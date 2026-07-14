# Windows dir-walk bodies — rung 3b, logic-complete, BACKEND-BLOCKED

`filesystem_impl.win_bodies.reference.txt` is the **complete, correct** windows
implementation of the fs portable-contract directory-walk family (rung 3b),
authored 2026-07-18 over the find-enumeration seam trio (rung 3a, landed in
`e155b72dd`). It is a `.txt`, not an imported `.omg`, because it does not
compile to a **running** program on this host yet — a backend bug blocks it
(below). The live `../filesystem_impl.omg` stays the posix-copy placeholder
(note_vault the one samples red) until the backend fix lands; then these bodies
drop straight in.

## What these bodies are

The windows directory paradigm behind the portable contract: no dirfd exists,
so listing is **handle enumeration** over a `dir/*` pattern (find_first /
find_next / find_close), and removal inside the walk goes by **joined full
path** (remove_name / remove_dir_name). The posix dirfd stack becomes a
**path-prefix stack** (`w_path` holds the current directory's full path,
`w_len_stack` holds each ancestor's path length; descend = append `/name` +
push, ascend = pop + truncate). Scratch fields (`w_*`) go on portable
`data Filesystem` (unused on posix, ZII-harmless) — see the field block these
bodies assume in `omega/language/std/filesystem.omg` (also shelved with this
revert; the field names are referenced throughout the `.txt`).

## Verification status (all reached this session)

- **Interpreter: GREEN on everything.** note_vault exits 14; every probe exits
  as designed. The logic is correct.
- **Native single dir-walk: GREEN.** A fresh `remove_dir_all` on a one-file dir
  and on a nested tree both exit 70; `read_dir_count` returns the right count.
- **Native REPEATED dir-walk: the blocker.** A *second* dir-walk wrapper call in
  the same process (scan-then-drain, drain-then-drain, or scan-then-scan) fails:
  the second call reads its slice parameter's length as **0**. Root-caused to a
  backend value-machine call-argument bug — the `&[u8] in Path` descriptor's
  length word is not correctly materialized on the second invocation of a
  wrapper machine (single calls are fine). This is BELOW the Omega layer; these
  bodies do not trigger it, they merely expose it.

## The path to green (banked in TASKS.md as rung 3b)

1. Fix the backend slice-parameter-descriptor bug (min repro pinned as
   `canaries/pending/filesystem/repeated_slice_param_walk_divergence`).
2. Restore these bodies into `../filesystem_impl.omg`, restore the `w_*` scratch
   field block into `filesystem.omg`, and re-apply the single-target-internal
   relaxation in `pipeline/target_machines.rs` (the windows walk has helper
   machines that exist on no posix target — the shared-name loud edge must fire
   only for names implemented by **two or more** targets; single-target names
   are paradigm internals).
3. note_vault goes compile-fail → green in one step; samples gate fully green.

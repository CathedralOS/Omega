# SQUALR-DEBUG-ASSERTIONS — scope verification (2026-09-20, `74537d6125`)

Bare mined stub at `TASKS.md:~11335` (no `**ITEM.**` marker — claimed
freeform). Re-mine of **SQUALR-DEBUG-ASSERTION-PARITY** (:11314, scope
verified at `e8bbe9fcc0` against upstream `568aa7589b68`), which mines
the "Rust debug-only assertions" gap in the GEOMETRY-PARITY residual
list.

## Verified state at `74537d6125c9a5e665c5f985cf5cacc2b12d7a5c`

- `samples/apps/squalr` is a git submodule pinned `5b0307c3` (not checked
  out in this worktree); the sibling row's audit carries the content:
  upstream `debug_assert!` sites live in
  `structures/scanning/filters/snapshot_region_filter.rs` (aligned base,
  size >= value width — the Omega port carries a comment at the same
  site), `structures/structs/valued_struct{,_field}.rs`, and the
  unported scanning/targets-native surfaces.
- Parity needs no new machinery: `configuration.md` excludes a
  debug/release mode and assertion primitive — authored `crash` checks
  or `requires` clauses on the ported machines are the vehicle.

## Fences (live at verification)

| Surface | Claim | Expiry |
| --- | --- | --- |
| `samples/apps/squalr` wholesale | GEOMETRY-ALIGNMENT-REGIONS — Zergling-112 | 01:18Z |
| `samples/apps/squalr` wholesale | SQUALR-WINDOWS-GEOMETRY-VALIDATION — dev-88738 / z175 | 05:49Z / 06:28Z |
| item-level, same lane | SQUALR-NAMED-TRAIT-OPERATORS (00:58Z), SQUALR-SEED-PARITY (02:08Z), SQUALR-GEOMETRY-PARITY-RESIDUE (03:32Z), SQUALR-SEED-REGION-OPERATIONS (07:44Z) | — |

Every ported counterpart sits inside the dir fence — the leg is
currently unworkable, same disposition as the sibling parity row:
coordinate with GEOMETRY-PARITY's owner lane.

Verdict: verified re-mine — coordinator should fold into
SQUALR-DEBUG-ASSERTION-PARITY / the SQUALR-GEOMETRY-PARITY cluster.
Record only; no code change.

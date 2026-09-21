# LOOKUP-MAP-MEASUREMENT-AUDIT — re-verification ledger

**Status:** resolved on main; re-verified at `baad84f97fb` (Linux x86-64, this host) under claim `2df0412c` (LOOKUP-MAP-MEASUREMENT-AUDIT, expires 2026-09-21T08:36Z).

## What the item is

Mined-candidate row in TASKS.md (`**LOOKUP-MAP-MEASUREMENT-AUDIT.**`, ~TASKS.md:8810), resolved as a re-mine of the landed sibling SCOPED-LOOKUP-MAP-AUDIT surface: the "measured reason" audit exists as the repeatable architecture gate `tests/architecture/scoped_lookup_maps.rs` enforcing the `omega-rust/pipeline.md` rule ("scoped symbol-tree lookup is the baseline; extra lookup maps require a measured reason") by census — every production `HashMap`/`BTreeMap` keyed by an authored-spelling token must appear in `JUSTIFIED_LOOKUP_MAP_FILES` with its recorded key domain, and the reverse staleness check fails cataloged files that no longer declare such a map. The one-shot census it encodes lives at `wiki/drafts/lookup_map_justification.md` (run at `c2ccb2a202`).

## Verification on this host

`cargo nextest run -p omega-architecture-test --test scoped_lookup_maps` at `baad84f97fb`: **2 tests run, 2 passed** — `every_name_keyed_lookup_map_file_is_cataloged` and `every_cataloged_file_still_observes_a_name_keyed_map` — matching the row's recorded green.

## Residuals

None. The gate is self-maintaining: a new name-keyed map without a recorded justification fails the build, so no independent slice remains on this surface.

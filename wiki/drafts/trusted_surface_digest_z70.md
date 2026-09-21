# TRUSTED-SURFACE-DIGEST-RE-RECORD — duty pass (2026-09-21, `53817f8759`, linux x86-64)

Swarm wave w9 / zergling-z70. Assigned name `TRUSTED-SURFACE-DIGEST-RE-RECORD`
is the alias of the standing-duty row `TRUSTED-SURFACE-DIGEST-RE-RECORDING`
(TASKS.md:~15966) — prior duty passes record the same assigned spelling.
Claim ticket `9204fa50` on `trusted_surface/sites.rs` +
`tools/trusted_surface_digests.py` + this draft (TASKS.md multiply-fenced —
nine live claims — so the pass note rides here).

## Result

- `python3 tools/trusted_surface_digests.py` → **"all recorded digests
  match the working tree"** — the ledger is current.
- `cargo nextest run -p terminal-verifier -E 'test(~trusted_surface)'` →
  **15/15 PASS** including `recorded_digests_match_the_working_tree`.

No drift since the `a5d958d724f1` pass, so no entry needed revalidation or
re-recording. The two open non-blocking observations from `b260ea749e`
(hook-target `entry_claims`/`content_entry_claims` pins →
`formation:machine-validation`; boundary-result-qualification "established
by" authorization under `formation:structural-qualification-rosters`) remain
open follow-ups, unchanged.

# RUST-RELEASE-RECORD — z70 re-verification

**Revision:** `72fc66d6c3` (origin/main tip, linux x86_64 host). Row resolved at
`479ceb0e68` (ancestor of this tip — `git merge-base --is-ancestor` ✓), covered
on both faces. Re-verification only confirms the recorded state; no new slice
was opened.

## Record face

- `tools/release/` still holds only `README.md` + `release_record.py` (landed
  `210ffe3c93`); **`tools/release/records/` does not exist at tip** — zero
  committed `omega-release-record/1` JSON. The contract's
  eight-gates-green-on-one-commit closure stays correctly open (RC-DIAGNOSTICS
  lane still mid-repair).
- Fence map refreshed: the production surface is double-fenced right now —
  `tools/release/{release_record.py,records,README.md}` +
  `tools/tests/test_release_record.py` under RC-HOST-RUNNER-LANES (z198,
  ~07:56Z), and `records` itself under RC-RELEASE-RECORD (z120, ~19:49Z).
  Even if a fresh record were measurable, committing it is another lane's
  lease.

## Producer face

- The TASKS_BOOTSTRAP.md:76 gate stands verbatim: "Full self-hosting remains
  dependent on settled exercised Omega behavior, the Rust product completion
  plan, complete D, and `OMEGA-PRODUCT-COMPILER-SOURCE`" — all open.
- Drift note: the sibling row's CONTRACT.md quote ("Rust remains a comparator,
  not bootstrap authority") no longer appears verbatim — CONTRACT.md now
  expresses the same policy as "These Alpha compilation obligations belong to
  D and C, not the Rust reference compiler" (:30) and Rust agreement as
  "diagnostic evidence, not compiler-correctness proofs" (:95). Policy
  content preserved; the citation aged.

## Verdict

Resolved row stands. The comparator-retirement decision stays gated on the
four open dependencies; the record face's open residual lives with the
release-record lanes, all fenced. No independent slice.

> Field note (f5eca6b2b0bd..5d9afb85f822 review): ledger-only re-verification of a resolved row; fold into RUST-RELEASE-RECORD and delete rather than re-stamping.

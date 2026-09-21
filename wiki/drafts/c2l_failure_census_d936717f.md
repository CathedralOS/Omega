# C2L failure census — d936717fd2

Fresh member-by-member `checked-trees-to-lowered-psi` census, supersedes the
member counts recorded at `23392bc467` in `known_baseline_failures.md` (which
remains the family-ownership ledger).

Run: `cargo nextest run -p checked-trees-to-lowered-psi --no-fail-fast`
on Linux x86-64, worktree at `d936717fd2` (origin/main at run start).

## Totals

| pin | members | passed | failed |
| --- | ---: | ---: | ---: |
| 23392bc467 | 2183 | 2127 | 56 |
| d936717fd2 | 2199 | 2175 | 24 |

## Failing members (24)

| count | area | family (known_baseline_failures.md ledger) |
| ---: | --- | --- |
| 6 | `provider_attachment_source::*` | missing transitive machine plans |
| 9 | `unit_state_graph::provider_attachments::*` | missing transitive machine plans |
| 1 | `guarded_scalar_returns_source::stored_returned_cases_support_borrowed_refined_getters` | missing transitive machine plans |
| 3 | `unit_plan_omissions::*` | missing entry claims on omitted local constructions ("no admitted body") |
| 3 | `owned_record_return_source::*` | scalar-return custody |
| 1 | `unit_state_graph::bindings::unranked_self_bindings_validate_without_claiming_finite_fuel` | ranked safe-point / fixed-fuel bound (expects `FixedFuelError::ControlCycle`) |
| 1 | `nominal_affine_source::integer_comparison::mixed_nominal_integer_comparison_converges_before_one_shared_cleanup_return` | proof-search blowup — SIGTERM'd at 1480.871s; same externally-killed behaviour recorded at ~1390s previously |

## Delta vs the 23392bc467 census

- **Bare `Service<R>` spellings closed (30 → 0):** the 30 `tests::*` members
  rejected by the `Service<R>` carrier fixture at `src/tests.rs:82`
  (ENTRY-CONTENT-ROOTS) now pass. Only the 3 `unit_plan_omissions` members of
  the former 33-member group remain, now failing on `missing a checked
  transitive machine plan` / `has no admitted body (local construction stopped
  at signature)` rather than the carrier-fixture rejection.
- **Ranked safe-point bounds 2 → 1:** one of the two recorded members now
  passes.
- **Scalar-return custody 4 → 4:** same total; the member set is
  `owned_record_return_source::*` ×3 + the guarded-scalar member above.
- **Proof-search blowup:** reproduced — still does not converge; killed at
  1480.871s. Remains the PROOF-SEARCH-MEASUREMENT member.
- Suite grew 2183 → 2199 members (+16, all passing).

## Verdict

**The unattributed tail is still empty.** Every one of the 24 failing members
lands inside an already-owned family in `known_baseline_failures.md`; no new
failure family appeared between `23392bc467` and `d936717fd2`. One family
(ENTRY-CONTENT-ROOTS carrier-fixture rejection) shrank by 30 members; all
sibling shrinkage is partial and inside owned families.

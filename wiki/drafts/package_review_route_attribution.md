# PACKAGE-REVIEW-ROUTE-ATTRIBUTION — landed-state record

Claimed slice: `wiki/drafts/package_review_route_attribution.md` (record),
verified at base `be496d9a9080` on linux x86-64.

## Landed mechanism (on main)

`UngrantedRestrictedBuildRequest` (packages/manager
`review/restricted_build_grants.rs`) carries
`request_path: Option<DependencyRequestPath>` — the occurrence's shortest
root-to-package route from `resolution/graph/reconcile/dependency_paths.rs`'s
BFS (requester/purpose/alias/target steps). Its `Display` renders
`    path <root-hex> -> "alias" <target-hex> …` in the decision document's
form, with `path none` when the requester is root-local. `request_path()`
exposes it beside package identity, purpose, and request meaning —
satisfying acceptance.md's restricted-build requirement that each request
report its originating package *and dependency path*.

All four join call-sites supply the source closure:

- `operations/check_project.rs:153`,
- `operations/compile_project.rs:197`,
- `operations/check_locked_sources.rs:156` — each calls
  `ungranted_restricted_build_requests(accepted, reviews, &source_closure)`;
- the in-compile checkpoint in `review/candidate/compilation/package_pass.rs`
  calls `RestrictedBuildCheckpoint::ungranted_requests` with
  `closure.dependency_path(review.key())`, so an ungranted occurrence
  rejects mid-compile with its route attached before a generated-source
  bundle can hand off.

## Re-verified legs (this pass)

`cargo nextest run -p package-manager -E 'test(restricted_build) or
test(supplied_host_scope) or test(ungranted)'`: 3/3 pass

- `review::candidate::compilation::tests::restricted_build_grants
  ::supplied_host_scope_requires_exact_retained_request_and_occurrence`
  (48.6 s — the test carrying the new route assertions),
- `suite locked_source_checking::restricted_build_grants
  ::confined_generator_checks_from_a_lock_without_restricted_host_decisions`
  (78.0 s),
- `…::armed_checkpoint_gates_restricted_requests_inside_the_pass`
  (99.1 s — exercises the package_pass in-compile checkpoint).

## Residual

None for this item's reading — the route is threaded end-to-end and asserted.
The row's historical fence list is stale: BUILD-ADMISSION-CHECKPOINT,
TWO-AXIS-TERMINAL-AUTHORITY-REVIEW, and PACKAGE-LOCK-SOURCE-IDENTITY are all
off the live claim map at verification time. The sibling item
PACKAGE-REVIEW-ROUTE-COST-ATTRIBUTION is a separate (already-resolved)
deliverable about pass/phase cost, not this route thread.

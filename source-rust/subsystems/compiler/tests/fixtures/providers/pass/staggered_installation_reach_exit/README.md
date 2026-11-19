# Staggered installation reach

The selected A row reaches B and C; B resolves to X immediately, while C
waits for D to resolve to Y. A must retain X when it later acquires Y.
All abstract rows deliberately allow X + Y: a selected row is its concrete
installed reach, not the preselection upper bound.

`main` observes both writes (11 and 22) through Terminal Psi and native
execution. `driver/tests/package_review.rs` also checks the selected rows and
requires successful package-review projection. Execution alone cannot expose
the missing metadata: the pre-repair program ran but its fresh package audit
was unavailable. The matching fail fixture removes Y from A's bound and must
reject the excess service.

This retains the installation-reach repair within the entry-point/Build
orchestration cleanup, not acceptance of the whole provider capability.

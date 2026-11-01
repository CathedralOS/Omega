# linear_submit_outcome

The blocking-executor admission shape: `[linear]` `Submission`/`Ticket` claims route through a `SubmitOutcome` sum — `Accepted(ticket)` vends the claim while `Rejected(submission, reason)` returns the moved submission whole. Path-aligned conserved-claim joins: each arm consumes or transfers the same obligations.

## Learnings

- One-member boundary-trait requirements (`boundary trait SubmissionRelease { machine release(submission: Submission) reaches; }`) are callable surfaces — `SubmissionRelease::release(submission)` is the authorized terminal disposition for a hook-less linear type.
- Sum payloads destructure by member projection: `outcome.submission` moves the `Rejected` payload out (path-sensitive — valid only when that case is active).
- `transition <subject> { arm -> state(params) }` state bodies end in a bare tail expression (`SubmitOutcome::Accepted { ticket: claim }`), no `transition { _ -> }` needed.
- A linear argument must be consumed or transferred on EVERY arm — route the non-transfer arm through a release/requirement call, not a payload.
- `drop` needs `use omega::language::core::drop` and works on hook-less linear sums.

Compile-checked only: run a check target, not native.

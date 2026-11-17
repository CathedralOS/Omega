# Checked byte observations

[PR #3746](https://github.com/CathedralOS/Omega/pull/3746) implements the byte
observations used by Room A1. Root owns the remaining multiplication-table work
in the same worktree, `worker-3`; the prior handoff has been accepted.

## Completed customer

`samples/cli/probes/bounded_concat` copies `"Room "` at offset zero and the live
`"A1"` label at offset five. Label capacity four/live length two and text capacity
sixteen/live length seven retain the `"Room A1"`/length-seven success check.
Explicit Core imports and Ascii/Wrapping casts preserve those bytes and bounds.

At `7a524376172b7cac2c3af55a3971ea4cb7eab037`, the optimized compiler
`e4c707d87d8acb2cc85b1c9ea39d9d0b721179546ae9d950db37af23cdc2c3a7`
runs Room with native exit 0 and Psi exit 0 in 16.83 seconds on macOS arm64.
The normal package update refreshed its compiler identity without a source change.
Independent inspection of the saved artifact also exits 0. Receipts are in
`build/byte-observers/saved-room-after-refresh/`; the artifact SHA256 is
`72d0856485b1070a9159ed04594bfebff68fbae20639c35ad72ea90028c9b6f7`.

Byte predicates retain their exact declaration and full call contract through
checking and saved artifacts. ASCII implies UTF-8; UTF-8 alone does not establish
ASCII. A partial slice must prove predicates that its cut does not preserve.
An annotation cannot establish its own source requirement, and mutation still
owes the lender's restoration contract.

Twelve affected crates pass all-target Clippy with warnings denied. The focused
byte controls retain one known diagnostic failure: the malformed restoration
program rejects, but claim verification replaces its precise restoration error
with a generic unavailable-storage error. `OWNER_CODE_CONCERNS.md` records that
path; the strict assertion remains unchanged. Three final proof-search controls
pass, including kernel-checked certificates and rejection without headroom.

## Remaining customer

`samples/cli/basics/multiplication_table` still needs all nine ordered 28-byte
rows from native and Psi, followed by independent artifact inspection. Its Std
dependency, twenty-eight spaces, writes, arithmetic, bounds and output oracle
remain. The recorded source migration makes Utf8/Wrapping qualifications and
explicit removal of Wrapping at plain-i32 helper calls agree with current rules.

The same `7a524376172` producer reaches repeated bound and case search before
either engine runs. Its latest bounded attempt was stopped after about four
minutes. The sampled peak fell from 18.5 GiB to 2.6 GiB after case proofs became
lazy; checking still did not complete, so this establishes no end-to-end speedup
or table acceptance. Receipts and profiles remain in
`build/byte-observers/byte-after-lazy-cases/`.

Continue from that measured proof-search failure. Keep the original table and
its acceptance, investigate the shared cause, and retain every feasible proof
branch and formation obligation. No task-board requirement closes with the Room
milestone, and no language-design question blocks this work.

Governing contracts: [byte views](../../spec/terminal-psi/byte_views.md),
[domains](../../spec/language/domains.md),
[invariant windows](../../spec/language/dependent_values.md#invariant-windows),
and [structural access](../../spec/terminal-psi/structural_access.md).

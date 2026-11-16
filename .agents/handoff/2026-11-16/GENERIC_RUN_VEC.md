# Generic Run/Vec: continuation

Root accepted PR #3764 and its unfinished Run/Vec outcome on 2026-11-16. Work continues in `worker-2`. The draft is unvalidated; do not merge it. No TASKS acceptance is closed.

The worktree retains fixed base `d3deb345653b9812607155ff4b51e17db35ca520`. The checker now derives mandatory claims from declared multiplicity and owned fields, rather than trusting the supplied claim flag. Both saved-artifact validation and direct source checking use that rule. Claim scratch storage follows the actual declarations, so a large sparse type ID does not determine allocation size.

## Original outcome

Check `tests/fixtures/pass/collections/run_split_merge/main.omg` with its exact policy/element applications, custody contracts and call bounds; reject the missing-cut-bound control. Then deliver the integrated ordinary generic library Vec required by `wiki/spec/resources/allocation.md#dense-collections-over-allocated-backing`: actual linear elements, zero-sized cleanup obligations, contained loans, fallible growth, rejected initialization, order, exactly-once disposition and backing return through native and Psi execution. The original Run fixture declares functions; checking it does not establish executable collection support.

## Current PR scope

The earlier branch distinguishes a type's declared multiplicity from its intrinsic logical claim throughout source metadata, canonical Psi, native consumers and the interpreter. On top of base `d1a75d1399041e80c3c663b020602a56af84dd21`, this checkpoint adds claim names and transport using the existing ownership walk, a Source05 metadata publisher, independent Source06 replay at ordinary call cuts, and one pure exact-transfer matcher reused by native ownership checking. The authored tests cover two independently linear siblings through an ordinary call and malformed missing, duplicate, foreign, swapped-path and wrong-argument transfer rows.

The Source05 executable-linear-data refusal is removed only in this unvalidated draft together with the new publisher. This does not establish that all newly reachable ownership cases are safe. Carry and other unsupported ownership guards remain. The broader const policy-value port, retained Run-element metadata, lifecycle publication, Content conservation and generic Vec work are excluded from this checkpoint.

## Preserved complete WIP

All additional authored work is pushed at `c53b684a5caf7e11b040fc26bd5f604a6d63194a` on `work/generic-run-vec-unvalidated-20261116`:

https://github.com/CathedralOS/Omega/tree/c53b684a5caf7e11b040fc26bd5f604a6d63194a

That branch includes all lifecycle/typed-static policy/retained-sequence/content source, ordinary generic Vec and its positive/negative executable customer, and an unconnected interpreter payload-index prototype. It also preserves the uncompiled numeric Content reduction as `.agents/handoff/2026-11-16/content_numeric.rs.wip`. Its own WIP note states the limitations. The index has a known successful-operation metadata reclamation gap; no populated Run runtime integration was completed. Do not interpret source authorship as execution or acceptance.

An independent fsynced snapshot and manifest remain at `/workspace/shared/generic-vec-preservation-2026-11-16`, including all then-untracked source and the ignored numeric draft. Local build receipts remain in `build/generic-vec-consumer`; preserve them and the warm cache.

## Validation, with source limits

- Earlier source `188ace9fdec52e1abedf6eb96d4325483ec7daca`: 28 ownership planner controls passed.
- The migration checkpoint at `6f9475e3b2fad0049dbadb4354db47a7f793d057`: a 40-crate all-target Clippy keep-going run found five mechanical test-target errors. Their focused five-crate retry passed after repairs; the other 35 results are source-equivalent historical evidence. This is not a fresh all-40 pass on this checkpoint.
- The combined WIP source, before this subset was extracted: terminal-semantics, terminal-wire and language-core all-target Clippy passed with an unchanged source snapshot. The semantic test executable SHA256 was `0a663498a4327cfc99d1278a1d32956f91d8c9d91008dcda114d29d58c648fb5`; three exact transfer controls and one borrowed-successor claim control passed. These are historical WIP-source results, not a build of this draft checkpoint.
- Two of three independent Run-authentication tests passed on that combined WIP. The third panicked because a mutation still expected Type(P) after the const policy-value migration. The fixture was corrected and preserved on the WIP branch, but not rerun. That receiver remains unwired in this PR's subset.
- Fresh local patch `6018d15b30c8367ec1b5cc1468ca8413ab20170b1da36a326507a4d37c989ddf` over `9e7b729ccfb9965c05205e8f472dfc1c116f53e3`: the ordinary sibling-call test and its five tamper cases pass; removing the linear-leaf flag and every dependent claim row now rejects. Both tests passed in 0.020 seconds after a two-minute optimized build. The next semantics test target stopped at three accesses to the replaced private map in an existing test; those accesses are repaired, with the remaining checks pending. Inputs and output are retained in `/tmp/omega-3764-erasure-bounded.{json,patch,log}`.
- The unchanged original Run positive and missing-cut twin both stop before custody checking because a transparent address predicate is read without its ordinary address interpretation. The twin therefore does not yet prove rejection of the missing bound. The address reader repair is underway. No populated Run, generic Vec, or native/Psi pass is claimed.

## Required next actions before a merge

1. Finish the nearby ownership checks: sparse type IDs, complete linear-field obligations, and interpreter constructor bindings. Run the authored native/Psi sibling-value program and its duplicate-transfer reject twin. The two source checker controls above do not replace executable evidence.
2. Ensure the general claim publisher does not accidentally admit populated Run while its hidden T custody and loans are absent. The original executable-linear-data guard is removed by this draft; preserve or add the exact Run floor until real retained-cell obligations exist. The original Run program's present refusal must be measured again, not inferred from an obsolete guard.
3. Check the affected native ownership path and resolve remaining findings. Run scoped lint and normal admission before merging. Keep historical results distinct from checks on the submitted source.
4. Continue the full allocation acceptance from its actual next failure using the preserved WIP as material, not as trusted implementation. No new language-design question is implied by these engineering gaps.

## Continuation state

Root owns continuation under claim `18e89b4f6d3691480000000018e98a76`. The original contributors stopped; the takeover team shares one local build executor. Work on the address-predicate failure continues alongside the remaining ownership checks. Neither a checkpoint nor this note closes Run/Vec acceptance.

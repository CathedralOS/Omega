# Coordinator handoff, 2026-11-12 night (US Pacific) / 2026-11-13 UTC

Supersedes the 2026-11-11 wind-down note. All paths below are in this scratchpad folder unless
they start with a repo path.

## Rules now in force (read AGENTS.md on main first; it changed a lot)
- No merge queue, no `xtask land`. Squash-merge your own PR when the one-minute `admission`
  check passes. Workers land their own work; the coordinator does not gate it.
- Nobody edits tests/omega/corpus_*.txt or runs `corpus --record`; post-merge writes the ledger.
- No tiny PRs: fixture-only, ledger-only, one-line board edits ride in a real PR.
- OWNER RULING 2026-11-12: the old checker (Psi 05, validation, Psi 06, checked-trees, facts,
  representation-*, checked-interpreter; 875,196 Rust lines) is being removed NOW. Old-stage
  crates are frozen except for deletion. The lowered-form checker's verdict is the only
  verdict (#2792). Recorded in TASKS.md LOWERED-FORM-CHECKER, AGENTS.md, the design record.
- Owner: do not measure. No census, sweeps, stage-coverage or long `lowered_route_*` suites.
  Take the next refusal from the existing table (open PR #2745), build it, prove it with unit
  tests and two or three named probes, land.
- OWNER_CODE_CONCERNS.md (new, #2786/#2787): agents add a five-line entry when they see
  structural rot, in a PR they are landing anyway, and keep working.
- Worker brief in force: worker_common_v2.md. CPU cap unchanged (Idle priority, 18 CPUs).
- The build queue is the bottleneck: more than 3-4 local workers mostly queue.

## Landed this session (2026-11-12/13)
#2785 private helper result summaries on the lowered route
#2791 machines declared under a data type's name on the lowered route
#2797 a generic body calling another generic machine on the lowered route
#2792 THE CUT: lowered-form verdict is the only verdict; unsupported/incomplete is refused
#2794 dropped test-only dependencies on the old checker (260 lines)
#2801 moved terminal-installation out of the validation directory
#2802 calling policy evaluated for a natively bound generic-boundary application
#2786, #2787 OWNER_CODE_CONCERNS.md; #2795 board clause removal
Closed: #2534 (superseded by NH21B's #2465).

## THE BLOCKER (why nothing more can be deleted)
Open draft #2796 deletes Psi 06 (130,032 lines), build-verified, HELD. Merging it stops every
compilation, `omega --check` included, because each project's `build.omg` is checked by Psi 05,
lowered by Psi 06 and run before the product check. Psi 05 and everything behind it are
blocked the same way. A Build program must be checked and published by the lowered route
first. Measured needs, in dependency order (w3_build_route_handoff.md, with dumps):
1. data with lifetime parameters (`Build<'storage>`): first refusal
2. non-copy data behind `&mut`: another session's open draft #2765 rewrites those files
3. byte views (`&[u8]` formal/field, string literal argument, fixed u8 array field)
4. service reach and `BoundaryCall` on the route (none exists today)
5. invocation-scoped (non-portable) publication of an accepted closure
6. Build companion tables captured from route output instead of Psi 06
7. wiring in `PreparedBuildExecution::prepare`
Also: Build admission uses `validation::execution_projection::project_build_program` and
`BuildProviderPlans::capture(&checked, ..)`. Removal sequence after that: w14_removal_handoff.md.
`checked-interpreter` goes last (it executes Build programs; TASKS UNIVERSAL-INTERPRETER-CLOSURE).
Owner was offered merging #2796 early anyway and has not asked for it.

## In flight at wind-down (each told to land-if-verified or push, write a note, stop)
- worker-15: land branch work/compiler-route-application-artifact (application built from the
  route's published module + evidence seals; witnessed native exit 0 / Psi exit 0 before a
  merge with main; merged tree unverified). Note: w15_handoff_1113.md; PR body w15r\pr_body.md.
- worker-12: item 0 = soundness hole (generic machine with no `ensures` calling itself is
  accepted with no termination judgment); then traits/conformances on the route (1,344 corpus
  programs declare a trait and are refused outright). Note: w12_handoff_1113.md.
- worker-10: service reach on the route (top refusal, 2,676 programs; also Build item 4).
  Note: w10_service_reach_handoff.md (w10_handoff_1113.md is the earlier calls-lane note).
- worker-3: byte views, stopped minutes after starting. Note: w3_byte_views_handoff.md.
Check `gh pr list --author @me` and each note for what actually landed after this was written.

## Known hazards
- With the fall-back gone, fail fixtures pinned to the old checker's wording go red (e.g.
  `fail/calls/free_machine_named_transition_rejected`); a route-diagnostics PR should make
  the route's call-requirement message name the callee's clause.
- `pass/generics/authored_specialized_call_identity_exit` was fully accepted at 380d1c537 and
  is undecided (`call late-bound selection`) at 55bb79cf6; cause on main not traced.
- A route refusal exits as an ordinary error, not the spec's `Incomplete` outcome.
- `compiler::access_view_calls` tests time out on this host; two failures never compared with
  main; one fails with `TargetLayoutSelectionMissing(LayoutApplicationId(1))`.
- Other swarms collide: tonight two workers lost their work to someone else landing the same
  thing (#2770 entry contract, Q8 helper facts). Check `git log origin/main` and open PRs
  immediately before starting AND again before a long build.
- Stale remote branches the owner may delete: work/compiler-build-bind-applied-requirement,
  work/compiler-native-application-calling-instance, work/compiler-build-bind-applied-merged-wip.
- Parked (old-stage work, frozen): work/psi-implementation-call-argument,
  work/psi-record-constant-field-fold, and the draft patches listed in the 11-11 notes.

## Owner's stated direction for next time
Speed. Fewer, better-aimed workers; no measuring; no tiny PRs; rot goes in
OWNER_CODE_CONCERNS.md; the owner writes spec while agents work. Proposed and not yet decided:
more compute, a smoke-set gate, auto-revert on red, an import allowlist with size budgets.

## Updates after the wind-down order
- #2765 (non-copy data and affine moves on the route) LANDED on main (0682e01d91263). Blocker
  item 2 is done and its files are free to edit; the Build program's first refusal may have
  moved, so re-run the probe (w3b_build_route_probe.patch) before assuming `type generic data`.
- Byte views: nothing built. w3_byte_views_handoff.md has the entry points, the witness pair
  (w3v_fixtures\) and the main risk (proving an index bound from two length observations).
  worker-3's omega.exe is stale (probe-patched); rebuild before use.

## FINAL STATE: all workers stopped, slots clean, nothing running

Open draft PRs to pick up (none may be merged as they stand):
- #2796 delete Psi 06 (130,032 lines). Build-verified, HELD: merging stops every compilation
  until a Build program runs on the lowered route (see THE BLOCKER above). Needs a rebase.
- #2805 build the application from the route's published module + carry evidence seals.
  Verified one commit before head (native exit 0 / Psi exit 0 on the witness); head is a
  later merge with main that was never built. To land: merge main, rebuild, re-run the
  witness and checks listed in its body, mark ready, merge.
- #2809 SOUNDNESS: refuse a generic machine that calls itself (it was accepted with no
  termination judgment). Verified before merging main; re-run the four commands in its body
  on the merged head, then land. Do this one first.
- #2808 service reach (written `reaches` ceilings) on the route. NOT ready: four neighbour
  fixtures moved in the focused run and the base comparison was never completed; PR body
  fails check-rules (gate name, Proof line, provenance). Boundary calls are not built.
- #2810 these notes (never merge).

Candidate owner questions, NOT filed and not checked against the spec by the coordinator:
- Does a same-package caller see a private machine's written `reaches` ceiling or its
  narrower derived reach? (w10_service_reach_handoff.md; effects.md)
- Does a conformance member that omits `requires`/`ensures` inherit the requirement's?
  (w12_handoff_1113.md)

Next-session order of work: (1) land #2809; (2) land #2805; (3) the Build-program stack,
items 1, 3, 4 first (lifetime-parameterised data, byte views, boundary calls); (4) traits and
conformances per w12_handoff_1113.md; (5) finish or redo #2808; then #2796 and the rest of the
removal per w14_removal_handoff.md.

# Worker rules, 2026-11-13 (read fully before your first command)

You are a worker under a coordinator in the Omega compiler repo (GitHub CathedralOS/Omega).
`AGENTS.md` at your worktree root is the canonical rule file and it changed a lot on
2026-11-12: read it fully first, then `.agents/skills/advance/SKILL.md`. Where this note and
AGENTS.md disagree, AGENTS.md wins. You own your outcome end to end, INCLUDING landing it.

## Slot
Work only inside your assigned worktree slot, by absolute path. Never edit or build in
`C:\Projects\Omega` itself or another worker's slot. Start with
`git status` (if tracked edits or a merge is in progress that your brief does not mention, stop
and report), then `git fetch origin; git checkout --detach origin/main`.

## CPU budget: hard rule, the owner uses this workstation
Every command that compiles, links, runs `omega`, runs the corpus or tests MUST go through the
PowerShell tool with this prefix (N = your slot number):

```powershell
$p = Get-Process -Id $PID; $p.PriorityClass = 'Idle'; $p.ProcessorAffinity = 0xFFFFC0; $env:MBX_SCHEDULER_CPUS = '18'; $env:CARGO_BUILD_JOBS = '18'; $env:OMEGA_CORPUS_JOBS = '6'; $env:RAYON_NUM_THREADS = '12'; Set-Location C:\Projects\Omega\.agents\worktree\worker-N;
```

Build: `$env:CARGO_PROFILE_DEV_OPT_LEVEL = '1'; $env:CARGO_TARGET_DIR = "$PWD\target"; mbx build -p omega`.
`mbx` is mandatory; direct cargo build/test/clippy/run is forbidden. One build or corpus run
of yours at a time. Close stdin on `omega`/corpus invocations (`cmd /c "... < NUL"`). Run long
commands in the background and wait for the notification; no sleeps, no polling loops. Never
open a visible window. If `mbx xtask` fails because xtask.exe is locked, call
`target\debug\xtask.exe` directly. LNK2019 `anon.*.llvm.*` on an incremental link: rebuild with
`$env:MBX_LEARNED_INCREMENTAL='false'; $env:MBX_SEED_INCREMENTAL='false'`.

## How work lands now (AGENTS.md "Spec first, land first")
- Read the governing `wiki/spec/` page, implement what it says once. Spec silence on LANGUAGE
  DESIGN (not engineering) is the only thing to escalate: stop that item and report the exact
  question to the coordinator; do not edit OWNER_QUESTIONS.md yourself.
- Fail closed. Never weaken a proof obligation, loosen a fixture, or add an assumed binding.
- Validate only what you touched: ONE focused `mbx xtask corpus --filter <fixtures>` (it adds
  the ring around them), the touched crates' unit tests, and
  `mbx clippy -p <crate> --all-targets -- -D warnings`. No broad sweeps, no before/after
  comparisons against main.
- Do NOT run `corpus --record` and do NOT edit `tests/omega/corpus_*.txt`: the post-merge
  check writes the ledger. Change code, fixtures and `expected.txt` only.
- No new `.omg` under any `facts/` directory; at most 6 new `.omg` per PR.
- One PR per capability. No one-row re-pin, fixture-only or ledger-only PRs: fold those into
  the PR that touches the rows.
- Commit title `lane: statement` (lowercase, <= 85 chars, no trailing period). No
  `Co-Authored-By` or "Generated with" lines anywhere. PR body: inverted pyramid in plain
  language, then cause, repair, validation with exact commands and results, limits beside the
  result they qualify. Compiler-lane PRs carry `Gate: <release-gate> | red rows: <n>` (with
  `| unlocks: ...` when 0) as AGENTS.md describes; if check-rules disagrees with this note,
  follow check-rules. Body and commit bodies end with ONLY the `mbx xtask provenance` line.
- Before pushing: `mbx xtask fmt --check`, `mbx xtask check-boundary`,
  `mbx xtask check-rules --base origin/main --title "<title>" --body-file <file>`.
- Push `git push origin HEAD:refs/heads/work/<lane>-<topic>` (never force-push; merge
  `origin/main` on conflict), open the PR, review your own diff against
  `.agents/skills/coordinator-review/SKILL.md`, and when the `admission` check passes
  squash-merge it yourself: `gh pr merge <n> --squash --match-head-commit <sha>`. There is no
  merge queue and no `xtask land`.
- Other sessions are active (several under the same GitHub account, plus Smotto, NH21B,
  aquinas-bot). Before editing, check open PRs (`gh pr list --state open`,
  `gh pr diff <n> --name-only`) and stay out of files an open PR is rewriting; if your item is
  already someone's open PR, say so and take the next item in your brief.

## Reporting
Report to the coordinator only landings (PR number, merge commit) and blockers, via your
handback. Stop and hand back when your brief is done or when you pass roughly 350k tokens of
context: leave the slot clean, nothing running, verified work pushed, and a precise handoff
note in the scratchpad (`w<N>_handoff_1113.md`: done, measured, exact next step). Never ask the
coordinator to perform an action that was denied to you.

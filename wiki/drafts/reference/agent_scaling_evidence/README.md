# Agent scaling evidence

These five reports measure what 14 days of agent work on `main` produced
(2026-11-10 to 2026-11-23, 3,016 first-parent commits, 2,943 with a
provenance line). They answer whether adding agents is moving the compiler
toward completion. They are model output from one session and are only
partly checked; delete this folder once a recorded baseline replaces it.

| Report | Question | Headline |
| --- | --- | --- |
| [progress.md](progress.md) | Is the compiler measurably closer to done? | No recorded whole-compiler number was repeated. `TASKS.md` grew from 55 to 77 top-level items. |
| [gate.md](gate.md) | What ran before each merge? | The required `admission` job compiles and tests nothing. In a sample of 120 Proof lines, 65% name one or a few tests and one states a workspace run. |
| [rework.md](rework.md) | How often is landed work repaired? | About 4% of commits repair a regression from an earlier landed change (95% range 2% to 8%), and about 4% finish work left undone. |
| [collisions.md](collisions.md) | Does the work split? | 84% of commits changed a code file another commit changed the same day. Ten same-day commits are all disjoint 12% of the time. |
| [cost.md](cost.md) | What does one landed change cost? | About 215 commits a day. The median change used 11.6M input tokens (98.8% cached), 29k output tokens and 18 minutes. |

## How far to trust them

One analyst per question read two tables extracted from
`git log origin/main --first-parent --since="14 days ago" --name-only`:
one row per commit with its provenance, Gate, Spec and Proof lines, and one
row per changed file. Each report's "How this was computed" section gives
its filters, and the script beside it reruns the numbers once the tables
are rebuilt.

A second agent was to recompute each report's headline numbers. That check
finished for `collisions.md` and `cost.md`, which end in a "Check" section.
It did not finish for `progress.md`, `gate.md` and `rework.md`: the session's
machine restarted. Treat those three as unchecked.

Rates in `rework.md` and the breadth classes in `gate.md` are estimates from
hand-classified samples, with the sample size stated. A Proof line is the
authoring agent's own statement of what it ran.

## Recorded baselines

[baseline-bafc9420b0f4.md](baseline-bafc9420b0f4.md) records the first
whole-compiler measure: at `bafc9420b0f4` the source check accepts 1,947 of
the 5,210 programs meant to compile for `linux_x86_64`, against 1,817 of 5,165
at `f76875c4106c`. The other four hosted targets have not been swept at that
revision.

`mbx xtask sweep` checks every program and groups where each stops, and
`mbx xtask sweep fleet` splits a target's sweep across machines. A first quick
sweep was tried on a 4-CPU cloud container and stopped: each program that uses
the standard library took about 3 minutes to check there.

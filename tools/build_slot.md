# Host build budget

Agent count and build load are separate resources. Several agents can read,
edit and reason at once, but every build, corpus run and compiled program they
start shares one machine. On a workstation its owner is using, that machine is
not the agents' to fill: on 2026-11-09 four local workers each ran
`mbx build -p omega` with default settings on a 24-CPU Windows workstation and
made it unusable for its owner twice.

`tools/build_slot.py`, the slot pool this page used to describe, was removed
with the Python tooling in #2080 and has no replacement. Records that cite it
describe how those runs were admitted at the time. Use the mechanisms below.

## What limits a build today

| Limit | Scope | How to set it |
| --- | --- | --- |
| mbx scheduler budget | Every mbx-run Cargo command that shares the mbx cache directory | `scheduler.cpus`, `scheduler.reserve_cpus` in the mbx configuration, or `MBX_SCHEDULER_CPUS`, `MBX_SCHEDULER_RESERVE_CPUS` in the environment |
| Cargo jobs | One Cargo invocation | `CARGO_BUILD_JOBS`, or `-j N` |
| Corpus workers | One corpus run | `OMEGA_CORPUS_JOBS` |
| Operating-system priority | One process tree | `nice -n 19 <command>`; on Windows, the Idle priority class |

The mbx scheduler is the only one of these that spans agents. Its
[documentation](https://github.com/jdx/mr-boxington/blob/main/docs/scheduling.md)
says independent Cargo commands share CPU and memory permits through one pool
under the cache directory, that `scheduler.cpus` is that pool's CPU budget, and
that `scheduler.reserve_cpus` withholds cores from it "to leave CPUs for other
work". The budget defaults to every logical CPU, so the default pool queues
agents behind each other but leaves nothing for anyone else. Cache hits take no
permit. `scheduler.priority = "low"` only changes admission order among mbx
builds; it does not lower operating-system priority.

Programs that do not run through mbx are outside that pool: `corpus_runner`
executing fixtures, `omega` itself, a compiled sample. Cap those with their own
setting, or charge them to the pool with
`mbx reserve --cpus N -- <command>`.

## On a workstation someone is using

Set the budget before the first build, not after the owner complains. A limit
the owner states replaces these defaults.

1. Start one expensive build at a time across all local agents unless a higher
   width has been measured on that host
   ([host setup](../.agents/skills/swarm/references/host-setup.md#admit-builds-not-just-agents)).
2. Leave most of the machine free: budget about a quarter of the logical CPUs
   for all agent work combined.
3. Run builds and corpus runs at the lowest operating-system priority, so
   interactive work preempts them.
4. Put the budget in every worker prompt. A subagent does not inherit the
   coordinator's shell environment.
5. After an unexplained restart or killed processes, suspect the agents' own
   load before starting the same work again.

POSIX shells:

```bash
export MBX_SCHEDULER_CPUS=6 CARGO_BUILD_JOBS=6 OMEGA_CORPUS_JOBS=4
nice -n 19 mbx build -p omega < /dev/null
```

Windows PowerShell. A process started from an Idle-class parent inherits that
class, so setting it on the shell covers `mbx`, `cargo` and every `rustc`:

```powershell
(Get-Process -Id $PID).PriorityClass = 'Idle'
$env:MBX_SCHEDULER_CPUS = '6'; $env:CARGO_BUILD_JOBS = '6'; $env:OMEGA_CORPUS_JOBS = '4'
mbx build -p omega
```

A host owner who wants one limit for every agent and session sets it once in
the global mbx configuration, for example `mbx settings set
scheduler.reserve_cpus 16`. That file belongs to the owner: an agent proposes
the value and does not write it uninvited.

Observed on that Windows host (i9-12900K, 24 logical CPUs, mbx 1.29.2,
2026-11-09): with the PowerShell form plus a six-CPU affinity mask, the `mbx`,
`cargo` and `rustc` processes of a running `mbx build -p omega` all showed the
Idle class and the mask, and total CPU load stayed between 10% and 25%.
`MBX_SCHEDULER_CPUS=6 mbx settings get scheduler.cpus` printed `6`. Not
measured here: how the mbx pool behaves when concurrent invocations carry
different budgets, and the POSIX form.

## One hold for a whole chain

Run a local chain (check, crate tests, scoped corpus gate) as sequential steps
in one shell under one budget, and finish it before pushing: integration builds
run on GitHub, not on the host ([landing](landing.md)). Docs-only changes need
no compiler build.

## Diagnostics don't queue with chains

A compile-error or red-test repair is a minutes-scale check-edit-check loop.
Keep those checks single-crate (`mbx check -p <crate>`) and inside the same
budget; under memory or CPU pressure, drop diagnostic work before the chain.

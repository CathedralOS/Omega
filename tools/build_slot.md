# Host build budget

Agent count and build load are separate resources. Several agents can read,
edit and reason at once, but every build, test run and compiled program they
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
| Operating-system priority | One process tree | `nice -n 19 <command>`; on Windows, the Idle priority class |
| CPU affinity or a cgroup CPU quota | One process tree, inherited by its children | `taskset`, a cgroup, or on Windows `ProcessorAffinity` |

Job counts are per invocation, so they multiply across agents: four agents at
six jobs each are twenty-four compilers. The mbx scheduler is the only setting
here that spans agents, and an affinity mask or cgroup quota is the only hard
ceiling. Its
[documentation](https://github.com/jdx/mr-boxington/blob/main/docs/scheduling.md)
says independent Cargo commands share CPU and memory permits through one pool
under the cache directory, that `scheduler.cpus` is that pool's CPU budget, and
that `scheduler.reserve_cpus` withholds cores from it "to leave CPUs for other
work". The budget defaults to every logical CPU, so the default pool queues
agents behind each other but leaves nothing for anyone else. Cache hits take no
permit. `scheduler.priority = "low"` only changes admission order among mbx
builds; it does not lower operating-system priority.

Programs that do not run through mbx are outside that pool: `omega` itself, a
compiled sample. Cap those with their own
setting, or charge them to the pool with
`mbx reserve --cpus N -- <command>`.

## On a workstation someone is using

Set the budget before the first build, not after the owner complains. A limit
the owner states replaces these defaults.

1. Cap the total, not each build. Either start one expensive build at a time
   across all local agents
   ([host setup](../.agents/skills/swarm/references/host-setup.md#admit-builds-not-just-agents)),
   or give every agent the same shared limit: one mbx pool budget plus one
   affinity mask or quota.
2. Leave headroom and never drive the machine to 100%. Without a stated limit,
   budget at most three quarters of the logical CPUs for all agent work
   combined and keep whole cores free for the owner. Do not overcorrect either:
   a build crawling on a sliver of an idle machine wastes it.
3. Run builds and test runs at the lowest operating-system priority, so
   interactive work preempts them.
4. Put the budget in every worker prompt. A subagent does not inherit the
   coordinator's shell environment.
5. After an unexplained restart or killed processes, suspect the agents' own
   load before starting the same work again.

The examples are for a 24-CPU host. POSIX shells:

```bash
export MBX_SCHEDULER_CPUS=18 CARGO_BUILD_JOBS=18
nice -n 19 mbx build -p omega < /dev/null
```

Windows PowerShell. A process started from an Idle-class parent inherits that
class, and every child inherits the affinity mask, so setting both on the shell
covers `mbx`, `cargo` and every `rustc`. The mask `0xFFFFC0` is logical CPUs 6
to 23, which leaves six for the owner:

```powershell
$p = Get-Process -Id $PID; $p.PriorityClass = 'Idle'; $p.ProcessorAffinity = 0xFFFFC0
$env:MBX_SCHEDULER_CPUS = '18'; $env:CARGO_BUILD_JOBS = '18'
mbx build -p omega
```

A host owner who wants one limit for every agent and session sets it once in
the global mbx configuration, for example `mbx settings set
scheduler.reserve_cpus 6`. That file belongs to the owner: an agent proposes
the value and does not write it uninvited.

Observed on that Windows host (i9-12900K, 24 logical CPUs, mbx 1.29.2,
2026-11-09): with the Idle class and a six-CPU mask set on the launching
PowerShell, the `mbx`, `cargo` and `rustc` processes of a running
`mbx build -p omega` all showed both, and total CPU load stayed between 10% and
25%. The owner called that an overcorrection and set the limit for that host:
never 100%, and 75% is fine. After the mask was widened to `0xFFFFC0`, the 32
build processes of four concurrent workers all carried it.
`MBX_SCHEDULER_CPUS=6 mbx settings get scheduler.cpus` printed `6`. Not
measured here: how the mbx pool behaves when concurrent invocations carry
different budgets, and the POSIX form.

## On the test VMs

One heavy job per machine. A build, a test run or a batch of compiler runs
is heavy; reading logs, `git` and editing are not. The VMs have 16 cores and 11
to 12 GB each, and memory is what runs out: on 2026-11-09 a long run of
compiler jobs lost one to the kernel's out-of-memory killer when a second batch
of compiler runs was started beside it, and lost others to settings changed
while it ran.

- `~/omega-agent/bin/vm-heavy <label> <command...>` on `omega-linux` and
  `omega-win`, installed from [`tools/vm_heavy.sh`](vm_heavy.sh), runs a
  command holding the machine's lock. It refuses with exit
  75 while a compiler build or another guarded job is active
  there. `omega-remote` routes every command that names `mbx`, `cargo` or a
  built binary through it. Wait, or use the other VM. For a deliberate overlap,
  start the command with `OMEGA_REMOTE_SHARE=1`.
- Do not change the settings of a job that is already running. Measure one
  unit first (one build), choose the settings, then leave them.
- On `omega-win`, Defender scanned every file a slot worktree touched until
  `C:\Users\jarod` was excluded; a checking run then dropped
  from 18 minutes to 6. A new Windows VM needs that exclusion, which is its
  owner's to add.

## One hold for a whole chain

Run a local chain (check, crate tests, a sample gate) as sequential steps
in one shell under one budget, and finish it before pushing: integration builds
run on GitHub, not on the host ([landing](landing.md)). Docs-only changes need
no compiler build.

## Diagnostics don't queue with chains

A compile-error or red-test repair is a minutes-scale check-edit-check loop.
Keep those checks single-crate (`mbx check -p <crate>`) and inside the same
budget; under memory or CPU pressure, drop diagnostic work before the chain.

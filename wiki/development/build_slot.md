# Host build budget

Several agents can read, edit and reason at once, but every build, test run
and compiled program they start shares one machine. On a workstation its owner
is using, agents must not fill that machine: on 2026-11-09 four local workers each ran
`mbx build -p omega` with default settings on a 24-CPU Windows workstation and
made it unusable for its owner twice.

## What limits a build today

| Limit | Scope | How to set it |
| --- | --- | --- |
| mbx host ceiling | Every process an mbx command starts, on every agent of the host | Follows the mbx scheduler budget below; nothing else to set |
| mbx scheduler budget | Every mbx-run Cargo command that shares the mbx cache directory | `scheduler.cpus`, `scheduler.reserve_cpus` in the mbx configuration, or `MBX_SCHEDULER_CPUS`, `MBX_SCHEDULER_RESERVE_CPUS` in the environment |
| Cargo jobs | One Cargo invocation | `CARGO_BUILD_JOBS`, or `-j N` |
| Operating-system priority | One process tree | `nice -n 19 <command>`; on Windows, the Idle priority class |

Job counts are per invocation, so they multiply across agents: four agents at
six jobs each are twenty-four compilers. The mbx
[scheduler](https://github.com/jdx/mr-boxington/blob/main/docs/scheduling.md)
spans agents, but it counts permits, not threads: one permit can run a compiler
with many threads, and test binaries and the `omega` programs they start run
outside it. On 2026-11-23 five agents on a 32-CPU workstation, each inside a
28-CPU pool, held it at 99-100% for minutes. `scheduler.cpus` is the pool's CPU
budget and `scheduler.reserve_cpus` withholds cores from it "to leave CPUs for
other work"; the budget defaults to every logical CPU. Cache hits take no
permit. `scheduler.priority = "low"` only changes admission order among mbx
builds.

## The host CPU ceiling

The pinned mbx (`1.29.2-omega.3` and later, from
[`mbx-host-ceiling.patch`](../../tools/xtask/mbx-host-ceiling.patch)) turns that
budget into a hard ceiling. When `scheduler.cpus` minus `scheduler.reserve_cpus`
is smaller than the machine, every mbx command that runs work (a Cargo command,
`exec`, `reserve`, `explain` or `prefetch`) limits itself before it starts
anything, and Cargo, every `rustc`, test binaries and the programs they spawn
inherit the limit. No wrapper or prompt flag is involved, so one setting
controls both the pool and the ceiling:

- **Windows:** mbx puts itself in a Job Object that keeps its tree off the
  lowest-numbered reserved CPUs and in the Idle priority class. Every mbx picks
  the same CPUs, so a 28-of-32 budget leaves logical CPUs 0 to 3 free whichever
  agent runs. Test harnesses that create their own kill-on-close jobs, such as
  `bounded-process`, nest inside it and keep the limit. When the job cannot be
  created, mbx sets the same affinity mask and priority class on its own
  process.
- **Linux:** mbx moves itself into the systemd user slice `omega-host.slice`,
  which carries `CPUQuota=<budget>00%` and `CPUWeight=idle` for every mbx of
  that user. It does so only when the user lingers; otherwise systemd would
  stop the slice, and any detached build in it, when the last login ends.
  Without a lingering user manager (containers, cloud sessions, `omega-linux`)
  mbx keeps its tree off the lowest-numbered CPUs with an affinity mask at
  nice 19, which every mbx shares the same way.
- **macOS:** there is no hard ceiling: macOS offers no CPU quota or affinity
  API. mbx runs its tree under the Darwin background policy, which children
  inherit; it lowers CPU and I/O priority and keeps the work on efficiency
  cores, so the pool and bounded test threads remain the only count limits.

The dedicated VMs leave the reserve at zero, so their budget covers every CPU
and mbx applies nothing there. `mbx xtask doctor` reports the mechanism and
value in its `host-ceiling` line, and warns when the budget asks for a ceiling
that its own process does not carry (an older mbx, or `cargo xtask doctor`
run without mbx).

Why a Windows job per mbx rather than one named, shared job: a process already
inside one job hierarchy cannot join a job another hierarchy owns. On this
workstation the agents' bash and PowerShell shells sit in different
hierarchies, and a shared `Local\OmegaHostBudget` job refused the second one
with "Access is denied". A shared job would also tie a build the owner starts
outside any job to an agent's job tree. The common affinity mask is what makes
the ceiling machine-wide, and it needs no shared object. A shared job's
CPU-rate cap would spread the work over every core instead of keeping cores
free, and it would only bind processes that could join.

Observed on the 32-CPU Windows workstation (AMD Ryzen 9 7950X, 2026-11-23)
while five other agents built with the earlier mbx: two concurrent
`mbx test` runs of a spinning load, each at `--test-threads=64` with 48
spinning tests plus 8 child processes of 4 threads (160 runnable threads),
used 18.0 to 22.7 CPUs (average 20.0) under a 28-CPU budget. All 18 load
processes carried affinity `0xfffffff0` and the Idle class, including the
children started through nested jobs. A four-thread spinner started outside mbx
kept affinity `0xffffffff`, the Normal class and 3.97 CPUs. The machine stayed
at 99-100% because the other agents' builds did not run under the ceiling yet.
With a development build, the same load under a shared job measured 25.0 CPUs
on the affinity mask and 24.8 CPUs under a 87.5% CPU-rate cap; the rate cap
put the load on CPUs 0 to 3 as well (99.9% busy against 90.9%).

Observed on `omega-linux-arm64` (8 CPUs, user lingering, 2026-11-23) with
`MBX_SCHEDULER_RESERVE_CPUS=2` and the same load: through the slice
(`CPUQuota=600%`, `CPUWeight=idle`) the load used 5.86 to 6.06 CPUs and the
machine sat at 75%, spread over every CPU. With the user manager unreachable,
the affinity fallback held the load at 5.98 to 6.02 CPUs on CPUs 2 to 7 at
nice 19, and CPUs 0 and 1 stayed 1.4 to 6.7% busy. In both modes a
two-thread spinner started outside mbx got 1.96 CPUs. Without a reserve, mbx
left the process in its ssh session scope with all 8 CPUs.

## On a workstation someone is using

Set the budget before the first build, not after the owner complains. A limit
the owner states replaces these defaults.

1. Cap the total once per host, in the global mbx configuration:
   `mbx settings set scheduler.reserve_cpus <cores to leave free>`. Every mbx
   of every agent on the host reads it, and the host ceiling follows it. That
   file belongs to the owner: an agent proposes the value and does not write
   it uninvited.
2. Leave headroom and never drive the machine to 100%. Without a stated limit,
   reserve at least a quarter of the logical CPUs and keep whole cores free for
   the owner. Do not overcorrect either: a build crawling on a few cores of an
   idle machine wastes it.
3. Run everything that builds or tests through the pinned mbx. Its ceiling
   already runs the work at the lowest operating-system priority; a program
   started outside mbx is outside the ceiling, so run it through `mbx exec` or
   count it with `mbx reserve --cpus N -- <command>`.
4. Worker prompts need no thread caps or priority settings once the global
   setting exists. Until the owner sets it, put `MBX_SCHEDULER_CPUS` in every
   worker prompt: a subagent does not inherit the coordinator's shell
   environment.
5. After an unexplained restart or killed processes, suspect the agents' own
   load before restarting the same work.

Earlier, on a 24-CPU Windows host (i9-12900K, mbx 1.29.2, 2026-11-09), agents
set the Idle class and an affinity mask on the launching PowerShell by hand. A
six-CPU mask held total load between 10% and 25%, which the owner called an
overcorrection; that host's limit was "never 100%, and 75% is fine".

## On the test VMs

Run one heavy job per machine. A build, a test run or a batch of compiler
runs is heavy; reading logs, `git` and editing are not. The VMs have 16 cores and 11
to 12 GB each, and memory is what runs out: on 2026-11-09 a long run of
compiler jobs lost one to the kernel's out-of-memory killer when a second batch
of compiler runs was started beside it, and lost others to settings changed
while it ran.

- `~/omega-agent/bin/vm-heavy <label> <command...>` on `omega-linux` and
  `omega-win`, installed from [`tools/vm_heavy.sh`](../../tools/vm_heavy.sh), runs a
  command holding the machine's lock. It refuses with exit
  75 while a compiler build or another guarded job is active
  there. `omega-remote` routes every command that names `mbx`, `cargo` or a
  built binary through it. Wait, or use the other VM. For a deliberate overlap,
  start the command with `OMEGA_REMOTE_SHARE=1`.
- `mbx xtask vm` starts a job under `vm-heavy` and shows its progress while it
  runs, so nobody has to ssh in and grep a log. Run it from any checkout:

  ```sh
  mbx xtask vm start omega-linux both-output both-output.sh  # or: -- <command...>
  mbx xtask vm status omega-linux both-output    # state, elapsed, counts, last lines
  mbx xtask vm follow omega-linux both-output    # stream the log until the job ends
  mbx xtask vm status omega-linux                # lock holder and every job
  ```

  `start` copies the script to `~/omega-agent/jobs/<label>/job.sh` on the VM,
  starts it detached at the lowest priority, and returns once `vm-heavy` has
  taken the lock or refused; a refusal exits 75 and prints why. The job's
  output goes to `~/omega-agent/jobs/<label>/log`, and the pinned mbx in
  `~/.cache/omega-mbx-bin/` comes first on its `PATH` when the VM has it.
  `status` counts `test ... ok`, `test ... FAILED` and `ignored` lines, lists
  the failed tests, and groups comparison diagnostics labeled `DIVERGENCE` and
  `error` lines by text. `follow` exits with the job's exit code, so an agent
  can run it in the background and be woken when the job ends; stopping
  `follow` leaves the job running. A job that is still running keeps its
  label: `start` refuses to replace it.
- Do not change the settings of a job that is already running. Measure one
  unit first (one build), choose the settings, then leave them.
- On `omega-win`, Defender scanned every file a slot worktree touched until
  `C:\Users\jarod` was excluded; a checking run then dropped
  from 18 minutes to 6. A new Windows VM needs that exclusion, which is its
  owner's to add.

## One hold for a whole chain

Run the focused checks, crate tests and sample gate one after another in the
same shell under one budget. Run full-workspace checks on a LAN VM under the
[heavy-check policy](../../AGENTS.md#heavy-checks-run-on-the-lan-vms).
Docs-only changes need no compiler build.

## Diagnostics don't queue with chains

A compile-error or red-test repair is a minutes-scale check-edit-check loop.
Keep those checks to one crate (`mbx check -p <crate>`) and inside the same
budget; under memory or CPU pressure, drop diagnostic work before the chain.

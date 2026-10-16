---
name: cursor-swarm
description: Coordinate a requested Cursor cloud wave, one Task cloud subagent per distinct sample outcome, each on its own VM and branch. Use when the user asks for a Cursor swarm, Cursor cloud sessions, or for this chat to start separate cloud sessions. Use dot-swarm for dot native workers, devin-swarm for Devin, codex-swarm for Codex, and local-swarm for agents sharing one machine.
---

# Cursor cloud session swarm

Run a user-requested compiler wave as Cursor cloud subagents. Each worker has
its own VM, checkout, and branch, and it carries one sample through acceptance
and landing. The coordinator selects outcomes, starts the sessions, and checks
that each landing is on remote main. Writing or editing this skill does not
launch a session or advance a compiler.

Apply the [law of completion](../../../AGENTS.md#law-of-completion).
Each implementation owner retains context through complete acceptance, required
review, and authorized landing verified on remote main, including the compiler,
library, and prerequisite repairs that sample needs. Do not split those stages
across workers or replace a landing with a branch, a pull request, or a report.
Follow the concrete path through completion; let working programs reveal design.

Read [AGENTS.md](../../../AGENTS.md), [advance](../advance/SKILL.md),
[claims](../../../tools/claims.md), [testing](../../../tools/testing.md), and
[landing](../../../tools/landing.md). Each worker repeats the
[cloud launch preflight](../dot-swarm/references/cloud-launch.md) on its own
machine before implementation. For interruption, follow
[recovery](../dot-swarm/references/recovery.md): a disconnected coordinator is
not a dead worker, and a local archive is not a landed result.

## Choose this skill

| Request | Skill |
| --- | --- |
| Cursor cloud sessions started from this chat | cursor-swarm |
| Devin cloud sessions | [devin-swarm](../devin-swarm/SKILL.md) |
| Codex cloud threads | [codex-swarm](../codex-swarm/SKILL.md) |
| Dot native workers on one shared computer | [dot-swarm](../dot-swarm/SKILL.md) |
| Agents sharing this machine | [local-swarm](../local-swarm/SKILL.md) |
| One sample and no wave | [advance](../advance/SKILL.md) |

A single advance remains one assignment. This skill applies when the user asks
for the wave. Honor the requested session count and the stopping condition.
Record that count. Launch one session per distinct demonstrated root cause
that the selected environment can execute. When the queue has fewer causes
than the requested count, launch the smaller number and report the remainder
as pending, with the missing cause or host. Idle slots stay empty. Board
items, line-count cleanup, and a second session on the same cause do not fill
the count.

## Discover the launch tool

Before every dispatch, read the current `Task` schema from the `cursor`
namespace. Pass only fields that schema accepts. The names below describe the
schema this skill was written against:

| Action | Task field |
| --- | --- |
| One cloud session | `environment` set to `cloud` |
| Implementation owner | `subagent_type` set to `generalPurpose` |
| Leave the coordinator free | `run_in_background` set to true |
| Start from fetched remote main | `cloud_base_branch` set to `main` |
| Boot a named environment build | `cloud_requested_environment_build_id` when the user names one |

A cloud Task subagent has its own VM and git branch. `environment` `local`
shares this machine; that is [local-swarm](../local-swarm/SKILL.md). Launch
the ready sessions as concurrent Task calls in one coordinator turn.

`tools/swarm/launch.py` creates Devin sessions. `collaboration.spawn_agent`
starts a dot worker on the parent computer. Detached shells, tmux, and guessed
HTTP routes are not this transport. If `Task` or `environment: cloud` is
absent from the discovered schema, report the missing field and stop the
launch. A pending slot is not a running session.

Retain every returned agent id. In the user-facing report, link each session
with its slot name and that id, using the chat's subagent link form. A spawn
that returns no id is uncertain: read the existing cloud-agent list before
creating the slot again.
A capacity error leaves the remaining slots pending; do not retry in a loop
or nest coordinators to multiply slots.

## Cloud VM size

`environment` `cloud` starts a separate VM and git branch. The spawn receipt
is an agent id. It does not include a VM id, hostname, vCPU count, or memory
size. There is no `Task` field that requests a larger or smaller machine.
Omit `cloud_requested_environment_build_id` and the worker boots the same
environment build as the coordinator.

Read the current environment with the `cursor-cloud` `environment-info` tool
before describing a wave's machines. That record names the environment, its
public id, and the build the pod booted from. It does not record vCPU or
memory. Measure the pod you are on with `nproc`, `free -h`, and `df -h`.

On 2026-10-16 the coordinator for the Omega personal environment, build
`bld-20261016-387208bb-7e97-436d-9c52-c104977f6347`, reported 4 vCPUs, 15 GiB
of RAM, and a 254 GB disk. That measurement is that coordinator pod. Worker
receipts from the same launch did not repeat those numbers. A later build is
unknown until a pod that booted it is measured.

## Baked compiler

An environment install can leave `/workspace/build/gate-cli/debug/omega` on
the disk. That binary surviving boot is not a skipped compile. On
2026-10-16 a cold `mbx build -p omega --bin omega` with
`CARGO_PROFILE_DEV_OPT_LEVEL=1` and `CARGO_TARGET_DIR=/workspace/build/gate-cli`
took 9 minutes 1 second. The same command against that exact source finished
in 0.22 seconds. Four sessions booted from build
`bld-20261016-63173d80-ba40-4378-bfa4-2de5a7a5f84e`, found the 148 MB binary,
and still spent 473, 474, 483, and 487 seconds in the first `mbx build`
because `main` was newer than the snapshot. Record `first_build_seconds`.
Do not treat a present binary as a fast build unless that number was measured
on the session's own checkout.

## Select outcomes

Complete the [mandatory main preflight](../../../AGENTS.md#sessions-and-subagents)
on the coordinator before dispatch. Fetch `origin/main`, record the exact
commit, and reread this skill from that commit. A failed fetch stops the new
launch. Cached `origin/main` is not a fallback. Preserve the coordinator's
dirty or non-main checkout; workers start from the fetched remote `main`, not
from the coordinator's branch.

Inspect live claims and assignments already written for this wave. Select
with [advance](../advance/SKILL.md): a main regression, a sample the user
names, then the smallest unclaimed failing sample. Several queued samples
with one demonstrated root cause are one session. Shared files are allowed.
When no recent queue record exists, run
`python3 tools/sample_native_gate.py --programs-only --find-work`, then read
`python3 tools/sample_queue.py`. Root causes named before that record are
hypotheses.

The worker boots the Cursor environment selected for it. The coordinator's OS
does not transfer. A Windows execution oracle needs a Windows environment
build, passed as `cloud_requested_environment_build_id` when the user supplies
one. Without that build, record the Windows outcome as pending. The worker
reports its own `uname` and treats oracles for other hosts as unavailable.

Pass `model` only when the user names a slug in the current exposed model
list. Never pass a slug ending in `-fast`. High, medium, and low mean
`grok-4.7-high`, `grok-4.7-medium`, and `grok-4.7-low`. Otherwise omit
`model` and the worker inherits the coordinator. Confirm the model from
returned runtime metadata when that metadata exists. Record an unreported
model as unverified. Text in the prompt does not change the model.
The dot default `gpt-6.1-sol` applies only when that slug is present in the
exposed list.

## Record, then start

Before the Task calls, write `build/swarm/<wave>/inflight.json` with one slot
per requested session: sample path, oracle, acceptance command, base SHA,
required host, model request, status `pending`, and the agent id once the
call returns. A draft file under `build/swarm/` is coordinator memory. It is
not a Devin manifest and it is not fed to `launch.py`.

## The worker's goal

The session has two finishes when the assigned program already runs. Keep
modifying that program until its first run fails, then repair the compiler
only far enough that the failing run passes. Or leave it and write another
useful sample, one whose first `omega run` fails, and repair only what that
failure requires. A sample that runs on the first try is a wasted session.
Do not hunt the compiler for something to change.

Copy this into every worker prompt:

> Run `omega run` on the assigned program. When it fails, repair the
> compiler only far enough that this run passes, including through a local,
> a field, a parameter, and a call result, and land that repair with the
> program through `landing.py`. When it already passes, stop. Either keep
> modifying this program until a run fails and then repair that failure, or
> go write another useful sample whose first `omega run` fails and repair
> that failure. Land the repair with the witness that failed. A sample,
> input file, or fixture that passed on the first try is unfinished. Do not
> search the compiler for an edit.

A host this VM cannot execute, or an owner decision in `OWNER_QUESTIONS.md`,
ends as `blocked` with the failure named. Publishing the original program
unchanged, or admitting a feature the compiler already required, does not
close the session.

Each prompt is the worker's whole assignment. The worker does not see this
conversation. Include:

- The goal above, in those words
- Wave, slot, sample path, authored oracle, and the `omega run` acceptance
- Fetched main SHA, and the instruction to repeat the main preflight
- Read `AGENTS.md` and run [advance](../advance/SKILL.md) restricted to this
  sample
- The law of completion: carry the failing run through the compiler repair,
  required review, and authorized landing verified on remote main; do not
  wait for another agent; do not publish half-finished functionality; do not
  bypass a gate; an explicitly read-only probe remains read-only
- Claim with `python3 tools/claims.py` before editing, and keep the note
  current: sample, acceptance, current failure, next action, prerequisites
- Publish only through `python3 tools/landing.py`. A Cursor branch or pull
  request for that branch is the worker's checkout, not the landing. Leave
  the pull request unmerged. After `landing.py` has fast-forwarded main,
  delete the candidate branch as AGENTS.md requires
- Report the actual OS. State Windows, macOS, and QEMU acceptance as
  unavailable on a Linux machine
- Jev advisories run when the worker environment already has
  `TYPESAFE_API_KEY` and can reach api.typesafe.ai. This transport has no
  `secret_ids` field. Otherwise record Jev unavailable and use the
  deterministic checks in [testing](../../../tools/testing.md)
- Verdict fields: `worker`, `item`, `result`, `base`, `branch`, `commits`,
  `claim_ticket`, `checks`, `witness`, `remaining_legs`. `result` is
  `landed`, `verification_only`, `blocked`, `superseded`, or `aborted`

The worker keeps one Rust build active on its own machine. The coordinator
does not build a compiler to warm the workers.

## Reuse the session

Reuse the session that already has the pod. A new VM spends a machine and
the tokens required to rebuild the investigation. Resume the same agent id.
A new Task call is for an id that is gone.

After a solve lands and the next assignment is a different outcome, compact
before that assignment. Compaction keeps the claim note: what landed, the
witness, and the next failure. The following prompt is that note. The
investigation trace does not carry forward.

When the next run branches from that solve, do not compact. The session
keeps the repair and cuts the next failing run from it.

## Drain

A background cloud subagent finishes by notifying the coordinator. Do not
poll it, wait on its shell, or send a keep-alive nudge. After the launches,
continue coordinator bookkeeping or end the turn.

On notification, drain that slot before starting a replacement. Read the
verdict, fetch `origin/main`, and accept `landed` only when the repair
commit is an ancestor of fetched main and the witness failed before that
commit. A sample that passed on the first try is unfinished: resume the
same agent and tell it to keep modifying the program until a run fails, or
write another useful sample whose first run fails. Do not merge the worker branch into
the coordinator branch. Resume the same agent id for the next step of the
same unfinished outcome. Resume that same agent for the next red run while
its pod is up. The pod already has the compiler in its target directory. A
new Task call starts a new VM and pays about eight minutes of `mbx build`.
Keep the session on one more failing `omega run`, then one repair. When the
next probe passes, the session stops. Do not replace a live pod with a fresh
one for the next sample. A new Task call is a new VM; use it when the
returned id is gone. Choose another sample only after this one has landed or
the user stops the wave.

Release a claim only for a slot whose owner has stopped and whose outcome has
landed or been abandoned. Keep dirty unpublished work. Remove only a clean
worktree whose head is an ancestor of fetched main.

Report the requested count, the launched count, each linked session, the base
SHA, the sample, and the observed result. A written skill and a session count
are not a compiler improvement.

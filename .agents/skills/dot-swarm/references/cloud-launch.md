# Launch an Omega cloud session

Use this preflight for a single dot cloud assignment as well as a native-agent
wave. It complements [recovery](recovery.md) and does not authorize a swarm,
new credentials, package authority decisions, or infrastructure provisioning.

## Select the requested model and environment

The owner default in [AGENTS.md](../../../../AGENTS.md#delegation) is
`gpt-6.1-sol` with `medium` reasoning. A later explicit user choice wins.
Inspect the currently exposed cloud-session launch schema and pass its supported
model and reasoning fields explicitly. Confirm the actual selection from the
returned runtime metadata when available; record an unreported selection as
unverified. A request in the task prompt, skill UI metadata, or a repository file
does not itself set a platform-wide default or change an already-running session.
If model selection is unavailable, report that limitation instead of claiming
the requested model was applied or silently choosing another one.

For an authorized native worker, the current `collaboration.spawn_agent`
override is `model: "gpt-6.1-sol"`, `reasoning_effort: "medium"`. A full-history
fork inherits its parent's model and rejects overrides: use `fork_turns: "none"`
with a self-contained assignment, or a supported positive turn count. Recheck
the actual schema before dispatch. Native workers share their parent's computer;
they do not launch separate cloud machines.

Select the existing Omega environment explicitly using its returned identifier,
not an inferred name, path, hostname, or a previous container's layout. If the
runtime reports it as starting, wait for that environment before reading its
files. Reconcile existing handles before creating another session. Keep the
initial assignment bounded: one requested advance means one outcome chain.

## Verify prerequisites before compiler work

Run these checks on the selected executor, using bounded commands. Preserve the
results in the task's ignored evidence directory, excluding credentials and
environment dumps. A check on the coordinator's machine does not validate a
different cloud executor.

1. **Checkout.** Confirm the actual working directory, Git root, origin and push
   destination resolve to `CathedralOS/Omega`; do not print credential-bearing
   URLs. Record HEAD, branch, status and existing worktrees. Complete the
   [mandatory main preflight](../../../../AGENTS.md#sessions-and-subagents): fetch
   successfully, fast-forward clean main or use an isolated checkout at the
   fetched commit, and reread updated instructions before implementation.
   A failed fetch stops a new launch; preserve unrelated and resumed work.
   The expected Linux path may be `/workspace/Omega`; discover it rather than
   assuming that `/workspace` is itself the repository.
2. **Commit identity.** Before the first claim or other remote write, inspect
   the effective author and committer identities, including environment
   overrides. Carry the user's already-authorized noreply address in the launch
   brief and set it repository-locally when needed; never invent an address or
   apply one user's identity to another contributor. If none is known, resolve
   it before a long build. A read-only fetch does not establish push permission
   or GitHub email-policy acceptance. The first real authorized claim supplies
   that evidence; do not create throwaway remote refs as a probe. On GH007,
   preserve the rejection and reconcile the push before changing the identity;
   keep GitHub's privacy setting enabled.
3. **Execution and custody.** Inspect the actual UID mapping and ownership of
   the cache directory and its ancestors. A prior Omega cloud sandbox mapped
   `/` to UID65534, while its normally approved host route exposed UID0 and
   passed the unchanged custody checks. This is a diagnostic lead, not a rule
   for every container. When that distinction exists, use only the execution
   tool's exposed per-command approval route. Do not disable ownership checks,
   recursively change system ownership, or enable a bypass mode. An actual
   permission denial must be reported with its exact action and error; do not
   route around it.
4. **Toolchain and limits.** Activate the environment's documented setup, then
   run [doctor](../../../../tools/doctor.py) and `mbx doctor` when available.
   Use the pinned Rust toolchain and nextest under
   [testing](../../../../tools/testing.md). Inspect CPU quota, available memory
   and disk as well as visible processors; size build jobs to the effective
   limits. Keep one Rust build active per executor. Record the compiler's source
   revision; an unversioned preinstalled binary is not evidence about main.
5. **A real smoke run.** With a compiler of recorded provenance, run a small
   package-free fixture through normal package preparation and execution.
   `tests/omega/pass/calls/runtime_static_entry_context_exit` has an authored
   exit of 0 and observes its state mutation. On Linux, use `omega update
   --project <fixture> --target linux_x86_64`, then `omega run --both
   <fixture>/main.omg`. Bound both commands and require native/interpreter 0.
   Use the actual target on another host. Normal acceptance of a fully checked
   package with no authority choices is distinct from approving authority;
   never add blanket approvals. Preserve pre-existing lockfiles, and clean up
   only files this probe generated after retaining its evidence. This validates
   the environment; it is not a new compiler advance.
6. **Coordination.** Read [claims](../../../../tools/claims.md) before claiming.
   A measured 42-second fetch exceeded the default 15-second coordination
   budget in one cloud run. Where observed latency warrants it, use the
   supported bounded `--timeout-seconds 120` before the subcommand. A timeout
   is unavailable coordination, not an empty registry; preserve the ticket and
   reconcile an uncertain write before retrying. Same-outcome ownership conflicts block
   edits. Registry outages otherwise follow AGENTS.md's isolated-worktree rule.
7. **Recovery and landing.** Before a long operation or integration, preserve
   the exact source checkpoint, command exits, pending operation identities and
   unrun gates through the available supported artifact workflow. Confirm the
   upload; a local archive alone is not a backup. Reconcile a disconnect with
   an actual read/command and process state before resuming. Keep builds and
   initial validation outside [landing](../../../../tools/landing.md)'s
   180-second lease, publish only through its helper, then verify fetched main
   and remove only this task's clean landed worktree and branch.

## Report the launch state

Before expensive implementation, report the selected environment/handle and
actual repository path, fetched base and preserved checkout, requested and
confirmed model/effort, execution approval route, toolchain, resource limits,
smoke result, claim status, and evidence location. Mark unavailable runtime
identity or model metadata explicitly; a guessed value is not a receipt.
Carry publication authorization and the stopping condition into the assignment.
Reuse authorization already supplied by the user; ordinary commits, pushes and
coding decisions do not require repeated confirmation.

The custody, identity, coordination and smoke steps above come from the cloud
advance landed as `f50b218adcc10e4d02b8ff6f05b0c904158330a8`. Its successful
execution does not establish a global model preference or guarantee that every
future environment has the same approval route. No keep-alive loop or automatic
session replacement is part of this procedure.

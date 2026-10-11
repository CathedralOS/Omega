# NEW078 validation evidence

The complete Omega application passes check and exact native/Terminal execution
for 10 selected scenarios, 12 invalid runtime inputs and 619 tiny configurations
(641 cases on each execution route). Integer and byte tolerance is zero.
The independent simple-path flood enumeration checks 256 air/vent assignments.
This is a synthetic 4×4 unit-cell model, not a physical pressure/viscosity model.

Base: `3747d4d512bb908f031ae07b86ca08255f3879b0` (fetched main).
Initial base: `b4008bb3c7894aebad47eaccf8af12bb4a8006b1`.
Worktree `/workspace/Omega/.codex/worktrees/new078`, branch `new078-mold`.
Original checkout `/workspace/Omega` was preserved. Candidate commit, complete
source hashes, patches, source archives and exact logs are in the review bundle.

The reference algorithm, selected expected outputs and tolerance were frozen
before the first Omega check; `oracle.sha256` still matches. All 619 tiny expected
reports were materialized from that pinned reference before runtime execution.
Validation compares observed stdout, empty stderr and exit codes; saved native
and Terminal `.stdout` files contain actual observed bytes.

Target: `linux_x86_64`; executor x86_64 Linux, UID 1000, 18 GiB RAM and 32 GiB disk.
Rust nightly 2026-09-04, rustc 1.100.0-nightly (a69a63265).
One Rust build and one Omega execution at most per executor; corpus jobs were one.
The exact build target is
`/workspace/cloud-setup/cache/mbx/targets/v1/a403f72889b86ea796848a0dace0582cc2024165fafe0e86a3744ff44f2d3ba4`.
Final verified compiler SHA-256:
`fdad7b2d12836960036f935f6bf4127d11781a3f7d95fef3e1fdd7c4e3c27796`.
Build artifact JSON inventories and runtime binary hashes accompany the logs.

The initial compiler rejects complete `controls/local_array_loan` native source
at a state-edge loan transfer. The candidate's only compiler change registers
an existing bounded byte-view carrier at that edge; reference access is still
checked at each use. It leaves generic loan/custody/provision guards intact.
The complete reproduction then realizes. Scoped Clippy succeeds; the checked
shared-read, exclusive-write and shared/exclusive rejection corpus gate is clean
(2 checked, 1 rejected for its expected reason). No broader borrowed-storage
restoration outcome is claimed.

The original division formatter is preserved in complete source at
`3a1b377e04b863bdd0daa1a933e4b8f07a397dac`. On current main plus the byte repair,
it reproducibly hits an i32 fixed-view allocation failure. Diagnostic experiments
exposed a further unresolved-transition frontier; the existing copy route
licenses i64 only. Those experimental allocator changes are retained separately
as evidence and are absent from the candidate. The final private formatter uses
bounded subtraction with at most nine reductions per digit. Independent source
review confirmed exact two/five-digit equivalence; all frozen runtime outputs
pass unchanged. No numerical library or other worker implementation is imported.

Ordinary package review accepted 286 exact rows for the app and unchanged bundled
standard library. Root terminal permissions are stdin, stdout and process exit.
The broader public std API and opaque external ABI disclosures remain trust
boundaries; they do not grant root filesystem execution authority. No
assumed-binding-contract findings or package-acceptance denials occurred.

Commands from the repository root after activating cloud-setup:

```
mbx build -p omega --release --message-format=json
omega update --project samples/cli/independent/mold_front_vent_analysis --target linux_x86_64
omega --check --target linux_x86_64 samples/cli/independent/mold_front_vent_analysis/main.omg
omega --target linux_x86_64 samples/cli/independent/mold_front_vent_analysis/main.omg
PYTHONDONTWRITEBYTECODE=1 python3 samples/cli/independent/mold_front_vent_analysis/validation/validate.py /workspace/Omega/.codex/worktrees/new078/samples/cli/independent/mold_front_vent_analysis/build/omega-program
samples/cli/independent/mold_front_vent_analysis/build/terminal-validator /workspace/Omega/.codex/worktrees/new078/samples/cli/independent/mold_front_vent_analysis
omega --check --target linux_x86_64 samples/cli/independent/mold_front_vent_analysis/controls/out_of_bounds/main.omg
omega --check --target linux_x86_64 samples/cli/independent/mold_front_vent_analysis/controls/last_cell/main.omg
```

The intended-invalid command exits 1 with exactly one diagnostic:
`cannot prove index '16' is within length 16` (rendered with backtick quotes).
The valid index-15 companion checks successfully. Wrong-reason rejection is not
accepted. `validation/build_terminal.py` links the private harness to exact
production rlibs/rmeta; the bundle records the production catalog and link command.
Terminal consumes the ordinary accepted native producer's retained canonical
artifact and original provisions, without a checked-tree execution fallback.

Default compiler execution rejects synthetic root ownership at cache custody.
Approved per-command `require_escalated` compiler execution retains UID 1000 and
real root ownership; no ownership or custody guards were changed. Successful
read-only execution and preserved-session/hash checks verified readiness after
the reported disconnect. Other native targets were outside this executor's run.

Remaining publication blocker: GitHub GH007 rejected the freeform claim push
because the configured Git email is private. No claim registration or main
publication is asserted. Pushes remain paused pending an authorized public or
GitHub noreply identity; the denied operation has not been retried manually.
Library bundle transfer/consumption and independent final review are recorded
in the delivery receipt, separate from this package's immutable source snapshot.

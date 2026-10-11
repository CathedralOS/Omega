# Validation protocol and provenance

This candidate was authored independently on `samples/new016`, in the isolated
worktree `/workspace/Omega/.codex/worktrees/new016`, based on current fetched main
`8fef9837644713285aa9ed9eee42b46cd3bd1da8`. The starting checkout was preserved.
No remote worker branch, shared export change, new numerics package or other worker
candidate is used. The supported executor startup check returned `ready`.

The oracle was frozen before the first Omega check. Its immutable `expected.json`
SHA-256 is `0b7b74dc28aaed61c8a6e6628abc7fd30744839f134e720560d7eb956df42fbe`.
`validate.py` also verifies every frozen input fixture hash. The intended-invalid
bounds fragment was authored before its execution and is pinned separately.

The executor is Linux x86_64 with 5 CPUs, 18007 MiB memory and no swap. Rust
builds use four jobs and private target directories. There is at most one Rust
build and one Omega execution running on the executor. The first build command was:

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/Omega/.codex/worktrees/new016/build/rust CARGO_BUILD_JOBS=4 mbx build --locked -p omega -j 4
```

It passed in 4m46s; its compiler SHA-256 was
`2d05a0231ad7da12ca29008f5756b97aabd9acfbb399c3a64ff1dbd470dc6d71`.
This binary reports `omega 0.1.0-dev (unrecorded; x86_64-unknown-linux-gnu)`;
its lineage comes from the exact base, isolated build directory and build command,
not an invented version stamp. The first default-sandbox check exited 1 with:

```text
cannot open private source resolver storage: git cache entry `/` is invalid: cache custody ancestry is owned by an unrelated user
```

Per-command normal-context (`require_escalated`) compiler execution was approved
and passed that custody boundary. No filesystem ownership or custody guard was
changed. A relative library path, transition expression parentheses and an
ordinary call to an internal state and missing public helper effect ceilings
were corrected as authoring errors. Those
observations do not establish compiler defects. The state-call rejection took
424.60 seconds with the unoptimized compiler. A second compiler build uses the
repository-documented session override to make iteration practical:

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/Omega/.codex/worktrees/new016/build/rust-optimized CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_BUILD_JOBS=4 mbx build --locked -p omega -j 4
```

The final outcome, binary identity, exact commands, exits and numeric output
observations are carried in the review bundle's evidence rather than guessed
from source. Generated observations are under `build/validation/`; each invocation
writes `LABEL.command.json`, `.exit`, `.stdout`, and `.stderr`. The native route
compiles once and executes the one retained ELF artifact serially on all 23
fixtures, then runs the independent frozen verifier. The comparison route uses
`omega run --both --keep` on the representative fixtures in the README, checks
Terminal/native exit agreement, and verifies the captured report independently.
`inspect-terminal` records the public Terminal product inspection separately.

The invalid control uses `control.omg` as its explicit root, keeping it out of
the generic valid-sample discovery of `main.omg`. Its first complete check
(before that filename change) passed the intended bounds rejection on this base.
The final renamed root is validated separately. The invalid control passes only if a check exits 1 with one diagnostic containing
`cannot prove index \`2\` is within length 2`; package, parser, unresolved-name or
unrelated entry failures are rejected by the validator. Compiler gaps, if observed,
must be reproduced against this main base with complete source before any bounded
repair. No such gap is inferred from an authoring rejection or sandbox failure.

After those authoring fixes the optimized check reached exactly one remaining
rejection: the Linux standard program-entry schema requires an accepted binding.
This is the documented package-review boundary, not a compiler defect.

Package review uses `omega update` and the generated exact review document. No
validation command edits acceptance. Any denied acceptance is a remaining blocker
and is retained without retry. Other hosts are described by build bindings, not
claimed as validated by this Linux executor.

Claim registration for `NEW016` timed out with pending ticket
`545b37be49e84e62ad351be0773b491a`; the subsequent status lookup also reported
coordination unavailable. Registration is not claimed successful. Scope and
acceptance are retained here and in the bundle's executor record. Independent
review receives a clean candidate and Library bundle. Publication follows
`tools/landing.py` and main-only landing after validation and review; no remote
worker branch or direct main push is made.

The candidate was rebased onto fetched main
`a8c42a443795d5aeb6f1a299ac7261fe3c548801`. A fresh optimized CLI build passed
in 8m59s with SHA-256
`ccbf2a1c752d3f2da02cc0bf1d519e4ff9d41e9725ff779f5c352a01b815c839`.
The documented package update refreshed changed compiler provenance, reported
zero changed source entries and requested no new acceptance decisions. Native
production then reproduced the actual compiler gap in 77.11 seconds:
`number`: `boundary result policy lost its checked selection`.

A complete standalone reproduction in the evidence identifies `Float::multiply`
inside a returned record field. Temporary diagnostic logging was removed. The
bounded repair replaces the requirement collector's partial child roster with
the same complete executable traversal used by value discovery. Record fields,
range endpoints and value patterns retain the exact enclosing origin; Atomic's
result edge remains evidence-only. Selection uniqueness, policy adaptation and
provider custody checks remain mandatory. The feature-shaped positive fixture
`tests/omega/pass/float/requirement_nested_record_arithmetic_exit` declares exit 70 and exact
stdout before execution; it covers nested returned records, initializers,
record arguments and ordinary IEEE arithmetic. A broader exploratory Saturating-field probe is preserved in the evidence with its unchanged expectations and unsupported outcomes; it is not claimed as passing. Its separate Trap-coverage control is
`tests/omega/fail/float/requirement_record_field_trapping_requires_coverage`.
Final repaired commands and their actual outcomes are retained in the bundle.

The repaired native attempt reached a second, independently traced boundary:
`RegisterAllocation(RuntimeSpill(Homes(NoCommonCandidate { function: 21,
register: 17 })))`. Temporary diagnostics identified a cyclic formatter parameter
crossing `HostedWriteByteI32` before the fixed dividend use of `WrappingDivideI64`.
The original range required a surviving general-purpose view at the write and
view 0 at division; their intersection was empty. A spill proposal in another
function exposed this original conflict. It did not create the conflict.

The bounded allocator repair separates this constraint conflict from ordinary
pressure. An untied nonaddress integer fixed-use operand receives one fresh
`CopyI64` immediately before its consumer, even when the source is a cyclic block
parameter. Other uses, incoming edges and the original source register remain
unchanged. The copy and register carry the exact source lineage; crossed boundary
settlements shift by one. Current liveness, ranges, legality and homes are rebuilt
through their existing owners before admission. Fresh fixed-use segments do not
join the pressure-victim roster. Unsupported types, ties and structural origins
continue to reject; no cache custody, provider, result-policy or home constraints
are relaxed. No shared export roster changes are needed.

The complete independent compiler control is
`tests/omega/pass/arithmetic/runtime_cyclic_parameter_fixed_use_after_console_exit`;
its exit 70 and `000\n` stdout were frozen before execution. The whole application
is retained unchanged across both compiler repairs. Actual post-repair outcomes,
including failures, are in the review bundle. Terminal artifact inspection is
separate from runtime execution: the normal corpus runtime route first demands a
native artifact and its provisions. A redundant eight-input attempt through that
route was interrupted with gate exit 4 after observing its native dependency; it
is not a passing runtime gate. Focused follow-up commands are recorded separately.

Final application acceptance uses repaired CLI SHA-256
`1d069809481afd8400378c24ecf018825abe8013b5c37d65066f7fafbd3b4985`.
Explicit source check passed (97.6105 s). Native production passed (94.8379 s);
one retained ELF SHA-256
`2cfa1aa95ef5d5b348c2793f564d5acb55a1ff341fd9d4f89e6b317e4091da76`
executed all 23 fixtures against the frozen Decimal oracle and tolerances.
Host `run --both --keep main.omg` passed `curved_hot`, `ambiguous`, and
`malformed`, with matching native/Terminal exits 0, 15, and 10 and the native
reports checked against that same oracle. The standalone bounds control
rejected with exactly its intended bounds diagnostic (41.9411 s).

An earlier authoring mistake supplied `--target` to `run --both`; the CLI
documents that explicit target selection requests target-only production.
The validator rejected the missing interpreter observation. The corrected host
commands omit `--target`; the failed commands and subsequent passes are both
retained. No expected result was changed to accommodate this mistake.

The last successful main fetch was the base above. Later fetches and claims
status reads were blocked by unavailable GitHub HTTPS credentials, including
in the supported normal UID execution context. No publication or successful
claim registration is asserted. The clean candidate and Library transfer are
for independent review and the documented main-only landing workflow. No
package acceptance was denied or retried after denial. Windows and macOS
routes were not executed on this Linux host.

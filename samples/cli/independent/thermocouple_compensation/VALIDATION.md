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
acceptance are retained here and in the bundle's executor record. The requested
handoff is a clean local candidate for independent review. Publication, if later
requested, follows `tools/landing.py` and main-only landing; no remote worker
branch or direct main push is made.
